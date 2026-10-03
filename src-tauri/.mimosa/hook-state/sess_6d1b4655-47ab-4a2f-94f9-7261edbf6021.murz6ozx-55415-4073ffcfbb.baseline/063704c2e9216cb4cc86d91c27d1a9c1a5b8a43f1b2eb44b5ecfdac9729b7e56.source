//! xiaomi_mimo 平台 ↔ OpenAI Completions 格式

use serde_json::Value;

/// OpenAI Completions 格式 → xiaomi_mimo 平台格式
pub fn from_openai_completions(body: &[u8]) -> Result<Value, String> {
    let value: Value = serde_json::from_slice(body)
        .map_err(|e| format!("{} to xiaomi_mimo error: {}", "OpenAI Completions", e))?;
    Ok(value)
}

/// xiaomi_mimo 平台格式 → OpenAI Completions 格式
pub fn to_openai_completions(body: &[u8]) -> Result<Value, String> {
    let value: Value = serde_json::from_slice(body)
        .map_err(|e| format!("{} to {} error: {}", "xiaomi_mimo", "OpenAI Completions", e))?;
    Ok(value)
}
