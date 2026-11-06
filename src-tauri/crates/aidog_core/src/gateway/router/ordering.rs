//! 候选排序策略：负载均衡（加权随机）/ 粘性绑定 / 轮换 / 用量 / 余量 / 智能（票 magpie 05/06）。

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering as AtomicOrdering};
use std::sync::{Mutex, OnceLock};

use super::super::models::*;
use super::candidates::ScheduleCtx;
use super::effective_weight;

/// 平台是否为 coding plan（订阅制）：任一协议端点标记 `coding_plan=true` 即视为是。
///
/// 说明：`coding_plan` 是 **per-endpoint** 标记（`PlatformEndpoint.coding_plan`），
/// Platform 自身无该布尔字段；与 `db/schema_late.rs` 迁移 025 判定口径一致
/// （存在 coding_plan 端点即 coding plan 平台）。
pub(crate) fn is_coding_plan(p: &Platform) -> bool {
    p.endpoints.iter().any(|ep| ep.coding_plan)
}

/// 过期时间排序键：`expires_at=0`（永不过期）视为 `i64::MAX` 排末尾，
/// 其余按 `expires_at` 升序（快过期先用，避免额度浪费）。
///
/// 仅对未过期候选调用（已过期的由 `candidate_state` 提前过滤）。
/// 作为各路由模式**同档位 tiebreak**的强 "用掉它" 信号：
/// - Failover：同 priority 内（插在 priority 之后）。
/// - LoadBalance / HealthAware / Sticky：同有效权重档内（影响基础重试序，不参与加权随机选首）。
///
/// 各模式均不改主排序键；统一在 `apply_coding_plan_priority` 与显式 mapping 提首之前。
pub(crate) fn expiry_sort_key(expires_at: i64) -> i64 {
    if expires_at > 0 { expires_at } else { i64::MAX }
}

/// coding plan 平台优先：在已按路由模式排好序的候选列表上做**稳定分桶上浮**，
/// 把 coding plan 平台整体提到非 coding plan 之前，每个桶内部保持入参已有顺序
/// （mode 排序结果）不变。
///
/// 语义：订阅制 coding plan 额度按月包干，无明确依据偏向某平台时优先消耗它以省钱。
/// 作为主排序键叠加在 per-mode 排序之上（Rust `sort_by_key` 稳定，桶内序保持）。
/// `!is_coding_plan` 作 key：false(0, coding plan) 排在 true(1) 之前。
///
/// 调用约束：须在各 mode 排序之后、`apply_sticky` 与显式 mapping 提首之前，
/// 对 active / probe 两桶**各自独立**调用（probe 整体在 active 之后，不跨桶上浮）。
pub(crate) fn apply_coding_plan_priority(platforms: &mut [&GroupPlatformDetail]) {
    platforms.sort_by_key(|gp| !is_coding_plan(&gp.platform));
}

/// 负载均衡排序：加权随机决定首个，其余按有效权重降序，保证所有候选都可被重试。
pub(crate) fn order_load_balance(platforms: &mut Vec<&GroupPlatformDetail>, seed: i64) {
    if platforms.len() <= 1 {
        return;
    }
    let total_weight: i32 = platforms.iter().map(|gp| effective_weight(gp)).sum();
    // 先按有效权重降序作为基础顺序；同权重档内按 expires_at 升序 tiebreak
    // （快过期先用，省额度）。该 tiebreak 只影响**基础重试顺序**，不参与下方的
    // 加权随机选首（pick 仍只基于 weight），故不破坏负载均衡首选随机性。
    platforms.sort_by(|a, b| {
        std::cmp::Reverse(effective_weight(a))
            .cmp(&std::cmp::Reverse(effective_weight(b)))
            .then_with(|| {
                expiry_sort_key(a.platform.expires_at).cmp(&expiry_sort_key(b.platform.expires_at))
            })
    });
    if total_weight <= 0 {
        return;
    }
    // 加权随机选首个（用时间种子），将其移到最前
    let mut rand_val = (seed.unsigned_abs() as i32) % total_weight;
    let mut pick = 0usize;
    for (i, gp) in platforms.iter().enumerate() {
        rand_val -= effective_weight(gp);
        if rand_val < 0 {
            pick = i;
            break;
        }
    }
    if pick != 0 {
        let gp = platforms.remove(pick);
        platforms.insert(0, gp);
    }
}

/// Sticky：若 session 键命中已绑定平台且该平台仍在健康候选集中，提到首位；
/// 否则把当前首选（加权随机已定）写为新绑定。失效 / 熔断的旧绑定自然回退（不在集中即重绑）。
/// 返回 Some(平台 id) = 绑定命中且已提首（票 01 sticky 位置标签）。
pub(crate) fn apply_sticky(
    platforms: &mut [&GroupPlatformDetail],
    ctx: Option<&ScheduleCtx<'_>>,
    now_ms: i64,
) -> Option<u64> {
    let c = ctx?;
    let key = c.sticky_key.as_ref()?;
    if platforms.is_empty() {
        return None;
    }
    if let Some(bound_id) = c.sticky.get(key, now_ms)
        && let Some(pos) = platforms.iter().position(|gp| gp.platform.id == bound_id)
    {
        platforms.swap(0, pos);
        return Some(bound_id); // 绑定健康，维持
    }
    // 绑定平台已失效 / 熔断 / 不在集 → 落到重绑（用新首选）
    // 写 / 重写绑定为当前首选平台
    c.sticky.put(key.clone(), platforms[0].platform.id, now_ms);
    None
}

/// HealthAware 同档内排序子键（`compute_health_order_keys` 快照，排序前一次性算好）：
/// coding plan = 各额度维度剩余比例的最小值（降序，余多者先用）；按量 = 静态估算单价
/// （升序，便宜者先用）。余额缺失按 1.0、条目缺失按 fallback 单价（同计费链口径）。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum HealthOrderKey {
    CodingRemaining(f64),
    PaygPrice(f64),
}

/// coding plan 最紧维度剩余比例：各 tier `100 - est_utilization` 的最小值（0..1）。
/// 无 est_coding_plan / 无 tier 数据 → 1.0（与 `quota_verdict`「Unknown = Available」口径一致）。
/// tier 利用率 NaN 视为该维度未知，不参与取最小。
pub(crate) fn coding_remaining_ratio(est_coding_plan: &str) -> f64 {
    let plan = crate::gateway::estimate::EstCodingPlan::from_json(est_coding_plan);
    let min = plan
        .tiers
        .iter()
        .map(|t| (100.0 - t.est_utilization).clamp(0.0, 100.0) / 100.0)
        .fold(f64::NAN, f64::min);
    if min.is_nan() { 1.0 } else { min }
}

/// HealthAware 排序（2026-11-06 拍板）：手动优先级 `clamp_level_priority` 保持主导
/// （coding plan 不压过手动优先级），同档内：coding plan 桶整体排前（桶内按最紧维度
/// 剩余比例降序）；按量桶内按静态估算单价升序（便宜先用）；并列项维持现状 tiebreak
/// （有效权重降序 + expires_at 升序 + 首部同键并列组加权随机选首）。
/// `keys` 缺项回落：coding plan → 1.0（按全可用），按量 → `f64::INFINITY`（排按量桶末尾）。
pub(crate) fn order_health_aware(
    platforms: &mut Vec<&GroupPlatformDetail>,
    seed: i64,
    keys: &HashMap<u64, HealthOrderKey>,
) {
    if platforms.len() <= 1 {
        return;
    }
    // 档内键快照 → (level_priority, 桶序, 键值)。cp 桶 0 在前；键值 cp=剩余比例（大者先）、
    // 按量=单价（小者先），比较方向在 sort_by 里按桶区分。
    let key_of = |gp: &GroupPlatformDetail| -> (i32, u8, f64) {
        let level = crate::gateway::models::clamp_level_priority(gp.level_priority);
        match keys.get(&gp.platform.id) {
            Some(HealthOrderKey::CodingRemaining(r)) => (level, 0, *r),
            Some(HealthOrderKey::PaygPrice(p)) => (level, 1, *p),
            None if is_coding_plan(&gp.platform) => (level, 0, 1.0),
            None => (level, 1, f64::INFINITY),
        }
    };
    platforms.sort_by(|a, b| {
        let (la, bucket_a, ka) = key_of(a);
        let (lb, bucket_b, kb) = key_of(b);
        // level_priority 降序 → cp 桶在前 → 桶内键（cp 剩余降序 / 按量单价升序）
        lb.cmp(&la)
            .then_with(|| bucket_a.cmp(&bucket_b))
            .then_with(|| {
                if bucket_a == 0 {
                    kb.total_cmp(&ka) // 剩余比例降序（余多者先用）
                } else {
                    ka.total_cmp(&kb) // 单价升序（便宜者先用）
                }
            })
            .then_with(|| {
                std::cmp::Reverse(effective_weight(a))
                    .cmp(&std::cmp::Reverse(effective_weight(b)))
            })
            .then_with(|| {
                expiry_sort_key(a.platform.expires_at).cmp(&expiry_sort_key(b.platform.expires_at))
            })
    });
    // 首部同键并列组（level / 桶 / 键 / 权重 / expiry 全同）内加权随机选首：维持分流性
    let mut tied = 1usize;
    let (l0, b0, k0) = key_of(platforms[0]);
    let w0 = effective_weight(platforms[0]);
    let e0 = expiry_sort_key(platforms[0].platform.expires_at);
    while tied < platforms.len() {
        let (l, b, k) = key_of(platforms[tied]);
        if l == l0
            && b == b0
            && k.total_cmp(&k0) == std::cmp::Ordering::Equal
            && effective_weight(platforms[tied]) == w0
            && expiry_sort_key(platforms[tied].platform.expires_at) == e0
        {
            tied += 1;
        } else {
            break;
        }
    }
    if tied > 1 {
        let total: i32 = platforms[..tied].iter().map(|gp| effective_weight(gp)).sum();
        if total > 0 {
            let mut rand_val = (seed.unsigned_abs() as i32) % total;
            let mut pick = 0usize;
            for (i, gp) in platforms[..tied].iter().enumerate() {
                rand_val -= effective_weight(gp);
                if rand_val < 0 {
                    pick = i;
                    break;
                }
            }
            if pick != 0 {
                let gp = platforms.remove(pick);
                platforms.insert(0, gp);
            }
        }
    }
}

/// rotate 模式请求级轮换计数器：per-group AtomicU64（票 magpie 05）。
/// 进程内计数、**重启归零**（轮换起点复位属可接受边界，票面已记录）；轮换只决定
/// 「可用候选内」的次序，不参与熔断 / 冷却 / 状态过滤。
static ROTATE_COUNTERS: OnceLock<Mutex<HashMap<u64, AtomicU64>>> = OnceLock::new();

/// 取该分组下一个轮换偏移（每次请求自增 1）。
pub(crate) fn next_rotate_offset(group_id: u64) -> u64 {
    let counters = ROTATE_COUNTERS.get_or_init(Default::default);
    let mut m = counters.lock().unwrap_or_else(|e| e.into_inner());
    m.entry(group_id)
        .or_default()
        .fetch_add(1, AtomicOrdering::Relaxed)
}

/// Rotate 排序（spec D3「严格轮换，同档请求级轮换」）：档键 = (level_priority 降序,
/// priority 升序)，档间顺序保持优先级语义；每档内按 `offset % 档长` 严格轮换
/// （不吃权重 / expires_at / coding plan 等其它键）。
pub(crate) fn order_rotate(platforms: &mut [&GroupPlatformDetail], offset: u64) {
    if platforms.len() <= 1 {
        return;
    }
    platforms.sort_by_key(|gp| (std::cmp::Reverse(gp.level_priority), gp.priority));
    let n = platforms.len();
    let mut start = 0usize;
    while start < n {
        let tier = (platforms[start].level_priority, platforms[start].priority);
        let mut end = start + 1;
        while end < n
            && (platforms[end].level_priority, platforms[end].priority) == tier
        {
            end += 1;
        }
        let k = (offset % (end - start) as u64) as usize;
        platforms[start..end].rotate_left(k);
        start = end;
    }
}

/// Usage 排序（spec D3「统计窗口内成功请求量最少优先」）：counts = 近 24h 每平台
/// 成功请求数（`platform_success_counts_24h`），缺失 / 查询失败按 0（从未用过先用）。
/// tiebreak：有效权重降序 → expires_at 升序（快过期先用）。输入已是状态过滤后的
/// 可用候选集，不改摘除逻辑；无 level_priority 分档（按量口径，计数即序）。
pub(crate) fn order_usage(platforms: &mut [&GroupPlatformDetail], counts: &HashMap<u64, i64>) {
    if platforms.len() <= 1 {
        return;
    }
    platforms.sort_by(|a, b| {
        let ca = counts.get(&a.platform.id).copied().unwrap_or(0);
        let cb = counts.get(&b.platform.id).copied().unwrap_or(0);
        ca.cmp(&cb)
            .then_with(|| {
                std::cmp::Reverse(effective_weight(a))
                    .cmp(&std::cmp::Reverse(effective_weight(b)))
            })
            .then_with(|| {
                expiry_sort_key(a.platform.expires_at).cmp(&expiry_sort_key(b.platform.expires_at))
            })
    });
}

/// pace / smart 排序键快照（票 magpie 06）：由 `platform.est_coding_plan` 纯解析，
/// 排序前一次性算好，比较闭包内零 JSON 解析（同 `HealthOrderKey` 快照模式）。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct PaceSmartKey {
    /// 最快重置时刻（unix ms）：各 tier `window_start + cycle` 的最小值；
    /// `<= now` 视为已重置 → 取 now（最新鲜）。
    /// None = 无 tier 有 window_start + 已知周期 → 重置时间不可得。
    pub reset_at: Option<i64>,
    /// 最紧 tier 剩余比例（0..1）：各 tier `(100 - est_utilization)/100` 的最小值。
    /// None = 无 tier 数据（非 coding plan / est_coding_plan 空 / 无 tier）→ 剩余额度不可得。
    pub remaining: Option<f64>,
}

/// 从 est_coding_plan 纯解析 pace/smart 键（audit 结论：剩余额度可得自 tier
/// `est_utilization`；重置时间可得自 `window_start + cycle_ms_for_tier`，
/// 后者仅真查拿过 resets_at 且周期已知时落地，故 Smart 是部分可得维度）。
pub(crate) fn compute_pace_smart_keys(
    platforms: &[GroupPlatformDetail],
    now_ms: i64,
) -> HashMap<u64, PaceSmartKey> {
    let mut keys = HashMap::new();
    for gp in platforms {
        let plan = crate::gateway::estimate::EstCodingPlan::from_json(&gp.platform.est_coding_plan);
        let mut reset_at: Option<i64> = None;
        let mut remaining: Option<f64> = None;
        for t in &plan.tiers {
            let rem = (100.0 - t.est_utilization).clamp(0.0, 100.0) / 100.0;
            remaining = Some(remaining.map_or(rem, |m: f64| m.min(rem)));
            if t.window_start > 0
                && let Some(cycle) = crate::gateway::usage_color::cycle_ms_for_tier(&t.name)
            {
                let r = (t.window_start + cycle).max(now_ms);
                reset_at = Some(reset_at.map_or(r, |m| m.min(r)));
            }
        }
        keys.insert(
            gp.platform.id,
            PaceSmartKey { reset_at, remaining },
        );
    }
    keys
}

/// Pace 回落判定：被排序候选中无任何平台有剩余额度数据 → 整模回落 usage
/// （spec D3「数据不可得时 pace 显式回落 usage」）。
pub(crate) fn pace_falls_back_to_usage(
    keys: &HashMap<u64, PaceSmartKey>,
    platforms: &[&GroupPlatformDetail],
) -> bool {
    !platforms
        .iter()
        .any(|gp| keys.get(&gp.platform.id).is_some_and(|k| k.remaining.is_some()))
}

/// Smart 回落判定：被排序候选中无任何平台有重置时间数据 → 整模回落 health_aware
/// （spec D3「数据不可得时 smart 显式回落 health_aware」）。
pub(crate) fn smart_falls_back_to_health_aware(
    keys: &HashMap<u64, PaceSmartKey>,
    platforms: &[&GroupPlatformDetail],
) -> bool {
    !platforms
        .iter()
        .any(|gp| keys.get(&gp.platform.id).is_some_and(|k| k.reset_at.is_some()))
}

/// Option 升序、None 排末尾（pace 的重置时间次键 / smart 主键共用）。
fn opt_ascending_none_last<T: Ord>(x: Option<T>, y: Option<T>) -> std::cmp::Ordering {
    match (x, y) {
        (Some(a), Some(b)) => a.cmp(&b),
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, None) => std::cmp::Ordering::Equal,
    }
}

/// Option 降序、None 排末尾（剩余比例键共用）。
fn opt_descending_none_last(x: Option<f64>, y: Option<f64>) -> std::cmp::Ordering {
    match (x, y) {
        (Some(a), Some(b)) => b.total_cmp(&a),
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, None) => std::cmp::Ordering::Equal,
    }
}

/// Pace 排序（spec D3「重置前剩余额度最多优先」）：有剩余额度数据的平台按剩余比例
/// 降序（重置前余最多优先），次键重置时间升序（同等余量先吃快重置的，避免临期浪费）；
/// 无数据平台（按量 / 无 tier）排有数据之后（pace 是套餐额度维度，按量无该维度），
/// 档内权重降序 + expires_at 升序 tiebreak。输入已是状态过滤后的可用候选集，
/// 不改摘除逻辑。整体不可得时由 `pace_falls_back_to_usage` 判定回落 usage。
pub(crate) fn order_pace(
    platforms: &mut [&GroupPlatformDetail],
    keys: &HashMap<u64, PaceSmartKey>,
) {
    if platforms.len() <= 1 {
        return;
    }
    let key = |gp: &GroupPlatformDetail| -> (Option<f64>, Option<i64>) {
        keys.get(&gp.platform.id)
            .map_or((None, None), |k| (k.remaining, k.reset_at))
    };
    platforms.sort_by(|a, b| {
        let (ra, ta) = key(a);
        let (rb, tb) = key(b);
        opt_descending_none_last(ra, rb)
            .then_with(|| opt_ascending_none_last(ta, tb))
            .then_with(|| {
                std::cmp::Reverse(effective_weight(a))
                    .cmp(&std::cmp::Reverse(effective_weight(b)))
            })
            .then_with(|| {
                expiry_sort_key(a.platform.expires_at).cmp(&expiry_sort_key(b.platform.expires_at))
            })
    });
}

/// Smart 排序（spec D3「最快重置的套餐优先，次键套餐余额」）：有重置时间的平台按
/// 重置时间升序（最快重置优先），次键剩余比例降序（套餐余额多者优先）；无重置时间的
/// 平台（按量 / tier 无 window_start）排有数据之后，仍用剩余比例降序排序（残缺数据
/// 不浪费），再权重降序 + expires_at 升序 tiebreak。输入已是状态过滤后的可用候选集，
/// 不改摘除逻辑。整体不可得时由 `smart_falls_back_to_health_aware` 判定回落 health_aware。
pub(crate) fn order_smart(
    platforms: &mut [&GroupPlatformDetail],
    keys: &HashMap<u64, PaceSmartKey>,
) {
    if platforms.len() <= 1 {
        return;
    }
    let key = |gp: &GroupPlatformDetail| -> (Option<i64>, Option<f64>) {
        keys.get(&gp.platform.id)
            .map_or((None, None), |k| (k.reset_at, k.remaining))
    };
    platforms.sort_by(|a, b| {
        let (ta, ra) = key(a);
        let (tb, rb) = key(b);
        opt_ascending_none_last(ta, tb)
            .then_with(|| opt_descending_none_last(ra, rb))
            .then_with(|| {
                std::cmp::Reverse(effective_weight(a))
                    .cmp(&std::cmp::Reverse(effective_weight(b)))
            })
            .then_with(|| {
                expiry_sort_key(a.platform.expires_at).cmp(&expiry_sort_key(b.platform.expires_at))
            })
    });
}

#[cfg(test)]
#[path = "test_ordering.rs"]
mod test_ordering;
