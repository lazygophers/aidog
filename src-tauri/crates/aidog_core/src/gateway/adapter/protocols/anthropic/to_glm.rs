//! Anthropic 格式 → GLM 平台

use crate::gateway::adapter::types::*;
use serde_json::Value;

/// Anthropic 请求 → GLM 平台格式
pub fn to_glm(req: &ChatRequest) -> Result<Value, String> {
    // GLM 使用 OpenAI 兼容格式，需要将 Anthropic 格式转换为 OpenAI 格式
    // TODO: 实现 Anthropic → OpenAI 格式转换
    // - system → messages 里的 system/developer role
    // - content[*].text → messages[*].content
    // - content[*].tool_use → messages[*].tool_calls
    // - content[*].tool_result → role=tool, tool_call_id

    serde_json::to_value(req)
        .map_err(|e| format!("Anthropic to GLM error: {}", e))
}
