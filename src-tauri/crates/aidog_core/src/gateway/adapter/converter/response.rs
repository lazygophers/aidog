//! 上游响应解析与转换：SSE 解析 / 非流式响应转换 / 按客户端协议渲染 SSE。

use crate::gateway::models::Protocol;
use serde_json::Value;

use super::super::types::*;

/// 将目标协议的 SSE event data 解析为统一的 ChatStreamEvent。
/// SSE 响应格式由 wire protocol（endpoint 协议）决定。
pub fn parse_sse(data: &Value, wire_protocol: &Protocol) -> Option<ChatStreamEvent> {
    match wire_protocol {
        Protocol::Anthropic => super::super::anthropic::parse_anthropic_sse(data),
        Protocol::Gemini => super::super::gemini::parse_gemini_sse(data),
        // 所有 OpenAI 系列共用 OpenAI SSE 解析
        _ => super::super::openai::parse_openai_sse(data),
    }
}

/// 非流式响应内部归一表示（基于 Anthropic 语义：text + tool_use 块 + stop_reason + usage）。
///
/// 上游响应（openai chat completion / anthropic messages / …）先 parse 为本结构，
/// 再按客户端协议 render，避免把上游原生格式直接透回致客户端解析失败。
pub struct NonStreamResponse {
    pub id: String,
    pub model: String,
    /// 文本段（按出现顺序，通常单段）
    pub text: Option<String>,
    /// 工具调用块：(id, name, input)
    pub tool_uses: Vec<(String, String, Value)>,
    /// 统一 stop_reason（anthropic 语义：end_turn / tool_use / max_tokens / stop_sequence）
    pub stop_reason: String,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub cache_read_tokens: i64,
    /// 思维链文本（按 wire 协议语义提取，anthropic thinking 块 / openai reasoning_content 等）
    pub reasoning: Option<String>,
}

/// 非流式上游响应 → 客户端协议响应体转换。
///
/// - `wire_protocol`: 上游响应体格式（endpoint 协议）。
/// - `client_protocol`: 客户端期望协议（入站 source_protocol）。
///
/// 返回 `Some(Value)` 表示已转换为客户端格式；`None` 表示无需 / 无法转换
/// （调用方应原样透传上游 body，保持既有行为）。
///
/// 转换路径：parse_<wire>(body) → NonStreamResponse → render_<client>(parsed) → Value
/// wire == client 时透传；缺实现时回退透传（向后兼容）。
pub fn convert_response(
    body: &Value,
    wire_protocol: &Protocol,
    client_protocol: &str,
    model: &str,
) -> Option<Value> {
    use crate::gateway::models::Protocol;

    // 同协议透传：wire == client → 跳过 parse/render（避免不必要转换）
    let wire_is_openai = matches!(
        wire_protocol,
        Protocol::OpenAI | Protocol::OpenAICompletions | Protocol::OpenAIResponses
    );
    let client_is_openai = matches!(client_protocol, "openai" | "openai_responses" | "openai_completions");

    // wire == client（同语义协议）→ 透传
    if (wire_is_openai && client_is_openai)
        || (matches!(wire_protocol, Protocol::Anthropic) && client_protocol == "anthropic")
        || (matches!(wire_protocol, Protocol::Gemini) && client_protocol == "gemini")
    {
        return None;
    }

    // parse 阶段：wire_protocol → NonStreamResponse
    let parsed = match wire_protocol {
        Protocol::Anthropic => super::super::anthropic::parse_anthropic_response(body, model),
        Protocol::OpenAI => super::super::openai::parse_openai_response(body, model),
        Protocol::OpenAIResponses => super::super::openai_responses::parse_responses_response(body, model),
        Protocol::OpenAICompletions => super::super::openai_completions::parse_completions_response(body, model),
        Protocol::Gemini => super::super::gemini::parse_gemini_response(body, model),
        _ => None, // 非目标协议回退透传
    };

    let parsed = parsed?;

    // render 阶段：NonStreamResponse → client_protocol
    match client_protocol {
        "anthropic" => Some(render_anthropic_response(&parsed)),
        "openai" => Some(super::super::openai::render_openai_response(&parsed)?),
        "openai_responses" => Some(super::super::openai_responses::render_responses_response(&parsed)?),
        "openai_completions" => Some(super::super::openai_completions::render_completions_response(&parsed)?),
        "gemini" => Some(super::super::gemini::render_gemini_response(&parsed)?),
        _ => None, // 未知客户端协议回退透传
    }
}

/// 渲染归一响应为 Anthropic Messages 非流式响应体。
pub fn render_anthropic_response(r: &NonStreamResponse) -> Value {
    let mut content: Vec<Value> = Vec::new();
    // reasoning 排首位（方案 B：禁 thinking 块避 signature 风险）
    if let Some(reasoning) = &r.reasoning
        && !reasoning.is_empty() {
            content.push(serde_json::json!({ "type": "text", "text": reasoning }));
        }
    if let Some(text) = &r.text
        && !text.is_empty() {
            content.push(serde_json::json!({ "type": "text", "text": text }));
        }
    for (id, name, input) in &r.tool_uses {
        content.push(serde_json::json!({
            "type": "tool_use",
            "id": id,
            "name": name,
            "input": input,
        }));
    }
    // 兜底：既无 text 也无 tool_use（异常上游）→ 空 text 块，保证 content 非空数组（Anthropic 合法）。
    if content.is_empty() {
        content.push(serde_json::json!({ "type": "text", "text": "" }));
    }
    serde_json::json!({
        "id": r.id,
        "type": "message",
        "role": "assistant",
        "model": r.model,
        "content": content,
        "stop_reason": r.stop_reason,
        "stop_sequence": null,
        "usage": {
            "input_tokens": r.input_tokens,
            "output_tokens": r.output_tokens,
            "cache_read_input_tokens": r.cache_read_tokens,
        }
    })
}

/// 将统一的 ChatStreamEvent 按客户端协议格式化为 SSE
pub fn to_client_sse(event: &ChatStreamEvent, source_protocol: &str, model: &str) -> Option<String> {
    match source_protocol {
        "openai" | "openai_responses" | "openai_completions" => super::super::openai::to_openai_sse(event, model),
        "gemini" => super::super::gemini::to_gemini_sse(event, model),
        // 默认 Anthropic 格式
        _ => to_anthropic_sse(event),
    }
}

/// 将统一的 ChatStreamEvent 转为 Anthropic SSE 格式（用于返回给 Claude Code 客户端）
pub fn to_anthropic_sse(event: &ChatStreamEvent) -> Option<String> {
    match event {
        ChatStreamEvent::Start { id, model } => Some(format!(
            "event: message_start\ndata: {}\n\n",
            serde_json::json!({
                "type": "message_start",
                "message": {
                    "id": id,
                    "type": "message",
                    "role": "assistant",
                    "model": model,
                    "content": [],
                    "stop_reason": null,
                    "stop_sequence": null,
                    "usage": { "input_tokens": 0, "output_tokens": 0 }
                }
            })
        )),
        ChatStreamEvent::Delta { text } => Some(format!(
            "event: content_block_delta\ndata: {}\n\n",
            serde_json::json!({
                "type": "content_block_delta",
                "index": 0,
                "delta": {
                    "type": "text_delta",
                    "text": text
                }
            })
        )),
        ChatStreamEvent::ReasoningDelta { text } => Some(format!(
            "event: content_block_delta\ndata: {}\n\n",
            serde_json::json!({
                "type": "content_block_delta",
                "index": 0,
                "delta": {
                    "type": "text_delta",
                    "text": text
                }
            })
        )),
        ChatStreamEvent::ToolDelta { index, id, name, input } => {
            let mut parts = Vec::new();

            // tool_use 开始
            if let (Some(id), Some(name)) = (id, name) {
                parts.push(format!(
                    "event: content_block_start\ndata: {}\n\n",
                    serde_json::json!({
                        "type": "content_block_start",
                        "index": index,
                        "content_block": {
                            "type": "tool_use",
                            "id": id,
                            "name": name,
                            "input": {}
                        }
                    })
                ));
            }

            // tool input delta
            if let Some(input) = input {
                parts.push(format!(
                    "event: content_block_delta\ndata: {}\n\n",
                    serde_json::json!({
                        "type": "content_block_delta",
                        "index": index,
                        "delta": {
                            "type": "input_json_delta",
                            "partial_json": input
                        }
                    })
                ));
            }

            if parts.is_empty() {
                None
            } else {
                Some(parts.join(""))
            }
        }
        ChatStreamEvent::Stop { finish_reason } => Some(format!(
            "event: message_delta\ndata: {}\n\nevent: message_stop\ndata: {{\"type\":\"message_stop\"}}\n\n",
            serde_json::json!({
                "type": "message_delta",
                "delta": {
                    "stop_reason": finish_reason.as_deref().unwrap_or("end_turn"),
                    "stop_sequence": null
                },
                "usage": { "output_tokens": 0 }
            })
        )),
        ChatStreamEvent::Usage { .. } => None,
    }
}

#[cfg(test)]
#[path = "test_response.rs"]
mod test_response;
