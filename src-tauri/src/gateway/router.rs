use super::db;
use super::models::*;
use super::scheduling::{Admission, BreakerThresholds, SchedulerState, StickyTable};

/// 出站 max_tokens 裁剪（convert_request 前调用）。
///
/// 保守策略（Q3）：仅当客户端显式传了 max_tokens **且**超过模型上限时裁剪到上限；
/// 未传（None）不注入默认值；模型无上限记录（None）不裁剪。
///
/// 返回 (裁剪后值, 是否发生裁剪)。
pub fn cap_max_tokens(req_max: Option<u32>, model_max: Option<i64>) -> (Option<u32>, bool) {
    match (req_max, model_max) {
        (Some(req), Some(limit)) if limit > 0 && (req as i64) > limit => (Some(limit as u32), true),
        _ => (req_max, false),
    }
}

/// 路由结果
pub struct RouteResult {
    pub platform: Platform,
    pub target_model: String,
    /// 匹配到的模型映射（如果有），用于读取超时等维度配置
    pub mapping: Option<ModelMapping>,
}

/// 候选选取结果：有序的候选平台列表（首个为最优先），用于失败逐个重试。
/// `target_model` / `mapping` 对每个候选独立解析（显式映射命中时全部候选共享映射目标模型；
/// 否则按各平台 PlatformModels 自动匹配）。
pub struct CandidateSet {
    pub candidates: Vec<RouteResult>,
}

/// 判定平台当前是否可作为候选纳入：
/// - Enabled：始终纳入
/// - AutoDisabled 且已过退避试探时间（now >= until）：纳入（末尾试探）
/// - Disabled（用户手动）/ AutoDisabled 未到试探时间：排除
fn candidate_state(platform: &Platform, now_ms: i64) -> Option<bool> {
    match platform.status {
        PlatformStatus::Enabled => Some(false),
        PlatformStatus::AutoDisabled if now_ms >= platform.auto_disabled_until => Some(true),
        _ => None,
    }
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

/// 调度上下文（proxy 持有；scheduler 为 per-platform 健康/熔断指标，sticky 为粘性绑定表）。
pub struct ScheduleCtx<'a> {
    pub scheduler: &'a SchedulerState,
    pub sticky: &'a StickyTable,
    pub settings: &'a SchedulingBreakerSettings,
    /// Sticky 模式 session 键（group_key + 客户端稳定标识，调用侧拼接）。
    pub sticky_key: Option<String>,
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
        let target_model = mapped_target_model
            .clone()
            .unwrap_or_else(|| resolve_model(&only.platform.models, source_model));
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
    for gp in &group_platforms {
        // auto_disabled 维度（DB 持久态）
        let auto_state = candidate_state(&gp.platform, now_ms);
        if auto_state.is_none() {
            continue; // 用户手动 disabled / auto_disabled 未到退避 → 跳过
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
            active.sort_by_key(|gp| gp.priority);
            probe.sort_by_key(|gp| gp.priority);
        }
        // LoadBalance / HealthAware：健康集加权随机（准入门已摘 Open，等价加权随机 on 健康集）
        RoutingMode::LoadBalance | RoutingMode::HealthAware => {
            order_load_balance(&mut active, now_ms);
            order_load_balance(&mut probe, now_ms);
        }
        // LeastLatency：按延迟 EMA 升序（无样本视为最大，排末尾）
        RoutingMode::LeastLatency => {
            order_least_latency(&mut active, ctx);
            order_least_latency(&mut probe, ctx);
        }
        // Sticky：绑定平台若健康提到首位，否则回退加权随机 + 写绑定
        RoutingMode::Sticky => {
            order_load_balance(&mut active, now_ms);
            order_load_balance(&mut probe, now_ms);
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
        return Err("no available platform (all disabled, backing off, or circuit-broken)".to_string());
    }

    // 5. 为每个候选解析目标模型
    let candidates: Vec<RouteResult> = ordered
        .into_iter()
        .map(|gp| {
            let target_model = if let Some(ref tm) = mapped_target_model {
                tm.clone()
            } else {
                resolve_model(&gp.platform.models, source_model)
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

/// 负载均衡排序：加权随机决定首个，其余按 weight 降序，保证所有候选都可被重试。
fn order_load_balance(platforms: &mut Vec<&GroupPlatformDetail>, seed: i64) {
    if platforms.len() <= 1 {
        return;
    }
    let total_weight: i32 = platforms.iter().map(|gp| gp.weight.max(0)).sum();
    // 先按 weight 降序作为基础顺序
    platforms.sort_by_key(|gp| std::cmp::Reverse(gp.weight));
    if total_weight <= 0 {
        return;
    }
    // 加权随机选首个（用时间种子），将其移到最前
    let mut rand_val = (seed.unsigned_abs() as i32) % total_weight;
    let mut pick = 0usize;
    for (i, gp) in platforms.iter().enumerate() {
        rand_val -= gp.weight.max(0);
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
fn order_least_latency(platforms: &mut [&GroupPlatformDetail], ctx: Option<&ScheduleCtx<'_>>) {
    let Some(c) = ctx else { return };
    platforms.sort_by(|a, b| {
        let la = c.scheduler.latency_ema(a.platform.id).unwrap_or(f64::MAX);
        let lb = c.scheduler.latency_ema(b.platform.id).unwrap_or(f64::MAX);
        la.partial_cmp(&lb).unwrap_or(std::cmp::Ordering::Equal)
    });
}

/// Sticky：若 session 键命中已绑定平台且该平台仍在健康候选集中，提到首位；
/// 否则把当前首选（加权随机已定）写为新绑定。失效 / 熔断的旧绑定自然回退（不在集中即重绑）。
fn apply_sticky(platforms: &mut [&GroupPlatformDetail], ctx: Option<&ScheduleCtx<'_>>, now_ms: i64) {
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

/// 根据分组路由规则选择平台
#[allow(dead_code)]
pub async fn select_platform(
    db: &db::Db,
    group: &Group,
    source_model: &str,
) -> Result<RouteResult, String> {
    // 1. 查找模型映射（内联于 group.model_mappings）
    let mapping = group.model_mappings.iter().find(|m| m.source_model == source_model);

    let (target_platform_id, target_model) = if let Some(m) = mapping {
        (m.target_platform_id, m.target_model.clone())
    } else {
        // 无显式映射（0 表示未指定）
        (0u64, source_model.to_string())
    };

    // 2. 获取分组中的平台列表
    let group_platforms = db::get_group_platforms(db, group.id).await?;
    if group_platforms.is_empty() {
        return Err("group has no platforms".to_string());
    }

    // 3. 如果有指定目标平台，优先使用
    if target_platform_id != 0 {
        if let Some(gp) = group_platforms.iter().find(|gp| gp.platform.id == target_platform_id) {
            tracing::info!(
                group = %group.name, source_model = %source_model, target_model = %target_model,
                platform = %gp.platform.name, platform_id = gp.platform.id,
                strategy = "explicit-mapping", "route selected"
            );
            return Ok(RouteResult {
                platform: gp.platform.clone(),
                target_model,
                mapping: mapping.cloned(),
            });
        }
        tracing::warn!(
            group = %group.name, target_platform_id,
            "mapped target platform not in group, falling back to routing mode"
        );
    }

    // 4. 根据路由模式选择平台
    let platform = match group.routing_mode {
        RoutingMode::Failover => select_failover(&group_platforms),
        // LoadBalance / HealthAware / LeastLatency / Sticky 在此简化路径均按加权随机
        // （本 fn 已 deprecated，仅 select_candidates_ctx 实现完整策略；保留编译）。
        _ => select_load_balance(&group_platforms),
    }?;

    // 5. 无显式映射时，按平台 PlatformModels 自动匹配模型
    let target_model = if mapping.is_none() {
        resolve_model(&platform.models, source_model)
    } else {
        target_model
    };

    tracing::info!(
        group = %group.name, source_model = %source_model, target_model = %target_model,
        platform = %platform.name, platform_id = platform.id,
        mode = ?group.routing_mode,
        strategy = if mapping.is_some() { "mapping+mode" } else { "auto-match" },
        "route selected"
    );

    Ok(RouteResult {
        platform,
        target_model,
        mapping: mapping.cloned(),
    })
}

/// 根据平台模型配置自动匹配请求模型。
/// 匹配规则：请求模型名（小写）包含槽位名（opus/sonnet/haiku/gpt）→ 使用该槽位值；
/// 全部不匹配 → 使用 default；无 default → 透传原始模型（去掉 [... ] 后缀）。
fn resolve_model(models: &PlatformModels, source_model: &str) -> String {
    // Strip Claude Code budget suffix like [1m], [128k]
    let base_model = source_model.split('[').next().unwrap_or(source_model);
    let lower = base_model.to_lowercase();
    let slots: [(&str, &Option<String>); 4] = [
        ("opus", &models.opus),
        ("sonnet", &models.sonnet),
        ("haiku", &models.haiku),
        ("gpt", &models.gpt),
    ];
    for (slot_name, slot_value) in &slots {
        if lower.contains(slot_name) {
            if let Some(v) = slot_value {
                return v.clone();
            }
        }
    }
    // 回退到 default
    if let Some(ref default) = models.default {
        return default.clone();
    }
    // 无匹配无 default — 透传（去掉 budget 后缀）
    base_model.to_string()
}

/// 故障转移：按 priority 升序选第一个 enabled 的
fn select_failover(platforms: &[GroupPlatformDetail]) -> Result<Platform, String> {
    let mut sorted: Vec<_> = platforms.iter().collect();
    sorted.sort_by_key(|gp| gp.priority);

    sorted
        .into_iter()
        .find(|gp| gp.platform.enabled)
        .map(|gp| gp.platform.clone())
        .ok_or_else(|| "no enabled platform for failover".to_string())
}

/// 负载均衡：加权随机选择
fn select_load_balance(platforms: &[GroupPlatformDetail]) -> Result<Platform, String> {
    let enabled: Vec<_> = platforms.iter().filter(|gp| gp.platform.enabled).collect();
    if enabled.is_empty() {
        return Err("no enabled platform for load balance".to_string());
    }

    let total_weight: i32 = enabled.iter().map(|gp| gp.weight).sum();
    if total_weight <= 0 {
        return Ok(enabled[0].platform.clone());
    }

    // 简单加权随机
    let mut rand_val = (std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as i32)
        % total_weight;

    for gp in &enabled {
        rand_val -= gp.weight;
        if rand_val < 0 {
            return Ok(gp.platform.clone());
        }
    }

    Ok(enabled[0].platform.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mk_platform(status: PlatformStatus, until: i64) -> Platform {
        Platform {
            id: 1,
            name: "p".into(),
            platform_type: Protocol::Anthropic,
            base_url: String::new(),
            api_key: String::new(),
            extra: String::new(),
            models: PlatformModels::default(),
            available_models: vec![],
            endpoints: vec![],
            enabled: status == PlatformStatus::Enabled,
            status,
            auto_disabled_until: until,
            auto_disable_strikes: 0,
            created_at: 0,
            updated_at: 0,
            deleted_at: 0,
            est_balance_remaining: 0.0,
            est_coding_plan: String::new(),
            last_real_query_at: 0,
            estimate_count: 0,
            show_in_tray: false,
            tray_display: String::new(),
            sort_order: 0,
            manual_budgets: vec![],
            balance_level: String::new(),
        }
    }

    fn mk_platform_id(id: u64) -> Platform {
        let mut p = mk_platform(PlatformStatus::Enabled, 0);
        p.id = id;
        p
    }

    fn mk_gp(id: u64, weight: i32, priority: i32) -> GroupPlatformDetail {
        GroupPlatformDetail { platform: mk_platform_id(id), priority, weight }
    }

    fn mk_settings() -> SchedulingBreakerSettings {
        SchedulingBreakerSettings::default()
    }

    #[test]
    fn least_latency_orders_by_ema_ascending() {
        let sched = SchedulerState::new();
        // p1 EMA=300, p2 EMA=100, p3 无样本(MAX)
        sched.inc_inflight(1); sched.record_success(1, 300);
        sched.inc_inflight(2); sched.record_success(2, 100);
        let sticky = StickyTable::new();
        let settings = mk_settings();
        let ctx = ScheduleCtx { scheduler: &sched, sticky: &sticky, settings: &settings, sticky_key: None };

        let gp1 = mk_gp(1, 1, 0);
        let gp2 = mk_gp(2, 1, 0);
        let gp3 = mk_gp(3, 1, 0);
        let mut v: Vec<&GroupPlatformDetail> = vec![&gp1, &gp2, &gp3];
        order_least_latency(&mut v, Some(&ctx));
        // 升序: p2(100) < p1(300) < p3(MAX)
        assert_eq!(v[0].platform.id, 2);
        assert_eq!(v[1].platform.id, 1);
        assert_eq!(v[2].platform.id, 3);
    }

    #[test]
    fn breaker_union_autodisabled_admission() {
        // 验证熔断 ∪ auto_disabled 取并集：分别独立判定。
        let sched = SchedulerState::new();
        let now = db::now();
        let th = BreakerThresholds { failure_threshold: 1, open_secs: 1800, half_open_max: 2 };
        // p1 熔断 Open
        sched.inc_inflight(1);
        sched.record_failure(1, &th, now);
        assert_eq!(sched.admission(1, &th, now, true), Admission::Reject);
        // p2 健康
        assert_eq!(sched.admission(2, &th, now, true), Admission::Allow);
        // auto_disabled 维度独立：candidate_state 判定（不被熔断改写）
        let p_auto = mk_platform(PlatformStatus::AutoDisabled, now + 5000);
        assert_eq!(candidate_state(&p_auto, now), None); // auto_disabled 未到期 → 排除
        // 熔断状态不影响 candidate_state（auto_disabled 维度）
        let p_enabled = mk_platform_id(1);
        assert_eq!(candidate_state(&p_enabled, now), Some(false)); // 仍 enabled（熔断不改 DB status）
    }

    #[test]
    fn breaker_does_not_overwrite_autodisabled() {
        // 熔断与 auto_disabled 状态互不覆盖：record_failure 只动内存 breaker，不动 platform.status。
        let sched = SchedulerState::new();
        let now = db::now();
        let th = BreakerThresholds { failure_threshold: 1, open_secs: 1800, half_open_max: 2 };
        sched.inc_inflight(1);
        sched.record_failure(1, &th, now);
        // platform.status 仍是 Enabled（熔断不写 DB 三态）
        let p = mk_platform_id(1);
        assert_eq!(p.status, PlatformStatus::Enabled);
        // 内存 breaker 是 Open
        assert!(matches!(sched.breaker_state(1), super::super::scheduling::BreakerState::Open { .. }));
    }

    #[test]
    fn sticky_binds_then_falls_back() {
        let sched = SchedulerState::new();
        let sticky = StickyTable::new();
        let settings = mk_settings();
        let now = db::now();
        let ctx = ScheduleCtx {
            scheduler: &sched, sticky: &sticky, settings: &settings,
            sticky_key: Some("grpA|client1".to_string()),
        };
        let gp1 = mk_gp(1, 1, 0);
        let gp2 = mk_gp(2, 1, 0);

        // 首次：无绑定 → 写绑定为首选 p1
        let mut v: Vec<&GroupPlatformDetail> = vec![&gp1, &gp2];
        apply_sticky(&mut v, Some(&ctx), now);
        assert_eq!(sticky.get("grpA|client1", now), Some(1));

        // 再次：绑定 p1 健康（在集中），无论入参顺序如何，p1 提首位
        let mut v2: Vec<&GroupPlatformDetail> = vec![&gp2, &gp1];
        apply_sticky(&mut v2, Some(&ctx), now);
        assert_eq!(v2[0].platform.id, 1);

        // 绑定平台 p1 不在候选集（熔断/失效）→ 回退首选并重绑 p2
        let gp3 = mk_gp(3, 1, 0);
        let mut v3: Vec<&GroupPlatformDetail> = vec![&gp2, &gp3];
        apply_sticky(&mut v3, Some(&ctx), now);
        assert_eq!(sticky.get("grpA|client1", now), Some(2)); // 重绑为新首选
    }

    #[test]
    fn candidate_state_filtering() {
        let now = 1_000_000i64;
        // enabled → 始终纳入（非试探）
        assert_eq!(candidate_state(&mk_platform(PlatformStatus::Enabled, 0), now), Some(false));
        // 用户手动 disabled → 排除
        assert_eq!(candidate_state(&mk_platform(PlatformStatus::Disabled, 0), now), None);
        // auto_disabled 未到退避时间 → 排除
        assert_eq!(candidate_state(&mk_platform(PlatformStatus::AutoDisabled, now + 5000), now), None);
        // auto_disabled 已过退避时间 → 纳入（末尾试探）
        assert_eq!(candidate_state(&mk_platform(PlatformStatus::AutoDisabled, now - 1), now), Some(true));
    }

    #[test]
    fn cap_max_tokens_logic() {
        // 超限 → 裁剪到上限
        assert_eq!(cap_max_tokens(Some(100_000), Some(8192)), (Some(8192), true));
        // 未超限 → 原值不变
        assert_eq!(cap_max_tokens(Some(4096), Some(8192)), (Some(4096), false));
        // 恰好等于上限 → 不裁剪
        assert_eq!(cap_max_tokens(Some(8192), Some(8192)), (Some(8192), false));
        // 客户端未传 → 不注入（None 透传）
        assert_eq!(cap_max_tokens(None, Some(8192)), (None, false));
        // 模型无上限记录 → 不裁剪（即便客户端传了巨大值）
        assert_eq!(cap_max_tokens(Some(1_000_000), None), (Some(1_000_000), false));
        // 模型上限为 0（异常数据）→ 视作无限制不裁剪
        assert_eq!(cap_max_tokens(Some(100_000), Some(0)), (Some(100_000), false));
    }

    async fn mk_test_db() -> db::Db {
        let db = db::Db::new(":memory:").await.expect("open memory db");
        db.init_tables().await.expect("init tables");
        db
    }

    async fn mk_db_platform(db: &db::Db, name: &str) -> Platform {
        db::create_platform(db, CreatePlatform {
            name: name.into(),
            platform_type: Protocol::Anthropic,
            base_url: "https://example.invalid".into(),
            api_key: "k".into(),
            extra: String::new(),
            models: None, available_models: None, endpoints: None, manual_budgets: None,
            auto_group: None, join_group_ids: None,
        }).await.expect("create platform")
    }

    async fn mk_db_group(db: &db::Db, name: &str, platform_ids: &[u64]) -> Group {
        let g = db::create_group(db, CreateGroup {
            name: name.into(),
            group_key: Some(name.into()),
            routing_mode: RoutingMode::Failover,
            auto_from_platform: String::new(),
            request_timeout_secs: 0, connect_timeout_secs: 0,
            source_protocol: Some("anthropic".into()),
            max_retries: 2, model_mappings: vec![],
        }).await.expect("create group");
        let inputs: Vec<GroupPlatformInput> = platform_ids.iter().enumerate()
            .map(|(i, &pid)| GroupPlatformInput { platform_id: pid, priority: Some(i as i32), weight: Some(1) })
            .collect();
        db::set_group_platforms(db, g.id, &inputs).await.expect("set group platforms");
        g
    }

    /// 单平台分组：唯一平台熔断 Open 时仍必请求（无视状态），不踢空 blackhole。
    #[tokio::test]
    async fn single_platform_forces_request_when_circuit_broken() {
        let db = mk_test_db().await;
        let p = mk_db_platform(&db, "GLM").await;
        let g = mk_db_group(&db, "single", &[p.id]).await;

        // 把唯一平台熔断 Open
        let sched = SchedulerState::new();
        let now = db::now();
        let th = BreakerThresholds { failure_threshold: 1, open_secs: 1800, half_open_max: 2 };
        sched.inc_inflight(p.id);
        sched.record_failure(p.id, &th, now);
        assert_eq!(sched.admission(p.id, &th, now, true), Admission::Reject);

        let sticky = StickyTable::new();
        // 总开关开，否则熔断维度不参与
        let settings = SchedulingBreakerSettings { enabled: true, ..Default::default() };
        let ctx = ScheduleCtx { scheduler: &sched, sticky: &sticky, settings: &settings, sticky_key: None };

        // 单平台短路：无视熔断必请求
        let set = select_candidates_ctx(&db, &g, "claude-opus-4-8", Some(&ctx)).await
            .expect("single platform must force request, not Err");
        assert_eq!(set.candidates.len(), 1);
        assert_eq!(set.candidates[0].platform.id, p.id);
    }

    /// 单平台分组：唯一平台 auto_disabled（401/403 退避中）时仍必请求。
    #[tokio::test]
    async fn single_platform_forces_request_when_auto_disabled() {
        let db = mk_test_db().await;
        let p = mk_db_platform(&db, "GLM").await;
        let g = mk_db_group(&db, "single", &[p.id]).await;
        // 置 auto_disabled（退避未到期）
        db::set_platform_auto_disabled(&db, p.id).await.expect("set auto_disabled");

        let sched = SchedulerState::new();
        let sticky = StickyTable::new();
        let settings = SchedulingBreakerSettings::default();
        let ctx = ScheduleCtx { scheduler: &sched, sticky: &sticky, settings: &settings, sticky_key: None };

        let set = select_candidates_ctx(&db, &g, "claude-opus-4-8", Some(&ctx)).await
            .expect("single platform auto_disabled must still force request");
        assert_eq!(set.candidates.len(), 1);
        assert_eq!(set.candidates[0].platform.id, p.id);
    }

    /// 单平台分组：唯一平台手动 Disabled 是显式关停 → 仍 Err（唯一硬停）。
    #[tokio::test]
    async fn single_platform_manual_disabled_errs() {
        let db = mk_test_db().await;
        let p = mk_db_platform(&db, "GLM").await;
        let g = mk_db_group(&db, "single", &[p.id]).await;
        db::update_platform(&db, UpdatePlatform {
            id: p.id, name: None, platform_type: None, base_url: None, api_key: None,
            extra: None, models: None, available_models: None, endpoints: None,
            enabled: None, status: Some(PlatformStatus::Disabled), manual_budgets: None,
            join_group_ids: None,
        }).await.expect("disable");

        let sched = SchedulerState::new();
        let sticky = StickyTable::new();
        let settings = SchedulingBreakerSettings::default();
        let ctx = ScheduleCtx { scheduler: &sched, sticky: &sticky, settings: &settings, sticky_key: None };

        let res = select_candidates_ctx(&db, &g, "claude-opus-4-8", Some(&ctx)).await;
        assert!(res.is_err(), "manually disabled sole platform must Err");
    }

    /// 多平台分组：仍按平台状态过滤（一坏一好 → 只选好的）；全坏熔断 → 回退透传。
    #[tokio::test]
    async fn multi_platform_respects_status_and_falls_back_when_all_broken() {
        let db = mk_test_db().await;
        let p1 = mk_db_platform(&db, "GLM").await;
        let p2 = mk_db_platform(&db, "GLM2").await;
        let g = mk_db_group(&db, "multi", &[p1.id, p2.id]).await;

        let sched = SchedulerState::new();
        let now = db::now();
        let th = BreakerThresholds { failure_threshold: 1, open_secs: 1800, half_open_max: 2 };
        // 仅 p1 熔断 Open，p2 健康
        sched.inc_inflight(p1.id);
        sched.record_failure(p1.id, &th, now);

        let sticky = StickyTable::new();
        let settings = SchedulingBreakerSettings { enabled: true, ..Default::default() };
        let ctx = ScheduleCtx { scheduler: &sched, sticky: &sticky, settings: &settings, sticky_key: None };

        // 有健康平台 → 只选 p2（坏的被过滤）
        let set = select_candidates_ctx(&db, &g, "claude-opus-4-8", Some(&ctx)).await.expect("ok");
        assert_eq!(set.candidates.len(), 1);
        assert_eq!(set.candidates[0].platform.id, p2.id);

        // p2 也熔断 → 全坏 → 回退透传，两候选都回（不 blackhole）
        sched.inc_inflight(p2.id);
        sched.record_failure(p2.id, &th, now);
        let set2 = select_candidates_ctx(&db, &g, "claude-opus-4-8", Some(&ctx)).await
            .expect("all-broken multi must fall back, not Err");
        assert_eq!(set2.candidates.len(), 2);
    }
}
