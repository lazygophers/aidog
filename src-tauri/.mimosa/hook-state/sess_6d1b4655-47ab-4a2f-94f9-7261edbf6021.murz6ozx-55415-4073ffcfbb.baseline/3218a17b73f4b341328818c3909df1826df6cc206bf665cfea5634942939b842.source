//! dmxapi 平台 ↔ OpenAI Chat 格式

use serde_json::Value;

/// OpenAI Chat 格式 → dmxapi 平台格式
pub fn from_openai_chat(body: &[u8]) -> Result<Value, String> {
    let value: Value =
        serde_json::from_slice(body).map_err(|e| format!("OpenAI Chat to dmxapi error: {}", e))?;
    Ok(value)
}

/// dmxapi 平台格式 → OpenAI Chat 格式
pub fn to_openai_chat(body: &[u8]) -> Result<Value, String> {
    let value: Value =
        serde_json::from_slice(body).map_err(|e| format!("dmxapi to OpenAI Chat error: {}", e))?;
    Ok(value)
}
