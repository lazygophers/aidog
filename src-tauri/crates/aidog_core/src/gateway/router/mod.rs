//! 路由子系统：分组匹配 + 模型映射 + 候选选取 + 排序策略 + 平台选择。
//!
//! 子模块划分：
//! - [`candidates`]：分组路由规则 → 有序候选平台列表（`select_candidates*`）。
//! - [`ordering`]：候选排序策略（负载均衡 / 最小延迟 / 粘性）。
//! - [`model_mapping`]：按平台模型配置自动匹配请求模型（`resolve_model`）。
//! - [`selection`]：旧版单平台选择路径（已 deprecated，保留兼容）。
//!
//! 对外路径保持 `gateway::router::X` 不变（经下方 `pub use` 重导出）。

use super::models::*;

mod candidates;
mod model_mapping;
mod ordering;
mod selection;

#[allow(unused_imports)]
pub use candidates::{select_candidates, select_candidates_ctx, CandidateSet, ScheduleCtx};
#[allow(unused_imports)]
pub use selection::select_platform;

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

/// 判定平台当前是否可作为候选纳入：
/// - 过期（`expires_at > 0 && now_ms >= expires_at`）：始终排除（等效自动禁用，
///   但独立于 status 三态枚举；用户改 expires_at 清空/延后即恢复，无需退避试探）
/// - 高峰禁用（`extra.disable_during_peak` on 且 now 命中 `peak_hours` 任一窗口）：
///   始终排除（独立维度，与 status 正交；临时闸门，用户关开关/出窗口即恢复）
/// - Enabled：始终纳入
/// - AutoDisabled 且已过退避试探时间（now >= until）：纳入（末尾试探）
/// - Disabled（用户手动）/ AutoDisabled 未到试探时间：排除
///
/// `request_model`：当前请求的模型名（用于 peak_hours model scope 过滤，PRD 07-09 D2）。
/// 传 `""` = 无 model 上下文 → 跳过 model 过滤（兼容旧行为）。
pub(crate) fn candidate_state(platform: &Platform, now_ms: i64, request_model: &str) -> Option<bool> {
    // 过期平台直接排除（独立维度，与 status 正交；enabled + 过期也排除）。
    if platform.expires_at > 0 && now_ms >= platform.expires_at {
        return None;
    }
    // 高峰禁用（与 status 正交，临时闸门，不改 status 三态）：
    // 开关 on 且 now 落在 peak window 任一窗口 → 排除。用户关开关/出窗口即恢复，无需退避试探。
    if is_peak_disabled(platform, now_ms, request_model) {
        return None;
    }
    match platform.status {
        PlatformStatus::Enabled => Some(false),
        PlatformStatus::AutoDisabled if now_ms >= platform.auto_disabled_until => Some(true),
        _ => None,
    }
}

/// 平台是否被高峰禁用（`extra.disable_during_peak` on 且当前命中 peak window）。
/// 路由排除用的纯判定函数：与 status 三态正交，不改 status。
/// 多平台组：candidate_state 返 None 跳过此平台；单平台组：bypass 覆盖（此开关优先级高于 status bypass）。
///
/// `request_model`：当前请求模型名（peak_hours model scope 过滤，PRD 07-09 D2）。
/// 传 `""` = 无 model 上下文 → 跳过 model 过滤（兼容旧行为）。
pub(crate) fn is_peak_disabled(platform: &Platform, now_ms: i64, request_model: &str) -> bool {
    if !super::peak_hours::parse_disable_during_peak(&platform.extra) {
        return false;
    }
    // serde rename 裸名（如 "anthropic"），同 spawn_estimate / calc_est_cost 取名模式
    let ptype = serde_json::to_string(&platform.platform_type)
        .unwrap_or_default()
        .trim_matches('"')
        .to_string();
    let windows = super::peak_hours::peak_hours_for(&platform.extra, &ptype);
    super::peak_hours::is_in_peak_window(&windows, now_ms, request_model)
}

/// 有效权重 = weight × level_priority（per-group 平台优先级 1~10 乘性放大）。
/// 默认全 level_priority=5 时各平台等比放大，相对分流比例不变（兼容现状）。
pub(crate) fn effective_weight(gp: &GroupPlatformDetail) -> i32 {
    gp.weight.max(0) * super::models::clamp_level_priority(gp.level_priority)
}

#[cfg(test)]
mod test_mod;
