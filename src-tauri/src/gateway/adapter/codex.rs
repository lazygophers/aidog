use serde_json::Value;

use super::types::*;

/// Codex (OpenAI) API 请求格式
/// 完全兼容 OpenAI Chat Completions API
#[allow(dead_code)]
pub fn to_codex(req: &ChatRequest) -> super::openai::OpenAIRequest {
    super::openai::to_openai(req)
}

/// Codex SSE 与 OpenAI 完全兼容
#[allow(dead_code)]
pub fn parse_codex_sse(data: &Value) -> Option<ChatStreamEvent> {
    super::openai::parse_openai_sse(data)
}
