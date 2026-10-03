//! OpenAI Responses 协议转换器

pub mod convert;

pub use convert::*;

use crate::converter::NonStreamResponse;
use crate::converter::traits::ProtocolConverter;
use crate::types::*;
use serde_json::Value;

/// OpenAI Responses 协议转换器实现
pub struct OpenAIResponsesConverter;

impl ProtocolConverter for OpenAIResponsesConverter {
    fn protocol_name(&self) -> &'static str {
        "openai_responses"
    }

    fn parse_incoming(&self, body: &[u8]) -> Result<ChatRequest, String> {
        let value: Value = serde_json::from_slice(body)
            .map_err(|e| format!("OpenAIResponses parse error: {}", e))?;
        from_responses(&value).ok_or_else(|| "OpenAIResponses parse failed".to_string())
    }

    fn serialize_request(&self, req: &ChatRequest) -> Result<(Value, String), String> {
        let responses_req = to_responses(req);
        let body = serde_json::to_value(responses_req)
            .map_err(|e| format!("OpenAIResponses serialize error: {}", e))?;
        Ok((body, "/v1/responses".to_string()))
    }

    fn parse_sse(&self, chunk: &[u8]) -> Result<Vec<ChatStreamEvent>, String> {
        let value: Value = serde_json::from_slice(chunk)
            .map_err(|e| format!("OpenAIResponses SSE parse error: {}", e))?;
        parse_responses_sse(&value)
            .map(|e| vec![e])
            .ok_or_else(|| "OpenAIResponses SSE parse failed".to_string())
    }

    fn to_client_sse(&self, event: &ChatStreamEvent) -> Result<String, String> {
        // Responses 客户端出站沿用 OpenAI chunk 格式（与 converter::response::to_client_sse 既有行为一致）
        super::openai::to_openai_sse(event, "openai")
            .ok_or_else(|| "OpenAIResponses to_client_sse failed".to_string())
    }

    fn parse_response(&self, body: &[u8]) -> Result<NonStreamResponse, String> {
        let value: Value = serde_json::from_slice(body)
            .map_err(|e| format!("OpenAIResponses response parse error: {}", e))?;
        parse_responses_response(&value, "")
            .ok_or_else(|| "OpenAIResponses response parse failed".to_string())
    }
}
