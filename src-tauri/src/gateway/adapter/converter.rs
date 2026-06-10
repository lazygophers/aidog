use crate::gateway::models::Protocol;
use serde_json::Value;

use super::types::*;

/// 将内部 ChatRequest 转为目标格式的 JSON body + API 路径。
///
/// - `wire_protocol`: 请求体格式（由 endpoint 协议决定：anthropic/openai/openai_responses/openai_completions/gemini）
/// - `platform_protocol`: 平台类型（由平台主协议决定，决定 OpenAI-compatible 平台的 API 路径）
pub fn convert_request(req: &ChatRequest, wire_protocol: &Protocol, platform_protocol: &Protocol) -> (Value, String) {
    match wire_protocol {
        Protocol::Anthropic => {
            let anthropic_req = super::anthropic::to_anthropic(req);
            let json = serde_json::to_value(&anthropic_req).unwrap();
            (json, "/v1/messages".to_string())
        }
        Protocol::Gemini => {
            let gemini_req = super::gemini::to_gemini(req);
            let json = serde_json::to_value(&gemini_req).unwrap();
            let path = format!("/v1beta/models/{}:streamGenerateContent", req.model);
            (json, path)
        }
        Protocol::OpenAIResponses => {
            let responses_req = super::openai_responses::to_responses(req);
            let json = serde_json::to_value(&responses_req).unwrap();
            (json, "/v1/responses".to_string())
        }
        Protocol::OpenAICompletions => {
            let completions_req = super::openai_completions::to_completions(req);
            let json = serde_json::to_value(&completions_req).unwrap();
            (json, "/v1/completions".to_string())
        }
        // OpenAI Chat Completions — 标准 /v1/chat/completions，OpenAI-compatible 平台用各自路径
        _ => {
            let openai_req = super::openai::to_openai(req);
            let json = serde_json::to_value(&openai_req).unwrap();
            let path = provider_api_path(platform_protocol);
            (json, path)
        }
    }
}

/// OpenAI Chat Completions 端点路径（统一后缀，base_url 负责版本前缀）
fn provider_api_path(_protocol: &Protocol) -> String {
    "/chat/completions".to_string()
}

/// 将目标协议的 SSE event data 解析为统一的 ChatStreamEvent。
/// SSE 响应格式由 wire protocol（endpoint 协议）决定。
pub fn parse_sse(data: &Value, wire_protocol: &Protocol) -> Option<ChatStreamEvent> {
    match wire_protocol {
        Protocol::Anthropic => super::anthropic::parse_anthropic_sse(data),
        Protocol::Gemini => super::gemini::parse_gemini_sse(data),
        // 所有 OpenAI 系列共用 OpenAI SSE 解析
        _ => super::openai::parse_openai_sse(data),
    }
}

/// 将入站请求按源协议解析为内部 ChatRequest（支持所有 AI 请求协议）
pub fn parse_incoming_request(source_protocol: &str, body: &Value) -> Option<ChatRequest> {
    match source_protocol {
        "openai" => super::openai::from_openai(body),
        "openai_responses" => super::openai_responses::from_responses(body),
        "openai_completions" => super::openai_completions::from_completions(body),
        "gemini" => super::gemini::from_gemini(body),
        // Anthropic / 默认: ChatRequest 结构已兼容 Anthropic 格式，直接反序列化
        _ => serde_json::from_value(body.clone()).ok(),
    }
}

/// 将统一的 ChatStreamEvent 按客户端协议格式化为 SSE
pub fn to_client_sse(event: &ChatStreamEvent, source_protocol: &str, model: &str) -> Option<String> {
    match source_protocol {
        "openai" | "openai_responses" | "openai_completions" => super::openai::to_openai_sse(event, model),
        "gemini" => super::gemini::to_gemini_sse(event, model),
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
