use super::*;

// ── 非流式 openai→anthropic 响应转换：content + tool_calls 并存(finish_reason=tool_calls) ──
// 复现 request 1ef25294 场景：上游 message 同时含 content / reasoning_content / tool_calls。
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
    assert_eq!(out["model"], "claude-opus-4", "model 回填为客户端请求模型");
    assert_eq!(out["stop_reason"], "tool_use", "tool_calls→tool_use");
    let content = out["content"].as_array().unwrap();
    // 第一块 text，第二块 tool_use（顺序：text 在前）
    assert_eq!(content[0]["type"], "text");
    assert_eq!(content[0]["text"], "Trellis SessionStart...");
    assert_eq!(content[1]["type"], "tool_use");
    assert_eq!(content[1]["id"], "call_-7518760127650854872");
    assert_eq!(content[1]["name"], "read_file");
    assert_eq!(content[1]["input"]["path"], "/a", "arguments JSON 解析为 input 对象");
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

// ── reasoning_content 存在不致崩 / 不产空（已隐含在上面，单测显式确认无 content+无 tool 时兜底空 text） ──
#[test]
fn convert_response_empty_message_yields_nonempty_content() {
    let upstream = serde_json::json!({
        "id": "c4", "model": "m",
        "choices": [{ "index": 0, "finish_reason": "stop", "message": {
            "role": "assistant", "reasoning_content": "只有思维链" } }]
    });
    let out = convert_response(&upstream, &Protocol::OpenAI, "anthropic", "claude").unwrap();
    let content = out["content"].as_array().unwrap();
    assert_eq!(content.len(), 1, "兜底空 text 块保证 content 非空");
    assert_eq!(content[0]["type"], "text");
    assert_eq!(content[0]["text"], "");
}

// ── 同协议（client=openai, wire=openai）→ None（透传，不转换） ──
#[test]
fn convert_response_same_proto_returns_none() {
    let upstream = serde_json::json!({ "choices": [] });
    assert!(convert_response(&upstream, &Protocol::OpenAI, "openai", "m").is_none());
}
