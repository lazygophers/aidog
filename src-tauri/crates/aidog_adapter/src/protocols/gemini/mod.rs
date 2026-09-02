//! Gemini 协议转换器

pub mod convert;
pub use convert::*;

use crate::converter::NonStreamResponse;
use crate::converter::traits::ProtocolConverter;
use crate::types::*;
use serde_json::Value;

/// Gemini 协议转换器实现
pub struct GeminiConverter;

impl ProtocolConverter for GeminiConverter {
    fn protocol_name(&self) -> &'static str {
        "gemini"
    }

    fn parse_incoming(&self, body: &[u8]) -> Result<ChatRequest, String> {
        let value: Value =
            serde_json::from_slice(body).map_err(|e| format!("Gemini parse error: {}", e))?;
        from_gemini(&value).ok_or_else(|| "Gemini parse failed".to_string())
    }

    fn serialize_request(&self, req: &ChatRequest) -> Result<(Value, String), String> {
        let gemini_req = to_gemini(req);
        let body = serde_json::to_value(gemini_req)
            .map_err(|e| format!("Gemini serialize error: {}", e))?;
        Ok((body, "/v1beta/models/{model}:generateContent".to_string()))
    }

    fn parse_sse(&self, chunk: &[u8]) -> Result<Vec<ChatStreamEvent>, String> {
        let value: Value =
            serde_json::from_slice(chunk).map_err(|e| format!("Gemini SSE parse error: {}", e))?;
        parse_gemini_sse(&value)
            .map(|e| vec![e])
            .ok_or_else(|| "Gemini SSE parse failed".to_string())
    }

    fn to_client_sse(&self, event: &ChatStreamEvent) -> Result<String, String> {
        to_gemini_sse(event, "gemini").ok_or_else(|| "Gemini to_client_sse failed".to_string())
    }

    fn parse_response(&self, body: &[u8]) -> Result<NonStreamResponse, String> {
        let value: Value = serde_json::from_slice(body)
            .map_err(|e| format!("Gemini response parse error: {}", e))?;
        parse_gemini_response(&value, "").ok_or_else(|| "Gemini response parse failed".to_string())
    }
}
