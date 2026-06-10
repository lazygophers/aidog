use serde_json::Value;

use super::types::*;

/// Kimi (Moonshot) API 请求格式
/// 完全兼容 OpenAI Chat Completions API
/// 直接复用 OpenAI 转换逻辑
#[allow(dead_code)]
pub fn to_kimi(req: &ChatRequest) -> super::openai::OpenAIRequest {
    super::openai::to_openai(req)
}

/// Kimi SSE 与 OpenAI 完全兼容
#[allow(dead_code)]
pub fn parse_kimi_sse(data: &Value) -> Option<ChatStreamEvent> {
    super::openai::parse_openai_sse(data)
}
