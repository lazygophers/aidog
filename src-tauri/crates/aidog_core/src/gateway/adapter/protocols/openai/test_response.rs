use serde_json::json;

use crate::gateway::adapter::converter::NonStreamResponse;
use super::render_openai_response;

fn make_non_stream() -> NonStreamResponse {
    NonStreamResponse {
        id: "test-id".to_string(),
        model: "gpt-4".to_string(),
        text: Some("Hello world".to_string()),
        tool_uses: vec![],
        stop_reason: "end_turn".to_string(),
        input_tokens: 10,
        output_tokens: 20,
        cache_read_tokens: 5,
        reasoning: None,
    }
}

#[test]
fn render_openai_text_only() {
    let r = make_non_stream();
    let out = render_openai_response(&r).unwrap();

    assert_eq!(out["id"], "test-id");
    assert_eq!(out["model"], "gpt-4");
    assert_eq!(out["choices"][0]["index"], 0);
    assert_eq!(out["choices"][0]["finish_reason"], "stop"); // end_turn → stop
    assert_eq!(out["choices"][0]["message"]["role"], "assistant");
    assert_eq!(out["choices"][0]["message"]["content"], "Hello world");
    assert!(out["choices"][0]["message"].get("tool_calls").is_none());
    assert!(out["choices"][0]["message"].get("reasoning_content").is_none());

    // usage 映射
    assert_eq!(out["usage"]["prompt_tokens"], 10);
    assert_eq!(out["usage"]["completion_tokens"], 20);
    assert_eq!(out["usage"]["total_tokens"], 30);
    assert_eq!(out["usage"]["prompt_tokens_details"]["cached_tokens"], 5);
}

#[test]
fn render_openai_with_reasoning() {
    let mut r = make_non_stream();
    r.reasoning = Some("Let me think about this...".to_string());

    let out = render_openai_response(&r).unwrap();

    // reasoning 独立字段，不拼 content
    assert_eq!(out["choices"][0]["message"]["content"], "Hello world");
    assert_eq!(out["choices"][0]["message"]["reasoning_content"], "Let me think about this...");
}

#[test]
fn render_openai_with_tool_calls() {
    let mut r = make_non_stream();
    r.tool_uses = vec![
        ("tool-1".to_string(), "read_file".to_string(), json!({"path": "/tmp"})),
    ];
    r.stop_reason = "tool_use".to_string();

    let out = render_openai_response(&r).unwrap();

    assert_eq!(out["choices"][0]["finish_reason"], "tool_calls"); // tool_use → tool_calls
    assert!(out["choices"][0]["message"].get("tool_calls").is_some());
    assert_eq!(out["choices"][0]["message"]["tool_calls"].as_array().unwrap().len(), 1);
    assert_eq!(out["choices"][0]["message"]["tool_calls"][0]["id"], "tool-1");
    assert_eq!(out["choices"][0]["message"]["tool_calls"][0]["type"], "function");
    assert_eq!(out["choices"][0]["message"]["tool_calls"][0]["function"]["name"], "read_file");
    assert_eq!(out["choices"][0]["message"]["tool_calls"][0]["function"]["arguments"], "{\"path\":\"/tmp\"}");
}

#[test]
fn render_openai_max_tokens_maps_length() {
    let mut r = make_non_stream();
    r.stop_reason = "max_tokens".to_string();

    let out = render_openai_response(&r).unwrap();
    assert_eq!(out["choices"][0]["finish_reason"], "length"); // max_tokens → length
}

#[test]
fn render_openai_with_reasoning_and_tools() {
    let mut r = make_non_stream();
    r.reasoning = Some("Thinking...".to_string());
    r.tool_uses = vec![
        ("tool-2".to_string(), "write".to_string(), json!({"content": "data"})),
    ];

    let out = render_openai_response(&r).unwrap();

    // 三字段并列
    assert_eq!(out["choices"][0]["message"]["content"], "Hello world");
    assert_eq!(out["choices"][0]["message"]["reasoning_content"], "Thinking...");
    assert!(out["choices"][0]["message"].get("tool_calls").is_some());
}

#[test]
fn render_openai_empty_message() {
    let r = NonStreamResponse {
        id: "empty".to_string(),
        model: "gpt-4".to_string(),
        text: None,
        tool_uses: vec![],
        stop_reason: "end_turn".to_string(),
        input_tokens: 0,
        output_tokens: 0,
        cache_read_tokens: 0,
        reasoning: None,
    };

    let out = render_openai_response(&r).unwrap();

    // 既无 content 也无 tool_calls，message 只有 role
    assert_eq!(out["choices"][0]["message"].as_object().unwrap().len(), 1);
    assert_eq!(out["choices"][0]["message"]["role"], "assistant");
}
