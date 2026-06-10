use serde_json::Value;

use super::types::*;

/// DashScope (阿里百炼) 兼容 OpenAI Chat Completions 格式
/// 直接复用 OpenAI 的请求/响应结构
pub use super::openai::OpenAIRequest;

/// 从内部 ChatRequest 转为百炼 (OpenAI-compatible) 格式
#[allow(dead_code)]
pub fn to_bailian(req: &ChatRequest) -> OpenAIRequest {
    // DashScope 兼容 OpenAI 格式，逻辑一致
    super::openai::to_openai(req)
}

/// 解析百炼 SSE 格式的流式事件（与 OpenAI 格式一致）
#[allow(dead_code)]
pub fn parse_bailian_sse(data: &Value) -> Option<ChatStreamEvent> {
    super::openai::parse_openai_sse(data)
}
