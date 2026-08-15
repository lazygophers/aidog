//! OpenAI Completions 协议转换器

pub mod convert;

pub use convert::*;

use crate::gateway::adapter::converter::traits::ProtocolConverter;
use crate::gateway::adapter::converter::NonStreamResponse;
use crate::gateway::adapter::types::*;
use serde_json::Value;

/// OpenAI Completions 协议转换器实现
pub struct OpenAICompletionsConverter;

impl ProtocolConverter for OpenAICompletionsConverter {
    fn protocol_name(&self) -> &'static str {
        "openai_completions"
    }

    fn parse_incoming(&self, _body: &[u8]) -> Result<ChatRequest, String> {
        Err("OpenAICompletions parse_incoming: TODO".to_string())
    }

    fn serialize_request(&self, _req: &ChatRequest) -> Result<(Value, String), String> {
        Err("OpenAICompletions serialize_request: TODO".to_string())
    }

    fn parse_sse(&self, _chunk: &[u8]) -> Result<Vec<ChatStreamEvent>, String> {
        Err("OpenAICompletions parse_sse: TODO".to_string())
    }

    fn to_client_sse(&self, _event: &ChatStreamEvent) -> Result<String, String> {
        Err("OpenAICompletions to_client_sse: TODO".to_string())
    }

    fn parse_response(&self, _body: &[u8]) -> Result<NonStreamResponse, String> {
        Err("OpenAICompletions parse_response: TODO".to_string())
    }
}
