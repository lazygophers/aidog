//! issue 70：Completions 流式渲染器测试。全部本地构造事件，无真实外呼。

use super::*;
use crate::types::ChatStreamEvent;

#[test]
fn completions_stream_text_chunk() {
    let out = to_completions_sse(
        &ChatStreamEvent::Delta { text: "Hi".into() },
        "gpt-5",
    )
    .unwrap();
    assert!(out.contains("\"object\":\"text_completion\""), "实得 {out:?}");
    assert!(out.contains("\"text\":\"Hi\""), "实得 {out:?}");
    assert!(!out.contains("chat.completion.chunk"), "不得渲染成 Chat 分块: {out:?}");
}

#[test]
fn completions_stream_start_and_stop() {
    let start = to_completions_sse(
        &ChatStreamEvent::Start { id: "cmpl-1".into(), model: "gpt-5".into() },
        "gpt-5",
    )
    .unwrap();
    assert!(start.contains("\"id\":\"cmpl-1\""), "实得 {start:?}");

    let stop = to_completions_sse(
        &ChatStreamEvent::Stop { finish_reason: Some("end_turn".into()) },
        "gpt-5",
    )
    .unwrap();
    assert!(stop.contains("\"finish_reason\":\"stop\""), "实得 {stop:?}");
    assert!(stop.contains("data: [DONE]"), "实得 {stop:?}");

    // max_tokens → length
    let stop = to_completions_sse(
        &ChatStreamEvent::Stop { finish_reason: Some("max_tokens".into()) },
        "gpt-5",
    )
    .unwrap();
    assert!(stop.contains("\"finish_reason\":\"length\""), "实得 {stop:?}");
}

#[test]
fn completions_client_dispatch_renders_text_completion() {
    let out = crate::converter::to_client_sse(
        &ChatStreamEvent::Delta { text: "Hi".into() },
        &aidog_db::models::Protocol::OpenAICompletions,
        "gpt-5",
    )
    .unwrap();
    assert!(out.contains("\"object\":\"text_completion\""), "实得 {out:?}");
}
