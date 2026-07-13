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

// ── openai 入站解析 ──
#[test]
fn openai_parse_incoming_request() {
    let body = serde_json::json!({
        "model": "gpt-4o",
        "messages": [
            { "role": "user", "content": "hello", "index": 0 }
        ]
    });
    let req = parse_incoming_request("openai", &body).expect("openai parse");
    assert_eq!(req.model, "gpt-4o");
}

// ── openai_responses 入站解析 ──
#[test]
fn openai_responses_parse_incoming_request() {
    let body = serde_json::json!({
        "model": "gpt-4o",
        "input": [
            { "role": "user", "content": "hello" }
        ]
    });
    let req = parse_incoming_request("openai_responses", &body).expect("openai_responses parse");
    assert_eq!(req.model, "gpt-4o");
}

// ── openai_completions 入站解析 ──
#[test]
fn openai_completions_parse_incoming_request() {
    let body = serde_json::json!({
        "model": "gpt-3.5-turbo-instruct",
        "prompt": "Hello world"
    });
    let req = parse_incoming_request("openai_completions", &body).expect("openai_completions parse");
    assert_eq!(req.model, "gpt-3.5-turbo-instruct");
}

// ── gemini 入站解析 ──
#[test]
fn gemini_parse_incoming_request() {
    let body = serde_json::json!({
        "model": "gemini-pro",
        "contents": [{ "role": "user", "parts": [{"text": "hi"}] }]
    });
    // May return None for incomplete format — just ensure no panic
    let _ = parse_incoming_request("gemini", &body);
}

// ── 无效 anthropic 请求返回 Err ──
#[test]
fn anthropic_parse_invalid_returns_err() {
    let body = serde_json::json!({"invalid": true});
    // Should error (missing required 'model' field)
    let result = parse_incoming_request("anthropic", &body);
    assert!(result.is_err(), "invalid anthropic body should return Err");
}

// ── convert_request 各 wire 协议 path 校验 ──
#[test]
fn convert_request_anthropic_path() {
    let req = ChatRequest {
        model: "claude-opus-4".to_string(),
        messages: vec![],
        system: None, max_tokens: None, temperature: None, top_p: None,
        stream: None, tools: None, tool_choice: None, extra: None,
    };
    let (_body, path) = convert_request(&req, &Protocol::Anthropic, &Protocol::Anthropic);
    assert_eq!(path, "/v1/messages");
}

#[test]
fn convert_request_openai_completions_path() {
    let req = ChatRequest {
        model: "gpt-3.5-turbo-instruct".to_string(),
        messages: vec![],
        system: None, max_tokens: None, temperature: None, top_p: None,
        stream: None, tools: None, tool_choice: None, extra: None,
    };
    let (_body, path) = convert_request(&req, &Protocol::OpenAICompletions, &Protocol::OpenAI);
    assert_eq!(path, "/v1/completions");
}

#[test]
fn convert_request_openai_responses_path() {
    let req = ChatRequest {
        model: "gpt-4o".to_string(),
        messages: vec![],
        system: None, max_tokens: None, temperature: None, top_p: None,
        stream: None, tools: None, tool_choice: None, extra: None,
    };
    let (_body, path) = convert_request(&req, &Protocol::OpenAIResponses, &Protocol::OpenAI);
    assert_eq!(path, "/v1/responses");
}

// ── CPA 4 协议 platform_type 作为 wire 回退时路径正确（design.md: grok→/v1/responses, 其余→gemini 路径）──
#[test]
fn convert_request_cpa_protocol_paths() {
    let req = ChatRequest {
        model: "grok-4".to_string(),
        messages: vec![],
        system: None, max_tokens: None, temperature: None, top_p: None,
        stream: None, tools: None, tool_choice: None, extra: None,
    };
    // cpa-grok → /v1/responses（OpenAI Responses 同语义）
    let (_body, path) = convert_request(&req, &Protocol::CpaGrok, &Protocol::CpaGrok);
    assert_eq!(path, "/v1/responses");

    // cpa-aistudio / cpa-antigravity / cpa-vertex → gemini path 占位（仅存配置）
    let req_g = ChatRequest { model: "gemini-2.5-pro".to_string(), ..req.clone() };
    for proto in [Protocol::CpaAistudio, Protocol::CpaAntigravity, Protocol::CpaVertex] {
        let (_body, path) = convert_request(&req_g, &proto, &proto);
        assert_eq!(path, "/v1beta/models/gemini-2.5-pro:streamGenerateContent",
            "cpa gemini-family path placeholder for {:?}", proto);
    }
}

// ── 透传 path 与 convert_request 对 CPA 4 协议产出一致（不转 body）──
#[test]
fn passthrough_path_matches_convert_request_cpa() {
    let req = ChatRequest {
        model: "grok-4".to_string(),
        messages: vec![],
        system: None, max_tokens: None, temperature: None, top_p: None,
        stream: None, tools: None, tool_choice: None, extra: None,
    };
    for proto in [Protocol::CpaGrok, Protocol::CpaAistudio, Protocol::CpaAntigravity, Protocol::CpaVertex] {
        let (_body, conv_path) = convert_request(&req, &proto, &proto);
        let pass_path = passthrough_api_path(&proto, &req.model, &proto);
        assert_eq!(conv_path, pass_path, "cpa path mismatch for {:?}", proto);
    }
}
