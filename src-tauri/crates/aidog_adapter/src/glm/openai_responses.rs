//! GLM 平台 ↔ OpenAI Responses 格式

use serde_json::Value;

/// OpenAI Responses 格式 → GLM 平台格式。
/// GLM wire 即 OpenAI 兼容格式（chat completions base_url，见 defaults platform-presets），
/// 跨协议转换走通用 converter（ChatRequest 中立层），本 shim 无字段差异需转换，透传即可。
pub fn from_openai_responses(body: &[u8]) -> Result<Value, String> {
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
