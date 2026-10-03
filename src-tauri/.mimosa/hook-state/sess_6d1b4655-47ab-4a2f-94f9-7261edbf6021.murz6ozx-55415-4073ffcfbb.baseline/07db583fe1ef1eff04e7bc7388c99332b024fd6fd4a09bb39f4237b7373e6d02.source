//! newapi 平台 ↔ OpenAI Completions 格式

use serde_json::Value;

/// OpenAI Completions 格式 → newapi 平台格式
pub fn from_openai_completions(body: &[u8]) -> Result<Value, String> {
    let value: Value = serde_json::from_slice(body)
        .map_err(|e| format!("OpenAI Completions to newapi error: {}", e))?;
    Ok(value)
}

/// newapi 平台格式 → OpenAI Completions 格式
pub fn to_openai_completions(body: &[u8]) -> Result<Value, String> {
    let value: Value = serde_json::from_slice(body)
        .map_err(|e| format!("newapi to OpenAI Completions error: {}", e))?;
    Ok(value)
}
