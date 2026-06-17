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

/// 同协议透传时的出站 API 路径：与 `convert_request` 对各 wire 协议产出的 path 保持一致，
/// 但**不转换 body**（透传保留原始请求体结构）。
///
/// - `wire_protocol`: 出站 wire 协议（= 入站协议，因为透传仅在精确同协议时触发）
/// - `model`: 用于 Gemini path 中的模型段（其余协议忽略）
/// - `platform_protocol`: 平台类型，决定 OpenAI-compatible 平台的 chat path 后缀
pub fn passthrough_api_path(wire_protocol: &Protocol, model: &str, platform_protocol: &Protocol) -> String {
    match wire_protocol {
        Protocol::Anthropic => "/v1/messages".to_string(),
        Protocol::Gemini => format!("/v1beta/models/{}:streamGenerateContent", model),
        Protocol::OpenAIResponses => "/v1/responses".to_string(),
        Protocol::OpenAICompletions => "/v1/completions".to_string(),
        _ => provider_api_path(platform_protocol),
    }
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

/// 将入站请求按源协议解析为内部 ChatRequest（支持所有 AI 请求协议）。
///
/// 返回 `Err(String)` 携带解析失败原因(serde 错误细节等)，供上层记录到日志便于诊断。
pub fn parse_incoming_request(source_protocol: &str, body: &Value) -> Result<ChatRequest, String> {
    match source_protocol {
        "openai" => super::openai::from_openai(body).ok_or_else(|| "openai from_openai returned None".to_string()),
        "openai_responses" => super::openai_responses::from_responses(body).ok_or_else(|| "openai_responses from_responses returned None".to_string()),
        "openai_completions" => super::openai_completions::from_completions(body).ok_or_else(|| "openai_completions from_completions returned None".to_string()),
        "gemini" => super::gemini::from_gemini(body).ok_or_else(|| "gemini from_gemini returned None".to_string()),
        // Anthropic / 默认: ChatRequest 结构已兼容 Anthropic 格式，直接反序列化;
        // ContentBlock 已对未知类型(thinking/image/…)降级 Unknown, 失败时返回 serde 错误细节供诊断。
        _ => serde_json::from_value(body.clone()).map_err(|e| e.to_string()),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gateway::adapter::types::{ContentBlock, MessageContent};

    // ── 透传 path 与 convert_request 各 wire 协议产出一致（不转 body）──
    #[test]
    fn passthrough_path_matches_convert_request() {
        let wires = [
            Protocol::Anthropic,
            Protocol::Gemini,
            Protocol::OpenAIResponses,
            Protocol::OpenAICompletions,
            Protocol::OpenAI,
        ];
        let req = ChatRequest {
            model: "gpt-4o".to_string(),
            messages: vec![],
            system: None,
            max_tokens: None,
            temperature: None,
            top_p: None,
            stream: None,
            tools: None,
            tool_choice: None,
            extra: None,
        };
        for wire in wires {
            let (_body, conv_path) = convert_request(&req, &wire, &Protocol::OpenAI);
            let pass_path = passthrough_api_path(&wire, &req.model, &Protocol::OpenAI);
            assert_eq!(conv_path, pass_path, "path mismatch for {:?}", wire);
        }
    }

    // ── Gemini path 含模型名 ──
    #[test]
    fn passthrough_path_gemini_embeds_model() {
        let path = passthrough_api_path(&Protocol::Gemini, "gemini-2.0-flash", &Protocol::Gemini);
        assert_eq!(path, "/v1beta/models/gemini-2.0-flash:streamGenerateContent");
    }

    // ── Anthropic 入站含未知 block(thinking/image) 不再 400，降级 Unknown ──
    #[test]
    fn anthropic_parse_tolerates_unknown_blocks() {
        let body = serde_json::json!({
            "model": "claude-opus-4-8",
            "messages": [{
                "role": "user",
                "content": [
                    { "type": "thinking", "thinking": "...", "signature": "sig" },
                    { "type": "text", "text": "hi" }
                ]
            }]
        });
        let req = parse_incoming_request("anthropic", &body).expect("anthropic parse should succeed");
        assert_eq!(req.model, "claude-opus-4-8");
        let blocks = match &req.messages[0].content {
            MessageContent::Blocks(b) => b,
            _ => panic!("expected blocks"),
        };
        assert!(blocks.iter().any(|b| matches!(b, ContentBlock::Unknown(_))), "thinking 应降级 Unknown");
        assert!(blocks.iter().any(|b| matches!(b, ContentBlock::Text { .. })), "text block 应保留");
    }

    // ── tool_result.content 为 array(Anthropic 富格式) 容错抽取文本 ──
    #[test]
    fn anthropic_parse_tool_result_array_content() {
        let body = serde_json::json!({
            "model": "claude-opus-4-8",
            "messages": [{
                "role": "user",
                "content": [
                    { "type": "tool_result", "tool_use_id": "t1", "content": [
                        { "type": "text", "text": "result chunk" }
                    ]}
                ]
            }]
        });
        let req = parse_incoming_request("anthropic", &body).expect("tool_result array content parse");
        match &req.messages[0].content {
            MessageContent::Blocks(b) => match &b[0] {
                ContentBlock::ToolResult { tool_use_id, content } => {
                    assert_eq!(tool_use_id, "t1");
                    assert_eq!(content, "result chunk");
                }
                _ => panic!("expected ToolResult"),
            },
            _ => panic!("expected blocks"),
        }
    }

    // ── 纯文本 anthropic 请求回归不受影响 ──
    #[test]
    fn anthropic_parse_plain_text_unchanged() {
        let body = serde_json::json!({
            "model": "claude-opus-4-8",
            "messages": [{ "role": "user", "content": "hello" }]
        });
        let req = parse_incoming_request("anthropic", &body).expect("plain parse");
        assert_eq!(req.model, "claude-opus-4-8");
        assert!(matches!(req.messages[0].content, MessageContent::Text(_)));
    }

    // ── 入站 anthropic 工具缺 input_schema(如服务端工具 web_search) 不再 400, 默认空对象 ──
    #[test]
    fn anthropic_parse_tool_missing_input_schema() {
        let body = serde_json::json!({
            "model": "claude-opus-4-8",
            "messages": [{ "role": "user", "content": "search it" }],
            "tools": [{ "name": "web_search" }]
        });
        let req = parse_incoming_request("anthropic", &body)
            .expect("tool missing input_schema should still parse");
        let tools = req.tools.as_ref().expect("tools present");
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].name, "web_search");
        assert_eq!(tools[0].input_schema, serde_json::json!({}), "缺失时默认空对象, 非 null");
    }
}
