//! OpenAI Responses 协议转换器

pub mod convert;

pub use convert::*;

use crate::converter::traits::ProtocolConverter;
use crate::converter::NonStreamResponse;
use crate::types::*;
use serde_json::Value;

/// OpenAI Responses 协议转换器实现
pub struct OpenAIResponsesConverter;

impl ProtocolConverter for OpenAIResponsesConverter {
    fn protocol_name(&self) -> &'static str {
        "openai_responses"
    }

    fn parse_incoming(&self, _body: &[u8]) -> Result<ChatRequest, String> {
        Err("OpenAIResponses parse_incoming: TODO".to_string())
    }

    fn serialize_request(&self, _req: &ChatRequest) -> Result<(Value, String), String> {
        Err("OpenAIResponses serialize_request: TODO".to_string())
    }

    fn parse_sse(&self, _chunk: &[u8]) -> Result<Vec<ChatStreamEvent>, String> {
        Err("OpenAIResponses parse_sse: TODO".to_string())
    }

    fn to_client_sse(&self, _event: &ChatStreamEvent) -> Result<String, String> {
        Err("OpenAIResponses to_client_sse: TODO".to_string())
    }

    fn parse_response(&self, _body: &[u8]) -> Result<NonStreamResponse, String> {
        Err("OpenAIResponses parse_response: TODO".to_string())
    }
}
