use serde_json::Value;

use super::types::*;

/// Claude Code (Anthropic) API 请求格式
/// 完全兼容 Anthropic Messages API
pub fn to_claude_code(req: &ChatRequest) -> super::anthropic::AnthropicRequest {
    super::anthropic::to_anthropic(req)
}

/// Claude Code SSE 与 Anthropic 完全兼容
pub fn parse_claude_code_sse(data: &Value) -> Option<ChatStreamEvent> {
    super::anthropic::parse_anthropic_sse(data)
}
