//! 候选选取：根据分组路由规则选择**有序候选平台列表**，用于失败逐个重试。

use super::super::db;
use super::super::models::*;
use super::super::scheduling::{Admission, BreakerThresholds, SchedulerState, StickyTable};
use super::super::time_models;
use super::super::peak_hours;
use super::model_mapping::resolve_model;
use super::ordering::{apply_coding_plan_priority, apply_sticky, expiry_sort_key, order_least_latency, order_load_balance};
use super::{candidate_state, RouteResult};
use std::collections::{HashMap, HashSet};

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

/// Platform extra 解析缓存：避免每个候选重复解析 time_models 和 peak_hours
#[derive(Clone, Debug)]
struct ExtraCache {
    time_models: Vec<serde_json::Value>,
    peak_windows: Vec<peak_hours::PeakWindow>,
}

impl ExtraCache {
    fn new(extra: &str) -> Self {
        let time_models = time_models::parse_platform_time_models(extra);
        let ptype = infer_protocol_from_extra(extra);
        let peak_windows = peak_hours::peak_hours_for(extra, &ptype);
        Self { time_models, peak_windows }
    }
}

/// 从 extra 字符串推断协议类型（用于 peak_hours 解析）
/// ponytail: 简化版，只处理常见情况；完整逻辑应参考 gateway/models/platform.rs
fn infer_protocol_from_extra(extra: &str) -> String {
    // 尝试从 extra 中提取 platform_type 字段
    if let Ok(v) = serde_json::from_str::<serde_json::Value>(extra) {
        if let Some(pt) = v.get("platform_type").and_then(|t| t.as_str()) {
            return pt.to_string();
        }
    }
    // 默认返回空字符串，peak_hours_for 会回落到 bundled preset
    String::new()
}

/// 候选平台的 extra 解析缓存（key: platform_id）
type ExtraCacheMap = HashMap<u64, ExtraCache>;

// ── cli-proxy（cpa-standalone-module s2）──

/// 从 `platform.extra` JSON 读 `cli_proxy_provider_id`（u64）。
/// 与 parse_disable_during_peak / parse_breaker 同 idiom：extra 是 JSON blob。
fn read_cli_proxy_provider_id(extra: &str) -> Option<u64> {
    if extra.trim().is_empty() {
        return None;
    }
    serde_json::from_str::<serde_json::Value>(extra)
        .ok()
        .and_then(|v| v.get("cli_proxy_provider_id").and_then(|x| x.as_u64()))
}

/// 用 provider 配置覆写 platform 的 wire 字段（endpoints/base_url/api_key）。
/// 注入单一合成 endpoint：protocol = provider.wire_protocol parse 为 Protocol；
/// parse 失败返回原 platform 不变（caller 已按 Protocol::CliProxy 过滤，正常应可 parse）。
fn apply_cli_proxy_override(mut p: Platform, provider: &CliProxyProvider) -> Platform {
    let wire: Protocol = serde_json::from_value(
        serde_json::Value::String(provider.wire_protocol.clone()),
    )
    .unwrap_or_else(|_| {
        tracing::warn!(
            platform_id = p.id, wire = %provider.wire_protocol,
            "cli-proxy: invalid wire_protocol, falling back to Anthropic"
        );
        Protocol::Anthropic
    });
    p.base_url = provider.base_url.clone();
    p.api_key = provider.api_key.clone();
    p.endpoints = vec![PlatformEndpoint {
        protocol: wire,
        base_url: provider.base_url.clone(),
        client_type: "default".to_string(),
        coding_plan: false,
    }];
    p
}

/// 解析 cli-proxy 平台的 effective target_model：source_model 在 provider.models 列表中
/// 则透传；否则回落到 provider.models 首项；列表空则透传 source（去 budget 后缀）。
fn resolve_cli_proxy_target_model(provider: &CliProxyProvider, source_model: &str) -> String {
    let base = source_model.split('[').next().unwrap_or(source_model);
    if provider.models.iter().any(|m| m == base) {
        return base.to_string();
    }
    provider.models.first().cloned().unwrap_or_else(|| base.to_string())
}

/// cli-proxy 平台预解析结果（per platform_id）。
/// Resolved = provider active 且 wire 可用；Skipped = provider 缺失/disabled → 平台排除。
#[derive(Default)]
struct CliProxyCache {
    providers: HashMap<u64, CliProxyProvider>,
    skip: HashSet<u64>,
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
    let effective_mode = group.routing_mode;

    // ── 阶段 -1: 预解析所有 platform.extra（避免每个候选重复解析）──
    // ponytail: 在入口处统一解析 time_models 和 peak_hours，缓存传递给 helper 函数
    let extra_cache: ExtraCacheMap = group_platforms.iter()
        .map(|gp| (gp.platform.id, ExtraCache::new(&gp.platform.extra)))
        .collect();

    // ── 阶段 -1b: cli-proxy 平台预解析（cpa-standalone-module s2）──
    // 每个 CliProxy 平台按 extra.cli_proxy_provider_id 拉 provider；缺失/disabled → skip（排除）。
    let mut cli_cache = CliProxyCache::default();
    for gp in &group_platforms {
        if gp.platform.platform_type != Protocol::CliProxy {
            continue;
        }
        let pid = match read_cli_proxy_provider_id(&gp.platform.extra) {
            Some(id) => id,
            None => {
                tracing::warn!(
                    platform_id = gp.platform.id, platform = %gp.platform.name,
                    "cli-proxy platform missing extra.cli_proxy_provider_id; excluding"
                );
                cli_cache.skip.insert(gp.platform.id);
                continue;
            }
        };
        match db::get_cli_proxy_provider(db, pid).await {
            Ok(Some(provider)) if provider.status == "active" => {
                cli_cache.providers.insert(gp.platform.id, provider);
            }
            Ok(Some(provider)) => {
                tracing::info!(
                    platform_id = gp.platform.id, provider_id = pid, status = %provider.status,
                    "cli-proxy provider not active; excluding platform"
                );
                cli_cache.skip.insert(gp.platform.id);
            }
            Ok(None) => {
                tracing::warn!(
                    platform_id = gp.platform.id, provider_id = pid,
                    "cli-proxy provider not found; excluding platform"
                );
                cli_cache.skip.insert(gp.platform.id);
            }
            Err(e) => {
                tracing::warn!(error = %e, platform_id = gp.platform.id, "cli-proxy provider fetch failed; excluding");
                cli_cache.skip.insert(gp.platform.id);
            }
        }
    }

    // ── 阶段 0: 单平台分组短路 ──
    if group_platforms.len() == 1 {
        return handle_single_platform(
            db, group, &group_platforms[0], source_model, ctx,
            &mapped_target_model, mapping, now_ms, &extra_cache, &cli_cache
        ).await;
    }

    // ── 阶段 1: 候选分桶过滤 ──
    let FilteredCandidates { mut active, mut probe, breaker_rejected, peak_disabled_count } =
        filter_candidates(&group_platforms, ctx, now_ms, source_model, &extra_cache, &cli_cache);

    // ── 阶段 2: 熔断全空回退透传 ──
    // 仅当熔断维度踢空（active+probe 皆空）且确有被熔断踢出的候选时回退；
    // 若空因 auto_disabled / 手动 disabled，则不回退（保持原 Err）。
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

    // ── 阶段 3: 按路由模式排序 ──
    sort_by_routing_mode(&mut active, &mut probe, effective_mode, ctx, now_ms);

    // ── 阶段 4: 合并 + 映射提升 ──
    // 显式映射目标平台不在候选集时记录 warn（沿用原逻辑）
    if let Some(target_id) = mapped_platform_id {
        let has_target = active.iter().any(|gp| gp.platform.id == target_id)
            || probe.iter().any(|gp| gp.platform.id == target_id);
        if !has_target {
            tracing::warn!(
                group = %group.name, target_platform_id = target_id,
                "mapped target platform not an available candidate, falling back to routing order"
            );
        }
    }
    let ordered = merge_and_promote_mapping(active, probe, mapped_platform_id);

    // ── 阶段 5: 空候选处理 ──
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

    // ── 阶段 6: 生成最终候选 ──
    let candidates = build_route_results(ordered, &mapped_target_model, now_ms, source_model, mapping, &extra_cache, &cli_cache);

    tracing::info!(
        group = %group.name, source_model = %source_model,
        candidate_count = candidates.len(), mode = ?group.routing_mode,
        first_platform = %candidates[0].platform.name,
        "candidates selected"
    );

    Ok(CandidateSet { candidates })
}

// ── Helper: 单平台分组短路逻辑 ──

/// 单平台分组：唯一平台熔断 Open / auto_disabled 时仍必请求（无视状态），
/// 手动 Disabled / 高峰禁用 + 命中窗口 / cli-proxy provider 缺失或 disabled → Err（唯一硬停）。
#[allow(clippy::too_many_arguments)]
async fn handle_single_platform(
    _db: &db::Db,
    group: &Group,
    only: &GroupPlatformDetail,
    source_model: &str,
    _ctx: Option<&ScheduleCtx<'_>>,
    mapped_target_model: &Option<String>,
    mapping: Option<&ModelMapping>,
    now_ms: i64,
    extra_cache: &ExtraCacheMap,
    cli_cache: &CliProxyCache,
) -> Result<CandidateSet, String> {
    // 手动 Disabled 是唯一硬停
    if only.platform.status == PlatformStatus::Disabled {
        return Err("group's only platform is manually disabled".to_string());
    }

    // cli-proxy 单平台且 provider 缺失/disabled → 硬停（同手动 disabled 语义）
    if only.platform.platform_type == Protocol::CliProxy && cli_cache.skip.contains(&only.platform.id) {
        return Err("group's only cli-proxy platform has missing or disabled provider".to_string());
    }

    // 高峰禁用优先级高于 status bypass（单平台组不 bypass 此维度）
    let cache = extra_cache.get(&only.platform.id);
    let peak_windows: &[peak_hours::PeakWindow] = cache.map(|c| c.peak_windows.as_slice()).unwrap_or_default();
    if is_in_peak_window_cached(peak_windows, now_ms, source_model) {
        tracing::info!(
            group = %group.name, platform = %only.platform.name,
            "single-platform group: peak-disabled, request blocked"
        );
        return Err("peak_disabled".to_string());
    }

    // 时段模型：从缓存获取 time_models，再用 resolve_model
    // cli-proxy 平台：effective_models 从 provider.models 覆盖（platform.models 只读）
    let target_model = if let Some(provider) = cli_cache.providers.get(&only.platform.id) {
        mapped_target_model
            .clone()
            .unwrap_or_else(|| resolve_cli_proxy_target_model(provider, source_model))
    } else {
        let time_rules: &[serde_json::Value] = cache.map(|c| c.time_models.as_slice()).unwrap_or_default();
        let effective_models = resolve_effective_models_cached(&only.platform, time_rules, now_ms, source_model);
        mapped_target_model
            .clone()
            .unwrap_or_else(|| resolve_model(&effective_models, source_model))
    };

    tracing::info!(
        group = %group.name, platform = %only.platform.name,
        status = ?only.platform.status,
        "single-platform group: bypassing status filter, forcing request"
    );

    // cli-proxy 平台：注入 provider 配置（wire/base_url/api_key/endpoints）
    let platform = if let Some(provider) = cli_cache.providers.get(&only.platform.id) {
        apply_cli_proxy_override(only.platform.clone(), provider)
    } else {
        only.platform.clone()
    };

    Ok(CandidateSet {
        candidates: vec![RouteResult {
            platform,
            target_model,
            mapping: mapping.cloned(),
        }],
    })
}

// ── Helper: 候选分桶过滤 ──

/// 候选分桶结果：active（健康/已过期试探）、probe（退避中）、breaker_rejected（熔断踢出）、peak_disabled_count
struct FilteredCandidates<'a> {
    active: Vec<&'a GroupPlatformDetail>,
    probe: Vec<&'a GroupPlatformDetail>,
    breaker_rejected: Vec<(&'a GroupPlatformDetail, Option<bool>)>,
    peak_disabled_count: usize,
}

/// 遍历 group_platforms 按 auto_disabled 三态分桶（enabled / 过期试探），
/// 再叠加熔断准入门（Open/HalfOpen 满踢出），高峰禁用计数。
/// cli-proxy 平台 provider 缺失/disabled（cli_cache.skip）→ 跳过（同 disabled 语义）。
fn filter_candidates<'a>(
    group_platforms: &'a [GroupPlatformDetail],
    ctx: Option<&ScheduleCtx<'_>>,
    now_ms: i64,
    source_model: &str,
    extra_cache: &ExtraCacheMap,
    cli_cache: &CliProxyCache,
) -> FilteredCandidates<'a> {
    let mut active = Vec::new();
    let mut probe = Vec::new();
    let mut breaker_rejected = Vec::new();
    let mut peak_disabled_count = 0;

    let breaker_enabled = ctx.map(|c| c.settings.enabled).unwrap_or(false);

    for gp in group_platforms {
        // cli-proxy provider 缺失/disabled → 直接跳过（等效 disabled，不进候选也不计入 peak）
        if cli_cache.skip.contains(&gp.platform.id) {
            continue;
        }
        // auto_disabled 维度（DB 持久态）
        let auto_state = candidate_state(&gp.platform, now_ms, source_model);
        if auto_state.is_none() {
            // 区分高峰禁用与其他排除原因（使用缓存避免重新解析）
            let cache = extra_cache.get(&gp.platform.id);
            let peak_windows: &[peak_hours::PeakWindow] = cache.map(|c| c.peak_windows.as_slice()).unwrap_or_default();
            if is_in_peak_window_cached(peak_windows, now_ms, source_model) {
                peak_disabled_count += 1;
            }
            continue; // 手动 disabled / auto_disabled 未到期 / 高峰禁用 → 跳过
        }

        // 熔断维度（内存态）：仅在有 ctx 且总开关开时判定
        if let Some(c) = ctx {
            if breaker_enabled {
                let (ft, os, hom) = c.settings.effective_thresholds(&gp.platform);
                let th = BreakerThresholds { failure_threshold: ft, open_secs: os, half_open_max: hom };
                match c.scheduler.admission(gp.platform.id, &th, now_ms, true) {
                    Admission::Reject => {
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

    FilteredCandidates { active, probe, breaker_rejected, peak_disabled_count }
}

// ── Helper: 按路由模式排序 ──

/// 按路由模式对 active/probe 桶排序（Failover/LoadBalance/LeastLatency/Sticky）。
fn sort_by_routing_mode(
    active: &mut Vec<&GroupPlatformDetail>,
    probe: &mut Vec<&GroupPlatformDetail>,
    mode: RoutingMode,
    ctx: Option<&ScheduleCtx<'_>>,
    now_ms: i64,
) {
    match mode {
        RoutingMode::Failover => {
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
            apply_coding_plan_priority(active);
            apply_coding_plan_priority(probe);
        }
        RoutingMode::LoadBalance | RoutingMode::HealthAware => {
            order_load_balance(active, now_ms);
            order_load_balance(probe, now_ms);
            apply_coding_plan_priority(active);
            apply_coding_plan_priority(probe);
        }
        RoutingMode::LeastLatency => {
            order_least_latency(active, ctx);
            order_least_latency(probe, ctx);
            apply_coding_plan_priority(active);
            apply_coding_plan_priority(probe);
        }
        RoutingMode::Sticky => {
            order_load_balance(active, now_ms);
            order_load_balance(probe, now_ms);
            apply_coding_plan_priority(active);
            apply_coding_plan_priority(probe);
            apply_sticky(active, ctx, now_ms);
        }
    }
}

// ── Helper: 合并 + 映射提升 ──

/// 合并 active+probe 桶（active 在前，probe 在后），再将显式映射目标平台提到最前。
fn merge_and_promote_mapping<'a>(
    active: Vec<&'a GroupPlatformDetail>,
    probe: Vec<&'a GroupPlatformDetail>,
    mapped_platform_id: Option<u64>,
) -> Vec<&'a GroupPlatformDetail> {
    let mut ordered = Vec::with_capacity(active.len() + probe.len());
    ordered.extend(active);
    ordered.extend(probe);

    // 显式映射目标平台提到最前（若它本身在候选集中）
    if let Some(target_id) = mapped_platform_id {
        if let Some(pos) = ordered.iter().position(|gp| gp.platform.id == target_id) {
            let gp = ordered.remove(pos);
            ordered.insert(0, gp);
        }
    }

    ordered
}

// ── Helper: 生成最终候选 ──

/// 为每个候选解析目标模型（时段模型 + resolve_model），构建 RouteResult 列表。
/// cli-proxy 平台：target_model 从 provider.models 覆盖，platform 注入 provider wire/base_url/api_key。
fn build_route_results(
    ordered: Vec<&GroupPlatformDetail>,
    mapped_target_model: &Option<String>,
    now_ms: i64,
    source_model: &str,
    mapping: Option<&ModelMapping>,
    extra_cache: &ExtraCacheMap,
    cli_cache: &CliProxyCache,
) -> Vec<RouteResult> {
    ordered
        .into_iter()
        .map(|gp| {
            // cli-proxy 分支：provider 已在 select 入口拉到缓存
            if let Some(provider) = cli_cache.providers.get(&gp.platform.id) {
                let target_model = mapped_target_model
                    .clone()
                    .unwrap_or_else(|| resolve_cli_proxy_target_model(provider, source_model));
                let platform = apply_cli_proxy_override(gp.platform.clone(), provider);
                return RouteResult {
                    platform,
                    target_model,
                    mapping: mapping.cloned(),
                };
            }
            let target_model = if let Some(tm) = mapped_target_model.as_ref() {
                tm.clone()
            } else {
                let cache = extra_cache.get(&gp.platform.id);
                let time_rules: &[serde_json::Value] = cache.map(|c| c.time_models.as_slice()).unwrap_or_default();
                let effective_models = resolve_effective_models_cached(&gp.platform, time_rules, now_ms, source_model);
                resolve_model(&effective_models, source_model)
            };
            RouteResult {
                platform: gp.platform.clone(),
                target_model,
                mapping: mapping.cloned(),
            }
        })
        .collect()
}

/// 缓存版本的 peak_hours 窗口判定（避免重新解析 peak_hours）
fn is_in_peak_window_cached(windows: &[peak_hours::PeakWindow], now_ms: i64, source_model: &str) -> bool {
    peak_hours::is_in_peak_window(windows, now_ms, source_model)
}

/// 解析当前时段的有效模型配置（effective_models）—— 使用已解析的 time_models 缓存。
///
/// 三层级联（优先级高 → 低）：
/// 1. **time_models**（用户级显式时段切换，`platform.extra.time_models`）：命中 → 用该时段 models；
///    用户已自定义 time_models 时不再应用 preset peak 分支（用户显式覆盖优先）。
/// 2. **preset.models.peak**（preset 级高峰分支，PRD 07-11）：用户未配 time_models 且
///    preset 提供本协议 `models.peak` 分支 + 当前命中 `peak_hours_for` 任一窗口 → 用 peak 替换。
///    **设计意图：peak 分支为 preset 级硬约束，覆盖用户手工定制的 `platform.models`**
///    （等同 coding_plan 端点维度优先级；用户显式定制在高峰窗口期内不保留，如需保留请配
///    `time_models` 显式时段切换，其优先级高于 peak）。非 bug — 见 CLAUDE.md `models.peak` 段。
/// 3. **platform.models**（用户级显式槽位 / 创建时填入 preset.models.default）：兜底默认。
///
/// `source_model`：请求模型名（透传给 peak_hours model scope 过滤；空串 = 无上下文跳过）。
fn resolve_effective_models_cached(
    platform: &Platform,
    time_rules: &[serde_json::Value],
    now_ms: i64,
    source_model: &str,
) -> PlatformModels {
    let mut effective = time_models::resolve_time_models(time_rules, &platform.models, now_ms);
    // PRD 07-11：time_models 未自定义时查 preset.models.peak 分支
    if time_rules.is_empty() {
        // serde rename 裸名（如 "glm_coding"），同 is_peak_disabled / calc_est_cost 取名模式
        let ptype = serde_json::to_string(&platform.platform_type)
            .unwrap_or_default()
            .trim_matches('"')
            .to_string();
        if let Some(peak_models) = super::super::peak_hours::default_peak_models(&ptype) {
            let windows = super::super::peak_hours::peak_hours_for(&platform.extra, &ptype);
            if super::super::peak_hours::is_in_peak_window(&windows, now_ms, source_model) {
                effective = peak_models;
            }
        }
    }
    effective
}

/// 解析当前时段的有效模型配置（effective_models）—— 原始版本（保留兼容）。
///
/// 注意：此函数会重新解析 platform.extra，推荐使用 resolve_effective_models_cached 或预解析缓存。
///
/// 三层级联（优先级高 → 低）：
/// 1. **time_models**（用户级显式时段切换，`platform.extra.time_models`）：命中 → 用该时段 models；
///    用户已自定义 time_models 时不再应用 preset peak 分支（用户显式覆盖优先）。
/// 2. **preset.models.peak**（preset 级高峰分支，PRD 07-11）：用户未配 time_models 且
///    preset 提供本协议 `models.peak` 分支 + 当前命中 `peak_hours_for` 任一窗口 → 用 peak 替换。
///    **设计意图：peak 分支为 preset 级硬约束，覆盖用户手工定制的 `platform.models`**
///    （等同 coding_plan 端点维度优先级；用户显式定制在高峰窗口期内不保留，如需保留请配
///    `time_models` 显式时段切换，其优先级高于 peak）。非 bug — 见 CLAUDE.md `models.peak` 段。
/// 3. **platform.models**（用户级显式槽位 / 创建时填入 preset.models.default）：兜底默认。
///
/// `source_model`：请求模型名（透传给 peak_hours model scope 过滤；空串 = 无上下文跳过）。
#[allow(dead_code)]
// 保留供 test_candidates.rs 直接调（cached 版走生产路径；test 需原始函数测基础逻辑）
fn resolve_effective_models(
    platform: &Platform,
    time_rules: &[serde_json::Value],
    now_ms: i64,
    source_model: &str,
) -> PlatformModels {
    let mut effective = time_models::resolve_time_models(time_rules, &platform.models, now_ms);
    // PRD 07-11：time_models 未自定义时查 preset.models.peak 分支
    if time_rules.is_empty() {
        // serde rename 裸名（如 "glm_coding"），同 is_peak_disabled / calc_est_cost 取名模式
        let ptype = serde_json::to_string(&platform.platform_type)
            .unwrap_or_default()
            .trim_matches('"')
            .to_string();
        if let Some(peak_models) = super::super::peak_hours::default_peak_models(&ptype) {
            let windows = super::super::peak_hours::peak_hours_for(&platform.extra, &ptype);
            if super::super::peak_hours::is_in_peak_window(&windows, now_ms, source_model) {
                effective = peak_models;
            }
        }
    }
    effective
}

#[cfg(test)]
#[path = "test_candidates.rs"]
mod test_candidates;
