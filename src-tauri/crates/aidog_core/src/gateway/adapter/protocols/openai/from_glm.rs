//! GLM 平台 → OpenAI 格式

use serde_json::Value;

/// GLM 平台响应 → OpenAI 格式
pub fn from_glm(body: &[u8]) -> Result<Value, String> {
    // GLM 原生就是 OpenAI 格式，直接透传
    let value: Value = serde_json::from_slice(body)
        .map_err(|e| format!("GLM to OpenAI parse error: {}", e))?;
    Ok(value)
}
