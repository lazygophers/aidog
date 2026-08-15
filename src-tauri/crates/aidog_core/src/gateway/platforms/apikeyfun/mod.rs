//! apikeyfun 平台转换器

use super::super::adapter::types::*;
use super::traits::PlatformConverter;
use serde_json::Value;

/// apikeyfun 平台转换器实现
pub struct ApikeyfunConverter;

impl PlatformConverter for ApikeyfunConverter {
    fn platform_name(&self) -> &'static str {
        "apikeyfun"
    }

    fn parse_openai_chat(&self, _body: &[u8]) -> Result<ChatRequest, String> {
        Err("apikeyfun parse_openai_chat: TODO".to_string())
    }

    fn parse_openai_completions(&self, _body: &[u8]) -> Result<ChatRequest, String> {
        Err("apikeyfun parse_openai_completions: TODO".to_string())
    }

    fn parse_openai_responses(&self, _body: &[u8]) -> Result<ChatRequest, String> {
        Err("apikeyfun parse_openai_responses: TODO".to_string())
    }

    fn parse_anthropic(&self, _body: &[u8]) -> Result<ChatRequest, String> {
        Err("apikeyfun parse_anthropic: TODO".to_string())
    }

    fn parse_gemini(&self, _body: &[u8]) -> Result<ChatRequest, String> {
        Err("apikeyfun parse_gemini: TODO".to_string())
    }

    fn to_openai_chat(&self, _req: &ChatRequest) -> Result<(Value, String), String> {
        Err("apikeyfun to_openai_chat: TODO".to_string())
    }

    fn to_anthropic(&self, _req: &ChatRequest) -> Result<(Value, String), String> {
        Err("apikeyfun to_anthropic: TODO".to_string())
    }

    fn to_gemini(&self, _req: &ChatRequest) -> Result<(Value, String), String> {
        Err("apikeyfun to_gemini: TODO".to_string())
    }
}
