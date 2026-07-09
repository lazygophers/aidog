//! 候选选取：根据分组路由规则选择**有序候选平台列表**，用于失败逐个重试。

use super::super::db;
use super::super::models::*;
use super::super::scheduling::{Admission, BreakerThresholds, SchedulerState, StickyTable};
use super::super::time_models;
use super::model_mapping::resolve_model;
use super::ordering::{apply_coding_plan_priority, apply_sticky, expiry_sort_key, order_least_latency, order_load_balance};
use super::{candidate_state, is_peak_disabled, RouteResult};

/// 候选选取结果：有序的候选平台列表（首个为最优先），用于失败逐个重试。
/// `target_model` / `mapping` 对每个候选独立解析（显式映射命中时全部候选共享映射目标模型；
/// 否则按各平台 PlatformModels 自动匹配）。
pub struct CandidateSet {
    pub candidates: Vec<RouteResult>,
}

/// 调度上下文（proxy 持有；scheduler 为 per-platform 健康/熔断指标，sticky 为粘性绑定表）。
pub struct ScheduleCtx<'a> {
    pub scheduler: &'a SchedulerState,
    pub sticky: &'a StickyTable,
    pub settings: &'a SchedulingBreakerSettings,
    /// Sticky 模式 session 键（group_key + 客户端稳定标识，调用侧拼接）。
    pub sticky_key: Option<String>,
}

/// 根据分组路由规则选择**有序候选平台列表**，用于失败逐个重试。
///
/// 排序：
/// - Failover：按 priority 升序
/// - LoadBalance：按权重加权随机决定首选，其余按权重降序排在后面（保证不同候选都能被试到）
///
/// 过滤：status==Enabled 优先纳入；auto_disabled 且已过退避试探时间的平台排在**末尾**惰性试探。
/// 显式 model_mapping 命中时，映射目标平台排在候选首位（最高优先），其余候选作为 failover 后备。
/// 无调度上下文重载（保持旧调用兼容，如测试 / 内部）：用默认（无熔断指标，仅 auto_disabled 过滤）。
#[allow(dead_code)]
pub async fn select_candidates(
    db: &db::Db,
    group: &Group,
    source_model: &str,
) -> Result<CandidateSet, String> {
    select_candidates_ctx(db, group, source_model, None).await
}

/// 带调度上下文的候选选取。`ctx=None` 时退化为无熔断 / 无指标的旧行为（仅 auto_disabled 过滤）。
pub async fn select_candidates_ctx(
    db: &db::Db,
    group: &Group,
    source_model: &str,
    ctx: Option<&ScheduleCtx<'_>>,
) -> Result<CandidateSet, String> {
    let mapping = group.model_mappings.iter().find(|m| m.source_model == source_model);
    let mapped_target_model = mapping.map(|m| m.target_model.clone());
    let mapped_platform_id = mapping.map(|m| m.target_platform_id).filter(|id| *id != 0);

    let group_platforms = db::get_group_platforms(db, group.id).await?;
    if group_platforms.is_empty() {
        return Err("group has no platforms".to_string());
    }

    let now_ms = db::now();

    // 有效调度策略：Group routing_mode 即为最终策略；旧 settings 中全局默认仅在无 ctx 时不参与。
    // （Group 总是携带 routing_mode；全局默认 default_routing_mode 是 GB 写 Group 时的初值来源。）
    let effective_mode = group.routing_mode;
    let breaker_enabled = ctx.map(|c| c.settings.enabled).unwrap_or(false);

    // ── 单平台分组：无视平台状态（auto_disabled / 熔断）必请求 ──
    // 用户语义：只有多平台分组才需要在乎平台状态做摘除（择优切换到健康平台）；
    // 单平台无可切目标，摘除只会 blackhole（返回 400 no available platform）。
    // 故单平台直接纳入唯一平台必请求，哪怕 auto_disabled / 熔断 Open 也尝试。
    // 例外：手动 Disabled 是用户显式关停意图，仍为唯一硬停（分组无效 → Err）。
    if group_platforms.len() == 1 {
        let only = &group_platforms[0];
        if only.platform.status == PlatformStatus::Disabled {
            return Err("group's only platform is manually disabled".to_string());
        }
        // 高峰禁用优先级高于 status bypass：单平台组高峰期请求直接 fail（PRD: 此开关优先级高于 status bypass，
        // 单平台组不 bypass 此维度）。status 维度照旧 bypass（auto_disabled / 熔断仍必请求）。
        if is_peak_disabled(&only.platform, now_ms) {
            tracing::info!(
                group = %group.name, platform = %only.platform.name,
                "single-platform group: peak-disabled, request blocked"
            );
            return Err("peak_disabled".to_string());
        }
        // 时段模型：先解析 time_models 获取 effective_models，再用 resolve_model
        let time_rules = time_models::parse_platform_time_models(&only.platform.extra);
        let effective_models = time_models::resolve_time_models(&time_rules, &only.platform.models, now_ms);
        let target_model = mapped_target_model
            .clone()
            .unwrap_or_else(|| resolve_model(&effective_models, source_model));
        tracing::info!(
            group = %group.name, platform = %only.platform.name,
            status = ?only.platform.status,
            "single-platform group: bypassing status filter, forcing request"
        );
        return Ok(CandidateSet {
            candidates: vec![RouteResult {
                platform: only.platform.clone(),
                target_model,
                mapping: mapping.cloned(),
            }],
        });
    }

    // 1. 拆分候选：先按 auto_disabled 三态分桶（enabled / 过期试探）。
    //    再叠加熔断准入门：[熔断 Open] ∪ [auto_disabled 未到期] 取并集踢出。
    //    HalfOpen 平台限量放行（计入 active）。二者状态独立判定，互不改写。
    let mut active: Vec<&GroupPlatformDetail> = Vec::new();
    let mut probe: Vec<&GroupPlatformDetail> = Vec::new();
    // 仅被「熔断」踢出的候选（区别于 auto_disabled / 手动 disabled 踢出的）暂存于此，
    // 携带其 auto_state 以便回退时正确分桶。用于「候选全被熔断踢空 → 回退透传」：
    // 熔断语义是在多个健康平台间择优摘坏，无可切目标时（单平台分组 / 多平台全坏）
    // 不应制造空候选 blackhole（否则丢失上游真实 429/5xx + retry-after，客户端无法退避）。
    let mut breaker_rejected: Vec<(&GroupPlatformDetail, Option<bool>)> = Vec::new();
    // 被高峰禁用排除的候选计数：整组全被高峰排除时返特殊 Err "peak_disabled"，
    // handler.rs 据此落 proxy_log blocked_reason='peak_hours'（区别于普通 NoCandidate）。
    let mut peak_disabled_count: usize = 0;
    for gp in &group_platforms {
        // auto_disabled 维度（DB 持久态）
        let auto_state = candidate_state(&gp.platform, now_ms);
        if auto_state.is_none() {
            // 区分高峰禁用与其他排除原因（disabled / auto_disabled 未到期 / 过期）
            if is_peak_disabled(&gp.platform, now_ms) {
                peak_disabled_count += 1;
            }
            continue; // 用户手动 disabled / auto_disabled 未到退避 / 高峰禁用 → 跳过
        }
        // 熔断维度（内存态）：仅在有 ctx 且总开关开时判定
        if let Some(c) = ctx {
            if breaker_enabled {
                let (ft, os, hom) = c.settings.effective_thresholds(&gp.platform);
                let th = BreakerThresholds { failure_threshold: ft, open_secs: os, half_open_max: hom };
                match c.scheduler.admission(gp.platform.id, &th, now_ms, true) {
                    Admission::Reject => {
                        // 熔断 Open / HalfOpen 名额满 → 暂踢出，留作全空回退候选
                        breaker_rejected.push((gp, auto_state));
                        continue;
                    }
                    Admission::Probe | Admission::Allow => {}
                }
            }
        }
        match auto_state {
            Some(false) => active.push(gp),
            Some(true) => probe.push(gp),
            None => {}
        }
    }

    // ── 候选全被熔断踢空 → 回退透传 ──
    // 仅当熔断维度踢空（active+probe 皆空）且确有被熔断踢出的候选时回退；
    // 若空因 auto_disabled / 手动 disabled，则不回退（保持原 Err，下游返回路由错误）。
    if active.is_empty() && probe.is_empty() && !breaker_rejected.is_empty() {
        tracing::warn!(
            group = %group.name, rejected = breaker_rejected.len(),
            "all candidates circuit-broken; bypassing breaker to passthrough real upstream status"
        );
        for (gp, st) in breaker_rejected {
            match st {
                Some(false) => active.push(gp),
                Some(true) => probe.push(gp),
                None => {}
            }
        }
    }

    // 2. 按路由模式排序两组
    match effective_mode {
        RoutingMode::Failover => {
            // 排序键：level_priority 降序（10 先）→ priority 升序 → expires_at 升序（快过期先用，
            // expires_at=0 视为 i64::MAX 排末尾）。expires_at 是同 priority 内最强"用掉它"信号，
            // 插在 priority 之后、负载均衡 / coding plan 偏好之前（[platform-expiry-priority]）。
            active.sort_by_key(|gp| {
                (
                    std::cmp::Reverse(gp.level_priority),
                    gp.priority,
                    expiry_sort_key(gp.platform.expires_at),
                )
            });
            probe.sort_by_key(|gp| {
                (
                    std::cmp::Reverse(gp.level_priority),
                    gp.priority,
                    expiry_sort_key(gp.platform.expires_at),
                )
            });
            apply_coding_plan_priority(&mut active);
            apply_coding_plan_priority(&mut probe);
        }
        // LoadBalance / HealthAware：健康集加权随机（准入门已摘 Open，等价加权随机 on 健康集）
        RoutingMode::LoadBalance | RoutingMode::HealthAware => {
            order_load_balance(&mut active, now_ms);
            order_load_balance(&mut probe, now_ms);
            apply_coding_plan_priority(&mut active);
            apply_coding_plan_priority(&mut probe);
        }
        // LeastLatency：按延迟 EMA 升序（无样本视为最大，排末尾）
        RoutingMode::LeastLatency => {
            order_least_latency(&mut active, ctx);
            order_least_latency(&mut probe, ctx);
            apply_coding_plan_priority(&mut active);
            apply_coding_plan_priority(&mut probe);
        }
        // Sticky：绑定平台若健康提到首位，否则回退加权随机 + 写绑定
        RoutingMode::Sticky => {
            order_load_balance(&mut active, now_ms);
            order_load_balance(&mut probe, now_ms);
            // coding plan 偏好须在 apply_sticky 之前应用：否则 sticky 提首后又被分桶打乱。
            apply_coding_plan_priority(&mut active);
            apply_coding_plan_priority(&mut probe);
            apply_sticky(&mut active, ctx, now_ms);
        }
    }

    // 3. 合并：正常候选在前，试探候选在后
    let mut ordered: Vec<&GroupPlatformDetail> = Vec::with_capacity(active.len() + probe.len());
    ordered.extend(active);
    ordered.extend(probe);

    // 4. 显式映射目标平台提到最前（若它本身在候选集中）
    if let Some(target_id) = mapped_platform_id {
        if let Some(pos) = ordered.iter().position(|gp| gp.platform.id == target_id) {
            let gp = ordered.remove(pos);
            ordered.insert(0, gp);
        } else {
            tracing::warn!(
                group = %group.name, target_platform_id = target_id,
                "mapped target platform not an available candidate, falling back to routing order"
            );
        }
    }

    if ordered.is_empty() {
        // 整组所有候选被高峰禁用排除 → 返特殊 Err，caller handler.rs 据此落审计 proxy_log
        // (blocked_by='router', blocked_reason='peak_hours', est_cost=0, status_code=503)。
        // 其他原因（disabled / auto_disabled / 熔断无回退）照旧 NoCandidate warn 不落库。
        if peak_disabled_count > 0 && peak_disabled_count == group_platforms.len() {
            tracing::info!(
                group = %group.name, peak_disabled = peak_disabled_count,
                "all candidates peak-disabled; returning peak_disabled error for audit log"
            );
            return Err("peak_disabled".to_string());
        }
        return Err("no available platform (all disabled, backing off, or circuit-broken)".to_string());
    }

    // 5. 为每个候选解析目标模型
    let candidates: Vec<RouteResult> = ordered
        .into_iter()
        .map(|gp| {
            let target_model = if let Some(ref tm) = mapped_target_model {
                tm.clone()
            } else {
                // 时段模型：先解析 time_models 获取 effective_models，再用 resolve_model
                let time_rules = time_models::parse_platform_time_models(&gp.platform.extra);
                let effective_models = time_models::resolve_time_models(&time_rules, &gp.platform.models, now_ms);
                resolve_model(&effective_models, source_model)
            };
            RouteResult {
                platform: gp.platform.clone(),
                target_model,
                mapping: mapping.cloned(),
            }
        })
        .collect();

    tracing::info!(
        group = %group.name, source_model = %source_model,
        candidate_count = candidates.len(), mode = ?group.routing_mode,
        first_platform = %candidates[0].platform.name,
        "candidates selected"
    );

    Ok(CandidateSet { candidates })
}

#[cfg(test)]
#[path = "test_candidates.rs"]
mod test_candidates;
