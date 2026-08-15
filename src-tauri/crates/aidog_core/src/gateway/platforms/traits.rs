//! 平台转换器统一接口

use crate::gateway::adapter::types::*;
use serde_json::Value;

/// 平台转换器 trait：定义各平台的入站解析和出站序列化接口
///
/// 每个平台支持多种协议格式的入站和出站转换
pub trait PlatformConverter: Send + Sync {
    /// 平台标识（对应 Protocol enum 变体名）
    fn platform_name(&self) -> &'static str;

    // === 入站：各种协议格式 → 平台内部格式（ChatRequest） ===

    /// OpenAI Chat Completions 格式 → ChatRequest
    fn parse_openai_chat(&self, body: &[u8]) -> Result<ChatRequest, String>;

    /// OpenAI Completions 格式 → ChatRequest
    fn parse_openai_completions(&self, body: &[u8]) -> Result<ChatRequest, String>;

    /// OpenAI Responses 格式 → ChatRequest
    fn parse_openai_responses(&self, body: &[u8]) -> Result<ChatRequest, String>;

    /// Anthropic Messages 格式 → ChatRequest
    fn parse_anthropic(&self, body: &[u8]) -> Result<ChatRequest, String>;

    /// Gemini 格式 → ChatRequest
    fn parse_gemini(&self, body: &[u8]) -> Result<ChatRequest, String>;

    // === 出站：平台内部格式（ChatRequest）→ 各种协议格式 ===

    /// ChatRequest → OpenAI Chat Completions 格式 (body_json, api_path)
    fn to_openai_chat(&self, req: &ChatRequest) -> Result<(Value, String), String>;

    /// ChatRequest → Anthropic 格式
    fn to_anthropic(&self, req: &ChatRequest) -> Result<(Value, String), String>;

    /// ChatRequest → Gemini 格式
    fn to_gemini(&self, req: &ChatRequest) -> Result<(Value, String), String>;
}
