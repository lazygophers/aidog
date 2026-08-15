//! Gemini 协议转换器
//!
//! 处理 Gemini 格式 ↔ 各平台实际格式的转换

use crate::gateway::adapter::types::*;
use serde_json::Value;

// === 入站：各平台格式 → Gemini ===

/// GLM 平台格式 → Gemini
pub fn from_glm(body: &[u8]) -> Result<Value, String> {
    // TODO: 实现 GLM 到 Gemini 格式的转换
    let value: Value = serde_json::from_slice(body)
        .map_err(|e| format!("GLM to Gemini parse error: {}", e))?;
    Ok(value)
}

/// Kimi 平台格式 → Gemini
pub fn from_kimi(body: &[u8]) -> Result<Value, String> {
    let value: Value = serde_json::from_slice(body)
        .map_err(|e| format!("Kimi to Gemini parse error: {}", e))?;
    Ok(value)
}

// === 出站：Gemini → 各平台格式 ===

/// Gemini → GLM 平台格式
pub fn to_glm(req: &ChatRequest) -> Result<Value, String> {
    // TODO: 实现 Gemini 到 GLM 格式的转换
    serde_json::to_value(req)
        .map_err(|e| format!("Gemini to GLM error: {}", e))
}

/// Gemini → Kimi 平台格式
pub fn to_kimi(req: &ChatRequest) -> Result<Value, String> {
    serde_json::to_value(req)
        .map_err(|e| format!("Gemini to Kimi error: {}", e))
}
