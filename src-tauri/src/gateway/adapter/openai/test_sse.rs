use serde_json::json;

use super::super::super::types::*;
use super::{parse_openai_sse, to_openai_sse};

#[test]
fn parse_text_delta() {
    let d = json!({"choices": [{"index": 0, "delta": {"content": "hi"}}]});
    match parse_openai_sse(&d) {
        Some(ChatStreamEvent::Delta { text }) => assert_eq!(text, "hi"),
        _ => panic!("expected delta"),
    }
}

#[test]
fn parse_empty_content_is_none() {
    let d = json!({"choices": [{"index": 0, "delta": {"content": ""}}]});
    assert!(parse_openai_sse(&d).is_none());
}

#[test]
fn parse_tool_delta() {
    let d = json!({"choices": [{"index": 0, "delta": {"tool_calls": [{
        "id": "c1", "function": {"name": "f", "arguments": "{}"}
    }]}}]});
    match parse_openai_sse(&d) {
        Some(ChatStreamEvent::ToolDelta { id, name, input, .. }) => {
            assert_eq!(id.as_deref(), Some("c1"));
            assert_eq!(name.as_deref(), Some("f"));
            assert_eq!(input.as_deref(), Some("{}"));
        }
        _ => panic!("expected tool_delta"),
    }
}

#[test]
fn parse_finish_reasons() {
    for r in ["stop", "tool_calls", "length"] {
        let d = json!({"choices": [{"index": 0, "delta": {}, "finish_reason": r}]});
        match parse_openai_sse(&d) {
            Some(ChatStreamEvent::Stop { finish_reason }) => {
                assert_eq!(finish_reason.as_deref(), Some(r));
            }
            _ => panic!("expected stop for {r}"),
        }
    }
}

#[test]
fn parse_unknown_finish_reason_none() {
    let d = json!({"choices": [{"index": 0, "delta": {}, "finish_reason": "weird"}]});
    assert!(parse_openai_sse(&d).is_none());
}

#[test]
fn parse_missing_choices_none() {
    assert!(parse_openai_sse(&json!({})).is_none());
    assert!(parse_openai_sse(&json!({"choices": []})).is_none());
}

#[test]
fn to_sse_start() {
    let ev = ChatStreamEvent::Start { id: "x".into(), model: "m".into() };
    let s = to_openai_sse(&ev, "m").expect("start");
    assert!(s.contains("chat.completion.chunk"));
    assert!(s.contains("\"role\":\"assistant\""));
}

#[test]
fn to_sse_delta() {
    let ev = ChatStreamEvent::Delta { text: "yo".into() };
    let s = to_openai_sse(&ev, "m").expect("delta");
    assert!(s.contains("yo"));
}

#[test]
fn to_sse_tool_delta_full() {
    let ev = ChatStreamEvent::ToolDelta {
        index: 0,
        id: Some("c1".into()),
        name: Some("f".into()),
        input: Some("{}".into()),
    };
    let s = to_openai_sse(&ev, "m").expect("tool delta");
    assert!(s.contains("tool_calls"));
    assert!(s.contains("\"name\":\"f\""));
}

#[test]
fn to_sse_tool_delta_input_only() {
    let ev = ChatStreamEvent::ToolDelta {
        index: 1,
        id: None,
        name: None,
        input: Some("{\"a\":1}".into()),
    };
    let s = to_openai_sse(&ev, "m").expect("tool input");
    assert!(s.contains("arguments"));
}

#[test]
fn to_sse_tool_delta_empty_none() {
    let ev = ChatStreamEvent::ToolDelta { index: 0, id: None, name: None, input: None };
    assert!(to_openai_sse(&ev, "m").is_none());
}

#[test]
fn to_sse_stop_maps_end_turn() {
    let ev = ChatStreamEvent::Stop { finish_reason: Some("end_turn".into()) };
    let s = to_openai_sse(&ev, "m").expect("stop");
    assert!(s.contains("\"finish_reason\":\"stop\""));
    assert!(s.contains("[DONE]"));
}

#[test]
fn to_sse_stop_passthrough_other() {
    let ev = ChatStreamEvent::Stop { finish_reason: Some("length".into()) };
    let s = to_openai_sse(&ev, "m").expect("stop");
    assert!(s.contains("\"finish_reason\":\"length\""));
}

#[test]
fn to_sse_stop_default_none() {
    let ev = ChatStreamEvent::Stop { finish_reason: None };
    let s = to_openai_sse(&ev, "m").expect("stop");
    assert!(s.contains("stop"));
}

#[test]
fn to_sse_usage_none() {
    let ev = ChatStreamEvent::Usage {
        usage: Usage { prompt_tokens: Some(1), completion_tokens: Some(1), total_tokens: Some(2) },
    };
    assert!(to_openai_sse(&ev, "m").is_none());
}
