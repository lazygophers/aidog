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
}

/// 非流式上游响应 → 客户端协议响应体转换。
///
/// - `wire_protocol`: 上游响应体格式（endpoint 协议）。
/// - `client_protocol`: 客户端期望协议（入站 source_protocol）。
///
/// 返回 `Some(Value)` 表示已转换为客户端格式；`None` 表示无需 / 无法转换
/// （调用方应原样透传上游 body，保持既有行为）。
///
/// 当前覆盖 openai(chat completion) → anthropic(messages) 真转换（修复 tool_calls/content
/// 并存时客户端拿到非 anthropic 结构致 "empty or malformed" 的 bug）；其余跨协议组合暂回退透传。
pub fn convert_response(
    body: &Value,
    wire_protocol: &Protocol,
    client_protocol: &str,
    model: &str,
) -> Option<Value> {
    // 同语义协议无需转换：客户端要 openai 且上游也是 openai 系列 → 透传。
    let wire_is_openai = matches!(
        wire_protocol,
        Protocol::OpenAI | Protocol::OpenAICompletions
    ) || !matches!(
        wire_protocol,
        Protocol::Anthropic | Protocol::Gemini | Protocol::OpenAIResponses
    );
    let client_is_anthropic = !matches!(client_protocol, "openai" | "openai_responses" | "openai_completions" | "gemini");

    // 修复目标场景：上游 openai chat completion → 客户端 anthropic messages。
    if wire_is_openai && client_is_anthropic {
        let mut parsed = super::openai::parse_openai_response(body, model)?;
        // 客户端响应 model 字段统一回填为客户端请求的模型名（与流式 model_for_response 一致），
        // 避免暴露上游真实模型名 / 触发 CC 模型校验歧义。
        parsed.model = model.to_string();
        return Some(render_anthropic_response(&parsed));
    }

    // 其余组合（同协议透传 / 暂未覆盖的跨协议）：返回 None，调用方透传上游原文。
    None
}

/// 渲染归一响应为 Anthropic Messages 非流式响应体。
fn render_anthropic_response(r: &NonStreamResponse) -> Value {
    let mut content: Vec<Value> = Vec::new();
    if let Some(text) = &r.text {
        if !text.is_empty() {
            content.push(serde_json::json!({ "type": "text", "text": text }));
        }
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

    // ── 非流式 openai→anthropic 响应转换：content + tool_calls 并存(finish_reason=tool_calls) ──
    // 复现 request 1ef25294 场景：上游 message 同时含 content / reasoning_content / tool_calls。
    #[test]
    fn convert_response_openai_to_anthropic_content_and_tools() {
        let upstream = serde_json::json!({
            "id": "chatcmpl-x",
            "model": "glm-4.6",
            "choices": [{
                "index": 0,
                "finish_reason": "tool_calls",
                "message": {
                    "role": "assistant",
                    "content": "Trellis SessionStart...",
                    "reasoning_content": "用户触发...(GLM思维链)",
                    "tool_calls": [{
                        "id": "call_-7518760127650854872",
                        "index": 2,
                        "type": "function",
                        "function": { "name": "read_file", "arguments": "{\"path\":\"/a\"}" }
                    }]
                }
            }],
            "usage": { "prompt_tokens": 100, "completion_tokens": 1002,
                       "prompt_tokens_details": { "cached_tokens": 40 } }
        });
        let out = convert_response(&upstream, &Protocol::OpenAI, "anthropic", "claude-opus-4")
            .expect("openai→anthropic should convert");
        assert_eq!(out["type"], "message");
        assert_eq!(out["role"], "assistant");
        assert_eq!(out["model"], "claude-opus-4", "model 回填为客户端请求模型");
        assert_eq!(out["stop_reason"], "tool_use", "tool_calls→tool_use");
        let content = out["content"].as_array().unwrap();
        // 第一块 text，第二块 tool_use（顺序：text 在前）
        assert_eq!(content[0]["type"], "text");
        assert_eq!(content[0]["text"], "Trellis SessionStart...");
        assert_eq!(content[1]["type"], "tool_use");
        assert_eq!(content[1]["id"], "call_-7518760127650854872");
        assert_eq!(content[1]["name"], "read_file");
        assert_eq!(content[1]["input"]["path"], "/a", "arguments JSON 解析为 input 对象");
        // usage 映射
        assert_eq!(out["usage"]["input_tokens"], 100);
        assert_eq!(out["usage"]["output_tokens"], 1002);
        assert_eq!(out["usage"]["cache_read_input_tokens"], 40);
    }

    // ── 纯 tool_calls 无 content：content 数组仅含 tool_use（合法非空） ──
    #[test]
    fn convert_response_openai_to_anthropic_tool_only() {
        let upstream = serde_json::json!({
            "id": "c1", "model": "glm-4.6",
            "choices": [{ "index": 0, "finish_reason": "tool_calls", "message": {
                "role": "assistant", "content": null,
                "tool_calls": [{ "id": "t1", "type": "function",
                    "function": { "name": "ls", "arguments": "{}" } }]
            }}],
            "usage": { "prompt_tokens": 5, "completion_tokens": 7 }
        });
        let out = convert_response(&upstream, &Protocol::OpenAI, "anthropic", "claude").unwrap();
        let content = out["content"].as_array().unwrap();
        assert_eq!(content.len(), 1, "无 text 时只含 tool_use");
        assert_eq!(content[0]["type"], "tool_use");
        assert_eq!(content[0]["input"], serde_json::json!({}));
        assert_eq!(out["stop_reason"], "tool_use");
    }

    // ── 纯文本（finish_reason=stop）→ end_turn + 单 text 块 ──
    #[test]
    fn convert_response_openai_to_anthropic_text_only() {
        let upstream = serde_json::json!({
            "id": "c2", "model": "glm-4.6",
            "choices": [{ "index": 0, "finish_reason": "stop", "message": {
                "role": "assistant", "content": "hello world" } }],
            "usage": { "prompt_tokens": 3, "completion_tokens": 2 }
        });
        let out = convert_response(&upstream, &Protocol::OpenAI, "anthropic", "claude").unwrap();
        let content = out["content"].as_array().unwrap();
        assert_eq!(content.len(), 1);
        assert_eq!(content[0]["type"], "text");
        assert_eq!(content[0]["text"], "hello world");
        assert_eq!(out["stop_reason"], "end_turn", "stop→end_turn");
    }

    // ── finish_reason=length → max_tokens ──
    #[test]
    fn convert_response_length_maps_max_tokens() {
        let upstream = serde_json::json!({
            "id": "c3", "model": "m",
            "choices": [{ "index": 0, "finish_reason": "length", "message": {
                "role": "assistant", "content": "truncated" } }]
        });
        let out = convert_response(&upstream, &Protocol::OpenAI, "anthropic", "claude").unwrap();
        assert_eq!(out["stop_reason"], "max_tokens");
    }

    // ── reasoning_content 存在不致崩 / 不产空（已隐含在上面，单测显式确认无 content+无 tool 时兜底空 text） ──
    #[test]
    fn convert_response_empty_message_yields_nonempty_content() {
        let upstream = serde_json::json!({
            "id": "c4", "model": "m",
            "choices": [{ "index": 0, "finish_reason": "stop", "message": {
                "role": "assistant", "reasoning_content": "只有思维链" } }]
        });
        let out = convert_response(&upstream, &Protocol::OpenAI, "anthropic", "claude").unwrap();
        let content = out["content"].as_array().unwrap();
        assert_eq!(content.len(), 1, "兜底空 text 块保证 content 非空");
        assert_eq!(content[0]["type"], "text");
        assert_eq!(content[0]["text"], "");
    }

    // ── 同协议（client=openai, wire=openai）→ None（透传，不转换） ──
    #[test]
    fn convert_response_same_proto_returns_none() {
        let upstream = serde_json::json!({ "choices": [] });
        assert!(convert_response(&upstream, &Protocol::OpenAI, "openai", "m").is_none());
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
