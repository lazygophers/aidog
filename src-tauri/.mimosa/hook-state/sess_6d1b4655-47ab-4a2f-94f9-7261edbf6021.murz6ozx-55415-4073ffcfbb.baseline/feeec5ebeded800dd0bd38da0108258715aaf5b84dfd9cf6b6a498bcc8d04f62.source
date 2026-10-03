//! kimi_coding 平台 ↔ OpenAI Responses 格式

use serde_json::Value;

/// OpenAI Responses 格式 → kimi_coding 平台格式
pub fn from_openai_responses(body: &[u8]) -> Result<Value, String> {
    let value: Value = serde_json::from_slice(body)
        .map_err(|e| format!("{} to kimi_coding error: {}", "OpenAI Responses", e))?;
    Ok(value)
}

/// kimi_coding 平台格式 → OpenAI Responses 格式
pub fn to_openai_responses(body: &[u8]) -> Result<Value, String> {
    let value: Value = serde_json::from_slice(body)
        .map_err(|e| format!("{} to {} error: {}", "kimi_coding", "OpenAI Responses", e))?;
    Ok(value)
}
