//! Anthropic 协议转换器

pub mod convert;
pub use convert::*;

use crate::gateway::adapter::converter::traits::ProtocolConverter;
use crate::gateway::adapter::converter::NonStreamResponse;
use crate::gateway::adapter::types::*;
use serde_json::Value;

/// Anthropic 协议转换器实现
pub struct AnthropicConverter;

impl ProtocolConverter for AnthropicConverter {
    fn protocol_name(&self) -> &'static str {
        "anthropic"
    }

    fn parse_incoming(&self, body: &[u8]) -> Result<ChatRequest, String> {
        // Anthropic 入站请求直接反序列化为 ChatRequest
        serde_json::from_slice(body)
            .map_err(|e| format!("Anthropic parse error: {}", e))
    }

    fn serialize_request(&self, req: &ChatRequest) -> Result<(Value, String), String> {
        let anthropic_req = to_anthropic(req);
        let body = serde_json::to_value(anthropic_req)
            .map_err(|e| format!("Anthropic serialize error: {}", e))?;
        Ok((body, "/v1/messages".to_string()))
    }

    fn parse_sse(&self, chunk: &[u8]) -> Result<Vec<ChatStreamEvent>, String> {
        let data: Value = serde_json::from_slice(chunk)
            .map_err(|e| format!("Anthropic SSE parse error: {}", e))?;
        parse_anthropic_sse(&data)
            .map(|e| vec![e])
            .ok_or_else(|| "Anthropic SSE parse failed".to_string())
    }

    fn to_client_sse(&self, event: &ChatStreamEvent) -> Result<String, String> {
        // Anthropic 客户端直接透传原始 SSE
        match event {
            ChatStreamEvent::Start { .. } => Ok("event: message_start\n".to_string()),
            ChatStreamEvent::Delta { .. } => Ok("event: content_block_delta\n".to_string()),
            ChatStreamEvent::Stop { .. } => Ok("event: message_delta\n".to_string()),
            _ => Ok(String::new()),
        }
    }

    fn parse_response(&self, body: &[u8]) -> Result<NonStreamResponse, String> {
        let value: Value = serde_json::from_slice(body)
            .map_err(|e| format!("Anthropic response parse error: {}", e))?;
        parse_anthropic_response(&value, "")
            .ok_or_else(|| "Anthropic response parse failed".to_string())
    }
}
