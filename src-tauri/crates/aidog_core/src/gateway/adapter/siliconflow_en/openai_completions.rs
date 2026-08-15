//! siliconflow_en 平台 ↔ OpenAI Completions 格式

use serde_json::Value;

/// OpenAI Completions 格式 → siliconflow_en 平台格式
pub fn from_openai_completions(body: &[u8]) -> Result<Value, String> {
    let value: Value = serde_json::from_slice(body)
        .map_err(|e| format!("{} to siliconflow_en error: {}", "OpenAI Completions", e))?;
    Ok(value)
}

/// siliconflow_en 平台格式 → OpenAI Completions 格式
pub fn to_openai_completions(body: &[u8]) -> Result<Value, String> {
    let value: Value = serde_json::from_slice(body)
        .map_err(|e| format!("{} to {} error: {}", "siliconflow_en", "OpenAI Completions", e))?;
    Ok(value)
}
