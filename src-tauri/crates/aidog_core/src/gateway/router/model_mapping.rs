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
