use serde_json::Value;

use super::types::*;

/// MiniMax API 请求格式
/// 兼容 OpenAI Chat Completions API，复用 OpenAI 消息转换
pub fn to_minimax(req: &ChatRequest) -> super::openai::OpenAIRequest {
    super::openai::to_openai(req)
}

/// MiniMax SSE 与 OpenAI 兼容，直接复用
pub fn parse_minimax_sse(data: &Value) -> Option<ChatStreamEvent> {
    super::openai::parse_openai_sse(data)
}
