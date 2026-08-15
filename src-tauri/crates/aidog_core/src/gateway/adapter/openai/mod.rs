//! OpenAI 协议转换器
//!
//! 处理 OpenAI Chat 格式 ↔ 各平台实际格式的转换

use crate::gateway::adapter::types::*;
use serde_json::Value;

// === 入站：各平台格式 → OpenAI ===

/// GLM 平台格式 → OpenAI
pub fn from_glm(body: &[u8]) -> Result<Value, String> {
    // GLM 原生就是 OpenAI 格式，直接透传
    let value: Value = serde_json::from_slice(body)
        .map_err(|e| format!("GLM to OpenAI parse error: {}", e))?;
    Ok(value)
}

/// Kimi 平台格式 → OpenAI
pub fn from_kimi(body: &[u8]) -> Result<Value, String> {
    let value: Value = serde_json::from_slice(body)
        .map_err(|e| format!("Kimi to OpenAI parse error: {}", e))?;
    Ok(value)
}

/// MiniMax 平台格式 → OpenAI
pub fn from_minimax(body: &[u8]) -> Result<Value, String> {
    let value: Value = serde_json::from_slice(body)
        .map_err(|e| format!("MiniMax to OpenAI parse error: {}", e))?;
    Ok(value)
}

/// DeepSeek 平台格式 → OpenAI
pub fn from_deepseek(body: &[u8]) -> Result<Value, String> {
    let value: Value = serde_json::from_slice(body)
        .map_err(|e| format!("DeepSeek to OpenAI parse error: {}", e))?;
    Ok(value)
}

// === 出站：OpenAI → 各平台格式 ===

/// OpenAI → GLM 平台格式
pub fn to_glm(req: &ChatRequest) -> Result<Value, String> {
    // TODO: 实现 OpenAI 到 GLM 格式的转换（如果有平台特定字段）
    serde_json::to_value(req)
        .map_err(|e| format!("OpenAI to GLM error: {}", e))
}

/// OpenAI → Kimi 平台格式
pub fn to_kimi(req: &ChatRequest) -> Result<Value, String> {
    serde_json::to_value(req)
        .map_err(|e| format!("OpenAI to Kimi error: {}", e))
}

/// OpenAI → MiniMax 平台格式
pub fn to_minimax(req: &ChatRequest) -> Result<Value, String> {
    serde_json::to_value(req)
        .map_err(|e| format!("OpenAI to MiniMax error: {}", e))
}

/// OpenAI → DeepSeek 平台格式
pub fn to_deepseek(req: &ChatRequest) -> Result<Value, String> {
    serde_json::to_value(req)
        .map_err(|e| format!("OpenAI to DeepSeek error: {}", e))
}
