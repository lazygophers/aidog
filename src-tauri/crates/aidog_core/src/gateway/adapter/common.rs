//! 通用转换工具函数

use crate::gateway::adapter::types::*;
use serde_json::Value;

/// OpenAI Chat → Anthropic（通用转换，适用于 GLM/Kimi 等 OpenAI 兼容平台）
pub fn openai_to_anthropic(body: &Value) -> Result<ChatRequest, String> {
    // TODO: 实现 OpenAI 格式到内部 ChatRequest 的转换
    serde_json::from_value::<ChatRequest>(body.clone())
        .map_err(|e| format!("OpenAI to Anthropic conversion error: {}", e))
}

/// Anthropic → OpenAI Chat（通用转换）
pub fn anthropic_to_openai(req: &ChatRequest) -> Result<Value, String> {
    // TODO: 实现 ChatRequest 到 OpenAI 格式的转换
    serde_json::to_value(req)
        .map_err(|e| format!("Anthropic to OpenAI conversion error: {}", e))
}
