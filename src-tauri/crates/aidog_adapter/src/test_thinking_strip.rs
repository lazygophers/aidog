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
    assert!(
        !strip_thinking_in_body(&mut body, &Protocol::Anthropic),
        "无思维链不算改动"
    );
}

#[test]
fn strip_openai_chat_reasoning_content() {
    let mut body = json!({
        "choices": [{ "message": {
            "role": "assistant",
            "content": "答案",
            "reasoning_content": "思考（deepseek-style）",
            "reasoning": "思考（StepFun general）"
        } }]
    });
    assert!(strip_thinking_in_body(&mut body, &Protocol::OpenAI));
    assert!(
        body["choices"][0]["message"]
            .get("reasoning_content")
            .is_none()
    );
    assert!(
        body["choices"][0]["message"]
            .get("reasoning")
            .is_none()
    );
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
    let parts = body["candidates"][0]["content"]["parts"]
        .as_array()
        .unwrap();
    assert_eq!(parts.len(), 1);
    assert_eq!(parts[0]["text"], "答案");
}

// ── 过滤后无内容判定（非流式）──

/// thinking-only 上游：剥完剩空 text 兜底块 → 判无可下发内容。
#[test]
fn contentless_after_strip_thinking_only() {
    let mut b = json!({
        "content": [{ "type": "thinking", "thinking": "想", "signature": "s" }],
        "stop_reason": "end_turn"
    });
    assert!(strip_thinking_in_body(&mut b, &Protocol::Anthropic));
    assert!(response_contentless_after_strip(&b, &Protocol::Anthropic));
}

/// 混合响应（thinking + text + tool_use）剥完仍有正文与工具调用 → 不误判空。
#[test]
fn content_not_misjudged_with_text_and_tool_use() {
    let mut b = json!({
        "content": [
            { "type": "thinking", "thinking": "想" },
            { "type": "text", "text": "答案" },
            { "type": "tool_use", "id": "t1", "name": "ls", "input": {} }
        ]
    });
    assert!(strip_thinking_in_body(&mut b, &Protocol::Anthropic));
    assert!(!response_contentless_after_strip(&b, &Protocol::Anthropic));
}

/// 空 text（纯空白）不算可下发内容；OpenAI 客户端协议恒 false（无独立 content 数组形态）。
#[test]
fn contentless_dispatch_and_blank_text() {
    let mut b = json!({
        "content": [
            { "type": "thinking", "thinking": "想" },
            { "type": "text", "text": "  " }
        ]
    });
    assert!(strip_thinking_in_body(&mut b, &Protocol::Anthropic));
    assert!(response_contentless_after_strip(&b, &Protocol::Anthropic));
    assert!(!response_contentless_after_strip(&b, &Protocol::OpenAI));
}

/// legacy message.function_call 同 tool_calls：不得判无可下发内容。
#[test]
fn contentless_openai_function_call_not_misjudged() {
    let b = json!({
        "choices": [{ "message": {
            "role": "assistant",
            "function_call": { "name": "ls", "arguments": "{}" }
        }}]
    });
    assert!(!response_contentless_after_strip(&b, &Protocol::OpenAI));
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
    assert!(
        !out.contains("thinking"),
        "被 chunk 切断的 thinking 帧也要丢: {out}"
    );
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

/// legacy function_call delta 也是有效输出：终止帧不得误报 thinking-only。
#[test]
fn sse_strip_openai_function_call_delta_counts_as_content() {
    let upstream = concat!(
        "data: {\"choices\":[{\"index\":0,\"delta\":{\"reasoning_content\":\"想\"}}]}\n\n",
        "data: {\"choices\":[{\"index\":0,\"delta\":{\"function_call\":{\"name\":\"ls\",\"arguments\":\"{}\"}}}]}\n\n",
        "data: {\"choices\":[{\"index\":0,\"delta\":{},\"finish_reason\":\"tool_calls\"}]}\n\n",
        "data: [DONE]\n\n",
    );
    let mut s = SseThinkingStripper::new(Protocol::OpenAI);
    let out = format!("{}{}", s.push(upstream), s.finish());
    assert!(out.contains("function_call"), "function_call delta 保留: {out}");
    assert!(
        !out.contains("thinking-only response"),
        "不得换 error 帧: {out}"
    );
}

#[test]
fn sse_strip_passes_through_when_no_thinking() {
    let upstream = "event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"hi\"}}\n\n";
    let mut s = SseThinkingStripper::new(Protocol::Anthropic);
    let out = format!("{}{}", s.push(upstream), s.finish());
    assert!(out.contains("text_delta") && out.contains("hi"));
}

// ── 过滤后既无正文也无工具调用 → 明确报错（保留上游终止原因）──

const SSE_THINKING_ONLY_STREAM: &str = concat!(
    "event: message_start\ndata: {\"type\":\"message_start\",\"message\":{\"id\":\"m1\",\"model\":\"m\"}}\n\n",
    "event: content_block_start\ndata: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"thinking\",\"thinking\":\"\"}}\n\n",
    "event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"thinking_delta\",\"thinking\":\"让我分析…\"}}\n\n",
    "event: content_block_stop\ndata: {\"type\":\"content_block_stop\",\"index\":0}\n\n",
    "event: content_block_start\ndata: {\"type\":\"content_block_start\",\"index\":1,\"content_block\":{\"type\":\"text\",\"text\":\"\"}}\n\n",
    "event: content_block_stop\ndata: {\"type\":\"content_block_stop\",\"index\":1}\n\n",
    "event: message_delta\ndata: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"end_turn\",\"stop_sequence\":null},\"usage\":{\"output_tokens\":9}}\n\n",
    "event: message_stop\ndata: {\"type\":\"message_stop\"}\n\n",
);

/// thinking-only 上游流：message_stop 被换成 error 帧报错，保留上游 stop_reason，思考文本不泄漏。
#[test]
fn sse_strip_thinking_only_replaces_message_stop_with_error() {
    let mut s = SseThinkingStripper::new(Protocol::Anthropic);
    let out = format!("{}{}", s.push(SSE_THINKING_ONLY_STREAM), s.finish());
    assert!(
        !out.contains("message_stop"),
        "message_stop 不得下发: {out}"
    );
    assert!(out.contains("event: error"), "必须报错: {out}");
    assert!(out.contains("\"type\":\"api_error\""), "error 类型: {out}");
    assert!(
        out.contains("upstream stop_reason: end_turn"),
        "保留上游终止原因: {out}"
    );
    assert!(
        !out.contains("让我分析"),
        "思考文本不泄漏: {out}"
    );
}

/// thinking 剔光后只剩空 text 块：仍按无内容处理（报错），不以空 text 块冒充正文。
#[test]
fn sse_strip_empty_text_block_not_counted_as_content() {
    let mut s = SseThinkingStripper::new(Protocol::Anthropic);
    let out = format!("{}{}", s.push(SSE_THINKING_ONLY_STREAM), s.finish());
    assert!(out.contains("event: error"), "空 text 兜底块不构成正文: {out}");
}

/// thinking + tool_use 流：有工具调用 → 不误报，message_stop 原样下发。
#[test]
fn sse_strip_tool_use_not_misreported_as_empty() {
    let upstream = concat!(
        "event: message_start\ndata: {\"type\":\"message_start\",\"message\":{\"id\":\"m1\",\"model\":\"m\"}}\n\n",
        "event: content_block_start\ndata: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"thinking\",\"thinking\":\"\"}}\n\n",
        "event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"thinking_delta\",\"thinking\":\"想\"}}\n\n",
        "event: content_block_stop\ndata: {\"type\":\"content_block_stop\",\"index\":0}\n\n",
        "event: content_block_start\ndata: {\"type\":\"content_block_start\",\"index\":1,\"content_block\":{\"type\":\"tool_use\",\"id\":\"t1\",\"name\":\"ls\",\"input\":{}}}\n\n",
        "event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":1,\"delta\":{\"type\":\"input_json_delta\",\"partial_json\":\"{}\"}}\n\n",
        "event: content_block_stop\ndata: {\"type\":\"content_block_stop\",\"index\":1}\n\n",
        "event: message_delta\ndata: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"tool_use\"},\"usage\":{\"output_tokens\":5}}\n\n",
        "event: message_stop\ndata: {\"type\":\"message_stop\"}\n\n",
    );
    let mut s = SseThinkingStripper::new(Protocol::Anthropic);
    let out = format!("{}{}", s.push(upstream), s.finish());
    assert!(!out.contains("thinking"), "思维链帧全丢: {out}");
    assert!(out.contains("\"type\":\"tool_use\""), "工具调用保留: {out}");
    assert!(
        out.contains("event: message_stop"),
        "有内容不报错: {out}"
    );
    assert!(!out.contains("event: error"), "不误报: {out}");
}

// ── G1：非流式 contentless 判定补 OpenAI chat / OpenAI Responses / Gemini ──

/// OpenAI chat：只剩 reasoning_content（content 空、无 tool_calls）→ 判无可下发。
#[test]
fn contentless_after_strip_openai_chat_reasoning_only() {
    let mut b = json!({
        "choices": [{ "message": { "role": "assistant", "content": serde_json::Value::Null, "reasoning_content": "想" } }]
    });
    assert!(strip_thinking_in_body(&mut b, &Protocol::OpenAI));
    assert!(response_contentless_after_strip(&b, &Protocol::OpenAI));
}

/// OpenAI chat：有正文或 tool_calls → 不误判。
#[test]
fn content_not_misjudged_openai_chat_text_and_tool_calls() {
    let mut b = json!({
        "choices": [{ "message": { "role": "assistant", "content": "答案", "reasoning_content": "想" } }]
    });
    assert!(strip_thinking_in_body(&mut b, &Protocol::OpenAI));
    assert!(!response_contentless_after_strip(&b, &Protocol::OpenAI));

    let mut b = json!({
        "choices": [{ "message": { "role": "assistant", "content": serde_json::Value::Null,
            "reasoning_content": "想",
            "tool_calls": [{ "id": "t1", "type": "function", "function": { "name": "ls", "arguments": "{}" } }] } }]
    });
    assert!(strip_thinking_in_body(&mut b, &Protocol::OpenAI));
    assert!(!response_contentless_after_strip(&b, &Protocol::OpenAI), "有 tool_calls 不得判空");
}

/// OpenAI Responses：output 只剩 reasoning 项 → 判空；有 message 项 → 不判空。
#[test]
fn contentless_after_strip_openai_responses_reasoning_only() {
    let mut b = json!({
        "output": [
            { "type": "reasoning", "summary": [] },
            { "type": "reasoning", "summary": [{ "type": "summary_text", "text": "想" }] }
        ]
    });
    assert!(strip_thinking_in_body(&mut b, &Protocol::OpenAIResponses));
    assert!(response_contentless_after_strip(&b, &Protocol::OpenAIResponses));
}

#[test]
fn content_not_misjudged_openai_responses_with_message() {
    let mut b = json!({
        "output": [
            { "type": "reasoning", "summary": [] },
            { "type": "message", "role": "assistant", "content": [{ "type": "output_text", "text": "答案" }] }
        ]
    });
    assert!(strip_thinking_in_body(&mut b, &Protocol::OpenAIResponses));
    assert!(!response_contentless_after_strip(&b, &Protocol::OpenAIResponses));
}

/// Gemini：parts 全是 thought → 判空；有 text / functionCall part → 不判空。
#[test]
fn contentless_after_strip_gemini_thought_only() {
    let mut b = json!({
        "candidates": [{ "content": { "parts": [ { "thought": true, "text": "想" } ] } }]
    });
    assert!(strip_thinking_in_body(&mut b, &Protocol::Gemini));
    assert!(response_contentless_after_strip(&b, &Protocol::Gemini));
}

#[test]
fn content_not_misjudged_gemini_text_and_function_call() {
    let mut b = json!({
        "candidates": [{ "content": { "parts": [
            { "thought": true, "text": "想" },
            { "text": "答案" }
        ]}}]
    });
    assert!(strip_thinking_in_body(&mut b, &Protocol::Gemini));
    assert!(!response_contentless_after_strip(&b, &Protocol::Gemini));

    let mut b = json!({
        "candidates": [{ "content": { "parts": [
            { "thought": true, "text": "想" },
            { "functionCall": { "name": "ls", "args": {} } }
        ]}}]
    });
    assert!(strip_thinking_in_body(&mut b, &Protocol::Gemini));
    assert!(!response_contentless_after_strip(&b, &Protocol::Gemini), "有 functionCall 不得判空");
}

// ── G2：流式透传 error 兑换补 OpenAI / OpenAI Responses / Gemini 三臂 ──

/// OpenAI chat thinking-only 流：finish_reason 帧（finish_reason=stop）即换 error 帧，
/// 自带 [DONE]，reasoning_content 不泄漏；converted_to_error 置位。
#[test]
fn sse_strip_openai_thinking_only_converts_finish_to_error() {
    let upstream = concat!(
        "data: {\"id\":\"c1\",\"choices\":[{\"index\":0,\"delta\":{\"role\":\"assistant\",\"reasoning_content\":\"让我分析…\"},\"finish_reason\":null}]}\n\n",
        "data: {\"id\":\"c1\",\"choices\":[{\"index\":0,\"delta\":{},\"finish_reason\":\"stop\"}]}\n\n",
        "data: [DONE]\n\n",
    );
    let mut s = SseThinkingStripper::new(Protocol::OpenAI);
    let out = format!("{}{}", s.push(upstream), s.finish());
    assert!(!out.contains("reasoning_content"), "思维链不泄漏: {out}");
    assert!(
        !out.contains("\"finish_reason\":\"stop\""),
        "finish 帧换 error: {out}"
    );
    assert!(out.contains("\"error\""), "须报错: {out}");
    assert!(out.contains("[DONE]"), "error 帧自带 [DONE] 终止: {out}");
    assert!(out.contains("upstream stop_reason: stop"), "保留上游终止原因: {out}");
    assert!(!out.contains("让我分析"), "思考文本不泄漏: {out}");
    assert!(s.converted_to_error(), "converted_to_error 置位（finish 层记 502）");
}

/// OpenAI chat：无 finish_reason 帧直接 [DONE]（thinking-only）→ [DONE] 处换 error。
#[test]
fn sse_strip_openai_thinking_only_converts_done_sentinel() {
    let upstream = concat!(
        "data: {\"id\":\"c1\",\"choices\":[{\"index\":0,\"delta\":{\"reasoning_content\":\"想\"}}]}\n\n",
        "data: [DONE]\n\n",
    );
    let mut s = SseThinkingStripper::new(Protocol::OpenAI);
    let out = format!("{}{}", s.push(upstream), s.finish());
    assert!(out.contains("\"error\""), "[DONE] 处须报错: {out}");
    assert_eq!(out.matches("[DONE]").count(), 1, "只一个 [DONE]: {out}");
    assert!(s.converted_to_error());
}

/// OpenAI chat：有正文 → 不报错，[DONE] 原样过。
#[test]
fn sse_strip_openai_with_content_no_error() {
    let upstream = concat!(
        "data: {\"id\":\"c1\",\"choices\":[{\"index\":0,\"delta\":{\"reasoning_content\":\"想\"}}]}\n\n",
        "data: {\"id\":\"c1\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\"答\"}}]}\n\n",
        "data: {\"id\":\"c1\",\"choices\":[{\"index\":0,\"delta\":{},\"finish_reason\":\"stop\"}]}\n\n",
        "data: [DONE]\n\n",
    );
    let mut s = SseThinkingStripper::new(Protocol::OpenAI);
    let out = format!("{}{}", s.push(upstream), s.finish());
    assert!(out.contains("\"content\":\"答\""), "正文保留: {out}");
    assert!(out.contains("\"finish_reason\":\"stop\""), "finish 帧原样: {out}");
    assert!(!out.contains("\"error\""), "有内容不报错: {out}");
    assert!(!s.converted_to_error());
}

/// OpenAI chat：tool_calls 流 → 不误报。
#[test]
fn sse_strip_openai_tool_calls_no_error() {
    let upstream = concat!(
        "data: {\"id\":\"c1\",\"choices\":[{\"index\":0,\"delta\":{\"reasoning_content\":\"想\"}}]}\n\n",
        "data: {\"id\":\"c1\",\"choices\":[{\"index\":0,\"delta\":{\"tool_calls\":[{\"index\":0,\"id\":\"t1\",\"function\":{\"name\":\"ls\",\"arguments\":\"{}\"}}]}}]}\n\n",
        "data: {\"id\":\"c1\",\"choices\":[{\"index\":0,\"delta\":{},\"finish_reason\":\"tool_calls\"}]}\n\n",
        "data: [DONE]\n\n",
    );
    let mut s = SseThinkingStripper::new(Protocol::OpenAI);
    let out = format!("{}{}", s.push(upstream), s.finish());
    assert!(out.contains("\"tool_calls\""), "工具调用保留: {out}");
    assert!(!out.contains("\"error\""), "有工具调用不报错: {out}");
    assert!(!s.converted_to_error());
}

/// OpenAI Responses thinking-only 流：reasoning 事件全丢，response.completed 换 error 帧。
#[test]
fn sse_strip_responses_thinking_only_converts_completed_to_error() {
    let upstream = concat!(
        "data: {\"type\":\"response.created\",\"response\":{\"id\":\"r1\"}}\n\n",
        "data: {\"type\":\"response.output_item.added\",\"output_index\":0,\"item\":{\"type\":\"reasoning\"}}\n\n",
        "data: {\"type\":\"response.reasoning_summary_text.delta\",\"delta\":\"让我分析…\"}\n\n",
        "data: {\"type\":\"response.completed\",\"response\":{\"id\":\"r1\",\"status\":\"completed\"}}\n\n",
    );
    let mut s = SseThinkingStripper::new(Protocol::OpenAIResponses);
    let out = format!("{}{}", s.push(upstream), s.finish());
    assert!(
        !out.contains("reasoning_summary"),
        "reasoning 增量事件全丢: {out}"
    );
    assert!(out.contains("\"error\""), "completed 处须报错: {out}");
    assert!(!out.contains("让我分析"), "思考文本不泄漏: {out}");
    assert!(s.converted_to_error());
}

/// OpenAI Responses：有 output_text.delta → 不报错，completed 原样过。
#[test]
fn sse_strip_responses_with_content_no_error() {
    let upstream = concat!(
        "data: {\"type\":\"response.output_item.added\",\"output_index\":0,\"item\":{\"type\":\"reasoning\"}}\n\n",
        "data: {\"type\":\"response.output_text.delta\",\"delta\":\"答\"}\n\n",
        "data: {\"type\":\"response.completed\",\"response\":{\"id\":\"r1\",\"status\":\"completed\"}}\n\n",
    );
    let mut s = SseThinkingStripper::new(Protocol::OpenAIResponses);
    let out = format!("{}{}", s.push(upstream), s.finish());
    assert!(out.contains("output_text.delta"), "正文事件保留: {out}");
    assert!(out.contains("response.completed"), "completed 原样: {out}");
    assert!(!out.contains("\"error\""), "有内容不报错: {out}");
    assert!(!s.converted_to_error());
}

/// Gemini thinking-only 流：parts 被剔空且 finishReason 帧到达 → 换 error 帧。
#[test]
fn sse_strip_gemini_thinking_only_converts_finish_reason_to_error() {
    let upstream = concat!(
        "data: {\"candidates\":[{\"content\":{\"parts\":[{\"thought\":true,\"text\":\"让我分析…\"}]}}]}\n\n",
        "data: {\"candidates\":[{\"content\":{\"parts\":[]},\"finishReason\":\"STOP\"}],\"usageMetadata\":{\"candidatesTokenCount\":9}}\n\n",
    );
    let mut s = SseThinkingStripper::new(Protocol::Gemini);
    let out = format!("{}{}", s.push(upstream), s.finish());
    assert!(!out.contains("让我分析"), "思考文本不泄漏: {out}");
    assert!(!out.contains("finishReason"), "finishReason 帧换 error: {out}");
    assert!(out.contains("\"error\""), "须报错: {out}");
    assert!(s.converted_to_error());
}

/// Gemini：有正文 part → 不报错，finishReason 帧原样过。
#[test]
fn sse_strip_gemini_with_content_no_error() {
    let upstream = concat!(
        "data: {\"candidates\":[{\"content\":{\"parts\":[{\"thought\":true,\"text\":\"想\"}]}}]}\n\n",
        "data: {\"candidates\":[{\"content\":{\"parts\":[{\"text\":\"答案\"}]},\"finishReason\":\"STOP\"}]}\n\n",
    );
    let mut s = SseThinkingStripper::new(Protocol::Gemini);
    let out = format!("{}{}", s.push(upstream), s.finish());
    assert!(out.contains("答案"), "正文保留: {out}");
    assert!(out.contains("finishReason"), "finishReason 帧原样: {out}");
    assert!(!out.contains("\"error\""), "有内容不报错: {out}");
    assert!(!s.converted_to_error());
}

/// Gemini：functionCall part → 不误报。
#[test]
fn sse_strip_gemini_function_call_no_error() {
    let upstream = concat!(
        "data: {\"candidates\":[{\"content\":{\"parts\":[{\"thought\":true,\"text\":\"想\"}]}}]}\n\n",
        "data: {\"candidates\":[{\"content\":{\"parts\":[{\"functionCall\":{\"name\":\"ls\",\"args\":{}}}]}},\"finishReason\":\"STOP\"}]}\n\n",
    );
    let mut s = SseThinkingStripper::new(Protocol::Gemini);
    let out = format!("{}{}", s.push(upstream), s.finish());
    assert!(out.contains("functionCall"), "工具调用保留: {out}");
    assert!(!out.contains("\"error\""), "有工具调用不报错: {out}");
    assert!(!s.converted_to_error());
}

// ── G3：无终止哨兵（Gemini）末帧不带空行结尾时由 finish() 冲出，不被吞 ──

#[test]
fn sse_strip_gemini_unterminated_tail_flushed_by_finish() {
    let mut s = SseThinkingStripper::new(Protocol::Gemini);
    let mut out = s.push("data: {\"candidates\":[{\"content\":{\"parts\":[{\"text\":\"尾帧\"}]}}]}\n\n");
    out.push_str(&s.push("data: {\"candidates\":[{\"content\":{\"parts\":[{\"text\":\"答案\"}]}}]}"));
    out.push_str(&s.finish());
    assert!(out.contains("答案"), "无空行结尾的末帧不得被吞: {out}");
}
