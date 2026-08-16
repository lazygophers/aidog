use serde_json::json;

use crate::protocols::anthropic::convert::*;

fn req(messages: Vec<Message>) -> ChatRequest {
    ChatRequest {
            thinking_budget: None,
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
                name: None,
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
        (ToolChoice::None, true),
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
fn parse_sse_thinking_delta() {
    let d = json!({"type": "content_block_delta", "delta": {"type": "thinking_delta", "thinking": "thinking..."}});
    match parse_anthropic_sse(&d) {
        Some(ChatStreamEvent::ReasoningDelta { text }) => assert_eq!(text, "thinking..."),
        _ => panic!("expected reasoning_delta"),
    }
}

#[test]
fn parse_sse_empty_thinking_delta_none() {
    let d = json!({"type": "content_block_delta", "delta": {"type": "thinking_delta", "thinking": ""}});
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

#[test]
fn parse_anthropic_response_with_thinking() {
    use crate::protocols::anthropic::convert::parse_anthropic_response;

    let body = json!({
        "id": "msg_123",
        "type": "message",
        "role": "assistant",
        "content": [
            {
                "type": "thinking",
                "thinking": "Let me analyze this step by step.\n\nFirst, I need to understand the requirements."
            },
            {
                "type": "text",
                "text": "Based on my analysis, here's the solution."
            }
        ],
        "stop_reason": "end_turn",
        "model": "claude-3-5-sonnet-20241022",
        "usage": {
            "input_tokens": 100,
            "output_tokens": 50
        }
    });

    let parsed = parse_anthropic_response(&body, "claude").expect("should parse");
    assert_eq!(parsed.id, "msg_123");
    assert_eq!(parsed.model, "claude-3-5-sonnet-20241022");
    assert_eq!(parsed.text.as_deref(), Some("Based on my analysis, here's the solution."));
    assert_eq!(parsed.reasoning.as_deref(), Some("Let me analyze this step by step.\n\nFirst, I need to understand the requirements."));
    assert_eq!(parsed.stop_reason, "end_turn");
    assert_eq!(parsed.input_tokens, 100);
    assert_eq!(parsed.output_tokens, 50);
    assert!(parsed.tool_uses.is_empty());
}

#[test]
fn parse_anthropic_response_with_tool_use() {
    use crate::protocols::anthropic::convert::parse_anthropic_response;

    let body = json!({
        "id": "msg_456",
        "type": "message",
        "role": "assistant",
        "content": [
            {"type": "text", "text": "I'll help you with that."},
            {
                "type": "tool_use",
                "id": "toolu_abc123",
                "name": "calculator",
                "input": {"operation": "add", "a": 1, "b": 2}
            }
        ],
        "stop_reason": "tool_use",
        "model": "claude-3-opus-20240229",
        "usage": {
            "input_tokens": 200,
            "output_tokens": 80,
            "cache_read_tokens": 50
        }
    });

    let parsed = parse_anthropic_response(&body, "claude").expect("should parse");
    assert_eq!(parsed.text.as_deref(), Some("I'll help you with that."));
    assert_eq!(parsed.tool_uses.len(), 1);
    assert_eq!(parsed.tool_uses[0].0, "toolu_abc123");
    assert_eq!(parsed.tool_uses[0].1, "calculator");
    assert_eq!(parsed.stop_reason, "tool_use");
    assert_eq!(parsed.cache_read_tokens, 50);
}

#[test]
fn parse_anthropic_response_minimal() {
    use crate::protocols::anthropic::convert::parse_anthropic_response;

    // 测试最简情况：只有 text 块，无 thinking
    let body = json!({
        "id": "msg_789",
        "type": "message",
        "role": "assistant",
        "content": [
            {"type": "text", "text": "Simple response"}
        ],
        "stop_reason": "end_turn",
        "model": "claude-3-haiku-20240307",
        "usage": {
            "input_tokens": 10,
            "output_tokens": 5
        }
    });

    let parsed = parse_anthropic_response(&body, "claude").expect("should parse");
    assert_eq!(parsed.text.as_deref(), Some("Simple response"));
    assert!(parsed.reasoning.is_none());
    assert!(parsed.tool_uses.is_empty());
}
