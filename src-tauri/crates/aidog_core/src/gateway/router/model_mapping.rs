//! 模型映射：根据平台模型配置自动匹配请求模型。

use super::super::models::*;

/// 根据平台模型配置自动匹配请求模型。
/// 匹配规则：请求模型名（小写）包含槽位名（opus/sonnet/haiku/gpt）→ 使用该槽位值；
/// 全部不匹配 → 使用 default；无 default → 透传原始模型（去掉 [... ] 后缀）。
pub(crate) fn resolve_model(models: &PlatformModels, source_model: &str) -> String {
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
        if lower.contains(slot_name)
            && let Some(v) = slot_value
        {
            return v.clone();
        }
    }
    // 回退到 default
    if let Some(ref default) = models.default {
        return default.clone();
    }
    // 无匹配无 default — 透传（去掉 budget 后缀）
    base_model.to_string()
}

/// 决策请求（/v1/systemone）的目标模型解析（jev-decision-proxy R8）。不走 opus/sonnet 子串匹配：
/// - 平台配了 `jev` 槽位 → **总是**用槽位值（版本固定，不透传请求模型）；
/// - 未配（靠 available_models 推导支持决策）→ 请求模型在 `available_models` 里就用它；
/// - 否则用 `available_models` 中第一个 decision 能力模型；
/// - 都不满足（理论不可达：路由过滤已剔除不支持决策的平台）→ 透传原始模型名（剥 budget 后缀）。
///
/// `decision_model_ids`：registry `model_entry` 中 capabilities 含 `decision` 的 model_id 集合，
/// 由 `select_candidates_ctx` 入口一次性查库构建，禁逐平台查库。
pub(crate) fn resolve_decision_model(
    models: &PlatformModels,
    available_models: &[String],
    decision_model_ids: &std::collections::HashSet<String>,
    source_model: &str,
) -> String {
    if let Some(jev) = models.jev.as_deref().filter(|s| !s.is_empty()) {
        return jev.to_string();
    }
    let base = source_model.split('[').next().unwrap_or(source_model);
    if !base.is_empty() && available_models.iter().any(|m| m == base) {
        return base.to_string();
    }
    if let Some(first) = available_models
        .iter()
        .find(|m| decision_model_ids.contains(m.as_str()))
    {
        return first.clone();
    }
    base.to_string()
}
