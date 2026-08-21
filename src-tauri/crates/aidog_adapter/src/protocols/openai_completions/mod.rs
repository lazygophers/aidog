//! OpenAI Completions 协议转换器

pub mod convert;

pub use convert::*;

use crate::converter::traits::ProtocolConverter;
use crate::converter::NonStreamResponse;
use crate::types::*;
use serde_json::Value;

/// OpenAI Completions 协议转换器实现
pub struct OpenAICompletionsConverter;

impl ProtocolConverter for OpenAICompletionsConverter {
    fn protocol_name(&self) -> &'static str {
        "openai_completions"
    }

    fn parse_incoming(&self, body: &[u8]) -> Result<ChatRequest, String> {
        let value: Value = serde_json::from_slice(body)
            .map_err(|e| format!("OpenAICompletions parse error: {}", e))?;
        from_completions(&value).ok_or_else(|| "OpenAICompletions parse failed".to_string())
    }

    fn serialize_request(&self, req: &ChatRequest) -> Result<(Value, String), String> {
        let completions_req = to_completions(req);
        let body = serde_json::to_value(completions_req)
            .map_err(|e| format!("OpenAICompletions serialize error: {}", e))?;
        Ok((body, "/v1/completions".to_string()))
    }

    fn parse_sse(&self, chunk: &[u8]) -> Result<Vec<ChatStreamEvent>, String> {
        let value: Value = serde_json::from_slice(chunk)
            .map_err(|e| format!("OpenAICompletions SSE parse error: {}", e))?;
        parse_completions_sse(&value)
            .map(|e| vec![e])
            .ok_or_else(|| "OpenAICompletions SSE parse failed".to_string())
    }

    fn to_client_sse(&self, event: &ChatStreamEvent) -> Result<String, String> {
        // Completions 客户端出站沿用 OpenAI chunk 格式（与 converter::response::to_client_sse 既有行为一致）
        super::openai::to_openai_sse(event, "openai")
            .ok_or_else(|| "OpenAICompletions to_client_sse failed".to_string())
    }

    fn parse_response(&self, body: &[u8]) -> Result<NonStreamResponse, String> {
        let value: Value = serde_json::from_slice(body)
            .map_err(|e| format!("OpenAICompletions response parse error: {}", e))?;
        parse_completions_response(&value, "")
            .ok_or_else(|| "OpenAICompletions response parse failed".to_string())
    }
}
