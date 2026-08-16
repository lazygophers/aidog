//! 纯文本对话 round-trip 回归网（ticket 01 / B1-B2）。
//!
//! 每个源协议一条真实形态 fixture，parse_incoming → convert_request 到
//! anthropic / openai / gemini 三目标，断言文本语义与 model 不丢。
//! 后续阶段（参数/工具/thinking/多模态）在此网上叠加新断言，回归锁行为。

use super::*;
use serde_json::json;

/// 5 源协议纯文本 fixture（真实请求形态，含 system + user 文本）
fn fixtures() -> Vec<(&'static str, Protocol, serde_json::Value)> {
    vec![
        (
            "anthropic",
            Protocol::Anthropic,
            json!({
                "model": "m-an",
                "max_tokens": 128,
                "system": "be brief",
                "messages": [
                    {"role": "user", "content": [{"type": "text", "text": "hello anthropic"}]}
                ]
            }),
        ),
        (
            "openai",
            Protocol::OpenAI,
            json!({
                "model": "m-oa",
                "messages": [
                    {"role": "system", "content": "be brief"},
                    {"role": "user", "content": "hello openai"}
                ]
            }),
        ),
        (
            "openai_responses",
            Protocol::OpenAIResponses,
            json!({
                "model": "m-rs",
                "instructions": "be brief",
                "input": [
                    {"role": "user", "content": "hello responses"}
                ]
            }),
        ),
        (
            "openai_completions",
            Protocol::OpenAICompletions,
            json!({
                "model": "m-cp",
                "prompt": "hello completions"
            }),
        ),
        (
            "gemini",
            Protocol::Gemini,
            json!({
                "model": "m-gm",
                "systemInstruction": {"parts": [{"text": "be brief"}]},
                "contents": [
                    {"role": "user", "parts": [{"text": "hello gemini"}]}
                ]
            }),
        ),
    ]
}

/// 从目标 body 抽 user 文本（三目标协议各自路径）
fn user_text_of(target: &Protocol, body: &serde_json::Value) -> String {
    match target {
        Protocol::Gemini => body["contents"]
            .as_array()
            .map(|cs| {
                cs.iter()
                    .filter(|c| c["role"] == "user")
                    .flat_map(|c| c["parts"].as_array().cloned().unwrap_or_default())
                    .filter_map(|p| p["text"].as_str().map(str::to_string))
                    .collect::<Vec<_>>()
                    .join("")
            })
            .unwrap_or_default(),
        Protocol::Anthropic => body["messages"]
            .as_array()
            .map(|ms| {
                ms.iter()
                    .filter(|m| m["role"] == "user")
                    .map(|m| match &m["content"] {
                        serde_json::Value::String(s) => s.clone(),
                        blocks => blocks
                            .as_array()
                            .map(|bs| {
                                bs.iter()
                                    .filter_map(|b| b["text"].as_str().map(str::to_string))
                                    .collect::<Vec<_>>()
                                    .join("")
                            })
                            .unwrap_or_default(),
                    })
                    .collect::<Vec<_>>()
                    .join("")
            })
            .unwrap_or_default(),
        _ => body["messages"]
            .as_array()
            .map(|ms| {
                ms.iter()
                    .filter(|m| m["role"] == "user")
                    .filter_map(|m| m["content"].as_str().map(str::to_string))
                    .collect::<Vec<_>>()
                    .join("")
            })
            .unwrap_or_default(),
    }
}

fn system_text_of(target: &Protocol, body: &serde_json::Value) -> String {
    match target {
        Protocol::Gemini => body["systemInstruction"]["parts"]
            .as_array()
            .map(|ps| {
                ps.iter()
                    .filter_map(|p| p["text"].as_str().map(str::to_string))
                    .collect::<Vec<_>>()
                    .join("")
            })
            .unwrap_or_default(),
        Protocol::Anthropic => body["system"]
            .as_str()
            .map(str::to_string)
            .unwrap_or_default(),
        _ => body["messages"]
            .as_array()
            .map(|ms| {
                ms.iter()
                    .filter(|m| m["role"] == "system")
                    .filter_map(|m| m["content"].as_str().map(str::to_string))
                    .collect::<Vec<_>>()
                    .join("")
            })
            .unwrap_or_default(),
    }
}

#[test]
fn plain_text_roundtrip_all_sources_to_three_targets() {
    let targets = [
        ("anthropic", Protocol::Anthropic),
        ("openai", Protocol::OpenAI),
        ("gemini", Protocol::Gemini),
    ];
    for (src_name, src_proto, body) in fixtures() {
        let req = parse_incoming_request(&src_proto, &body)
            .unwrap_or_else(|e| panic!("{src_name} parse failed: {e}"));
        for (tgt_name, tgt_proto) in &targets {
            let (out, _path) = convert_request(&req, tgt_proto, &Protocol::OpenAI);
            let text = user_text_of(tgt_proto, &out);
            assert!(
                text.starts_with("hello"),
                "{src_name} → {tgt_name}: user 文本丢失，实得 body: {out}"
            );
            assert!(!text.is_empty(), "{src_name} → {tgt_name}: user 文本为空");
        }
    }
}

#[test]
fn system_roundtrip_preserved_where_source_has_it() {
    // completions 无 system 概念，单独跳过；其余 4 源都带 "be brief"
    for (src_name, src_proto, body) in fixtures() {
        if src_name == "openai_completions" {
            continue;
        }
        let req = parse_incoming_request(&src_proto, &body)
            .unwrap_or_else(|e| panic!("{src_name} parse failed: {e}"));
        for (tgt_name, tgt_proto) in [
            ("anthropic", Protocol::Anthropic),
            ("openai", Protocol::OpenAI),
            ("gemini", Protocol::Gemini),
        ] {
            let (out, _) = convert_request(&req, &tgt_proto, &Protocol::OpenAI);
            assert!(
                system_text_of(&tgt_proto, &out).contains("be brief"),
                "{src_name} → {tgt_name}: system 丢失"
            );
        }
    }
}

#[test]
fn model_roundtrip_preserved_all_directions() {
    for (src_name, src_proto, body) in fixtures() {
        let req = parse_incoming_request(&src_proto, &body)
            .unwrap_or_else(|e| panic!("{src_name} parse failed: {e}"));
        for (tgt_name, tgt_proto) in [
            ("anthropic", Protocol::Anthropic),
            ("openai", Protocol::OpenAI),
            ("gemini", Protocol::Gemini),
        ] {
            let (out, path) = convert_request(&req, &tgt_proto, &Protocol::OpenAI);
            // Gemini 官方形态 model 在 URL 路径段（body 无 model 字段），按协议约定分别断言
            let carried = if tgt_name == "gemini" {
                path.contains(&req.model).then_some(String::new()).map(|_| path.clone())
            } else {
                out.get("model").and_then(|m| m.as_str()).map(str::to_string)
            };
            let Some(carried) = carried else {
                panic!("{src_name} → {tgt_name}: model 丢失（body 与 path 均无）");
            };
            if tgt_name != "gemini" {
                assert_eq!(carried, req.model, "{src_name} → {tgt_name}: model 改写");
            }
        }
    }
}
