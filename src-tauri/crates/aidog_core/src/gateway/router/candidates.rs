//! 候选选取：根据分组路由规则选择**有序候选平台列表**，用于失败逐个重试。

use super::super::models::*;
use super::super::peak;
use super::super::scheduling::{Admission, BreakerThresholds, SchedulerState, StickyTable};
use super::super::time_windows;
use super::model_mapping::{resolve_decision_model, resolve_model};
use super::ordering::{
    apply_coding_plan_priority, apply_sticky, expiry_sort_key, is_coding_plan,
    order_load_balance,
};
use super::{RouteResult, candidate_state, sole_platform};
use aidog_db as db;
use aidog_db::models::{RouteCandidate, RouteReason, RouteTag, RoutingSnapshot};
use std::collections::{HashMap, HashSet};

/// 候选选取结果：有序的候选平台列表（首个为最优先），用于失败逐个重试。
/// `target_model` / `mapping` 对每个候选独立解析：显式映射命中时先把模型名换成映射目标名，
/// 再按各平台 PlatformModels 槽位匹配（映射不跳过槽位规则）。
#[derive(Debug)]
pub struct CandidateSet {
    pub candidates: Vec<RouteResult>,
    /// 路由决策快照（票 01）：随 proxy_log.routing 落库。
    pub routing: RoutingSnapshot,
}

/// 路由失败：`reason` 语义与旧 `String` 错误完全一致（audit 字面量匹配经
/// Deref<Target=str> / PartialEq<&str> / Display 兼容），额外携带已评估快照，
/// 供调用方随 proxy_log.routing 落库（票 01）。
#[derive(Debug, Clone)]
pub struct RouteFail {
    pub reason: String,
    pub routing: RoutingSnapshot,
}

impl std::fmt::Display for RouteFail {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.reason)
    }
}

impl std::ops::Deref for RouteFail {
    type Target = str;
    fn deref(&self) -> &str {
        &self.reason
    }
}

impl PartialEq<&str> for RouteFail {
    fn eq(&self, other: &&str) -> bool {
        self.reason == *other
    }
}

impl From<String> for RouteFail {
    fn from(reason: String) -> Self {
        Self {
            reason,
            routing: RoutingSnapshot::default(),
        }
    }
}

/// 请求类型（jev-decision-proxy §3.3.3）：候选过滤按类型剔除不支持的平台。
/// - `Chat`：普通聊天请求，剔除纯决策平台（`is_decision_only`，R3）
/// - `Decision`：`/v1/systemone` 决策请求，剔除不支持决策的平台（R2）
/// - `Tools(path)`：独立搜索/tools 请求（spec-独立搜索转发 语义 3）：候选 = 组内
///   `tools_endpoints` 含 path 与入站 path 相等条目的平台；携带入站 path 供逐平台匹配。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RequestKind<'a> {
    Chat,
    Decision,
    Tools(&'a str),
}

impl RequestKind<'_> {
    /// 该类型下平台不满足要求、且全部候选均因此被剔除时的路由 Err 字符串（R7）。
    pub(crate) fn no_platform_err(&self) -> &'static str {
        match self {
            RequestKind::Chat => "no_chat_platform",
            RequestKind::Decision => "no_decision_platform",
            RequestKind::Tools(_) => "no_tools_platform",
        }
    }
}

/// registry 中 capabilities 含 `decision` 的模型，按 `platform_code → model_id 集合` 键控（R2 推导源）：
/// 能力看**该平台自己的** registry 条目（同名模型在别的平台有 decision 能力不算）。
#[derive(Default)]
pub(crate) struct DecisionModels(HashMap<String, HashSet<String>>);

impl DecisionModels {
    /// `models` 里第一个在 `platform` 对应 registry 条目上带 decision 能力的模型。
    pub(crate) fn first_in<'a>(&self, platform: &Platform, models: &'a [String]) -> Option<&'a String> {
        let ids = self.0.get(&platform.platform_type.wire_str())?;
        models.iter().find(|m| ids.contains(m.as_str()))
    }
}

/// 平台是否支持该请求类型（R2 / R3）：
/// - Decision ⇔ `models.jev` 非空，或 `available_models` 含该平台 registry decision 能力模型；
/// - Chat ⇔ 非纯决策平台（jev 配置且其余槽位全空才剔除）；
/// - Tools(path) ⇔ 平台 tools_endpoints 含 path 与入站 path 相等条目（语义 3，strict 逐平台）。
pub(crate) fn platform_supports_kind(
    platform: &Platform,
    kind: RequestKind<'_>,
    decision_models: &DecisionModels,
) -> bool {
    match kind {
        RequestKind::Decision => {
            platform.models.supports_decision_slot()
                || decision_models
                    .first_in(platform, &platform.available_models)
                    .is_some()
        }
        RequestKind::Chat => !platform.models.is_decision_only(),
        RequestKind::Tools(path) => {
            crate::gateway::billing::default_tools_endpoints(&platform.platform_type.wire_str())
                .is_some_and(|m| {
                    m.values()
                        .any(|u| crate::gateway::billing::tools_url_matches(u, path))
                })
        }
    }
}

/// 一次全表读出 registry decision 能力模型（R2 推导源）。
/// 仅决策请求调用（热路径聊天请求零开销）。
async fn load_decision_models(db: &db::Db) -> DecisionModels {
    match db::list_model_entries(db, None).await {
        Ok(entries) => {
            let mut map: HashMap<String, HashSet<String>> = HashMap::new();
            for e in entries
                .into_iter()
                .filter(|e| e.capabilities.iter().any(|c| c == "decision"))
            {
                map.entry(e.platform_code).or_default().insert(e.model_id);
            }
            DecisionModels(map)
        }
        Err(e) => {
            // 读库失败按空集处理：退化为仅 jev 槽位判支持（可用性优先，不阻断路由）
            tracing::warn!(error = %e, "decision routing: list_model_entries failed, slot-only matching");
            DecisionModels::default()
        }
    }
}

/// 调度上下文（proxy 持有；scheduler 为 per-platform 健康/熔断指标，sticky 为粘性绑定表）。
pub struct ScheduleCtx<'a> {
    pub scheduler: &'a SchedulerState,
    pub sticky: &'a StickyTable,
    pub settings: &'a SchedulingBreakerSettings,
    /// Sticky 模式 session 键（group_key + 客户端稳定标识，调用侧拼接）。
    pub sticky_key: Option<String>,
}

/// Platform extra 解析缓存：避免每个候选重复解析 time_windows 和 peak
#[derive(Clone, Debug)]
struct ExtraCache {
    time_windows: Vec<serde_json::Value>,
    peak_windows: Vec<peak::TimeWindow>,
    /// extra.disable_during_peak（用户开关）。单平台短路的 peak 拦截必须以此为准：
    /// 只看窗口命中会把「配了倍率窗口但没开禁用」的单平台组也拦成 503（2026-10-23 修复）。
    disable_during_peak: bool,
    /// extra.direct_models（直通模型声明，直接模型转发）。
    direct_models: Vec<String>,
}

impl ExtraCache {
    /// `platform_type` 直接取自 `Platform` 顶层字段（非 extra 内字段，caller 传入）。
    fn new(extra: &str, platform_type: &Protocol) -> Self {
        let time_windows = time_windows::parse_platform_time_windows(extra);
        let peak_windows = peak::peak_for(extra, &platform_type.wire_str());
        let disable_during_peak = peak::parse_disable_during_peak(extra);
        let direct_models = db::models::PlatformExtra::parse(extra).direct_models;
        Self {
            time_windows,
            peak_windows,
            disable_during_peak,
            direct_models,
        }
    }
}

/// 候选平台的 extra 解析缓存（key: platform_id）
type ExtraCacheMap = HashMap<u64, ExtraCache>;

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
) -> Result<CandidateSet, RouteFail> {
    select_candidates_ctx(db, group, source_model, None, RequestKind::Chat).await
}

/// 带调度上下文的候选选取。`ctx=None` 时退化为无熔断 / 无指标的旧行为（仅 auto_disabled 过滤）。
/// `kind` 按请求类型过滤候选（jev-decision-proxy R2–R6；Tools 见 spec-独立搜索转发 语义 3）。
pub async fn select_candidates_ctx(
    db: &db::Db,
    group: &Group,
    source_model: &str,
    ctx: Option<&ScheduleCtx<'_>>,
    kind: RequestKind<'_>,
) -> Result<CandidateSet, RouteFail> {
    let mapping = group
        .model_mappings
        .iter()
        .find(|m| m.source_model == source_model);
    let mapped_target_model = mapping.map(|m| m.target_model.clone());
    let mapped_platform_id = mapping.map(|m| m.target_platform_id).filter(|id| *id != 0);

    let group_platforms = db::get_group_platforms(db, group.id)
        .await
        .map_err(RouteFail::from)?;
    if group_platforms.is_empty() {
        return Err("group has no platforms".to_string().into());
    }

    // ── 阶段 -0.5: 请求类型维度（jev-decision-proxy）──
    // 决策请求一次性取 registry decision 能力模型集合（禁逐平台查库）；聊天请求零查询。
    let decision_models = if kind == RequestKind::Decision {
        load_decision_models(db).await
    } else {
        DecisionModels::default()
    };
    // R6：分组 model_mappings 显式指定的目标平台不支持该请求类型 → 直接报错（不忽略映射）。
    if let Some(target_id) = mapped_platform_id
        && let Some(gp) = group_platforms
            .iter()
            .find(|gp| gp.platform.id == target_id)
        && !platform_supports_kind(&gp.platform, kind, &decision_models)
    {
        tracing::warn!(
            group = %group.name, target_platform_id = target_id,
            "mapped target platform does not support this request kind"
        );
        return Err(kind.no_platform_err().to_string().into());
    }

    let now_ms = db::now();
    let effective_mode = group.routing_mode;

    // ── 阶段 -1: 预解析所有 platform.extra（避免每个候选重复解析）──
    // ponytail: 在入口处统一解析 time_windows 和 peak，缓存传递给 helper 函数
    let extra_cache: ExtraCacheMap = group_platforms
        .iter()
        .map(|gp| {
            (
                gp.platform.id,
                ExtraCache::new(&gp.platform.extra, &gp.platform.platform_type),
            )
        })
        .collect();

    // ── 直通模型链（直接模型转发）：有效模型名（映射命中取 target_model，否则请求原名）
    // 被组内任一平台 extra.direct_models 声明 → 只在声明它的平台里走全维度过滤 + 现有排序
    // failover。此链映射只翻译不锁平台（跳过提首与静默回落），转发模型名 = 原名（不进槽位匹配）。
    // 无人声明该模型 → 默认链原样（下方阶段 0 起，含提首/回落）。
    // 排序在阶段 0 之前但仅限多平台组：需求原话「声明平台全不可用 → 直接报错」优先于
    // 等效单平台分组（恰好 1 个 Enabled）短路；字面单平台组（len==1）保留 bypass 语义不动。
    let direct_model = mapped_target_model
        .clone()
        .unwrap_or_else(|| source_model.to_string());
    let direct_ids: HashSet<u64> = group_platforms
        .iter()
        .filter(|gp| {
            extra_cache
                .get(&gp.platform.id)
                .is_some_and(|c| c.direct_models.iter().any(|m| m == &direct_model))
        })
        .map(|gp| gp.platform.id)
        .collect();
    if !direct_ids.is_empty() && group_platforms.len() > 1 {
        return select_direct_candidates(
            group,
            &group_platforms,
            mapping,
            &extra_cache,
            &direct_ids,
            &direct_model,
            ctx,
            now_ms,
            effective_mode,
            kind,
            &decision_models,
        );
    }

    // ── 阶段 0: 单启用平台分组短路 ──
    if let Some(only) = sole_platform(&group_platforms) {
        return handle_single_platform(
            db,
            group,
            only,
            source_model,
            ctx,
            &mapped_target_model,
            mapping,
            now_ms,
            &extra_cache,
            kind,
            &decision_models,
        )
        .await;
    }

    // ── 阶段 1: 候选分桶过滤 ──
    let FilteredCandidates {
        mut active,
        mut probe,
        breaker_rejected,
        mut excluded,
        peak_disabled_count,
        kind_rejected_count,
        unknown_protocol_count,
    } = filter_candidates(&group_platforms, ctx, now_ms, source_model, &extra_cache, kind, &decision_models);

    // ── 阶段 2: 熔断全空回退透传 ──
    // 仅当熔断维度踢空（active+probe 皆空）且确有被熔断踢出的候选时回退；
    // 若空因 auto_disabled / 手动 disabled，则不回退（保持原 Err）。
    let breaker_bypass = active.is_empty() && probe.is_empty() && !breaker_rejected.is_empty();
    if breaker_bypass {
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
    } else {
        // 非回退路径：熔断踢出的候选不再参与本次路由 → 进排除清单（票 01 快照）。
        excluded.extend(
            breaker_rejected
                .into_iter()
                .map(|(gp, _)| (gp, RouteReason::Breaker)),
        );
    }

    // ── 阶段 3: 按路由模式排序 ──
    let (sticky_id, demoted_ids) =
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

    // ── 票 01: 路由决策快照 ── 选中者按最终尝试顺序 + 位置标签；排除者附细分原因。
    let mut entries: Vec<RouteCandidate> = ordered
        .iter()
        .enumerate()
        .map(|(i, gp)| {
            let mut tags = Vec::new();
            if mapped_platform_id == Some(gp.platform.id) {
                tags.push(RouteTag::MappingPromoted);
            }
            if is_coding_plan(&gp.platform) {
                tags.push(RouteTag::CodingPlan);
            }
            if demoted_ids.contains(&gp.platform.id) {
                tags.push(RouteTag::Demoted);
            }
            if sticky_id == Some(gp.platform.id) {
                tags.push(RouteTag::Sticky);
            }
            RouteCandidate {
                order: (i + 1) as u32,
                platform: gp.platform.name.clone(),
                platform_id: gp.platform.id,
                level_priority: gp.level_priority,
                priority: gp.priority,
                reason: RouteReason::Selected,
                target_model: None,
                tags,
            }
        })
        .collect();
    let selected_count = entries.len() as u32;
    entries.extend(
        excluded
            .iter()
            .enumerate()
            .map(|(i, (gp, reason))| RouteCandidate {
                order: selected_count + i as u32 + 1,
                platform: gp.platform.name.clone(),
                platform_id: gp.platform.id,
                level_priority: gp.level_priority,
                priority: gp.priority,
                reason: *reason,
                target_model: None,
                tags: Vec::new(),
            }),
    );
    let mut routing = RoutingSnapshot {
        mode: Some(effective_mode),
        entries,
    };

    // ── 阶段 5: 空候选处理 ──
    if ordered.is_empty() {
        // 整组所有候选是未知协议（DB platform_type 枚举外，票 06）→ 返特殊 Err，
        // caller 据此落审计 proxy_log (blocked_by='router', blocked_reason='unknown_protocol',
        // est_cost=0, status_code=503)。
        if unknown_protocol_count > 0 && unknown_protocol_count == group_platforms.len() {
            tracing::info!(
                group = %group.name, unknown_protocol = unknown_protocol_count,
                "all candidates have unknown protocol; returning unknown_protocol error for audit log"
            );
            return Err(RouteFail {
                reason: "unknown_protocol".to_string(),
                routing,
            });
        }
        // 整组所有候选被高峰禁用排除 → 返特殊 Err，caller handler.rs 据此落审计 proxy_log
        // (blocked_by='router', blocked_reason='peak', est_cost=0, status_code=503)。
        // 其他原因（disabled / auto_disabled / 熔断无回退）照旧 NoCandidate warn 不落库。
        if peak_disabled_count > 0 && peak_disabled_count == group_platforms.len() {
            tracing::info!(
                group = %group.name, peak_disabled = peak_disabled_count,
                "all candidates peak-disabled; returning peak_disabled error for audit log"
            );
            return Err(RouteFail {
                reason: "peak_disabled".to_string(),
                routing,
            });
        }
        // 整组所有候选被请求类型维度剔除 → 返特殊 Err，caller 落审计 proxy_log
        // (blocked_by='router', blocked_reason=no_decision_platform / no_chat_platform, status_code=400)。
        if kind_rejected_count > 0 && kind_rejected_count == group_platforms.len() {
            tracing::info!(
                group = %group.name, kind = ?kind, kind_rejected = kind_rejected_count,
                "all candidates rejected by request kind"
            );
            return Err(RouteFail {
                reason: kind.no_platform_err().to_string(),
                routing,
            });
        }
        return Err(RouteFail {
            reason: "no available platform (all disabled, backing off, or circuit-broken)"
                .to_string(),
            routing,
        });
    }

    // ── 阶段 6: 生成最终候选 ──
    let candidates = build_route_results(
        ordered,
        &mapped_target_model,
        now_ms,
        source_model,
        mapping,
        &extra_cache,
        kind,
        &decision_models,
    );

    // 快照回填目标模型：选中候选按解析结果填 target_model（排除候选算不到，留 None）
    fill_snapshot_target_models(&mut routing, &candidates);

    tracing::info!(
        group = %group.name, source_model = %source_model,
        candidate_count = candidates.len(), mode = ?group.routing_mode,
        first_platform = %candidates[0].platform.name,
        "candidates selected"
    );

    Ok(CandidateSet { candidates, routing })
}

// ── Helper: 单平台分组短路逻辑 ──

/// 单平台分组：唯一平台熔断 Open / auto_disabled 时仍必请求（无视状态），
/// 手动 Disabled / 高峰禁用 + 命中窗口 → Err（唯一硬停）。
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
    kind: RequestKind<'_>,
    decision_models: &DecisionModels,
) -> Result<CandidateSet, RouteFail> {
    // 未知协议（票 06）：无 wire 格式可转，唯一硬停之一（照 peak_disabled 先例，caller 落审计）。
    if matches!(only.platform.platform_type, Protocol::Unknown) {
        tracing::info!(
            group = %group.name, platform = %only.platform.name,
            "single-platform group: unknown protocol, request blocked"
        );
        return Err(RouteFail {
            reason: "unknown_protocol".to_string(),
            routing: single_snapshot(only, group.routing_mode, RouteReason::UnknownProtocol),
        });
    }

    // 请求类型不匹配也是硬停（R5：单平台短路同样检查，2026-09-30 复核拍板）。
    if !platform_supports_kind(&only.platform, kind, decision_models) {
        tracing::info!(
            group = %group.name, platform = %only.platform.name, kind = ?kind,
            "single-platform group: request kind unsupported"
        );
        return Err(RouteFail {
            reason: kind.no_platform_err().to_string(),
            routing: single_snapshot(only, group.routing_mode, RouteReason::KindUnsupported),
        });
    }

    // 手动 Disabled 是唯一硬停
    if only.platform.status == PlatformStatus::Disabled {
        return Err(RouteFail {
            reason: "group's only platform is manually disabled".to_string(),
            routing: single_snapshot(only, group.routing_mode, RouteReason::DisabledManual),
        });
    }

    // 审核拒绝自动禁用是硬停：即使单平台组也不能绕过，否则同一 session 会再次撞回。
    // 透传平台例外（ADR 0008 决策 2 永远在线）：auto_disabled 维度整体不参与，含审核硬停。
    let is_passthrough = matches!(only.platform.platform_type, Protocol::Passthrough);
    if !is_passthrough
        && only.platform.status == PlatformStatus::AutoDisabled
        && only.platform.auto_disabled_until > now_ms
        && only.platform.last_error.contains("[censorship_blocked]")
    {
        return Err(RouteFail {
            reason: "group's only platform is censorship-blocked".to_string(),
            routing: single_snapshot(only, group.routing_mode, RouteReason::DisabledAuto),
        });
    }

    // 高峰禁用优先级高于 status bypass（单平台组不 bypass 此维度）；
    // 透传平台例外（ADR 0008 决策 2）：高峰禁用维度同样跳过，永远在线。
    // 开关（disable_during_peak）与窗口命中缺一不可——与多平台路径 is_peak_disabled 同判定。
    let cache = extra_cache.get(&only.platform.id);
    let peak_windows: &[peak::TimeWindow] =
        cache.map(|c| c.peak_windows.as_slice()).unwrap_or_default();
    if !is_passthrough
        && cache.is_some_and(|c| c.disable_during_peak)
        && is_in_peak_window_cached(peak_windows, now_ms, source_model)
    {
        tracing::info!(
            group = %group.name, platform = %only.platform.name,
            "single-platform group: peak-disabled, request blocked"
        );
        return Err(RouteFail {
            reason: "peak_disabled".to_string(),
            routing: single_snapshot(only, group.routing_mode, RouteReason::PeakDisabled),
        });
    }

    // 时段模型：从缓存获取 time_windows，再用 resolve_model
    let time_rules: &[serde_json::Value] =
        cache.map(|c| c.time_windows.as_slice()).unwrap_or_default();
    let effective_models =
        resolve_effective_models(&only.platform, time_rules, now_ms, source_model);
    let target_model = resolve_target_model(
        kind,
        &effective_models,
        &only.platform,
        decision_models,
        mapped_target_model.as_deref().unwrap_or(source_model),
    );

    tracing::info!(
        group = %group.name, platform = %only.platform.name,
        status = ?only.platform.status,
        "single-platform group: bypassing status filter, forcing request"
    );

    let mut routing = single_snapshot(only, group.routing_mode, RouteReason::Selected);
    routing.entries[0].target_model = Some(target_model.clone());
    Ok(CandidateSet {
        candidates: vec![RouteResult {
            platform: only.platform.clone(),
            target_model,
            mapping: mapping.cloned(),
        }],
        routing,
    })
}

/// 单平台组快照（票 01）：唯一候选一个条目，排除时带原因。
fn single_snapshot(
    only: &GroupPlatformDetail,
    mode: RoutingMode,
    reason: RouteReason,
) -> RoutingSnapshot {
    RoutingSnapshot {
        mode: Some(mode),
        entries: vec![RouteCandidate {
            order: 1,
            platform: only.platform.name.clone(),
            platform_id: only.platform.id,
            level_priority: only.level_priority,
            priority: only.priority,
            reason,
            target_model: None,
            tags: Vec::new(),
        }],
    }
}

// 按 platform_id 回填已解析候选模型；排除候选保持 None。
fn fill_snapshot_target_models(routing: &mut RoutingSnapshot, candidates: &[RouteResult]) {
    for entry in &mut routing.entries {
        if entry.reason == RouteReason::Selected {
            entry.target_model = candidates
                .iter()
                .find(|candidate| candidate.platform.id == entry.platform_id)
                .map(|candidate| candidate.target_model.clone());
        }
    }
}

// ── Helper: 直通模型链（直接模型转发）──

/// 有效模型被组内平台 `extra.direct_models` 声明时的候选选取：对整组跑 `filter_candidates`
/// 全维度过滤后只保留声明平台，再按 `sort_by_routing_mode` 现有排序 failover（无映射提首）。
/// 转发模型名 = 直通原名（不进槽位匹配）。声明平台全不可用（含熔断踢出——直通链无
/// breaker bypass）→ `RouteFail("direct_model_unavailable")`，caller handler.rs 据此落
/// 审计 proxy_log（blocked_by='router', status_code=503）。
#[allow(clippy::too_many_arguments)]
fn select_direct_candidates(
    group: &Group,
    group_platforms: &[GroupPlatformDetail],
    mapping: Option<&ModelMapping>,
    extra_cache: &ExtraCacheMap,
    direct_ids: &HashSet<u64>,
    direct_model: &str,
    ctx: Option<&ScheduleCtx<'_>>,
    now_ms: i64,
    mode: RoutingMode,
    kind: RequestKind<'_>,
    decision_models: &DecisionModels,
) -> Result<CandidateSet, RouteFail> {
    let in_direct = |gp: &GroupPlatformDetail| direct_ids.contains(&gp.platform.id);
    let FilteredCandidates {
        mut active,
        mut probe,
        breaker_rejected,
        excluded,
        ..
    } = filter_candidates(
        group_platforms,
        ctx,
        now_ms,
        direct_model,
        extra_cache,
        kind,
        decision_models,
    );
    active.retain(|gp| in_direct(gp));
    probe.retain(|gp| in_direct(gp));
    // 排除清单只留直通平台；熔断踢出的直通平台计入不可用（直通链无 breaker bypass）
    let mut excluded: Vec<(&GroupPlatformDetail, RouteReason)> = excluded
        .into_iter()
        .filter(|(gp, _)| in_direct(gp))
        .collect();
    excluded.extend(
        breaker_rejected
            .into_iter()
            .filter(|(gp, _)| in_direct(gp))
            .map(|(gp, _)| (gp, RouteReason::Breaker)),
    );

    let (sticky_id, demoted_ids) = sort_by_routing_mode(&mut active, &mut probe, mode, ctx, now_ms);
    let ordered: Vec<&GroupPlatformDetail> = active.into_iter().chain(probe).collect();

    // 决策快照（与默认链同形状：选中按最终顺序 + 位置标签，排除附细分原因；无 MappingPromoted）
    let selected_count = ordered.len() as u32;
    let mut entries: Vec<RouteCandidate> = ordered
        .iter()
        .enumerate()
        .map(|(i, gp)| {
            let mut tags = Vec::new();
            if is_coding_plan(&gp.platform) {
                tags.push(RouteTag::CodingPlan);
            }
            if demoted_ids.contains(&gp.platform.id) {
                tags.push(RouteTag::Demoted);
            }
            if sticky_id == Some(gp.platform.id) {
                tags.push(RouteTag::Sticky);
            }
            RouteCandidate {
                order: (i + 1) as u32,
                platform: gp.platform.name.clone(),
                platform_id: gp.platform.id,
                level_priority: gp.level_priority,
                priority: gp.priority,
                reason: RouteReason::Selected,
                target_model: None,
                tags,
            }
        })
        .collect();
    entries.extend(excluded.iter().enumerate().map(|(i, (gp, reason))| RouteCandidate {
        order: selected_count + i as u32 + 1,
        platform: gp.platform.name.clone(),
        platform_id: gp.platform.id,
        level_priority: gp.level_priority,
        priority: gp.priority,
        reason: *reason,
        target_model: None,
        tags: Vec::new(),
    }));
    let mut routing = RoutingSnapshot {
        mode: Some(mode),
        entries,
    };

    if ordered.is_empty() {
        tracing::info!(
            group = %group.name, model = %direct_model, declared = direct_ids.len(),
            "all direct-model platforms unavailable; returning direct_model_unavailable for audit log"
        );
        return Err(RouteFail {
            reason: "direct_model_unavailable".to_string(),
            routing,
        });
    }

    let candidates: Vec<RouteResult> = ordered
        .into_iter()
        .map(|gp| RouteResult {
            platform: gp.platform.clone(),
            target_model: direct_model.to_string(),
            mapping: mapping.cloned(),
        })
        .collect();

    tracing::info!(
        group = %group.name, model = %direct_model, candidate_count = candidates.len(),
        mode = ?mode, first_platform = %candidates[0].platform.name,
        "direct-model candidates selected"
    );

    fill_snapshot_target_models(&mut routing, &candidates);

    Ok(CandidateSet { candidates, routing })
}

// ── Helper: 候选分桶过滤 ──

/// 候选分桶结果：active（健康/已过期试探）、probe（退避中）、breaker_rejected（熔断踢出）、
/// excluded（票 01 排除候选 + 细分原因）、peak_disabled_count。
struct FilteredCandidates<'a> {
    active: Vec<&'a GroupPlatformDetail>,
    probe: Vec<&'a GroupPlatformDetail>,
    breaker_rejected: Vec<(&'a GroupPlatformDetail, Option<bool>)>,
    /// 排除候选 + 细分原因（取代旧「手动禁用/auto_disabled/高峰禁用三合一」，票 01）。
    excluded: Vec<(&'a GroupPlatformDetail, RouteReason)>,
    /// 仍按旧窗口判定计数（阶段 5 audit Err 语义不变，见 CLAUDE.md peak 段）。
    peak_disabled_count: usize,
    kind_rejected_count: usize,
    unknown_protocol_count: usize,
}

/// 遍历 group_platforms 按 auto_disabled 三态分桶（enabled / 过期试探），
/// 再叠加配额冷却（429 配额耗尽，内存态，不改 DB status）与熔断准入门
/// （Open/HalfOpen 满踢出），高峰禁用计数。排除候选逐个带细分原因标签（票 01）。
fn filter_candidates<'a>(
    group_platforms: &'a [GroupPlatformDetail],
    ctx: Option<&ScheduleCtx<'_>>,
    now_ms: i64,
    source_model: &str,
    extra_cache: &ExtraCacheMap,
    kind: RequestKind<'_>,
    decision_models: &DecisionModels,
) -> FilteredCandidates<'a> {
    let mut active = Vec::new();
    let mut probe = Vec::new();
    let mut breaker_rejected = Vec::new();
    let mut excluded: Vec<(&GroupPlatformDetail, RouteReason)> = Vec::new();
    let mut peak_disabled_count = 0;
    let mut kind_rejected_count = 0;
    let mut unknown_protocol_count = 0;

    let breaker_enabled = ctx.map(|c| c.settings.enabled).unwrap_or(false);

    for gp in group_platforms {
        // 未知协议（票 06，用户裁决 2026-10-16）：DB platform_type 枚举外值，不进任何桶
        // （无 wire 格式可转，路由必错）；整组皆未知 → 阶段 5 返 Err("unknown_protocol")。
        if matches!(gp.platform.platform_type, Protocol::Unknown) {
            unknown_protocol_count += 1;
            excluded.push((gp, RouteReason::UnknownProtocol));
            continue;
        }

        // 请求类型维度（R2/R3）：不支持该类型的平台剔除（先于 status 判定，
        // 纯决策平台 / 无决策能力的平台不进任何桶）。
        if !platform_supports_kind(&gp.platform, kind, decision_models) {
            kind_rejected_count += 1;
            excluded.push((gp, RouteReason::KindUnsupported));
            continue;
        }

        // auto_disabled 维度（DB 持久态）；state_reason 与判定同源（candidate_state 单一真值源）
        let (auto_state, state_reason) = candidate_state(&gp.platform, now_ms, source_model);
        if auto_state.is_none() {
            // 高峰禁用计数沿用缓存窗口命中口径（与归因解耦，行为与旧实现一致）
            let cache = extra_cache.get(&gp.platform.id);
            let peak_windows: &[peak::TimeWindow] =
                cache.map(|c| c.peak_windows.as_slice()).unwrap_or_default();
            if is_in_peak_window_cached(peak_windows, now_ms, source_model) {
                peak_disabled_count += 1;
            }
            excluded.push((gp, state_reason));
            continue; // 手动 disabled / auto_disabled 未到期 / 高峰禁用 → 跳过
        }

        // 透传平台永远在线（ADR 0008 决策 2）：不参与失败计数，跳过配额/auth 冷却与熔断
        // （candidate_state 已按 Passthrough 跳过高峰禁用与 auto_disabled）。
        if matches!(gp.platform.platform_type, Protocol::Passthrough) {
            active.push(gp);
            continue;
        }

        // 冷却维度（内存态，与熔断总开关无关）：
        // - 配额冷却：429 配额耗尽 + 上游给出重置时间 → 重置前不调度（DB status 不变）。
        // - auth 冷却：401 权限 / 402 余额 → 固定 5 分钟不调度（内存态，DB status 不变，
        //   取代旧 auto_disabled 指数退避）。
        if let Some(c) = ctx
            && (c.scheduler.quota_cooled(gp.platform.id, now_ms)
                || c.scheduler.auth_cooled(gp.platform.id, now_ms))
        {
            let reason = if c.scheduler.quota_cooled(gp.platform.id, now_ms) {
                RouteReason::QuotaCooldown
            } else {
                RouteReason::AuthCooldown
            };
            excluded.push((gp, reason));
            continue;
        }

        // 熔断维度（内存态）：仅在有 ctx 且总开关开时判定
        if let Some(c) = ctx
            && breaker_enabled
        {
            let (ft, os, hom) = c.settings.effective_thresholds(&gp.platform);
            let th = BreakerThresholds {
                failure_threshold: ft,
                open_secs: os,
                half_open_max: hom,
            };
            match c.scheduler.admission(gp.platform.id, &th, now_ms, true) {
                Admission::Reject => {
                    breaker_rejected.push((gp, auto_state));
                    continue;
                }
                Admission::Probe | Admission::Allow => {}
            }
        }

        match auto_state {
            Some(false) => active.push(gp),
            Some(true) => probe.push(gp),
            None => {}
        }
    }

    FilteredCandidates {
        active,
        probe,
        breaker_rejected,
        excluded,
        peak_disabled_count,
        kind_rejected_count,
        unknown_protocol_count,
    }
}

// ── Helper: 按路由模式排序 ──

/// 按路由模式对 active/probe 桶排序（Failover/LoadBalance/Sticky）。
/// 返回 (sticky 提首的平台 id, 失败降权沉底的平台 id 集)（票 01 位置标签采集）。
fn sort_by_routing_mode(
    active: &mut Vec<&GroupPlatformDetail>,
    probe: &mut Vec<&GroupPlatformDetail>,
    mode: RoutingMode,
    ctx: Option<&ScheduleCtx<'_>>,
    now_ms: i64,
) -> (Option<u64>, Vec<u64>) {
    let sticky_id = match mode {
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
            None
        }
        RoutingMode::LoadBalance | RoutingMode::HealthAware => {
            order_load_balance(active, now_ms);
            order_load_balance(probe, now_ms);
            apply_coding_plan_priority(active);
            apply_coding_plan_priority(probe);
            None
        }
        RoutingMode::Sticky => {
            order_load_balance(active, now_ms);
            order_load_balance(probe, now_ms);
            apply_coding_plan_priority(active);
            apply_coding_plan_priority(probe);
            apply_sticky(active, ctx, now_ms)
        }
    };

    // ── 失败降权沉底（health-aware，2026-09-29）：降权窗口内的平台整体沉到 active 桶
    //    末尾（降权平台之间保持 mode 排序原序），probe 桶本身已是末位试探层不重复处理。
    //    放在 apply_coding_plan_priority 之后：降权优先级高于 coding plan 上浮。
    let mut demoted_ids = Vec::new();
    if let Some(c) = ctx {
        let mut demoted = Vec::new();
        active.retain(|gp| {
            if c.scheduler.penalty_active(gp.platform.id, now_ms) {
                demoted_ids.push(gp.platform.id);
                demoted.push(*gp);
                false
            } else {
                true
            }
        });
        active.extend(demoted);
    }
    (sticky_id, demoted_ids)
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
    if let Some(target_id) = mapped_platform_id
        && let Some(pos) = ordered.iter().position(|gp| gp.platform.id == target_id)
    {
        let gp = ordered.remove(pos);
        ordered.insert(0, gp);
    }

    ordered
}

// ── Helper: 生成最终候选 ──

/// 为每个候选解析目标模型（时段模型 + resolve_model），构建 RouteResult 列表。
#[allow(clippy::too_many_arguments)]
fn build_route_results(
    ordered: Vec<&GroupPlatformDetail>,
    mapped_target_model: &Option<String>,
    now_ms: i64,
    source_model: &str,
    mapping: Option<&ModelMapping>,
    extra_cache: &ExtraCacheMap,
    kind: RequestKind<'_>,
    decision_models: &DecisionModels,
) -> Vec<RouteResult> {
    ordered
        .into_iter()
        .map(|gp| {
            let cache = extra_cache.get(&gp.platform.id);
            let time_rules: &[serde_json::Value] =
                cache.map(|c| c.time_windows.as_slice()).unwrap_or_default();
            let effective_models =
                resolve_effective_models(&gp.platform, time_rules, now_ms, source_model);
            let target_model = resolve_target_model(
                kind,
                &effective_models,
                &gp.platform,
                decision_models,
                mapped_target_model.as_deref().unwrap_or(source_model),
            );
            RouteResult {
                platform: gp.platform.clone(),
                target_model,
                mapping: mapping.cloned(),
            }
        })
        .collect()
}

/// 按请求类型做槽位匹配。`model_name` = 分组映射命中时的映射目标名，否则请求模型名：
/// 映射只改名，不跳过槽位匹配（映射名不含档位词时同样回落 default 槽，用户 2026-10-01 确认）。
fn resolve_target_model(
    kind: RequestKind<'_>,
    effective_models: &PlatformModels,
    platform: &Platform,
    decision_models: &DecisionModels,
    model_name: &str,
) -> String {
    // 透传平台通配命中（ADR 0008）：不吃模型槽改写——body 原样转发，模型名仅作日志元数据。
    if matches!(platform.platform_type, Protocol::Passthrough) {
        return model_name.to_string();
    }
    match kind {
        RequestKind::Decision => {
            resolve_decision_model(effective_models, platform, decision_models, model_name)
        }
        RequestKind::Chat => resolve_model(effective_models, model_name),
        // Tools 不做任何模型改写（语义 3）：映射命中取映射目标名（仅日志元数据），否则原样。
        RequestKind::Tools(_) => model_name.to_string(),
    }
}

/// 缓存版本的 peak 窗口判定（避免重新解析 peak）
fn is_in_peak_window_cached(windows: &[peak::TimeWindow], now_ms: i64, source_model: &str) -> bool {
    peak::is_in_peak_window(windows, now_ms, source_model)
}

/// 解析当前时段的有效模型配置（effective_models）—— 使用已解析的 time_windows 缓存。
///
/// 三层级联（优先级高 → 低）：
/// 1. **time_windows**（用户级显式时段切换，`platform.extra.time_windows`）：命中 → 用该时段 models；
///    用户已自定义 time_windows 时不再应用 preset peak 分支（用户显式覆盖优先）。
/// 2. **preset.models.peak**（preset 级高峰分支，PRD 07-11）：用户未配 time_windows 且
///    preset 提供本协议 `models.peak` 分支 + 当前命中 `peak_for` 任一窗口 → 用 peak 替换。
///    **设计意图：peak 分支为 preset 级硬约束，覆盖用户手工定制的 `platform.models`**
///    （等同 coding_plan 端点维度优先级；用户显式定制在高峰窗口期内不保留，如需保留请配
///    `time_windows` 显式时段切换，其优先级高于 peak）。非 bug — 见 CLAUDE.md `models.peak` 段。
/// 3. **platform.models**（用户级显式槽位 / 创建时填入 preset.models.default）：兜底默认。
///
/// `source_model`：请求模型名（透传给 peak model scope 过滤；空串 = 无上下文跳过）。
fn resolve_effective_models(
    platform: &Platform,
    time_rules: &[serde_json::Value],
    now_ms: i64,
    source_model: &str,
) -> PlatformModels {
    let mut effective = time_windows::resolve_time_windows(time_rules, &platform.models, now_ms);
    // PRD 07-11：time_windows 未自定义时查 preset.models.peak 分支
    if time_rules.is_empty() {
        // serde rename 裸名（如 "glm_coding"），同 is_peak_disabled / calc_est_cost 取名模式
        let ptype = platform.platform_type.wire_str();
        if let Some(peak_models) = super::super::peak::default_peak_models(&ptype) {
            let windows = super::super::peak::peak_for(&platform.extra, &ptype);
            if super::super::peak::is_in_peak_window(&windows, now_ms, source_model) {
                effective = peak_models;
            }
        }
    }
    effective
}

#[cfg(test)]
#[path = "test_candidates.rs"]
mod test_candidates;
