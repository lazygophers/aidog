//! aicodemirror 平台转换器

use super::super::adapter::types::*;
use super::traits::PlatformConverter;
use serde_json::Value;

/// aicodemirror 平台转换器实现
pub struct AicodemirrorConverter;

impl PlatformConverter for AicodemirrorConverter {
    fn platform_name(&self) -> &'static str {
        "aicodemirror"
    }

    fn parse_openai_chat(&self, _body: &[u8]) -> Result<ChatRequest, String> {
        Err("aicodemirror parse_openai_chat: TODO".to_string())
    }

    fn parse_openai_completions(&self, _body: &[u8]) -> Result<ChatRequest, String> {
        Err("aicodemirror parse_openai_completions: TODO".to_string())
    }

    fn parse_openai_responses(&self, _body: &[u8]) -> Result<ChatRequest, String> {
        Err("aicodemirror parse_openai_responses: TODO".to_string())
    }

    fn parse_anthropic(&self, _body: &[u8]) -> Result<ChatRequest, String> {
        Err("aicodemirror parse_anthropic: TODO".to_string())
    }

    fn parse_gemini(&self, _body: &[u8]) -> Result<ChatRequest, String> {
        Err("aicodemirror parse_gemini: TODO".to_string())
    }

    fn to_openai_chat(&self, _req: &ChatRequest) -> Result<(Value, String), String> {
        Err("aicodemirror to_openai_chat: TODO".to_string())
    }

    fn to_anthropic(&self, _req: &ChatRequest) -> Result<(Value, String), String> {
        Err("aicodemirror to_anthropic: TODO".to_string())
    }

    fn to_gemini(&self, _req: &ChatRequest) -> Result<(Value, String), String> {
        Err("aicodemirror to_gemini: TODO".to_string())
    }
}
