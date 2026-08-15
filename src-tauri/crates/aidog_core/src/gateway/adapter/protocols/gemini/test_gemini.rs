use serde_json::json;

use super::super::gemini::*;

fn req(messages: Vec<Message>) -> ChatRequest {
    ChatRequest {
        model: "gemini".into(),
        messages,
        system: None,
        max_tokens: None,
        temperature: None,
        top_p: None,
        stream: None,
        tools: None,
        tool_choice: None,
        extra: None,
    }
}

#[test]
fn to_gemini_system_text_and_blocks() {
    let mut r = req(vec![]);
    r.system = Some(SystemContent::Text("sys".into()));
    let g = to_gemini(&r);
    assert_eq!(g.system_instruction.as_ref().unwrap().parts[0].text.as_deref(), Some("sys"));

    let mut r2 = req(vec![]);
    r2.system = Some(SystemContent::Blocks(vec![json!({"text": "a"}), json!({"text": "b"})]));
    let g2 = to_gemini(&r2);
    assert_eq!(g2.system_instruction.unwrap().parts[0].text.as_deref(), Some("a\nb"));
}

#[test]
fn to_gemini_roles_and_blocks() {
    let r = req(vec![
        Message { role: Role::User, content: MessageContent::Text("hi".into()) },
        Message {
            role: Role::Assistant,
            content: MessageContent::Blocks(vec![
                ContentBlock::Text { text: "t".into() },
                ContentBlock::ToolUse { id: "i".into(), name: "f".into(), input: json!({"a": 1}) },
            ]),
        },
        Message {
            role: Role::Tool,
            content: MessageContent::Blocks(vec![
                ContentBlock::ToolResult { tool_use_id: "f".into(), content: "res".into() },
                ContentBlock::Unknown(json!({"type": "thinking", "text": "th"})),
            ]),
        },
    ]);
    let g = to_gemini(&r);
    assert_eq!(g.contents.len(), 3);
    assert_eq!(g.contents[0].role, "user");
    assert_eq!(g.contents[1].role, "model");
    assert!(g.contents[1].parts[1].function_call.is_some());
    assert!(g.contents[2].parts[0].function_response.is_some());
    assert_eq!(g.contents[2].parts[1].text.as_deref(), Some("th"));
}

#[test]
fn to_gemini_tools_and_gen_config() {
    let mut r = req(vec![]);
    r.tools = Some(vec![Tool { name: "f".into(), description: Some("d".into()), input_schema: json!({}) }]);
    r.max_tokens = Some(100);
    r.temperature = Some(0.5);
    let g = to_gemini(&r);
    assert!(g.tools.is_some());
    let gc = g.generation_config.unwrap();
    assert_eq!(gc.max_output_tokens, Some(100));
}

#[test]
fn to_gemini_no_gen_config_when_all_none() {
    let g = to_gemini(&req(vec![]));
    assert!(g.generation_config.is_none());
}

#[test]
fn from_gemini_basic() {
    let body = json!({
        "contents": [
            {"role": "user", "parts": [{"text": "hello"}]},
            {"role": "model", "parts": [{"text": "a"}, {"text": "b"}]}
        ],
        "systemInstruction": {"parts": [{"text": "sys"}]},
        "generationConfig": {"maxOutputTokens": 50, "temperature": 0.7, "topP": 0.9}
    });
    let r = from_gemini(&body).expect("parsed");
    assert_eq!(r.messages.len(), 2);
    matches!(r.messages[1].role, Role::Assistant);
    assert_eq!(r.max_tokens, Some(50));
    assert!(r.system.is_some());
}

#[test]
fn from_gemini_empty_parts_text() {
    let body = json!({"contents": [{"role": "user", "parts": []}]});
    let r = from_gemini(&body).expect("parsed");
    matches!(r.messages[0].content, MessageContent::Text(_));
}

#[test]
fn from_gemini_missing_contents_none() {
    assert!(from_gemini(&json!({})).is_none());
}

#[test]
fn parse_gemini_sse_text() {
    let d = json!({"candidates": [{"content": {"parts": [{"text": "hi"}]}}]});
    match parse_gemini_sse(&d) {
        Some(ChatStreamEvent::Delta { text }) => assert_eq!(text, "hi"),
        _ => panic!("delta"),
    }
}

#[test]
fn parse_gemini_sse_function_call() {
    let d = json!({"candidates": [{"content": {"parts": [{"functionCall": {"name": "f", "args": {"a": 1}}}]}}]});
    match parse_gemini_sse(&d) {
        Some(ChatStreamEvent::ToolDelta { name, input, .. }) => {
            assert_eq!(name.as_deref(), Some("f"));
            assert!(input.unwrap().contains("a"));
        }
        _ => panic!("tool delta"),
    }
}

#[test]
fn parse_gemini_sse_finish() {
    for r in ["STOP", "MAX_TOKENS"] {
        let d = json!({"candidates": [{"content": {"parts": [{}]}, "finishReason": r}]});
        match parse_gemini_sse(&d) {
            Some(ChatStreamEvent::Stop { finish_reason }) => {
                assert_eq!(finish_reason.as_deref(), Some(r.to_lowercase().as_str()));
            }
            _ => panic!("stop for {r}"),
        }
    }
}

#[test]
fn parse_gemini_sse_none() {
    assert!(parse_gemini_sse(&json!({})).is_none());
    let d = json!({"candidates": [{"content": {"parts": [{}]}, "finishReason": "OTHER"}]});
    assert!(parse_gemini_sse(&d).is_none());
}

#[test]
fn parse_gemini_sse_thought_part() {
    let d = json!({"candidates": [{"content": {"parts": [{"thought": true, "text": "thinking..."}]}}]});
    match parse_gemini_sse(&d) {
        Some(ChatStreamEvent::ReasoningDelta { text }) => assert_eq!(text, "thinking..."),
        _ => panic!("expected reasoning_delta"),
    }
}

#[test]
fn to_gemini_sse_reasoning_delta() {
    let s = to_gemini_sse(&ChatStreamEvent::ReasoningDelta { text: "thought".into() }, "m").unwrap();
    assert!(s.contains("\"thought\":true"));
    assert!(s.contains("thought"));
}

#[test]
fn to_gemini_sse_wire_frame_has_data_prefix_and_terminator() {
    // 与 to_openai_sse / to_anthropic_sse 同一约定：返回完整 wire 帧（`data: ` 前缀 + `\n\n` 终止）。
    let s = to_gemini_sse(&ChatStreamEvent::Delta { text: "x".into() }, "m").unwrap();
    assert!(s.starts_with("data: "), "gemini SSE 帧须带 data: 前缀");
    assert!(s.ends_with("\n\n"), "gemini SSE 帧须以空行终止");
}

#[test]
fn to_gemini_sse_variants() {
    assert!(to_gemini_sse(&ChatStreamEvent::Start { id: "i".into(), model: "m".into() }, "m").is_none());
    assert!(to_gemini_sse(&ChatStreamEvent::Delta { text: "x".into() }, "m").unwrap().contains("x"));
    assert!(to_gemini_sse(&ChatStreamEvent::Usage { usage: Usage { prompt_tokens: None, completion_tokens: None, total_tokens: None } }, "m").is_none());

    for (fr, expect) in [(Some("end_turn"), "STOP"), (Some("max_tokens"), "MAX_TOKENS"), (None, "STOP")] {
        let s = to_gemini_sse(&ChatStreamEvent::Stop { finish_reason: fr.map(String::from) }, "m").unwrap();
        assert!(s.contains(expect));
    }

    let td = ChatStreamEvent::ToolDelta { index: 0, id: Some("f".into()), name: Some("f".into()), input: Some("{\"a\":1}".into()) };
    assert!(to_gemini_sse(&td, "m").unwrap().contains("functionCall"));
    // bad input → defaults to {}
    let td2 = ChatStreamEvent::ToolDelta { index: 0, id: None, name: Some("f".into()), input: Some("bad".into()) };
    assert!(to_gemini_sse(&td2, "m").unwrap().contains("functionCall"));
}

// ── parse_gemini_response 测试 ──
#[test]
fn parse_gemini_response_with_thinking() {
    use super::super::gemini::parse_gemini_response;

    let body = json!({
        "id": "gemini_123",
        "model": "gemini-2.5-flash",
        "candidates": [{
            "content": {
                "parts": [
                    {
                        "thought": true,
                        "text": "Let me think about this...\n\nAnalysis complete."
                    },
                    {
                        "text": "Here is my answer."
                    }
                ],
                "role": "model"
            },
            "finishReason": "STOP",
            "index": 0
        }],
        "usageMetadata": {
            "promptTokenCount": 20,
            "candidatesTokenCount": 30,
            "totalTokenCount": 50
        }
    });

    let parsed = parse_gemini_response(&body, "gemini-2.5-flash").expect("should parse");
    assert_eq!(parsed.id, "gemini_123");
    assert_eq!(parsed.model, "gemini-2.5-flash");
    assert_eq!(parsed.text.as_deref(), Some("Here is my answer."));
    assert_eq!(parsed.reasoning.as_deref(), Some("Let me think about this...\n\nAnalysis complete."));
    assert_eq!(parsed.stop_reason, "end_turn");
    assert_eq!(parsed.input_tokens, 20);
    assert_eq!(parsed.output_tokens, 30);
    assert!(parsed.tool_uses.is_empty());
}

#[test]
fn parse_gemini_response_with_function_call() {
    use super::super::gemini::parse_gemini_response;

    let body = json!({
        "id": "gemini_456",
        "model": "gemini-2.0-flash",
        "candidates": [{
            "content": {
                "parts": [
                    {"text": "I'll call a function."},
                    {
                        "functionCall": {
                            "name": "calculator",
                            "args": {"operation": "add", "x": 1, "y": 2}
                        }
                    }
                ],
                "role": "model"
            },
            "finishReason": "STOP",
            "index": 0
        }],
        "usageMetadata": {
            "promptTokenCount": 15,
            "candidatesTokenCount": 25,
            "totalTokenCount": 40,
            "cachedContentTokenCount": 5
        }
    });

    let parsed = parse_gemini_response(&body, "gemini-2.0-flash").expect("should parse");
    assert_eq!(parsed.text.as_deref(), Some("I'll call a function."));
    assert_eq!(parsed.tool_uses.len(), 1);
    assert_eq!(parsed.tool_uses[0].1, "calculator");
    assert_eq!(parsed.tool_uses[0].2, serde_json::json!({"operation": "add", "x": 1, "y": 2}));
    assert_eq!(parsed.stop_reason, "end_turn");
    assert_eq!(parsed.cache_read_tokens, 5);
}

#[test]
fn parse_gemini_response_max_tokens() {
    use super::super::gemini::parse_gemini_response;

    let body = json!({
        "id": "gemini_789",
        "model": "gemini-1.5-pro",
        "candidates": [{
            "content": {
                "parts": [{"text": "Response cut off"}],
                "role": "model"
            },
            "finishReason": "MAX_TOKENS",
            "index": 0
        }],
        "usageMetadata": {
            "promptTokenCount": 100,
            "candidatesTokenCount": 150,
            "totalTokenCount": 250
        }
    });

    let parsed = parse_gemini_response(&body, "gemini-1.5-pro").expect("should parse");
    assert_eq!(parsed.stop_reason, "max_tokens"); // MAX_TOKENS → max_tokens
    assert_eq!(parsed.text.as_deref(), Some("Response cut off"));
}

#[test]
fn parse_gemini_response_minimal() {
    use super::super::gemini::parse_gemini_response;

    // 最简情况：只有普通 text，无 thinking
    let body = json!({
        "candidates": [{
            "content": {
                "parts": [{"text": "Simple response"}],
                "role": "model"
            },
            "finishReason": "STOP"
        }],
        "usageMetadata": {
            "promptTokenCount": 10,
            "candidatesTokenCount": 5,
            "totalTokenCount": 15
        }
    });

    let parsed = parse_gemini_response(&body, "gemini-pro").expect("should parse");
    assert_eq!(parsed.text.as_deref(), Some("Simple response"));
    assert!(parsed.reasoning.is_none());
    assert!(parsed.tool_uses.is_empty());
}

// ── render_gemini_response 测试 ──
#[test]
fn render_gemini_text_only() {
    use crate::gateway::adapter::converter::NonStreamResponse;
    use super::render_gemini_response;

    let r = NonStreamResponse {
        id: "test".to_string(),
        model: "gemini-pro".to_string(),
        text: Some("Hello world".to_string()),
        tool_uses: vec![],
        stop_reason: "end_turn".to_string(),
        input_tokens: 10,
        output_tokens: 5,
        cache_read_tokens: 0,
        reasoning: None,
    };

    let out = render_gemini_response(&r).unwrap();
    assert_eq!(out["candidates"][0]["content"]["role"], "model");
    assert_eq!(out["candidates"][0]["finishReason"], "STOP"); // end_turn → STOP
    assert_eq!(out["candidates"][0]["content"]["parts"].as_array().unwrap().len(), 1);
    assert_eq!(out["candidates"][0]["content"]["parts"][0]["text"], "Hello world");
    assert_eq!(out["modelVersion"], "gemini-pro");
}

#[test]
fn render_gemini_with_reasoning() {
    use crate::gateway::adapter::converter::NonStreamResponse;
    use super::render_gemini_response;

    let r = NonStreamResponse {
        id: "test".to_string(),
        model: "gemini-pro".to_string(),
        text: Some("Answer".to_string()),
        tool_uses: vec![],
        stop_reason: "end_turn".to_string(),
        input_tokens: 20,
        output_tokens: 10,
        cache_read_tokens: 0,
        reasoning: Some("Let me think...".to_string()),
    };

    let out = render_gemini_response(&r).unwrap();
    let parts = out["candidates"][0]["content"]["parts"].as_array().unwrap();
    assert_eq!(parts.len(), 2);
    assert_eq!(parts[0]["text"], "Answer");
    assert_eq!(parts[1]["thought"], true);
    assert_eq!(parts[1]["text"], "Let me think...");
}

#[test]
fn render_gemini_with_function_call() {
    use crate::gateway::adapter::converter::NonStreamResponse;
    use super::render_gemini_response;

    let r = NonStreamResponse {
        id: "test".to_string(),
        model: "gemini-pro".to_string(),
        text: Some("Calling function".to_string()),
        tool_uses: vec![
            ("tool_0".to_string(), "read_file".to_string(), json!({"path": "/tmp"})),
        ],
        stop_reason: "tool_use".to_string(),
        input_tokens: 15,
        output_tokens: 8,
        cache_read_tokens: 0,
        reasoning: None,
    };

    let out = render_gemini_response(&r).unwrap();
    let parts = out["candidates"][0]["content"]["parts"].as_array().unwrap();
    assert_eq!(parts.len(), 2);
    assert_eq!(parts[0]["text"], "Calling function");
    assert_eq!(parts[1]["functionCall"]["name"], "read_file");
    assert_eq!(parts[1]["functionCall"]["args"]["path"], "/tmp");
}

#[test]
fn render_gemini_max_tokens_maps_max_tokens() {
    use crate::gateway::adapter::converter::NonStreamResponse;
    use super::render_gemini_response;

    let r = NonStreamResponse {
        id: "test".to_string(),
        model: "gemini-pro".to_string(),
        text: Some("Truncated".to_string()),
        tool_uses: vec![],
        stop_reason: "max_tokens".to_string(),
        input_tokens: 5,
        output_tokens: 3,
        cache_read_tokens: 0,
        reasoning: None,
    };

    let out = render_gemini_response(&r).unwrap();
    assert_eq!(out["candidates"][0]["finishReason"], "MAX_TOKENS"); // max_tokens → MAX_TOKENS
}

#[test]
fn render_gemini_with_all() {
    use crate::gateway::adapter::converter::NonStreamResponse;
    use super::render_gemini_response;

    let r = NonStreamResponse {
        id: "test".to_string(),
        model: "gemini-pro".to_string(),
        text: Some("Result".to_string()),
        tool_uses: vec![
            ("tool_0".to_string(), "write".to_string(), json!({"content": "data"})),
        ],
        stop_reason: "tool_use".to_string(),
        input_tokens: 25,
        output_tokens: 12,
        cache_read_tokens: 0,
        reasoning: Some("Analyzing...".to_string()),
    };

    let out = render_gemini_response(&r).unwrap();
    let parts = out["candidates"][0]["content"]["parts"].as_array().unwrap();
    // text + thought + functionCall
    assert_eq!(parts.len(), 3);
    assert_eq!(parts[0]["text"], "Result");
    assert_eq!(parts[1]["thought"], true);
    assert_eq!(parts[1]["text"], "Analyzing...");
    assert_eq!(parts[2]["functionCall"]["name"], "write");
}

#[test]
fn render_gemini_empty_message() {
    use crate::gateway::adapter::converter::NonStreamResponse;
    use super::render_gemini_response;

    let r = NonStreamResponse {
        id: "empty".to_string(),
        model: "gemini-pro".to_string(),
        text: None,
        tool_uses: vec![],
        stop_reason: "end_turn".to_string(),
        input_tokens: 0,
        output_tokens: 0,
        cache_read_tokens: 0,
        reasoning: None,
    };

    let out = render_gemini_response(&r).unwrap();
    // 兜底空 text part
    assert_eq!(out["candidates"][0]["content"]["parts"].as_array().unwrap().len(), 1);
    assert_eq!(out["candidates"][0]["content"]["parts"][0]["text"], "");
}
