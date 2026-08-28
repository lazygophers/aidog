//! xiaomi_mimo_coding_en 平台 ↔ OpenAI Completions 格式

use serde_json::Value;

/// OpenAI Completions 格式 → xiaomi_mimo_coding_en 平台格式
pub fn from_openai_completions(body: &[u8]) -> Result<Value, String> {
    let value: Value = serde_json::from_slice(body)
        .map_err(|e| format!("OpenAI Completions to xiaomi_mimo_coding_en error: {}", e))?;
    Ok(value)
}

/// xiaomi_mimo_coding_en 平台格式 → OpenAI Completions 格式
pub fn to_openai_completions(body: &[u8]) -> Result<Value, String> {
    let value: Value = serde_json::from_slice(body)
        .map_err(|e| format!("xiaomi_mimo_coding_en to OpenAI Completions error: {}", e))?;
    Ok(value)
}
