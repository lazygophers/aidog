use crate::gateway::models::Protocol;
use serde_json::Value;

use super::types::*;

/// 将内部 ChatRequest 转为目标协议的 JSON body
pub fn convert_request(req: &ChatRequest, protocol: &Protocol) -> (Value, String) {
    match protocol {
        Protocol::Anthropic => {
            let anthropic_req = super::anthropic::to_anthropic(req);
            let json = serde_json::to_value(&anthropic_req).unwrap();
            (json, "/v1/messages".to_string())
        }
        Protocol::OpenAI => {
            let openai_req = super::openai::to_openai(req);
            let json = serde_json::to_value(&openai_req).unwrap();
            (json, "/v1/chat/completions".to_string())
        }
        Protocol::Glm => {
            let glm_req = super::glm::to_glm(req);
            let json = serde_json::to_value(&glm_req).unwrap();
            (json, "/api/paas/v4/chat/completions".to_string())
        }
        Protocol::Kimi => {
            let kimi_req = super::kimi::to_kimi(req);
            let json = serde_json::to_value(&kimi_req).unwrap();
            (json, "/v1/chat/completions".to_string())
        }
        Protocol::MiniMax => {
            let minimax_req = super::minimax::to_minimax(req);
            let json = serde_json::to_value(&minimax_req).unwrap();
            (json, "/v1/text/chatcompletion_v2".to_string())
        }
        Protocol::Codex => {
            let codex_req = super::codex::to_codex(req);
            let json = serde_json::to_value(&codex_req).unwrap();
            (json, "/v1/chat/completions".to_string())
        }
        Protocol::Bailian => {
            let bailian_req = super::bailian::to_bailian(req);
            let json = serde_json::to_value(&bailian_req).unwrap();
            (json, "/compatible-mode/v1/chat/completions".to_string())
        }
        Protocol::Gemini => {
            let gemini_req = super::gemini::to_gemini(req);
            let json = serde_json::to_value(&gemini_req).unwrap();
            let path = format!("/v1beta/models/{}:streamGenerateContent", req.model);
            (json, path)
        }
    }
}

/// 将目标协议的 SSE event data 解析为统一的 ChatStreamEvent
pub fn parse_sse(data: &Value, protocol: &Protocol) -> Option<ChatStreamEvent> {
    match protocol {
        Protocol::Anthropic => super::anthropic::parse_anthropic_sse(data),
        Protocol::OpenAI => super::openai::parse_openai_sse(data),
        Protocol::Glm => super::glm::parse_glm_sse(data),
        Protocol::Kimi => super::kimi::parse_kimi_sse(data),
        Protocol::MiniMax => super::minimax::parse_minimax_sse(data),
        Protocol::Codex => super::codex::parse_codex_sse(data),
        Protocol::Bailian => super::bailian::parse_bailian_sse(data),
        Protocol::Gemini => super::gemini::parse_gemini_sse(data),
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

/// 将入站请求按源协议解析为内部 ChatRequest
pub fn parse_incoming_request(source_protocol: &str, body: &Value) -> Option<ChatRequest> {
    match source_protocol {
        "openai" => super::openai::from_openai(body),
        // Anthropic / 默认: ChatRequest 结构已兼容 Anthropic 格式，直接反序列化
        _ => serde_json::from_value(body.clone()).ok(),
    }
}

/// 将统一的 ChatStreamEvent 按客户端协议格式化为 SSE
pub fn to_client_sse(event: &ChatStreamEvent, source_protocol: &str, model: &str) -> Option<String> {
    match source_protocol {
        "openai" => super::openai::to_openai_sse(event, model),
        // 默认 Anthropic 格式
        _ => to_anthropic_sse(event),
    }
}
