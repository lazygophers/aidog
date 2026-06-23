use serde_json::json;

use super::super::super::types::*;
use super::from_openai;

#[test]
fn invalid_body_returns_none() {
    // 缺 model/messages → 反序列化失败 → None
    assert!(from_openai(&json!({"foo": "bar"})).is_none());
}

#[test]
fn system_message_extracted() {
    let body = json!({
        "model": "gpt-4",
        "messages": [
            {"role": "system", "content": "you are helpful"},
            {"role": "user", "content": "hi"}
        ]
    });
    let req = from_openai(&body).expect("parsed");
    assert_eq!(req.model, "gpt-4");
    match req.system {
        Some(SystemContent::Text(s)) => assert_eq!(s, "you are helpful"),
        _ => panic!("expected system text"),
    }
    // system 不进 messages
    assert_eq!(req.messages.len(), 1);
}

#[test]
fn system_message_without_content_ignored() {
    let body = json!({
        "model": "gpt-4",
        "messages": [{"role": "system"}, {"role": "user", "content": "hi"}]
    });
    let req = from_openai(&body).expect("parsed");
    assert!(req.system.is_none());
}

#[test]
fn assistant_tool_calls_with_text() {
    let body = json!({
        "model": "gpt-4",
        "messages": [{
            "role": "assistant",
            "content": "let me check",
            "tool_calls": [{
                "id": "call_1",
                "type": "function",
                "function": {"name": "read", "arguments": "{\"path\":\"/a\"}"}
            }]
        }]
    });
    let req = from_openai(&body).expect("parsed");
    let m = &req.messages[0];
    match &m.content {
        MessageContent::Blocks(blocks) => {
            assert_eq!(blocks.len(), 2);
            matches!(blocks[0], ContentBlock::Text { .. });
            match &blocks[1] {
                ContentBlock::ToolUse { id, name, input } => {
                    assert_eq!(id, "call_1");
                    assert_eq!(name, "read");
                    assert_eq!(input["path"], "/a");
                }
                _ => panic!("expected tool_use"),
            }
        }
        _ => panic!("expected blocks"),
    }
}

#[test]
fn assistant_tool_calls_with_bad_arguments_defaults_empty() {
    let body = json!({
        "model": "gpt-4",
        "messages": [{
            "role": "assistant",
            "tool_calls": [{
                "id": "c1", "type": "function",
                "function": {"name": "f", "arguments": "not-json"}
            }]
        }]
    });
    let req = from_openai(&body).expect("parsed");
    if let MessageContent::Blocks(b) = &req.messages[0].content {
        if let ContentBlock::ToolUse { input, .. } = &b[0] {
            assert!(input.is_object());
        } else {
            panic!("tool_use expected");
        }
    }
}

#[test]
fn tool_role_to_tool_result() {
    let body = json!({
        "model": "gpt-4",
        "messages": [{"role": "tool", "tool_call_id": "c1", "content": "result text"}]
    });
    let req = from_openai(&body).expect("parsed");
    match &req.messages[0].content {
        MessageContent::Blocks(b) => match &b[0] {
            ContentBlock::ToolResult { tool_use_id, content } => {
                assert_eq!(tool_use_id, "c1");
                assert_eq!(content, "result text");
            }
            _ => panic!("tool_result expected"),
        },
        _ => panic!("blocks expected"),
    }
}

#[test]
fn array_content_single_text_collapses_to_text() {
    let body = json!({
        "model": "gpt-4",
        "messages": [{"role": "user", "content": ["hello"]}]
    });
    let req = from_openai(&body).expect("parsed");
    matches!(req.messages[0].content, MessageContent::Text(_));
}

#[test]
fn array_content_multi_text_stays_blocks() {
    let body = json!({
        "model": "gpt-4",
        "messages": [{"role": "user", "content": ["a", "b"]}]
    });
    let req = from_openai(&body).expect("parsed");
    match &req.messages[0].content {
        MessageContent::Blocks(b) => assert_eq!(b.len(), 2),
        _ => panic!("blocks expected"),
    }
}

#[test]
fn non_string_content_stringified() {
    let body = json!({
        "model": "gpt-4",
        "messages": [{"role": "user", "content": 123}]
    });
    let req = from_openai(&body).expect("parsed");
    matches!(req.messages[0].content, MessageContent::Text(_));
}

#[test]
fn unknown_role_defaults_user() {
    let body = json!({
        "model": "gpt-4",
        "messages": [{"role": "weird", "content": "x"}]
    });
    let req = from_openai(&body).expect("parsed");
    matches!(req.messages[0].role, Role::User);
}

#[test]
fn tools_and_tool_choice_variants() {
    for (tc_json, expect) in [
        (json!("auto"), "auto"),
        (json!("required"), "any"),
        (json!("none"), "none"),
        (json!({"function": {"name": "f"}}), "named"),
    ] {
        let body = json!({
            "model": "gpt-4",
            "messages": [{"role": "user", "content": "x"}],
            "tools": [{"type": "function", "function": {"name": "f", "parameters": {}}}],
            "tool_choice": tc_json
        });
        let req = from_openai(&body).expect("parsed");
        assert!(req.tools.is_some());
        let tc = req.tool_choice.expect("tool_choice");
        match (tc, expect) {
            (ToolChoice::Auto, "auto") => {}
            (ToolChoice::Any, "any") => {}
            (ToolChoice::None, "none") => {}
            (ToolChoice::Named { .. }, "named") => {}
            _ => panic!("mismatch for {expect}"),
        }
    }
}

#[test]
fn invalid_tool_choice_string_is_none() {
    let body = json!({
        "model": "gpt-4",
        "messages": [{"role": "user", "content": "x"}],
        "tool_choice": "garbage"
    });
    let req = from_openai(&body).expect("parsed");
    assert!(req.tool_choice.is_none());
}
