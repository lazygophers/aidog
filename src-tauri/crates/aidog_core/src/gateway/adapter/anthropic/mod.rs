//! Anthropic 协议转换器
//!
//! 处理 Anthropic 格式 ↔ 各平台实际格式的转换

use crate::gateway::adapter::types::*;
use serde_json::Value;

// === 入站：各平台格式 → Anthropic ===

/// GLM 平台格式 → Anthropic
pub fn from_glm(body: &[u8]) -> Result<Value, String> {
    // GLM 使用 OpenAI 兼容格式，直接转换
    let value: Value = serde_json::from_slice(body)
        .map_err(|e| format!("GLM to Anthropic parse error: {}", e))?;
    Ok(value)
}

/// Kimi 平台格式 → Anthropic
pub fn from_kimi(body: &[u8]) -> Result<Value, String> {
    let value: Value = serde_json::from_slice(body)
        .map_err(|e| format!("Kimi to Anthropic parse error: {}", e))?;
    Ok(value)
}

/// MiniMax 平台格式 → Anthropic
pub fn from_minimax(body: &[u8]) -> Result<Value, String> {
    let value: Value = serde_json::from_slice(body)
        .map_err(|e| format!("MiniMax to Anthropic parse error: {}", e))?;
    Ok(value)
}

// === 出站：Anthropic → 各平台格式 ===

/// Anthropic → GLM 平台格式
pub fn to_glm(req: &ChatRequest) -> Result<Value, String> {
    // TODO: 实现 Anthropic 到 GLM 格式的转换
    serde_json::to_value(req)
        .map_err(|e| format!("Anthropic to GLM error: {}", e))
}

/// Anthropic → Kimi 平台格式
pub fn to_kimi(req: &ChatRequest) -> Result<Value, String> {
    serde_json::to_value(req)
        .map_err(|e| format!("Anthropic to Kimi error: {}", e))
}

/// Anthropic → MiniMax 平台格式
pub fn to_minimax(req: &ChatRequest) -> Result<Value, String> {
    serde_json::to_value(req)
        .map_err(|e| format!("Anthropic to MiniMax error: {}", e))
}
