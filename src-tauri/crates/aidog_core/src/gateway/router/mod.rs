//! 路由子系统：分组匹配 + 模型映射 + 候选选取 + 排序策略 + 平台选择。
//!
//! 子模块划分：
//! - [`candidates`]：分组路由规则 → 有序候选平台列表（`select_candidates*`）。
//! - [`ordering`]：候选排序策略（负载均衡 / 最小延迟 / 粘性）。
//! - [`model_mapping`]：按平台模型配置自动匹配请求模型（`resolve_model`）。
//!
//! 对外路径保持 `gateway::router::X` 不变（经下方 `pub use` 重导出）。

use super::models::*;
use aidog_db::models::RouteReason;

mod candidates;
mod model_mapping;
mod ordering;

#[allow(unused_imports)]
pub use candidates::{
    CandidateSet, ClientGate, ClientProbe, QuotaExhausted, RequestKind, RouteFail, ScheduleCtx,
    select_candidates_gated,
};
pub(crate) use candidates::{RouteInputs, select_candidates_preloaded};
#[cfg(test)]
pub use candidates::select_candidates_for_test;

// issue 68：出站 max_tokens 裁剪纯函数迁入 aidog_adapter::platform::hooks（平台适配器框架），
// 此处 re-export 保持 `router::cap_max_tokens` 路径与既有调用 / 测试不变。
pub use aidog_adapter::platform::hooks::cap_max_tokens;

/// 路由结果
#[derive(Debug, Clone)]
pub struct RouteResult {
    pub platform: Platform,
    pub target_model: String,
    /// 命中槽位里排在链首之后的有序备选（完整链 = `target_model` + 本字段）。
    /// 透传平台 / 直通模型 / 请求名透传恒空；本票只携带，换模型动作见票 06。
    pub fallback_models: Vec<String>,
    /// 匹配到的模型映射（如果有），用于读取超时等维度配置
    pub mapping: Option<ModelMapping>,
}

/// 判定平台当前是否可作为候选纳入：
/// - 过期（`expires_at > 0 && now_ms >= expires_at`）：始终排除（等效自动禁用，
///   但独立于 status 三态枚举；用户改 expires_at 清空/延后即恢复，无需退避试探）
/// - 高峰禁用（`extra.disable_during_peak` on 且 now 命中 `peak` 任一窗口）：
///   始终排除（独立维度，与 status 正交；临时闸门，用户关开关/出窗口即恢复）
/// - Enabled：始终纳入
/// - AutoDisabled 且已过退避试探时间（now >= until）：纳入（末尾试探）
/// - Disabled（用户手动）/ AutoDisabled 未到试探时间：排除
///
/// `_request_model` 保留现有调用形状；高峰禁用不按模型范围过滤。
///
/// 返回 `(state, reason)`：`reason` 在 state 为 `None`（被排除）时给出归因，
/// 与 state 判定同分支产出——单一真值源，调用方不得再镜像本函数分支归因。
pub(crate) fn candidate_state(
    platform: &Platform,
    now_ms: i64,
    _request_model: &str,
) -> (Option<bool>, RouteReason) {
    // 过期平台直接排除（独立维度，与 status 正交；enabled + 过期也排除）。
    if platform.expires_at > 0 && now_ms >= platform.expires_at {
        return (None, RouteReason::DisabledAuto);
    }
    // 高峰禁用不按模型范围过滤；窗口仍决定路由是否停用。
    if is_peak_disabled(platform, now_ms) {
        return (None, RouteReason::PeakDisabled);
    }
    // 透传平台仍跳过 auto_disabled 自动摘除，但高峰禁用和手动 Disabled 生效。
    if matches!(platform.platform_type, Protocol::Passthrough) {
        return match platform.status {
            PlatformStatus::Disabled => (None, RouteReason::DisabledManual),
            _ => (Some(false), RouteReason::Selected),
        };
    }
    match platform.status {
        PlatformStatus::Enabled => (Some(false), RouteReason::Selected),
        PlatformStatus::AutoDisabled if now_ms >= platform.auto_disabled_until => {
            (Some(true), RouteReason::Selected)
        }
        PlatformStatus::Disabled => (None, RouteReason::DisabledManual),
        _ => (None, RouteReason::DisabledAuto),
    }
}

/// 平台是否因 `extra.disable_during_peak` 和当前 peak window 而从路由中排除。
/// 路由禁用覆盖窗口内所有模型；窗口的 model scope 仍用于计费。
pub(crate) fn is_peak_disabled(platform: &Platform, now_ms: i64) -> bool {
    if !super::peak::parse_disable_during_peak(&platform.extra) {
        return false;
    }
    let ptype = platform.platform_type.wire_str();
    let windows = super::peak::peak_for(&platform.extra, &ptype);
    super::peak::is_in_peak_window(&windows, now_ms, "")
}

/// 有效权重 = weight × level_priority（per-group 平台优先级 1~10 乘性放大）。
/// 默认全 level_priority=5 时各平台等比放大，相对分流比例不变（兼容现状）。
pub(crate) fn effective_weight(gp: &GroupPlatformDetail) -> i32 {
    gp.weight.max(0) * super::models::clamp_level_priority(gp.level_priority)
}

/// 分组内「唯一可选平台」判定 —— 单平台分组短路的唯一真值源。
///
/// 两个分支：
/// 1. 组内平台总数 == 1 —— 物理单平台，无论 status 为何都短路（保「唯一平台 auto_disabled
///    仍必请求」的既有契约，见 test_candidates.rs）。
/// 2. 组内 `status == Enabled` 的平台恰好 1 个 —— 用户只启用了一个，等效单平台分组。
///
/// 与 status 正交的临时闸门（expires_at 过期 / disable_during_peak）**不参与本判定**，
/// 由各自路径处理（短路后 handle_single_platform 仍会做高峰禁用硬停）。
pub(crate) fn sole_platform(gps: &[GroupPlatformDetail]) -> Option<&GroupPlatformDetail> {
    if gps.len() == 1 {
        return Some(&gps[0]);
    }
    let mut it = gps
        .iter()
        .filter(|gp| gp.platform.status == PlatformStatus::Enabled);
    match (it.next(), it.next()) {
        (Some(only), None) => Some(only),
        _ => None,
    }
}

#[cfg(test)]
pub(crate) mod test_mod;
