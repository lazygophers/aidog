//! 候选排序策略：负载均衡（加权随机）/ 最小延迟 / 粘性绑定。

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
    // 先按有效权重降序作为基础顺序
    platforms.sort_by_key(|gp| std::cmp::Reverse(effective_weight(gp)));
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

/// LeastLatency 排序：按 per-platform 延迟 EMA 升序；无样本（None）视为最大排末尾。
/// 无 ctx（无指标）时退化为不变序（保持入参顺序）。
pub(crate) fn order_least_latency(platforms: &mut [&GroupPlatformDetail], ctx: Option<&ScheduleCtx<'_>>) {
    let Some(c) = ctx else { return };
    platforms.sort_by(|a, b| {
        let la = c.scheduler.latency_ema(a.platform.id).unwrap_or(f64::MAX);
        let lb = c.scheduler.latency_ema(b.platform.id).unwrap_or(f64::MAX);
        // 延迟 EMA 升序为主键；同延迟档时 level_priority 降序（高优先先）为次级 tiebreaker
        la.partial_cmp(&lb)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| b.level_priority.cmp(&a.level_priority))
    });
}

/// Sticky：若 session 键命中已绑定平台且该平台仍在健康候选集中，提到首位；
/// 否则把当前首选（加权随机已定）写为新绑定。失效 / 熔断的旧绑定自然回退（不在集中即重绑）。
pub(crate) fn apply_sticky(platforms: &mut [&GroupPlatformDetail], ctx: Option<&ScheduleCtx<'_>>, now_ms: i64) {
    let Some(c) = ctx else { return };
    let Some(ref key) = c.sticky_key else { return };
    if platforms.is_empty() {
        return;
    }
    if let Some(bound_id) = c.sticky.get(key, now_ms) {
        if let Some(pos) = platforms.iter().position(|gp| gp.platform.id == bound_id) {
            platforms.swap(0, pos);
            return; // 绑定健康，维持
        }
        // 绑定平台已失效 / 熔断 / 不在集 → 落到重绑（用新首选）
    }
    // 写 / 重写绑定为当前首选平台
    c.sticky.put(key.clone(), platforms[0].platform.id, now_ms);
}

#[cfg(test)]
#[path = "test_ordering.rs"]
mod test_ordering;
