use serde_json::json;

use super::super::openai_completions::*;

#[test]
fn to_completions_joins_messages() {
    let req = ChatRequest {
        model: "m".into(),
        messages: vec![
            Message { role: Role::System, content: MessageContent::Text("sys".into()) },
            Message { role: Role::User, content: MessageContent::Text("hi".into()) },
            Message {
                role: Role::Assistant,
                content: MessageContent::Blocks(vec![
                    ContentBlock::Text { text: "a".into() },
                    ContentBlock::ToolUse { id: "i".into(), name: "f".into(), input: json!({}) },
                ]),
            },
            Message {
                role: Role::Tool,
                content: MessageContent::Blocks(vec![ContentBlock::ToolResult {
                    tool_use_id: "i".into(),
                    content: "r".into(),
                }]),
            },
        ],
        system: None,
        max_tokens: Some(50),
        temperature: Some(0.5),
        top_p: None,
        stream: Some(true),
        tools: None,
        tool_choice: None,
        extra: None,
    };
    let c = to_completions(&req);
    assert!(c.prompt.contains("System: sys"));
    assert!(c.prompt.contains("User: hi"));
    assert!(c.prompt.contains("Assistant: a"));
    assert!(c.prompt.contains("Tool: "));
    assert_eq!(c.max_tokens, Some(50));
    assert_eq!(c.stream, Some(true));
}

#[test]
fn from_completions_basic() {
    let body = json!({"model": "m", "prompt": "hello", "max_tokens": 10, "temperature": 0.2, "top_p": 0.9, "stream": false});
    let r = from_completions(&body).expect("parsed");
    assert_eq!(r.model, "m");
    assert_eq!(r.messages.len(), 1);
    matches!(r.messages[0].role, Role::User);
    assert_eq!(r.max_tokens, Some(10));
    assert_eq!(r.stream, Some(false));
}

#[test]
fn from_completions_missing_model_none() {
    assert!(from_completions(&json!({"prompt": "x"})).is_none());
}

#[test]
fn from_completions_missing_prompt_defaults_empty() {
    let r = from_completions(&json!({"model": "m"}));
    // prompt is required (get("prompt")?) → None
    assert!(r.is_none());
}

#[test]
fn sse_passthrough_helpers() {
    let d = json!({"choices": [{"index": 0, "delta": {"content": "x"}}]});
    assert!(parse_completions_sse(&d).is_some());
    let ev = ChatStreamEvent::Delta { text: "y".into() };
    assert!(to_completions_sse(&ev, "m").is_some());
}
