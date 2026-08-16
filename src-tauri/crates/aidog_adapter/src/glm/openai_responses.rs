//! GLM 平台 ↔ OpenAI Responses 格式

use serde_json::Value;

/// OpenAI Responses 格式 → GLM 平台格式
pub fn from_openai_responses(body: &[u8]) -> Result<Value, String> {
    // TODO: 实现 OpenAI Responses 到 GLM 的转换
    let value: Value = serde_json::from_slice(body)
        .map_err(|e| format!("OpenAI Responses to GLM error: {}", e))?;
    Ok(value)
}

/// GLM 平台格式 → OpenAI Responses 格式
pub fn to_openai_responses(body: &[u8]) -> Result<Value, String> {
    let value: Value = serde_json::from_slice(body)
        .map_err(|e| format!("GLM to OpenAI Responses error: {}", e))?;
    Ok(value)
}
