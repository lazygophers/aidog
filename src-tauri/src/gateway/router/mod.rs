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
/// - Enabled：始终纳入
/// - AutoDisabled 且已过退避试探时间（now >= until）：纳入（末尾试探）
/// - Disabled（用户手动）/ AutoDisabled 未到试探时间：排除
pub(crate) fn candidate_state(platform: &Platform, now_ms: i64) -> Option<bool> {
    match platform.status {
        PlatformStatus::Enabled => Some(false),
        PlatformStatus::AutoDisabled if now_ms >= platform.auto_disabled_until => Some(true),
        _ => None,
    }
}

/// 有效权重 = weight × level_priority（per-group 平台优先级 1~10 乘性放大）。
/// 默认全 level_priority=5 时各平台等比放大，相对分流比例不变（兼容现状）。
pub(crate) fn effective_weight(gp: &GroupPlatformDetail) -> i32 {
    gp.weight.max(0) * super::models::clamp_level_priority(gp.level_priority)
}

#[cfg(test)]
mod test_mod;
