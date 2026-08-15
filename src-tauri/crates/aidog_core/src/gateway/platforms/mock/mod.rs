//! Mock 平台转换器

use super::super::adapter::types::*;
use super::traits::PlatformConverter;
use serde_json::Value;

/// Mock 平台转换器实现
pub struct MockConverter;

impl PlatformConverter for MockConverter {
    fn platform_name(&self) -> &'static str {
        "mock"
    }

    fn parse_openai_chat(&self, body: &[u8]) -> Result<ChatRequest, String> {
        serde_json::from_slice(body)
            .map_err(|e| format!("Mock parse OpenAI Chat error: {}", e))
    }

    fn parse_openai_completions(&self, body: &[u8]) -> Result<ChatRequest, String> {
        serde_json::from_slice(body)
            .map_err(|e| format!("Mock parse OpenAI Completions error: {}", e))
    }

    fn parse_openai_responses(&self, body: &[u8]) -> Result<ChatRequest, String> {
        serde_json::from_slice(body)
            .map_err(|e| format!("Mock parse OpenAI Responses error: {}", e))
    }

    fn parse_anthropic(&self, body: &[u8]) -> Result<ChatRequest, String> {
        serde_json::from_slice(body)
            .map_err(|e| format!("Mock parse Anthropic error: {}", e))
    }

    fn parse_gemini(&self, body: &[u8]) -> Result<ChatRequest, String> {
        serde_json::from_slice(body)
            .map_err(|e| format!("Mock parse Gemini error: {}", e))
    }

    fn to_openai_chat(&self, _req: &ChatRequest) -> Result<(Value, String), String> {
        // Mock 不需要真实上游请求
        Ok((Value::Null, String::new()))
    }

    fn to_anthropic(&self, _req: &ChatRequest) -> Result<(Value, String), String> {
        Ok((Value::Null, String::new()))
    }

    fn to_gemini(&self, _req: &ChatRequest) -> Result<(Value, String), String> {
        Ok((Value::Null, String::new()))
    }
}
