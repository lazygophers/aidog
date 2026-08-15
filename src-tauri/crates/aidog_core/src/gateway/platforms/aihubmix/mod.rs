//! aihubmix 平台转换器

use super::super::adapter::types::*;
use super::traits::PlatformConverter;
use serde_json::Value;

/// aihubmix 平台转换器实现
pub struct AihubmixConverter;

impl PlatformConverter for AihubmixConverter {
    fn platform_name(&self) -> &'static str {
        "aihubmix"
    }

    fn parse_openai_chat(&self, _body: &[u8]) -> Result<ChatRequest, String> {
        Err("aihubmix parse_openai_chat: TODO".to_string())
    }

    fn parse_openai_completions(&self, _body: &[u8]) -> Result<ChatRequest, String> {
        Err("aihubmix parse_openai_completions: TODO".to_string())
    }

    fn parse_openai_responses(&self, _body: &[u8]) -> Result<ChatRequest, String> {
        Err("aihubmix parse_openai_responses: TODO".to_string())
    }

    fn parse_anthropic(&self, _body: &[u8]) -> Result<ChatRequest, String> {
        Err("aihubmix parse_anthropic: TODO".to_string())
    }

    fn parse_gemini(&self, _body: &[u8]) -> Result<ChatRequest, String> {
        Err("aihubmix parse_gemini: TODO".to_string())
    }

    fn to_openai_chat(&self, _req: &ChatRequest) -> Result<(Value, String), String> {
        Err("aihubmix to_openai_chat: TODO".to_string())
    }

    fn to_anthropic(&self, _req: &ChatRequest) -> Result<(Value, String), String> {
        Err("aihubmix to_anthropic: TODO".to_string())
    }

    fn to_gemini(&self, _req: &ChatRequest) -> Result<(Value, String), String> {
        Err("aihubmix to_gemini: TODO".to_string())
    }
}
