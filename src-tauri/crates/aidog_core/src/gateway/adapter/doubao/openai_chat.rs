//! doubao 平台 ↔ OpenAI Chat 格式

use serde_json::Value;

/// OpenAI Chat 格式 → doubao 平台格式
pub fn from_openai_chat(body: &[u8]) -> Result<Value, String> {
    let value: Value = serde_json::from_slice(body)
        .map_err(|e| format!("{} to doubao error: {}", "OpenAI Chat", e))?;
    Ok(value)
}

/// doubao 平台格式 → OpenAI Chat 格式
pub fn to_openai_chat(body: &[u8]) -> Result<Value, String> {
    let value: Value = serde_json::from_slice(body)
        .map_err(|e| format!("{} to {} error: {}", "doubao", "OpenAI Chat", e))?;
    Ok(value)
}
