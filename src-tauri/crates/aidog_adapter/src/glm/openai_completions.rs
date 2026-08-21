//! GLM 平台 ↔ OpenAI Completions 格式

use serde_json::Value;

/// OpenAI Completions 格式 → GLM 平台格式。
/// GLM wire 即 OpenAI 兼容格式（chat completions base_url，见 defaults platform-presets），
/// 无字段差异需转换，JSON 解析校验后透传。
pub fn from_openai_completions(body: &[u8]) -> Result<Value, String> {
    let value: Value = serde_json::from_slice(body)
        .map_err(|e| format!("OpenAI Completions to GLM error: {}", e))?;
    Ok(value)
}

/// GLM 平台格式 → OpenAI Completions 格式
pub fn to_openai_completions(body: &[u8]) -> Result<Value, String> {
    let value: Value = serde_json::from_slice(body)
        .map_err(|e| format!("GLM to OpenAI Completions error: {}", e))?;
    Ok(value)
}
