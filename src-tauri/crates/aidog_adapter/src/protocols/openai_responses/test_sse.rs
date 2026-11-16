//! issue 70：Responses 流式渲染器测试。全部本地构造事件，无真实外呼。

use super::*;
use crate::types::ChatStreamEvent;
use serde_json::{Value, json};

/// 解析拼好的 SSE 文本为 (event 名, data JSON) 序列。
fn frames(text: &str) -> Vec<(String, Value)> {
    let mut parsed = Vec::new();
    let mut event = String::new();
    for line in text.lines() {
        if let Some(e) = line.strip_prefix("event: ") {
            event = e.to_string();
        } else if let Some(d) = line.strip_prefix("data: ") {
            parsed.push((event.clone(), serde_json::from_str::<Value>(d).unwrap()));
            event = String::new();
        }
    }
    parsed
}

fn types(frames: &[(String, Value)]) -> Vec<&str> {
    frames.iter().map(|(e, _)| e.as_str()).collect()
}

#[test]
fn responses_stream_text_full_sequence() {
    let mut st = ResponsesSseState::default();
    let mut out = String::new();
    out.push_str(&st.push(&ChatStreamEvent::Start { id: "chatcmpl-1".into(), model: "gpt-5".into() }, "gpt-5").unwrap());
    out.push_str(&st.push(&ChatStreamEvent::Delta { text: "He".into() }, "gpt-5").unwrap());
    out.push_str(&st.push(&ChatStreamEvent::Delta { text: "llo".into() }, "gpt-5").unwrap());
    out.push_str(&st.push(&ChatStreamEvent::Stop { finish_reason: Some("end_turn".into()) }, "gpt-5").unwrap());

    let f = frames(&out);
    let names = types(&f);
    assert_eq!(names.first().copied(), Some("response.created"), "首帧 response.created: {names:?}");
    // 官方序列：created 之后紧跟 in_progress
    assert_eq!(names.get(1).copied(), Some("response.in_progress"), "created 后须为 in_progress: {names:?}");
    assert_eq!(names.last().copied(), Some("response.completed"), "末帧 response.completed: {names:?}");
    // item 生命周期：added 恰一次，delta 两次
    assert_eq!(names.iter().filter(|n| **n == "response.output_item.added").count(), 1, "{names:?}");
    assert_eq!(names.iter().filter(|n| **n == "response.content_part.added").count(), 1, "{names:?}");
    assert_eq!(names.iter().filter(|n| **n == "response.output_text.delta").count(), 2, "{names:?}");
    // 官方序列：output_text.done 在最后一个 delta 之后、content_part.done 之前
    let text_done_pos = names.iter().position(|n| *n == "response.output_text.done").expect("缺 output_text.done: {names:?}");
    let last_delta_pos = names.iter().rposition(|n| *n == "response.output_text.delta").unwrap();
    let part_done_pos = names.iter().position(|n| *n == "response.content_part.done").unwrap();
    assert!(text_done_pos > last_delta_pos, "output_text.done 须在最后一个 delta 之后: {names:?}");
    assert!(text_done_pos < part_done_pos, "output_text.done 须在 content_part.done 之前: {names:?}");
    let text_done = &f[text_done_pos].1;
    assert_eq!(text_done["text"], "Hello", "done 帧带全量文本");
    // openai-python 必填 logprobs：response_text_delta_event.py / response_text_done_event.py 无默认值，缺字段严格反序列化拒帧
    for (e, d) in f.iter().filter(|(e, _)| e == "response.output_text.delta" || e == "response.output_text.done") {
        assert!(d["logprobs"].is_array(), "{e} 缺必填 logprobs 数组: {d}");
    }

    let created = &f[0].1["response"];
    assert_eq!(created["id"], "chatcmpl-1");
    assert_eq!(created["model"], "gpt-5");
    assert_eq!(created["status"], "in_progress");
    // Response 类型必填 created_at（openai-python response.py）
    let created_at = created["created_at"].as_i64().expect("created 缺 created_at");
    assert!(created_at > 0, "created_at 须为正 unix 秒");

    // sequence_number 严格递增
    for (i, (_, d)) in f.iter().enumerate() {
        assert_eq!(d["sequence_number"], json!(i as u64), "第 {i} 帧 sequence_number");
    }

    // completed 回放全量文本 + created_at 复用
    let completed = f.last().unwrap().1["response"].clone();
    assert_eq!(completed["status"], "completed");
    assert_eq!(completed["output"][0]["type"], "message");
    assert_eq!(completed["output"][0]["content"][0]["text"], "Hello");
    assert_eq!(completed["created_at"], created_at, "completed 复用 created_at");
    // output_item.done 帧带全量文本
    let done = f.iter().find(|(e, _)| e == "response.output_item.done").unwrap();
    assert_eq!(done.1["item"]["content"][0]["text"], "Hello");
}

#[test]
fn responses_stream_reasoning_summary_delta() {
    let mut st = ResponsesSseState::default();
    st.push(&ChatStreamEvent::Start { id: "c1".into(), model: "m".into() }, "m");
    let out = st.push(&ChatStreamEvent::ReasoningDelta { text: "思考".into() }, "m").unwrap();
    let f = frames(&out);
    let names = types(&f);
    assert!(names.contains(&"response.reasoning_summary_text.delta"), "{names:?}");
    // 官方序列：summary_part.added 在首个 summary delta 之前
    let part_added_pos = names.iter().position(|n| *n == "response.reasoning_summary_part.added")
        .expect("缺 reasoning_summary_part.added: {names:?}");
    let delta_pos = names.iter().position(|n| *n == "response.reasoning_summary_text.delta").unwrap();
    assert!(part_added_pos < delta_pos, "summary_part.added 须在首个 summary delta 之前: {names:?}");
    let part_added = &f[part_added_pos].1;
    assert_eq!(part_added["summary_index"], 0);
    assert_eq!(part_added["part"]["type"], "summary_text");
    let delta = f.iter().find(|(e, _)| e == "response.reasoning_summary_text.delta").unwrap();
    assert_eq!(delta.1["delta"], "思考");
    assert_eq!(delta.1["summary_index"], 0);
    let added = f.iter().find(|(e, _)| e == "response.output_item.added").unwrap();
    assert_eq!(added.1["item"]["type"], "reasoning");
    // 终止序列：summary_text.done → summary_part.done → output_item.done，全文回放进 summary
    let out = st.push(&ChatStreamEvent::Stop { finish_reason: Some("stop".into()) }, "m").unwrap();
    let f = frames(&out);
    let names = types(&f);
    let text_done_pos = names.iter().position(|n| *n == "response.reasoning_summary_text.done")
        .expect("缺 reasoning_summary_text.done: {names:?}");
    let part_done_pos = names.iter().position(|n| *n == "response.reasoning_summary_part.done")
        .expect("缺 reasoning_summary_part.done: {names:?}");
    let item_done_pos = names.iter().position(|n| *n == "response.output_item.done").unwrap();
    assert!(text_done_pos < part_done_pos && part_done_pos < item_done_pos, "终止顺序须 text.done→part.done→item.done: {names:?}");
    assert_eq!(f[text_done_pos].1["text"], "思考");
    assert_eq!(f[part_done_pos].1["part"]["text"], "思考");
    let done = f.iter().find(|(e, _)| e == "response.output_item.done").unwrap();
    assert_eq!(done.1["item"]["summary"][0]["text"], "思考");
}

#[test]
fn responses_stream_function_call_arguments() {
    let mut st = ResponsesSseState::default();
    st.push(&ChatStreamEvent::Start { id: "c1".into(), model: "m".into() }, "m");
    let mut out = st.push(&ChatStreamEvent::ToolDelta {
        index: 0, id: Some("call_1".into()), name: Some("get_weather".into()), input: None,
    }, "m").unwrap();
    out.push_str(&st.push(&ChatStreamEvent::ToolDelta {
        index: 0, id: None, name: None, input: Some("{\"ci".into()),
    }, "m").unwrap());
    out.push_str(&st.push(&ChatStreamEvent::ToolDelta {
        index: 0, id: None, name: None, input: Some("ty\":\"SF\"}".into()),
    }, "m").unwrap());
    out.push_str(&st.push(&ChatStreamEvent::Stop { finish_reason: Some("tool_calls".into()) }, "m").unwrap());

    let f = frames(&out);
    let names = types(&f);
    assert!(names.contains(&"response.function_call_arguments.delta"), "{names:?}");
    assert_eq!(names.iter().filter(|n| **n == "response.function_call_arguments.delta").count(), 2);
    // 官方序列：function_call_arguments.done 在最后一个参数 delta 之后、output_item.done 之前
    let args_done_pos = names.iter().position(|n| *n == "response.function_call_arguments.done")
        .expect("缺 function_call_arguments.done: {names:?}");
    let last_delta_pos = names.iter().rposition(|n| *n == "response.function_call_arguments.delta").unwrap();
    let item_done_pos = names.iter().position(|n| *n == "response.output_item.done").unwrap();
    assert!(args_done_pos > last_delta_pos && args_done_pos < item_done_pos, "done 须在最后 delta 之后、item.done 之前: {names:?}");
    assert_eq!(f[args_done_pos].1["arguments"], "{\"city\":\"SF\"}", "done 帧带全量 arguments");

    let added = f.iter().find(|(e, _)| e == "response.output_item.added").unwrap();
    assert_eq!(added.1["item"]["type"], "function_call");
    assert_eq!(added.1["item"]["call_id"], "call_1");
    assert_eq!(added.1["item"]["name"], "get_weather");

    // completed 回放全量 arguments
    let completed = f.last().unwrap().1["response"].clone();
    assert_eq!(completed["output"][0]["arguments"], "{\"city\":\"SF\"}");
    assert_eq!(completed["status"], "completed");
    assert!(st.push(&ChatStreamEvent::Stop { finish_reason: Some("end_turn".into()) }, "m").is_none());
}

#[test]
fn responses_stream_incomplete_and_usage() {
    let mut st = ResponsesSseState::default();
    st.push(&ChatStreamEvent::Start { id: "c1".into(), model: "m".into() }, "m");
    st.push(&ChatStreamEvent::Usage { usage: crate::types::Usage {
        prompt_tokens: Some(3), completion_tokens: Some(4), total_tokens: Some(7),
        cache_read_tokens: None, cache_write_tokens: None,
    } }, "m");
    let out = st.push(&ChatStreamEvent::Stop { finish_reason: Some("length".into()) }, "m").unwrap();
    let f = frames(&out);
    assert_eq!(types(&f).last().copied(), Some("response.incomplete"), "{:?}", types(&f));
    let resp = &f.last().unwrap().1["response"];
    assert_eq!(resp["status"], "incomplete");
    assert_eq!(resp["usage"]["input_tokens"], 3);
    assert_eq!(resp["usage"]["output_tokens"], 4);
    assert_eq!(resp["usage"]["total_tokens"], 7);
    // incomplete 须带 incomplete_details.reason（官方 Response 类型字段）
    assert_eq!(resp["incomplete_details"]["reason"], "max_output_tokens", "缺 incomplete_details.reason");
    // created_at 在 incomplete 终帧同样在场
    assert!(resp["created_at"].as_i64().unwrap_or(0) > 0, "incomplete 缺 created_at");
}

#[test]
fn stateful_dispatch_switches_to_responses_state() {
    use crate::converter::{ClientSseState, to_client_sse_stateful};
    let mut state = ClientSseState::default();
    let out = to_client_sse_stateful(
        &ChatStreamEvent::Delta { text: "Hi".into() },
        &mut state,
        &aidog_db::models::Protocol::OpenAIResponses,
        "m",
        false,
    )
    .expect("Responses 客户端 stateful 渲染");
    assert!(out.contains("response.output_text.delta"), "实得 {out:?}");
    // 状态已换轨为 Responses，下一帧继续同一 item 生命周期
    let out = to_client_sse_stateful(
        &ChatStreamEvent::Delta { text: "!".into() },
        &mut state,
        &aidog_db::models::Protocol::OpenAIResponses,
        "m",
        false,
    )
    .unwrap();
    let f = frames(&out);
    assert_eq!(
        f.iter().filter(|(e, _)| e == "response.output_item.added").count(),
        0,
        "第二帧不得重开 item: {out:?}"
    );
    // Anthropic 客户端仍走原状态机
    let mut state = ClientSseState::default();
    let out = to_client_sse_stateful(
        &ChatStreamEvent::Delta { text: "Hi".into() },
        &mut state,
        &aidog_db::models::Protocol::Anthropic,
        "m",
        false,
    )
    .unwrap();
    assert!(out.contains("content_block_delta"), "实得 {out:?}");
}
