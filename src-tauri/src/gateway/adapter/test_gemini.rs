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
