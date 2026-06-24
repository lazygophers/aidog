use serde_json::json;

use super::super::anthropic::*;

fn req(messages: Vec<Message>) -> ChatRequest {
    ChatRequest {
        model: "claude".into(),
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
fn to_anthropic_skips_system_role_maps_tool_to_user() {
    let r = req(vec![
        Message { role: Role::System, content: MessageContent::Text("s".into()) },
        Message { role: Role::User, content: MessageContent::Text("u".into()) },
        Message {
            role: Role::Tool,
            content: MessageContent::Blocks(vec![ContentBlock::ToolResult {
                tool_use_id: "c".into(),
                content: "r".into(),
            }]),
        },
        Message { role: Role::Assistant, content: MessageContent::Text("a".into()) },
    ]);
    let a = to_anthropic(&r);
    assert_eq!(a.messages.len(), 3);
    assert_eq!(a.messages[0].role, "user");
    assert_eq!(a.messages[1].role, "user"); // tool→user
    assert!(a.messages[1].content.is_array());
    assert_eq!(a.messages[2].role, "assistant");
}

#[test]
fn to_anthropic_system_and_defaults() {
    let mut r = req(vec![]);
    r.system = Some(SystemContent::Text("sys".into()));
    let a = to_anthropic(&r);
    assert_eq!(a.system, Some(json!("sys")));
    assert_eq!(a.max_tokens, 4096); // default

    let mut r2 = req(vec![]);
    r2.system = Some(SystemContent::Blocks(vec![json!({"text": "x"})]));
    r2.max_tokens = Some(10);
    let a2 = to_anthropic(&r2);
    assert!(a2.system.unwrap().is_array());
    assert_eq!(a2.max_tokens, 10);
}

#[test]
fn to_anthropic_tools_and_tool_choice() {
    for (tc, has) in [
        (ToolChoice::Auto, true),
        (ToolChoice::Any, true),
        (ToolChoice::None, false),
        (ToolChoice::Named { name: "f".into() }, true),
    ] {
        let mut r = req(vec![]);
        r.tools = Some(vec![Tool { name: "f".into(), description: None, input_schema: json!({}) }]);
        r.tool_choice = Some(tc);
        let a = to_anthropic(&r);
        assert!(a.tools.is_some());
        assert_eq!(a.tool_choice.is_some(), has);
    }
}

#[test]
fn parse_sse_message_start() {
    let d = json!({"type": "message_start", "message": {"id": "m1", "model": "claude"}});
    match parse_anthropic_sse(&d) {
        Some(ChatStreamEvent::Start { id, model }) => {
            assert_eq!(id, "m1");
            assert_eq!(model, "claude");
        }
        _ => panic!("start"),
    }
}

#[test]
fn parse_sse_text_delta() {
    let d = json!({"type": "content_block_delta", "delta": {"type": "text_delta", "text": "hi"}});
    match parse_anthropic_sse(&d) {
        Some(ChatStreamEvent::Delta { text }) => assert_eq!(text, "hi"),
        _ => panic!("delta"),
    }
}

#[test]
fn parse_sse_input_json_delta() {
    let d = json!({"type": "content_block_delta", "index": 0, "delta": {"type": "input_json_delta", "partial_json": "{}"}});
    match parse_anthropic_sse(&d) {
        Some(ChatStreamEvent::ToolDelta { input, .. }) => assert_eq!(input.as_deref(), Some("{}")),
        _ => panic!("tool delta"),
    }
}

#[test]
fn parse_sse_unknown_delta_type_none() {
    let d = json!({"type": "content_block_delta", "delta": {"type": "weird"}});
    assert!(parse_anthropic_sse(&d).is_none());
}

#[test]
fn parse_sse_content_block_start_tool_use() {
    let d = json!({"type": "content_block_start", "index": 1, "content_block": {"type": "tool_use", "id": "c", "name": "f"}});
    match parse_anthropic_sse(&d) {
        Some(ChatStreamEvent::ToolDelta { index, id, name, .. }) => {
            assert_eq!(index, 1);
            assert_eq!(id.as_deref(), Some("c"));
            assert_eq!(name.as_deref(), Some("f"));
        }
        _ => panic!("tool start"),
    }
}

#[test]
fn parse_sse_content_block_start_text_none() {
    let d = json!({"type": "content_block_start", "index": 0, "content_block": {"type": "text"}});
    assert!(parse_anthropic_sse(&d).is_none());
}

#[test]
fn parse_sse_message_delta_and_stop() {
    let d = json!({"type": "message_delta", "delta": {"stop_reason": "end_turn"}});
    match parse_anthropic_sse(&d) {
        Some(ChatStreamEvent::Stop { finish_reason }) => assert_eq!(finish_reason.as_deref(), Some("end_turn")),
        _ => panic!("stop"),
    }
    let d2 = json!({"type": "message_stop"});
    matches!(parse_anthropic_sse(&d2), Some(ChatStreamEvent::Stop { .. }));
}

#[test]
fn parse_sse_unknown_type_none() {
    assert!(parse_anthropic_sse(&json!({"type": "ping"})).is_none());
    assert!(parse_anthropic_sse(&json!({})).is_none());
}
