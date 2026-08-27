use super::*;
use serde_json::json;

// ── 非流式剥离 ──

/// 复现 request 3ed5a698：disable_thinking=true，MiniMax-M2 仍只回 thinking 块。
#[test]
fn strip_anthropic_thinking_only_leaves_empty_text_block() {
    let mut body = json!({
        "type": "message", "role": "assistant", "model": "haiku",
        "content": [{ "type": "thinking", "thinking": "让我分析…", "signature": "abc" }],
        "stop_reason": "max_tokens"
    });
    assert!(strip_thinking_in_body(&mut body, &Protocol::Anthropic));
    let content = body["content"].as_array().unwrap();
    assert_eq!(content.len(), 1, "Anthropic 拒收空 content，补空 text 块");
    assert_eq!(content[0]["type"], "text");
    assert_eq!(content[0]["text"], "");
}

#[test]
fn strip_anthropic_keeps_text_and_tool_use() {
    let mut body = json!({
        "content": [
            { "type": "thinking", "thinking": "想" },
            { "type": "redacted_thinking", "data": "enc" },
            { "type": "text", "text": "答案" },
            { "type": "tool_use", "id": "t1", "name": "ls", "input": {} }
        ]
    });
    assert!(strip_thinking_in_body(&mut body, &Protocol::Anthropic));
    let content = body["content"].as_array().unwrap();
    assert_eq!(content.len(), 2);
    assert_eq!(content[0]["type"], "text");
    assert_eq!(content[1]["type"], "tool_use");
}

#[test]
fn strip_noop_when_no_thinking() {
    let mut body = json!({ "content": [{ "type": "text", "text": "hi" }] });
    assert!(!strip_thinking_in_body(&mut body, &Protocol::Anthropic), "无思维链不算改动");
}

#[test]
fn strip_openai_chat_reasoning_content() {
    let mut body = json!({
        "choices": [{ "message": { "role": "assistant", "content": "答案", "reasoning_content": "想" } }]
    });
    assert!(strip_thinking_in_body(&mut body, &Protocol::OpenAI));
    assert!(body["choices"][0]["message"].get("reasoning_content").is_none());
    assert_eq!(body["choices"][0]["message"]["content"], "答案");
}

#[test]
fn strip_gemini_thought_parts() {
    let mut body = json!({
        "candidates": [{ "content": { "parts": [
            { "thought": true, "text": "想" },
            { "text": "答案" }
        ]}}]
    });
    assert!(strip_thinking_in_body(&mut body, &Protocol::Gemini));
    let parts = body["candidates"][0]["content"]["parts"].as_array().unwrap();
    assert_eq!(parts.len(), 1);
    assert_eq!(parts[0]["text"], "答案");
}

// ── 流式剥离（同协议透传分支）──

#[test]
fn sse_strip_anthropic_thinking_block_and_renumber() {
    let upstream = concat!(
        "event: message_start\ndata: {\"type\":\"message_start\",\"message\":{\"id\":\"m1\"}}\n\n",
        "event: content_block_start\ndata: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"thinking\",\"thinking\":\"\"}}\n\n",
        "event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"thinking_delta\",\"thinking\":\"想\"}}\n\n",
        "event: content_block_stop\ndata: {\"type\":\"content_block_stop\",\"index\":0}\n\n",
        "event: content_block_start\ndata: {\"type\":\"content_block_start\",\"index\":1,\"content_block\":{\"type\":\"text\",\"text\":\"\"}}\n\n",
        "event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":1,\"delta\":{\"type\":\"text_delta\",\"text\":\"答案\"}}\n\n",
        "event: content_block_stop\ndata: {\"type\":\"content_block_stop\",\"index\":1}\n\n",
        "event: message_stop\ndata: {\"type\":\"message_stop\"}\n\n",
    );
    let mut s = SseThinkingStripper::new(Protocol::Anthropic);
    let out = format!("{}{}", s.push(upstream), s.finish());

    assert!(!out.contains("thinking"), "思维链帧全丢: {out}");
    assert!(out.contains("\"text_delta\""), "正文保留");
    // text 块被重编号到 0（原 1），客户端按 index 装配无空洞
    assert!(out.contains("\"index\":0"), "重编号到 0: {out}");
    assert!(!out.contains("\"index\":1"), "不得留原 index 1: {out}");
    assert!(out.contains("message_start") && out.contains("message_stop"));
}

#[test]
fn sse_strip_handles_frame_split_across_chunks() {
    let mut s = SseThinkingStripper::new(Protocol::Anthropic);
    let mut out = s.push("event: content_block_start\ndata: {\"type\":\"content_block_start\",\"index\":0,\"content_bl");
    out.push_str(&s.push("ock\":{\"type\":\"thinking\"}}\n\nevent: content_block_start\ndata: {\"type\":\"content_block_start\",\"index\":1,\"content_block\":{\"type\":\"text\"}}\n\n"));
    out.push_str(&s.finish());
    assert!(!out.contains("thinking"), "被 chunk 切断的 thinking 帧也要丢: {out}");
    assert!(out.contains("\"type\":\"text\""));
}

#[test]
fn sse_strip_openai_reasoning_delta_frames() {
    let upstream = concat!(
        "data: {\"choices\":[{\"index\":0,\"delta\":{\"reasoning_content\":\"想\"}}]}\n\n",
        "data: {\"choices\":[{\"index\":0,\"delta\":{\"content\":\"答\"}}]}\n\n",
        "data: [DONE]\n\n",
    );
    let mut s = SseThinkingStripper::new(Protocol::OpenAI);
    let out = format!("{}{}", s.push(upstream), s.finish());
    assert!(!out.contains("reasoning_content"), "思维链 delta 丢: {out}");
    assert!(out.contains("\"content\":\"答\""), "正文 delta 保留");
    assert!(out.contains("[DONE]"), "终止哨兵原样过: {out}");
}

#[test]
fn sse_strip_passes_through_when_no_thinking() {
    let upstream = "event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"hi\"}}\n\n";
    let mut s = SseThinkingStripper::new(Protocol::Anthropic);
    let out = format!("{}{}", s.push(upstream), s.finish());
    assert!(out.contains("text_delta") && out.contains("hi"));
}
