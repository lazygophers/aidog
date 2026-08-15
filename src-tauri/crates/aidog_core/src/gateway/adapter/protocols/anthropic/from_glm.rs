//! GLM 平台 → Anthropic 格式

use serde_json::Value;

/// GLM 平台响应 → Anthropic 格式
pub fn from_glm(body: &[u8]) -> Result<Value, String> {
    // GLM 使用 OpenAI 兼容格式，需要转换为 Anthropic 格式
    let value: Value = serde_json::from_slice(body)
        .map_err(|e| format!("GLM to Anthropic parse error: {}", e))?;

    // TODO: 将 OpenAI 格式响应转换为 Anthropic 格式
    // - choices[*].message.content → content[*].text
    // - choices[*].message.tool_calls → content[*].tool_use
    // - finish_reason 映射 (tool_calls→tool_use, length→max_tokens, stop→end_turn)

    Ok(value)
}
