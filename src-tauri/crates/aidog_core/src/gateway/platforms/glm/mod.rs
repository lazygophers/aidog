//! GLM 平台转换器

use super::super::adapter::types::*;
use super::super::adapter::protocols::openai::{to_openai, from_openai};
use super::super::adapter::protocols::anthropic::{to_anthropic};
use super::super::adapter::protocols::gemini::{to_gemini};
use super::traits::PlatformConverter;
use serde_json::Value;

/// GLM 平台转换器实现
pub struct GlmConverter;

impl PlatformConverter for GlmConverter {
    fn platform_name(&self) -> &'static str {
        "glm"
    }

    // === 入站：各种协议格式 → ChatRequest ===

    fn parse_openai_chat(&self, body: &[u8]) -> Result<ChatRequest, String> {
        let value: Value = serde_json::from_slice(body)
            .map_err(|e| format!("GLM parse OpenAI Chat error: {}", e))?;
        from_openai(&value).ok_or_else(|| "GLM parse OpenAI Chat failed".to_string())
    }

    fn parse_openai_completions(&self, body: &[u8]) -> Result<ChatRequest, String> {
        // TODO: 实现 OpenAI Completions → ChatRequest
        Err("GLM parse_openai_completions: TODO".to_string())
    }

    fn parse_openai_responses(&self, body: &[u8]) -> Result<ChatRequest, String> {
        // TODO: 实现 OpenAI Responses → ChatRequest
        Err("GLM parse_openai_responses: TODO".to_string())
    }

    fn parse_anthropic(&self, body: &[u8]) -> Result<ChatRequest, String> {
        // Anthropic 入站直接反序列化为 ChatRequest
        serde_json::from_slice(body)
            .map_err(|e| format!("GLM parse Anthropic error: {}", e))
    }

    fn parse_gemini(&self, body: &[u8]) -> Result<ChatRequest, String> {
        // TODO: 实现 Gemini → ChatRequest
        Err("GLM parse_gemini: TODO".to_string())
    }

    // === 出站：ChatRequest → 各种协议格式 ===

    fn to_openai_chat(&self, req: &ChatRequest) -> Result<(Value, String), String> {
        let openai_req = to_openai(req);
        let body = serde_json::to_value(openai_req)
            .map_err(|e| format!("GLM to OpenAI Chat error: {}", e))?;
        Ok((body, "/v1/chat/completions".to_string()))
    }

    fn to_anthropic(&self, req: &ChatRequest) -> Result<(Value, String), String> {
        let anthropic_req = to_anthropic(req);
        let body = serde_json::to_value(anthropic_req)
            .map_err(|e| format!("GLM to Anthropic error: {}", e))?;
        Ok((body, "/v1/messages".to_string()))
    }

    fn to_gemini(&self, req: &ChatRequest) -> Result<(Value, String), String> {
        let gemini_req = to_gemini(req);
        let body = serde_json::to_value(gemini_req)
            .map_err(|e| format!("GLM to Gemini error: {}", e))?;
        Ok((body, "/v1beta/models/{model}:generateContent".to_string()))
    }
}
