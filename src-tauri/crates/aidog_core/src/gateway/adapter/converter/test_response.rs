use super::*;
use serde_json::json;

// ── 非流式 openai→anthropic 响应转换：reasoning + content + tool_calls 并存(finish_reason=tool_calls) ──
// 复现 request 1ef25294 场景：上游 message 同时含 reasoning_content / content / tool_calls。
#[test]
fn convert_response_openai_to_anthropic_content_and_tools() {
    let upstream = serde_json::json!({
        "id": "chatcmpl-x",
        "model": "glm-4.6",
        "choices": [{
            "index": 0,
            "finish_reason": "tool_calls",
            "message": {
                "role": "assistant",
                "content": "Trellis SessionStart...",
                "reasoning_content": "用户触发...(GLM思维链)",
                "tool_calls": [{
                    "id": "call_-7518760127650854872",
                    "index": 2,
                    "type": "function",
                    "function": { "name": "read_file", "arguments": "{\"path\":\"/a\"}" }
                }]
            }
        }],
        "usage": { "prompt_tokens": 100, "completion_tokens": 1002,
                   "prompt_tokens_details": { "cached_tokens": 40 } }
    });
    let out = convert_response(&upstream, &Protocol::OpenAI, "anthropic", "claude-opus-4")
        .expect("openai→anthropic should convert");
    assert_eq!(out["type"], "message");
    assert_eq!(out["role"], "assistant");
    assert_eq!(out["model"], "glm-4.6", "model 使用上游响应的模型（未覆盖时）");
    assert_eq!(out["stop_reason"], "tool_use", "tool_calls→tool_use");
    let content = out["content"].as_array().unwrap();
    // 三块：reasoning 排首位，然后 text，最后 tool_use（顺序：reasoning → text → tool_use）
    assert_eq!(content.len(), 3, "reasoning + text + tool_use 三块");
    // 第一块 reasoning（text 块）
    assert_eq!(content[0]["type"], "text");
    assert_eq!(content[0]["text"], "用户触发...(GLM思维链)", "reasoning_content 排首位");
    // 第二块 text
    assert_eq!(content[1]["type"], "text");
    assert_eq!(content[1]["text"], "Trellis SessionStart...");
    // 第三块 tool_use
    assert_eq!(content[2]["type"], "tool_use");
    assert_eq!(content[2]["id"], "call_-7518760127650854872");
    assert_eq!(content[2]["name"], "read_file");
    assert_eq!(content[2]["input"]["path"], "/a", "arguments JSON 解析为 input 对象");
    // usage 映射
    assert_eq!(out["usage"]["input_tokens"], 100);
    assert_eq!(out["usage"]["output_tokens"], 1002);
    assert_eq!(out["usage"]["cache_read_input_tokens"], 40);
}

// ── 纯 tool_calls 无 content：content 数组仅含 tool_use（合法非空） ──
#[test]
fn convert_response_openai_to_anthropic_tool_only() {
    let upstream = serde_json::json!({
        "id": "c1", "model": "glm-4.6",
        "choices": [{ "index": 0, "finish_reason": "tool_calls", "message": {
            "role": "assistant", "content": null,
            "tool_calls": [{ "id": "t1", "type": "function",
                "function": { "name": "ls", "arguments": "{}" } }]
        }}],
        "usage": { "prompt_tokens": 5, "completion_tokens": 7 }
    });
    let out = convert_response(&upstream, &Protocol::OpenAI, "anthropic", "claude").unwrap();
    let content = out["content"].as_array().unwrap();
    assert_eq!(content.len(), 1, "无 text 时只含 tool_use");
    assert_eq!(content[0]["type"], "tool_use");
    assert_eq!(content[0]["input"], serde_json::json!({}));
    assert_eq!(out["stop_reason"], "tool_use");
}

// ── 纯文本（finish_reason=stop）→ end_turn + 单 text 块 ──
#[test]
fn convert_response_openai_to_anthropic_text_only() {
    let upstream = serde_json::json!({
        "id": "c2", "model": "glm-4.6",
        "choices": [{ "index": 0, "finish_reason": "stop", "message": {
            "role": "assistant", "content": "hello world" } }],
        "usage": { "prompt_tokens": 3, "completion_tokens": 2 }
    });
    let out = convert_response(&upstream, &Protocol::OpenAI, "anthropic", "claude").unwrap();
    let content = out["content"].as_array().unwrap();
    assert_eq!(content.len(), 1);
    assert_eq!(content[0]["type"], "text");
    assert_eq!(content[0]["text"], "hello world");
    assert_eq!(out["stop_reason"], "end_turn", "stop→end_turn");
}

// ── finish_reason=length → max_tokens ──
#[test]
fn convert_response_length_maps_max_tokens() {
    let upstream = serde_json::json!({
        "id": "c3", "model": "m",
        "choices": [{ "index": 0, "finish_reason": "length", "message": {
            "role": "assistant", "content": "truncated" } }]
    });
    let out = convert_response(&upstream, &Protocol::OpenAI, "anthropic", "claude").unwrap();
    assert_eq!(out["stop_reason"], "max_tokens");
}

// ── reasoning_content 存在时排在 content 首位 ──
#[test]
fn convert_response_empty_message_yields_nonempty_content() {
    let upstream = serde_json::json!({
        "id": "c4", "model": "m",
        "choices": [{ "index": 0, "finish_reason": "stop", "message": {
            "role": "assistant", "reasoning_content": "只有思维链" } }]
    });
    let out = convert_response(&upstream, &Protocol::OpenAI, "anthropic", "claude").unwrap();
    let content = out["content"].as_array().unwrap();
    assert_eq!(content.len(), 1, "reasoning 非空时排首位，content 含一个 text 块");
    assert_eq!(content[0]["type"], "text");
    assert_eq!(content[0]["text"], "只有思维链", "reasoning_content 排在 content 首位");
}

// ── 同协议（client=openai, wire=openai）→ None（透传，不转换） ──
#[test]
fn convert_response_same_proto_returns_none() {
    let upstream = serde_json::json!({ "choices": [] });
    assert!(convert_response(&upstream, &Protocol::OpenAI, "openai", "m").is_none());
}

// ── to_anthropic_sse: Start event ──
#[test]
fn to_anthropic_sse_start_event() {
    let event = ChatStreamEvent::Start {
        id: "msg_01".to_string(),
        model: "claude-opus-4".to_string(),
    };
    let sse = to_anthropic_sse(&event).expect("Start should produce SSE");
    assert!(sse.contains("event: message_start"), "should have message_start event");
    assert!(sse.contains("msg_01"), "should contain message id");
    assert!(sse.contains("claude-opus-4"), "should contain model");
    assert!(sse.contains("\"type\":\"message_start\""), "should contain type");
}

// ── to_anthropic_sse: Delta event ──
#[test]
fn to_anthropic_sse_delta_event() {
    let event = ChatStreamEvent::Delta { text: "hello world".to_string() };
    let sse = to_anthropic_sse(&event).expect("Delta should produce SSE");
    assert!(sse.contains("event: content_block_delta"));
    assert!(sse.contains("hello world"), "should contain text");
    assert!(sse.contains("text_delta"));
}

// ── to_anthropic_sse: ToolDelta with id+name+input ──
#[test]
fn to_anthropic_sse_tool_delta_full() {
    let event = ChatStreamEvent::ToolDelta {
        index: 0,
        id: Some("tool_1".to_string()),
        name: Some("read_file".to_string()),
        input: Some("{\"path\":\"/a\"}".to_string()),
    };
    let sse = to_anthropic_sse(&event).expect("ToolDelta should produce SSE");
    assert!(sse.contains("content_block_start"), "should have content_block_start");
    assert!(sse.contains("tool_1"), "should contain tool id");
    assert!(sse.contains("read_file"), "should contain tool name");
    assert!(sse.contains("input_json_delta"), "should contain input delta");
}

// ── to_anthropic_sse: ToolDelta only input (no id/name) ──
#[test]
fn to_anthropic_sse_tool_delta_input_only() {
    let event = ChatStreamEvent::ToolDelta {
        index: 1,
        id: None,
        name: None,
        input: Some("{}".to_string()),
    };
    let sse = to_anthropic_sse(&event).expect("ToolDelta input only should produce SSE");
    assert!(!sse.contains("content_block_start"), "no start without id/name");
    assert!(sse.contains("input_json_delta"), "should have input delta");
}

// ── to_anthropic_sse: ToolDelta empty (no id/name/input) → None ──
#[test]
fn to_anthropic_sse_tool_delta_empty_returns_none() {
    let event = ChatStreamEvent::ToolDelta {
        index: 0,
        id: None,
        name: None,
        input: None,
    };
    assert!(to_anthropic_sse(&event).is_none(), "empty ToolDelta should be None");
}

// ── to_anthropic_sse: Stop event ──
#[test]
fn to_anthropic_sse_stop_event() {
    let event = ChatStreamEvent::Stop { finish_reason: Some("end_turn".to_string()) };
    let sse = to_anthropic_sse(&event).expect("Stop should produce SSE");
    assert!(sse.contains("event: message_delta"));
    assert!(sse.contains("event: message_stop"));
    assert!(sse.contains("end_turn"));
}

// ── to_anthropic_sse: Stop with no reason → defaults end_turn ──
#[test]
fn to_anthropic_sse_stop_no_reason_defaults_end_turn() {
    let event = ChatStreamEvent::Stop { finish_reason: None };
    let sse = to_anthropic_sse(&event).expect("Stop None should produce SSE");
    assert!(sse.contains("end_turn"), "None reason should default to end_turn");
}

// ── to_anthropic_sse: Usage → None ──
#[test]
fn to_anthropic_sse_usage_returns_none() {
    use crate::gateway::adapter::types::Usage;
    let event = ChatStreamEvent::Usage {
        usage: Usage { prompt_tokens: Some(100), completion_tokens: Some(50), total_tokens: None },
    };
    assert!(to_anthropic_sse(&event).is_none(), "Usage should produce None");
}

// ── to_client_sse: anthropic protocol ──
#[test]
fn to_client_sse_anthropic_protocol() {
    let event = ChatStreamEvent::Delta { text: "hi".to_string() };
    let sse = to_client_sse(&event, "anthropic", "m");
    assert!(sse.is_some(), "anthropic protocol should produce SSE");
    assert!(sse.unwrap().contains("content_block_delta"));
}

// ── to_client_sse: openai protocol ──
#[test]
fn to_client_sse_openai_protocol() {
    let event = ChatStreamEvent::Delta { text: "hi".to_string() };
    let sse = to_client_sse(&event, "openai", "gpt-4");
    // openai SSE should contain "data:" prefix
    assert!(sse.is_some(), "openai protocol should produce SSE");
}

// ── to_client_sse: gemini protocol ──
#[test]
fn to_client_sse_gemini_protocol() {
    let event = ChatStreamEvent::Delta { text: "hello".to_string() };
    // gemini protocol — may or may not produce SSE depending on implementation
    // just ensure it doesn't panic
    let _ = to_client_sse(&event, "gemini", "gemini-pro");
}

// ── render_anthropic_response 测试（方案 B：reasoning 排 text 块首位） ──
#[test]
fn render_anthropic_with_reasoning_first() {
    use super::*;

    let r = NonStreamResponse {
        id: "test".to_string(),
        model: "claude-3".to_string(),
        text: Some("Answer".to_string()),
        tool_uses: vec![],
        stop_reason: "end_turn".to_string(),
        input_tokens: 20,
        output_tokens: 10,
        cache_read_tokens: 0,
        reasoning: Some("Thinking...".to_string()),
    };

    let out = render_anthropic_response(&r);
    let content = out["content"].as_array().unwrap();

    // 方案 B：reasoning 排 text 块首位（禁 thinking 块）
    assert_eq!(content.len(), 2);
    assert_eq!(content[0]["type"], "text");
    assert_eq!(content[0]["text"], "Thinking..."); // reasoning 排首位
    assert_eq!(content[1]["type"], "text");
    assert_eq!(content[1]["text"], "Answer");
}

#[test]
fn render_anthropic_reasoning_with_tools() {
    use super::*;

    let r = NonStreamResponse {
        id: "test".to_string(),
        model: "claude-3".to_string(),
        text: Some("Calling function".to_string()),
        tool_uses: vec![
            ("tool-1".to_string(), "read".to_string(), json!({"path": "/tmp"})),
        ],
        stop_reason: "tool_use".to_string(),
        input_tokens: 25,
        output_tokens: 12,
        cache_read_tokens: 0,
        reasoning: Some("Analyzing...".to_string()),
    };

    let out = render_anthropic_response(&r);
    let content = out["content"].as_array().unwrap();

    // reasoning + text + tool_use
    assert_eq!(content.len(), 3);
    assert_eq!(content[0]["type"], "text");
    assert_eq!(content[0]["text"], "Analyzing...");
    assert_eq!(content[1]["type"], "text");
    assert_eq!(content[1]["text"], "Calling function");
    assert_eq!(content[2]["type"], "tool_use");
}

// ── N×N 矩阵测试：anthropic→openai ──
#[test]
fn convert_response_anthropic_to_openai_with_reasoning() {
    let upstream = serde_json::json!({
        "id": "msg-1",
        "type": "message",
        "role": "assistant",
        "model": "claude-3",
        "content": [
            {"type": "thinking", "thinking": "Analyzing step by step..."},
            {"type": "text", "text": "Final answer"}
        ],
        "stop_reason": "end_turn",
        "stop_sequence": null,
        "usage": {
            "input_tokens": 20,
            "output_tokens": 10,
            "cache_read_tokens": 0
        }
    });
    let out = convert_response(&upstream, &Protocol::Anthropic, "openai", "gpt-4")
        .expect("anthropic→openai should convert");
    let message = &out["choices"][0]["message"];
    // anthropic 的 thinking 块视为 reasoning，放入 reasoning_content
    assert_eq!(message["reasoning_content"], "Analyzing step by step...");
    // text 块放入 content
    assert_eq!(message["content"], "Final answer");
    assert_eq!(out["choices"][0]["finish_reason"], "stop", "end_turn→stop");
    assert_eq!(out["usage"]["prompt_tokens"], 20);
    assert_eq!(out["usage"]["completion_tokens"], 10);
}

// ── N×N 矩阵测试：gemini→anthropic ──
#[test]
fn convert_response_gemini_to_anthropic() {
    let upstream = serde_json::json!({
        "candidates": [{
            "index": 0,
            "finishReason": "STOP",
            "content": {
                "role": "model",
                "parts": [{"text": "Hello from Gemini"}]
            }
        }],
        "usageMetadata": {
            "promptTokenCount": 5,
            "candidatesTokenCount": 3,
            "totalTokenCount": 8
        }
    });
    let out = convert_response(&upstream, &Protocol::Gemini, "anthropic", "claude")
        .expect("gemini→anthropic should convert");
    assert_eq!(out["type"], "message");
    assert_eq!(out["role"], "assistant");
    let content = out["content"].as_array().unwrap();
    assert_eq!(content.len(), 1);
    assert_eq!(content[0]["type"], "text");
    assert_eq!(content[0]["text"], "Hello from Gemini");
    assert_eq!(out["stop_reason"], "end_turn", "STOP→end_turn");
    assert_eq!(out["usage"]["input_tokens"], 5);
    assert_eq!(out["usage"]["output_tokens"], 3);
}

// ── N×N 矩阵测试：openai→gemini ──
#[test]
fn convert_response_openai_to_gemini() {
    let upstream = serde_json::json!({
        "id": "c1",
        "model": "gpt-4",
        "choices": [{
            "index": 0,
            "finish_reason": "stop",
            "message": {
                "role": "assistant",
                "content": "Response from OpenAI"
            }
        }],
        "usage": {
            "prompt_tokens": 10,
            "completion_tokens": 5
        }
    });
    let out = convert_response(&upstream, &Protocol::OpenAI, "gemini", "gemini-pro")
        .expect("openai→gemini should convert");
    let candidates = out["candidates"].as_array().unwrap();
    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0]["finishReason"], "STOP", "stop→STOP");
    assert_eq!(candidates[0]["content"]["role"], "model");
    assert_eq!(candidates[0]["content"]["parts"][0]["text"], "Response from OpenAI");
    assert_eq!(out["usageMetadata"]["promptTokenCount"], 10);
    assert_eq!(out["usageMetadata"]["completionTokenCount"], 5);
}

// ── 本案重放：request cd7ff24d 场景 ──
// openai 上游含 reasoning_content + 空 content, client=anthropic → reasoning 排 content 首位
#[test]
fn convert_response_case_cd7ff24d_openai_reasoning_to_anthropic() {
    let upstream = serde_json::json!({
        "id": "chatcmpl-cd7ff24d",
        "model": "glm-4.6",
        "choices": [{
            "index": 0,
            "finish_reason": "stop",
            "message": {
                "role": "assistant",
                "content": null,
                "reasoning_content": "Let me analyze this step by step..."
            }
        }],
        "usage": {
            "prompt_tokens": 50,
            "completion_tokens": 30
        }
    });
    let out = convert_response(&upstream, &Protocol::OpenAI, "anthropic", "claude-3-opus")
        .expect("case cd7ff24d: openai reasoning_content → anthropic content with reasoning");
    let content = out["content"].as_array().unwrap();
    assert_eq!(content.len(), 1, "仅有 reasoning 块");
    assert_eq!(content[0]["type"], "text");
    assert_eq!(content[0]["text"], "Let me analyze this step by step...", "reasoning_content 排首位");
    assert_eq!(out["stop_reason"], "end_turn", "stop→end_turn");
    assert_eq!(out["usage"]["input_tokens"], 50);
    assert_eq!(out["usage"]["output_tokens"], 30);
}
