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

// ═══ ticket 02：入站参数提取 round-trip ═══

/// 参数保留断言：目标 body 按协议形态抽三参（Gemini 走 generationConfig camelCase）
fn params_of(target: &Protocol, body: &serde_json::Value) -> (Option<u32>, Option<f32>, Option<f32>) {
    match target {
        Protocol::Gemini => {
            let g = &body["generationConfig"];
            (
                g["maxOutputTokens"].as_u64().map(|v| v as u32),
                g["temperature"].as_f64().map(|v| v as f32),
                g["topP"].as_f64().map(|v| v as f32),
            )
        }
        _ => (
            body["max_tokens"].as_u64().map(|v| v as u32),
            body["temperature"].as_f64().map(|v| v as f32),
            body["top_p"].as_f64().map(|v| v as f32),
        ),
    }
}

#[test]
fn ticket02_params_roundtrip_all_sources_to_three_targets() {
    let sources: Vec<(&str, Protocol, serde_json::Value)> = vec![
        (
            "anthropic",
            Protocol::Anthropic,
            json!({
                "model": "m", "max_tokens": 777, "temperature": 0.5, "top_p": 0.9,
                "messages": [{"role": "user", "content": "hi"}]
            }),
        ),
        (
            "openai",
            Protocol::OpenAI,
            json!({
                "model": "m", "max_tokens": 777, "temperature": 0.5, "top_p": 0.9,
                "messages": [{"role": "user", "content": "hi"}]
            }),
        ),
        (
            "gemini",
            Protocol::Gemini,
            json!({
                "model": "m",
                "generationConfig": {"maxOutputTokens": 777, "temperature": 0.5, "topP": 0.9},
                "contents": [{"role": "user", "parts": [{"text": "hi"}]}]
            }),
        ),
    ];
    for (src_name, src_proto, body) in sources {
        let req = parse_incoming_request(&src_proto, &body)
            .unwrap_or_else(|e| panic!("{src_name} parse failed: {e}"));
        assert_eq!(req.max_tokens, Some(777), "{src_name}: parse 丢 max_tokens");
        assert_eq!(req.temperature, Some(0.5), "{src_name}: parse 丢 temperature");
        assert_eq!(req.top_p, Some(0.9), "{src_name}: parse 丢 top_p");
        for (tgt_name, tgt_proto) in [
            ("anthropic", Protocol::Anthropic),
            ("openai", Protocol::OpenAI),
            ("gemini", Protocol::Gemini),
        ] {
            let (out, _) = convert_request(&req, &tgt_proto, &Protocol::OpenAI);
            let (mx, t, p) = params_of(&tgt_proto, &out);
            assert_eq!(mx, Some(777), "{src_name} → {tgt_name}: max_tokens 丢失, body: {out}");
            assert_eq!(t, Some(0.5), "{src_name} → {tgt_name}: temperature 丢失");
            assert_eq!(p, Some(0.9), "{src_name} → {tgt_name}: top_p 丢失");
        }
    }
}

/// 守卫式：无参数请求 → 目标 body 不强加默认参数（Gemini 不产 generationConfig 节点）
#[test]
fn ticket02_no_params_no_defaults() {
    let body = json!({
        "model": "m",
        "messages": [{"role": "user", "content": "hi"}]
    });
    let req = parse_incoming_request(&Protocol::OpenAI, &body).expect("parse");
    let (out, _) = convert_request(&req, &Protocol::Gemini, &Protocol::OpenAI);
    assert!(out.get("generationConfig").is_none(), "无参请求 Gemini 不应产 generationConfig: {out}");
}

// ═══ ticket 03：工具调用 OpenAI↔Anthropic（非流式请求体双向） ═══

/// OpenAI 带工具三件套请求（tools 定义 + assistant tool_calls + tool 结果）
fn openai_tool_body() -> serde_json::Value {
    json!({
        "model": "m",
        "tools": [{
            "type": "function",
            "function": {
                "name": "get_weather",
                "description": "query weather",
                "parameters": {"type": "object", "properties": {"city": {"type": "string"}}}
            }
        }],
        "messages": [
            {"role": "user", "content": "weather in Beijing?"},
            {"role": "assistant", "content": null, "tool_calls": [{
                "id": "call_w1", "type": "function",
                "function": {"name": "get_weather", "arguments": "{\"city\":\"Beijing\"}"}
            }]},
            {"role": "tool", "tool_call_id": "call_w1", "content": "{\"temp\": 25}"}
        ]
    })
}

#[test]
fn ticket03_openai_to_anthropic_tool_roundtrip() {
    let req = parse_incoming_request(&Protocol::OpenAI, &openai_tool_body()).expect("parse");
    let (out, _) = convert_request(&req, &Protocol::Anthropic, &Protocol::OpenAI);

    // tools 定义：input_schema 映射
    let tools = out["tools"].as_array().expect("anthropic tools");
    assert_eq!(tools.len(), 1);
    assert_eq!(tools[0]["name"], "get_weather");
    assert_eq!(tools[0]["description"], "query weather");
    assert_eq!(tools[0]["input_schema"]["properties"]["city"]["type"], "string");

    // assistant tool_use block：id/name/input(对象)
    let msgs = out["messages"].as_array().expect("messages");
    let tool_use_msgs: Vec<&Value> = msgs.iter()
        .filter(|m| m["content"].as_array()
            .map(|bs| bs.iter().any(|b| b["type"] == "tool_use"))
            .unwrap_or(false))
        .collect();
    assert_eq!(tool_use_msgs.len(), 1, "应恰一条含 tool_use 的 assistant 消息: {out}");
    let tu = tool_use_msgs[0]["content"].as_array().unwrap().iter()
        .find(|b| b["type"] == "tool_use").unwrap();
    assert_eq!(tu["id"], "call_w1");
    assert_eq!(tu["name"], "get_weather");
    assert_eq!(tu["input"]["city"], "Beijing");

    // user tool_result block：tool_use_id 关联
    let tr = msgs.iter()
        .filter_map(|m| m["content"].as_array()
            .map(|bs| bs.iter().find(|b| b["type"] == "tool_result").cloned())
            .unwrap_or(None))
        .next().expect("tool_result block 缺失");
    assert_eq!(tr["tool_use_id"], "call_w1");
    assert!(tr["content"].as_str().unwrap().contains("temp"));
}

/// Anthropic 带工具请求 → OpenAI（arguments 须序列化为 JSON 字符串）
#[test]
fn ticket03_anthropic_to_openai_tool_roundtrip() {
    let body = json!({
        "model": "m",
        "max_tokens": 100,
        "tools": [{
            "name": "get_weather",
            "description": "query weather",
            "input_schema": {"type": "object", "properties": {"city": {"type": "string"}}}
        }],
        "messages": [
            {"role": "user", "content": "weather in Beijing?"},
            {"role": "assistant", "content": [
                {"type": "tool_use", "id": "call_w1", "name": "get_weather", "input": {"city": "Beijing"}}
            ]},
            {"role": "user", "content": [
                {"type": "tool_result", "tool_use_id": "call_w1", "content": "{\"temp\": 25}"}
            ]}
        ]
    });
    let req = parse_incoming_request(&Protocol::Anthropic, &body).expect("parse");
    let (out, _) = convert_request(&req, &Protocol::OpenAI, &Protocol::OpenAI);

    let tools = out["tools"].as_array().expect("openai tools");
    assert_eq!(tools[0]["function"]["name"], "get_weather");
    assert_eq!(tools[0]["function"]["parameters"]["properties"]["city"]["type"], "string");

    let msgs = out["messages"].as_array().unwrap();
    // assistant tool_calls：arguments 是合法 JSON 字符串且语义等价
    let asst = msgs.iter().find(|m| m["role"] == "assistant" && m["tool_calls"].is_array())
        .expect("assistant tool_calls 消息缺失");
    let tc = &asst["tool_calls"][0];
    assert_eq!(tc["id"], "call_w1");
    assert_eq!(tc["function"]["name"], "get_weather");
    let args = tc["function"]["arguments"].as_str().expect("arguments 须为字符串");
    let parsed: Value = serde_json::from_str(args).expect("arguments 须为合法 JSON");
    assert_eq!(parsed["city"], "Beijing");

    // tool message：tool_call_id 关联
    let tool_msg = msgs.iter().find(|m| m["role"] == "tool").expect("tool 消息缺失");
    assert_eq!(tool_msg["tool_call_id"], "call_w1");
    assert!(tool_msg["content"].as_str().unwrap().contains("temp"));
}

/// 多工具并发：两个 tool_use id 不串
#[test]
fn ticket03_multiple_tool_uses_no_id_mixup() {
    let body = json!({
        "model": "m", "max_tokens": 100,
        "messages": [
            {"role": "user", "content": "go"},
            {"role": "assistant", "content": [
                {"type": "tool_use", "id": "c1", "name": "f1", "input": {"a": 1}},
                {"type": "tool_use", "id": "c2", "name": "f2", "input": {"b": 2}}
            ]}
        ]
    });
    let req = parse_incoming_request(&Protocol::Anthropic, &body).expect("parse");
    let (out, _) = convert_request(&req, &Protocol::OpenAI, &Protocol::OpenAI);
    let msgs = out["messages"].as_array().unwrap();
    let asst = msgs.iter().find(|m| m["tool_calls"].is_array()).unwrap();
    let tcs = asst["tool_calls"].as_array().unwrap();
    assert_eq!(tcs.len(), 2);
    assert_eq!(tcs[0]["id"], "c1");
    assert_eq!(tcs[1]["id"], "c2");
    assert_eq!(tcs[0]["function"]["name"], "f1");
    assert_eq!(tcs[1]["function"]["name"], "f2");
}

/// 无工具请求行为不变（守卫式）
#[test]
fn ticket03_no_tools_unchanged() {
    let body = json!({"model": "m", "max_tokens": 10, "messages": [{"role": "user", "content": "hi"}]});
    let req = parse_incoming_request(&Protocol::Anthropic, &body).expect("parse");
    let (out, _) = convert_request(&req, &Protocol::OpenAI, &Protocol::OpenAI);
    assert!(out.get("tools").is_none());
}

// ═══ ticket 04：tool_choice 映射 ═══

#[test]
fn ticket04_tool_choice_openai_to_anthropic() {
    for (oa, expect_type, expect_name) in [
        (json!("auto"), "auto", None),
        (json!("none"), "none", None),
        (json!("required"), "any", None),
        (json!({"type": "function", "function": {"name": "get_weather"}}), "tool", Some("get_weather")),
    ] {
        let mut body = openai_tool_body();
        body["tool_choice"] = oa.clone();
        let req = parse_incoming_request(&Protocol::OpenAI, &body).expect("parse");
        let (out, _) = convert_request(&req, &Protocol::Anthropic, &Protocol::OpenAI);
        let tc = &out["tool_choice"];
        assert_eq!(tc["type"], expect_type, "tool_choice {oa} → {expect_type} 映射错: {tc}");
        if let Some(name) = expect_name {
            assert_eq!(tc["name"], name);
        }
    }
}

/// 未指定 tool_choice → 目标不输出该字段
#[test]
fn ticket04_no_tool_choice_no_field() {
    let req = parse_incoming_request(&Protocol::OpenAI, &openai_tool_body()).expect("parse");
    let (out_an, _) = convert_request(&req, &Protocol::Anthropic, &Protocol::OpenAI);
    assert!(out_an.get("tool_choice").is_none());
    let (out_oa, _) = convert_request(&req, &Protocol::OpenAI, &Protocol::OpenAI);
    assert!(out_oa.get("tool_choice").is_none() || out_oa["tool_choice"].is_null());
}

// ═══ ticket 05：工具调用 Gemini 双向 ═══

#[test]
fn ticket05_openai_tools_to_gemini() {
    let req = parse_incoming_request(&Protocol::OpenAI, &openai_tool_body()).expect("parse");
    let (out, _) = convert_request(&req, &Protocol::Gemini, &Protocol::OpenAI);

    // functionDeclarations
    let decls = out["tools"][0]["functionDeclarations"].as_array().expect("functionDeclarations");
    assert_eq!(decls[0]["name"], "get_weather");
    assert_eq!(decls[0]["description"], "query weather");
    assert_eq!(decls[0]["parameters"]["properties"]["city"]["type"], "string");

    let contents = out["contents"].as_array().unwrap();
    // functionCall part：name + args 对象
    let fc = contents.iter()
        .flat_map(|c| c["parts"].as_array().cloned().unwrap_or_default())
        .find(|p| p["functionCall"].is_object())
        .expect("functionCall part 缺失");
    assert_eq!(fc["functionCall"]["name"], "get_weather");
    assert_eq!(fc["functionCall"]["args"]["city"], "Beijing");

    // functionResponse part：name = 工具名（非 tool_use_id）
    let fr = contents.iter()
        .flat_map(|c| c["parts"].as_array().cloned().unwrap_or_default())
        .find(|p| p["functionResponse"].is_object())
        .expect("functionResponse part 缺失");
    assert_eq!(fr["functionResponse"]["name"], "get_weather",
        "Gemini 靠 name 关联 functionResponse ↔ functionCall，禁用 tool_use_id");
    assert!(fr["functionResponse"]["response"]["result"].as_str().unwrap().contains("temp"));
}

#[test]
fn ticket05_gemini_tools_to_openai() {
    let body = json!({
        "model": "m",
        "tools": [{"functionDeclarations": [{
            "name": "get_weather",
            "description": "query weather",
            "parameters": {"type": "object", "properties": {"city": {"type": "string"}}}
        }]}],
        "contents": [
            {"role": "user", "parts": [{"text": "weather in Beijing?"}]},
            {"role": "model", "parts": [{"functionCall": {"name": "get_weather", "args": {"city": "Beijing"}}}]},
            {"role": "user", "parts": [{"functionResponse": {"name": "get_weather", "response": {"result": "{\"temp\":25}"}}}]}
        ]
    });
    let req = parse_incoming_request(&Protocol::Gemini, &body).expect("parse");

    // tools 定义 parse
    assert_eq!(req.tools.as_ref().expect("tools")[0].name, "get_weather");
    assert_eq!(req.tools.as_ref().unwrap()[0].input_schema["properties"]["city"]["type"], "string");

    // functionCall → ToolUse（无 id 自生成）；functionResponse → ToolResult（name 回填 id 规则配对）
    let fc_msg = req.messages.iter().find(|m| m.content.blocks().iter().any(|b| matches!(b, ContentBlock::ToolUse { .. })))
        .expect("ToolUse 消息缺失");
    let tu = fc_msg.content.blocks().into_iter()
        .find_map(|b| if let ContentBlock::ToolUse { id, name, input } = b { Some((id, name, input)) } else { None })
        .unwrap();
    assert_eq!(tu.1, "get_weather");
    assert_eq!(tu.2["city"], "Beijing");
    assert!(!tu.0.is_empty(), "Gemini 无 id 须自生成");

    let tr = req.messages.iter()
        .find_map(|m| m.content.blocks().into_iter()
            .find_map(|b| if let ContentBlock::ToolResult { tool_use_id, content, name } = b { Some((tool_use_id, content, name)) } else { None }))
        .expect("ToolResult 消息缺失");
    assert_eq!(tr.2.as_deref(), Some("get_weather"));
    assert!(tr.1.contains("temp"));

    // → OpenAI 出站：arguments JSON 字符串、tool_call_id 关联
    let (out, _) = convert_request(&req, &Protocol::OpenAI, &Protocol::OpenAI);
    let msgs = out["messages"].as_array().unwrap();
    let asst = msgs.iter().find(|m| m["tool_calls"].is_array()).expect("tool_calls");
    let tc = &asst["tool_calls"][0];
    assert_eq!(tc["function"]["name"], "get_weather");
    let args: Value = serde_json::from_str(tc["function"]["arguments"].as_str().unwrap()).unwrap();
    assert_eq!(args["city"], "Beijing");
    let tool_msg = msgs.iter().find(|m| m["role"] == "tool").expect("tool message");
    assert_eq!(tool_msg["tool_call_id"], tc["id"], "tool_call_id 须与自生成 id 一致");
}
