//! xiaomi_mimo_coding 平台 ↔ OpenAI Responses 格式

use serde_json::Value;

/// OpenAI Responses 格式 → xiaomi_mimo_coding 平台格式
pub fn from_openai_responses(body: &[u8]) -> Result<Value, String> {
    let value: Value = serde_json::from_slice(body)
        .map_err(|e| format!("OpenAI Responses to xiaomi_mimo_coding error: {}", e))?;
    Ok(value)
}

/// xiaomi_mimo_coding 平台格式 → OpenAI Responses 格式
pub fn to_openai_responses(body: &[u8]) -> Result<Value, String> {
    let value: Value = serde_json::from_slice(body)
        .map_err(|e| format!("xiaomi_mimo_coding to OpenAI Responses error: {}", e))?;
    Ok(value)
}
