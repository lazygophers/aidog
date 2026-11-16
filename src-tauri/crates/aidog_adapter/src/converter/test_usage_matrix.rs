//! issue 71 验收矩阵：上游 wire 协议 × 客户端协议流式 usage 回传。
//!
//! 每组上游 SSE 帧文本经 `parse_upstream_sse` 解析 + `to_client_sse_stateful` 渲染 +
//! `flush`，断言客户端收到的 input / output / cache token 与上游一致。全部走本地帧文本，
//! 无真实外呼。

use super::*;
use aidog_db::models::Protocol;
use serde_json::json;

const WIRES: [Protocol; 5] = [
    Protocol::Anthropic,
    Protocol::OpenAI,
    Protocol::Gemini,
    Protocol::OpenAIResponses,
    Protocol::OpenAICompletions,
];

const CLIENTS: [Protocol; 5] = [
    Protocol::Anthropic,
    Protocol::OpenAI,
    Protocol::Gemini,
    Protocol::OpenAIResponses,
    Protocol::OpenAICompletions,
];

/// 跑一条 wire → client 流水线，返回客户端收到的 SSE 全文。
fn run_stream(wire: &Protocol, client: &Protocol, include_usage: bool) -> String {
    let text = upstream_frames(wire);
    let events = parse_upstream_sse(&text, wire);
    let mut state = ClientSseState::default();
    let mut out = String::new();
    for e in &events {
        out.push_str(
            &to_client_sse_stateful(e, &mut state, client, "m", include_usage)
                .unwrap_or_default(),
        );
    }
    out.push_str(&state.flush().unwrap_or_default());
    out
}

/// 各上游 wire 协议的代表帧序列：文本 delta + usage 帧（含 finish）。
fn upstream_frames(wire: &Protocol) -> String {
    match wire {
        Protocol::Anthropic => format!(
            "data: {}\n\ndata: {}\n\ndata: {}\n\ndata: {}\n\n",
            json!({"type":"message_start","message":{"id":"c1","type":"message","role":"assistant","model":"m","usage":{"input_tokens":10,"output_tokens":1,"cache_read_input_tokens":6,"cache_creation_input_tokens":2}}}),
            json!({"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"hi"}}),
            json!({"type":"message_delta","delta":{"stop_reason":"end_turn","stop_sequence":null},"usage":{"output_tokens":9}}),
            json!({"type":"message_stop"}),
        ),
        Protocol::Gemini => format!(
            "data: {}\n\ndata: {}\n\n",
            json!({"candidates":[{"content":{"parts":[{"text":"hi"}],"role":"model"}}]}),
            json!({"candidates":[{"finishReason":"STOP","content":{"parts":[],"role":"model"}}],"usageMetadata":{"promptTokenCount":10,"candidatesTokenCount":7,"thoughtsTokenCount":2,"cachedContentTokenCount":6,"totalTokenCount":19}}),
        ),
        Protocol::OpenAIResponses => format!(
            "data: {}\n\ndata: {}\n\ndata: {}\n\n",
            json!({"type":"response.created","response":{"id":"c1"}}),
            json!({"type":"response.output_text.delta","delta":"hi"}),
            json!({"type":"response.completed","response":{"id":"c1","usage":{"input_tokens":10,"output_tokens":9,"total_tokens":19,"input_tokens_details":{"cached_tokens":6}}}}),
        ),
        Protocol::OpenAICompletions => format!(
            "data: {}\n\ndata: {}\n\ndata: [DONE]\n\n",
            json!({"choices":[{"index":0,"text":"hi"}]}),
            json!({"choices":[{"index":0,"text":"","finish_reason":"stop"}],"usage":{"prompt_tokens":10,"completion_tokens":9,"total_tokens":19}}),
        ),
        // openai chat：官方分帧——finish 块先到，usage 块（choices 空）后到，再 [DONE]
        _ => format!(
            "data: {}\n\ndata: {}\n\ndata: {}\n\ndata: [DONE]\n\n",
            json!({"choices":[{"index":0,"delta":{"content":"hi"},"finish_reason":null}]}),
            json!({"choices":[{"index":0,"delta":{},"finish_reason":"stop"}]}),
            json!({"choices":[],"usage":{"prompt_tokens":10,"completion_tokens":9,"total_tokens":19,"prompt_tokens_details":{"cached_tokens":6}}}),
        ),
    }
}

/// 上游各 wire 的期望 token：（input, output, cache_read）。
fn expect(wire: &Protocol) -> (u32, u32, Option<u32>) {
    match wire {
        // legacy Completions 上游帧无缓存字段
        Protocol::OpenAICompletions => (10, 9, None),
        _ => (10, 9, Some(6)),
    }
}

fn frames_json(out: &str) -> Vec<serde_json::Value> {
    out.lines()
        .filter_map(|l| l.strip_prefix("data: "))
        .filter_map(|d| serde_json::from_str::<serde_json::Value>(d).ok())
        .collect()
}

/// 客户端协议渲染形态断言：从客户端输出里提取 input / output / cache_read。
fn assert_client_tokens(client: &Protocol, out: &str, wire: &Protocol) {
    let (inp, outp, cr) = expect(wire);
    match client {
        Protocol::Anthropic => {
            let md: Vec<serde_json::Value> = frames_json(out)
                .into_iter()
                .filter(|j| j["type"] == "message_delta")
                .collect();
            assert!(!md.is_empty(), "anthropic 客户端缺 message_delta: {out:?}");
            let last = md.last().unwrap();
            assert_eq!(last["usage"]["input_tokens"], inp, "{out:?}");
            assert_eq!(last["usage"]["output_tokens"], outp, "{out:?}");
            if let Some(c) = cr {
                assert_eq!(last["usage"]["cache_read_input_tokens"], c, "{out:?}");
            }
        }
        Protocol::OpenAI | Protocol::OpenAICompletions => {
            let frames = frames_json(out);
            let u: Vec<&serde_json::Value> = frames
                .iter()
                .filter(|j| j.get("usage").is_some() && !j.get("usage").unwrap().is_null())
                .collect();
            assert_eq!(u.len(), 1, "{client:?} 客户端应恰有一个 usage 块: {out:?}");
            let u = u[0]["usage"].as_object().unwrap();
            assert_eq!(u["prompt_tokens"], inp, "{client:?}←{wire:?} {out:?}");
            assert_eq!(u["completion_tokens"], outp, "{client:?}←{wire:?} {out:?}");
            // legacy Completions wire 无 prompt_tokens_details 字段，仅 OpenAI Chat 校验缓存读
            if let Some(c) = cr
                && *client == Protocol::OpenAI
            {
                let got = u
                    .get("prompt_tokens_details")
                    .and_then(|d| d.get("cached_tokens"))
                    .cloned();
                assert_eq!(got, Some(serde_json::json!(c)), "{client:?}←{wire:?} {out:?}");
            }
            // [DONE] 必须在 usage 块之后（标准 SDK 读到 [DONE] 停止迭代）
            let done_pos = out.find("data: [DONE]").expect("缺 [DONE] 终止哨兵");
            let usage_pos = out.find("\"prompt_tokens\"").expect("缺 usage 块");
            assert!(usage_pos < done_pos, "usage 块必须在 [DONE] 之前: {out:?}");
        }
        Protocol::Gemini => {
            let frames = frames_json(out);
            let meta: Vec<&serde_json::Value> = frames
                .iter()
                .filter(|j| j.get("usageMetadata").is_some())
                .collect();
            assert_eq!(meta.len(), 1, "gemini 客户端应恰有一帧 usageMetadata: {out:?}");
            let m = meta[0]["usageMetadata"].as_object().unwrap();
            assert_eq!(m["promptTokenCount"], inp, "{out:?}");
            assert_eq!(m["candidatesTokenCount"], outp, "{out:?}");
            if let Some(c) = cr {
                assert_eq!(m["cachedContentTokenCount"], c, "{out:?}");
            }
        }
        Protocol::OpenAIResponses => {
            let frames = frames_json(out);
            let comp: Vec<&serde_json::Value> = frames
                .iter()
                .filter(|j| {
                    matches!(
                        j["type"].as_str(),
                        Some("response.completed") | Some("response.incomplete")
                    )
                })
                .collect();
            assert_eq!(comp.len(), 1, "responses 客户端应恰有一帧终态: {out:?}");
            let u = comp[0]["response"]["usage"].as_object().unwrap();
            assert_eq!(u["input_tokens"], inp, "{out:?}");
            assert_eq!(u["output_tokens"], outp, "{out:?}");
            if let Some(c) = cr {
                assert_eq!(u["input_tokens_details"]["cached_tokens"], c, "{out:?}");
            }
            assert!(out.contains("response.created"), "responses 客户端缺 created: {out:?}");
        }
        _ => panic!("未覆盖客户端协议 {client:?}"),
    }
}

#[test]
fn usage_matrix_all_wire_client_pairs_pass_tokens_through() {
    for wire in WIRES {
        for client in CLIENTS {
            let out = run_stream(&wire, &client, true);
            assert_client_tokens(&client, &out, &wire);
        }
    }
}

#[test]
fn openai_family_client_without_include_usage_gets_no_usage_chunk() {
    // OpenAI Chat / Completions 客户端未带 stream_options.include_usage → 无 usage 块；
    // [DONE] 仍须发出（流末 flush 补发）。
    for client in [Protocol::OpenAI, Protocol::OpenAICompletions] {
        for wire in WIRES {
            let out = run_stream(&wire, &client, false);
            let has_usage = frames_json(&out).iter().any(|j| {
                (j.get("usage").is_some() && !j.get("usage").unwrap().is_null())
                    || j.get("usageMetadata").is_some()
            });
            assert!(
                !has_usage,
                "{wire:?}→{client:?} 未带 include_usage 不得有 usage 块: {out:?}"
            );
            assert!(
                out.contains("data: [DONE]"),
                "{wire:?}→{client:?} 缺 [DONE] 终止哨兵: {out:?}"
            );
        }
    }
}

#[test]
fn ungated_clients_get_usage_without_include_usage_flag() {
    // Anthropic / Gemini / Responses 客户端无 include_usage 门控（官方契约恒回传 usage）
    for client in [
        Protocol::Anthropic,
        Protocol::Gemini,
        Protocol::OpenAIResponses,
    ] {
        for wire in WIRES {
            let out = run_stream(&wire, &client, false);
            assert_client_tokens(&client, &out, &wire);
        }
    }
}
