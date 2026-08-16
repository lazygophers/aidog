use serde_json::json;

use crate::protocols::openai_completions::convert::*;

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

// ── render_completions_response 测试 ──
#[test]
fn render_completions_text_only() {
    use crate::converter::NonStreamResponse;
    use super::render_completions_response;

    let r = NonStreamResponse {
        id: "test".to_string(),
        model: "gpt-3.5".to_string(),
        text: Some("Hello".to_string()),
        tool_uses: vec![],
        stop_reason: "end_turn".to_string(),
        input_tokens: 10,
        output_tokens: 5,
        cache_read_tokens: 0,
        reasoning: None,
    };

    let out = render_completions_response(&r).unwrap();
    assert_eq!(out["id"], "test");
    assert_eq!(out["model"], "gpt-3.5");
    assert_eq!(out["choices"][0]["text"], "Hello");
    assert_eq!(out["choices"][0]["finish_reason"], "stop"); // end_turn → stop
}

#[test]
fn render_completions_with_reasoning() {
    use crate::converter::NonStreamResponse;
    use super::render_completions_response;

    let r = NonStreamResponse {
        id: "test".to_string(),
        model: "gpt-3.5".to_string(),
        text: Some("Answer".to_string()),
        tool_uses: vec![],
        stop_reason: "end_turn".to_string(),
        input_tokens: 20,
        output_tokens: 10,
        cache_read_tokens: 0,
        reasoning: Some("Thinking...".to_string()),
    };

    let out = render_completions_response(&r).unwrap();
    // legacy 格式：reasoning 拼 text 前缀
    assert_eq!(out["choices"][0]["text"], "Thinking...Answer");
}

#[test]
fn render_completions_max_tokens_maps_length() {
    use crate::converter::NonStreamResponse;
    use super::render_completions_response;

    let r = NonStreamResponse {
        id: "test".to_string(),
        model: "gpt-3.5".to_string(),
        text: Some("Truncated".to_string()),
        tool_uses: vec![],
        stop_reason: "max_tokens".to_string(),
        input_tokens: 5,
        output_tokens: 3,
        cache_read_tokens: 0,
        reasoning: None,
    };

    let out = render_completions_response(&r).unwrap();
    assert_eq!(out["choices"][0]["finish_reason"], "length"); // max_tokens → length
}

#[test]
fn render_completions_empty_message() {
    use crate::converter::NonStreamResponse;
    use super::render_completions_response;

    let r = NonStreamResponse {
        id: "empty".to_string(),
        model: "gpt-3.5".to_string(),
        text: None,
        tool_uses: vec![],
        stop_reason: "end_turn".to_string(),
        input_tokens: 0,
        output_tokens: 0,
        cache_read_tokens: 0,
        reasoning: None,
    };

    let out = render_completions_response(&r).unwrap();
    assert_eq!(out["choices"][0]["text"], ""); // 空 text
}

// ── parse_completions_response 测试 ──
#[test]
fn parse_completions_response_legacy_format() {
    use super::parse_completions_response;

    // legacy /v1/completions 格式：choices[0].text
    let body = json!({
        "id": "cmpl_123",
        "object": "text_completion",
        "created": 1699000000,
        "model": "gpt-3.5-turbo-instruct",
        "choices": [{
            "index": 0,
            "text": "This is a legacy completion.",
            "finish_reason": "stop"
        }],
        "usage": {
            "prompt_tokens": 5,
            "completion_tokens": 10,
            "total_tokens": 15
        }
    });

    let parsed = parse_completions_response(&body, "gpt-3.5-turbo-instruct").expect("should parse");
    assert_eq!(parsed.id, "cmpl_123");
    assert_eq!(parsed.model, "gpt-3.5-turbo-instruct");
    assert_eq!(parsed.text.as_deref(), Some("This is a legacy completion."));
    assert!(parsed.reasoning.is_none());
    assert!(parsed.tool_uses.is_empty());
    assert_eq!(parsed.stop_reason, "end_turn");
    assert_eq!(parsed.input_tokens, 5);
    assert_eq!(parsed.output_tokens, 10);
}

#[test]
fn parse_completions_response_chat_format_with_reasoning() {
    use super::parse_completions_response;

    // 部分提供商复用 chat 格式 choices[0].message（含 reasoning_content）
    let body = json!({
        "id": "cmpl_456",
        "model": "glm-3-turbo",
        "choices": [{
            "index": 0,
            "message": {
                "role": "assistant",
                "content": "Final answer here.",
                "reasoning_content": "Step-by-step reasoning."
            },
            "finish_reason": "stop"
        }],
        "usage": {
            "prompt_tokens": 15,
            "completion_tokens": 20,
            "total_tokens": 35
        }
    });

    let parsed = parse_completions_response(&body, "glm-3-turbo").expect("should parse");
    assert_eq!(parsed.text.as_deref(), Some("Final answer here."));
    assert_eq!(parsed.reasoning.as_deref(), Some("Step-by-step reasoning."));
    assert_eq!(parsed.stop_reason, "end_turn");
}

#[test]
fn parse_completions_response_max_tokens_maps() {
    use super::parse_completions_response;

    let body = json!({
        "id": "cmpl_789",
        "model": "text-davinci-003",
        "choices": [{
            "index": 0,
            "text": "Truncated response",
            "finish_reason": "length"
        }],
        "usage": {
            "prompt_tokens": 100,
            "completion_tokens": 150,
            "total_tokens": 250
        }
    });

    let parsed = parse_completions_response(&body, "text-davinci-003").expect("should parse");
    assert_eq!(parsed.stop_reason, "max_tokens"); // length → max_tokens
    assert_eq!(parsed.text.as_deref(), Some("Truncated response"));
}
