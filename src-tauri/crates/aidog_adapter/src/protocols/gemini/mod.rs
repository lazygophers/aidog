//! Gemini 协议转换器

pub mod convert;
pub use convert::*;

use crate::converter::traits::ProtocolConverter;
use crate::converter::NonStreamResponse;
use crate::types::*;
use serde_json::Value;

/// Gemini 协议转换器实现
pub struct GeminiConverter;

impl ProtocolConverter for GeminiConverter {
    fn protocol_name(&self) -> &'static str {
        "gemini"
    }

    fn parse_incoming(&self, body: &[u8]) -> Result<ChatRequest, String> {
        // Gemini 入站先转为内部格式
        let _gemini_req: GeminiRequest = serde_json::from_slice(body)
            .map_err(|e| format!("Gemini parse error: {}", e))?;
        // TODO: GeminiRequest → ChatRequest 转换
        Err("Gemini parse_incoming: TODO".to_string())
    }

    fn serialize_request(&self, req: &ChatRequest) -> Result<(Value, String), String> {
        let gemini_req = to_gemini(req);
        let body = serde_json::to_value(gemini_req)
            .map_err(|e| format!("Gemini serialize error: {}", e))?;
        Ok((body, "/v1beta/models/{model}:generateContent".to_string()))
    }

    fn parse_sse(&self, _chunk: &[u8]) -> Result<Vec<ChatStreamEvent>, String> {
        // TODO: Gemini SSE 解析
        Err("Gemini parse_sse: TODO".to_string())
    }

    fn to_client_sse(&self, _event: &ChatStreamEvent) -> Result<String, String> {
        // TODO: Gemini SSE 输出
        Err("Gemini to_client_sse: TODO".to_string())
    }

    fn parse_response(&self, _body: &[u8]) -> Result<NonStreamResponse, String> {
        // TODO: Gemini 响应解析
        Err("Gemini parse_response: TODO".to_string())
    }
}
