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

// ─── ticket 06: thinking / reasoning 双向 ───

/// Anthropic thinking 开关 → 中立 → Gemini thinkingBudget / OpenAI reasoning_effort
#[test]
fn ticket06_anthropic_thinking_switch_outbound() {
    let body = json!({
        "model": "claude-x", "max_tokens": 1024,
        "thinking": { "type": "enabled", "budget_tokens": 10240 },
        "messages": [{ "role": "user", "content": "hi" }]
    });
    let req = parse_incoming_request(&Protocol::Anthropic, &body).unwrap();
    assert_eq!(req.thinking_budget, Some(10240), "入站解析须提取 budget_tokens");

    let (g, _) = convert_request(&req, &Protocol::Gemini, &Protocol::Gemini);
    assert_eq!(g["generationConfig"]["thinkingConfig"]["thinkingBudget"], 10240);

    let (o, _) = convert_request(&req, &Protocol::OpenAI, &Protocol::OpenAI);
    assert_eq!(o["reasoning_effort"], "high", "10240 > 8192 → high");
}

/// OpenAI reasoning_effort → 中立 → Anthropic thinking.budget_tokens / Gemini thinkingBudget
#[test]
fn ticket06_openai_reasoning_effort_outbound() {
    let body = json!({
        "model": "gpt-x", "reasoning_effort": "low",
        "messages": [{ "role": "user", "content": "hi" }]
    });
    let req = parse_incoming_request(&Protocol::OpenAI, &body).unwrap();
    assert!(req.thinking_budget.is_some(), "low → budget 映射");

    let (a, _) = convert_request(&req, &Protocol::Anthropic, &Protocol::Anthropic);
    assert_eq!(a["thinking"]["type"], "enabled");
    assert_eq!(a["thinking"]["budget_tokens"], req.thinking_budget.unwrap());

    let (g, _) = convert_request(&req, &Protocol::Gemini, &Protocol::Gemini);
    assert_eq!(g["generationConfig"]["thinkingConfig"]["thinkingBudget"], req.thinking_budget.unwrap());
}

/// Gemini thinkingBudget → 中立 → Anthropic thinking.budget_tokens
#[test]
fn ticket06_gemini_thinking_budget_outbound() {
    let body = json!({
        "contents": [{ "role": "user", "parts": [{ "text": "hi" }] }],
        "generationConfig": { "thinkingConfig": { "thinkingBudget": 8192 } }
    });
    let req = parse_incoming_request(&Protocol::Gemini, &body).unwrap();
    assert_eq!(req.thinking_budget, Some(8192));

    let (a, _) = convert_request(&req, &Protocol::Anthropic, &Protocol::Anthropic);
    assert_eq!(a["thinking"]["budget_tokens"], 8192);
}

/// 无 thinking 开关的请求不回归：不出 thinking / thinkingConfig / reasoning_effort 字段
#[test]
fn ticket06_no_thinking_no_fields() {
    let body = json!({
        "model": "claude-x", "max_tokens": 100,
        "messages": [{ "role": "user", "content": "hi" }]
    });
    let req = parse_incoming_request(&Protocol::Anthropic, &body).unwrap();
    assert_eq!(req.thinking_budget, None);

    let (a, _) = convert_request(&req, &Protocol::Anthropic, &Protocol::Anthropic);
    assert!(a.get("thinking").is_none());
    let (g, _) = convert_request(&req, &Protocol::Gemini, &Protocol::Gemini);
    assert!(g["generationConfig"].get("thinkingConfig").is_none() || g["generationConfig"]["thinkingConfig"].is_null());
    let (o, _) = convert_request(&req, &Protocol::OpenAI, &Protocol::OpenAI);
    assert!(o.get("reasoning_effort").is_none());
}

/// thinking 内容 block：Anthropic 带 signature round-trip 不丢；转 Gemini 成 thought part
#[test]
fn ticket06_thinking_block_anthropic_roundtrip_and_gemini() {
    let body = json!({
        "model": "claude-x", "max_tokens": 1024,
        "messages": [
            { "role": "user", "content": "hi" },
            { "role": "assistant", "content": [
                { "type": "thinking", "thinking": "pondering", "signature": "sig-123" },
                { "type": "text", "text": "answer" }
            ]},
            { "role": "user", "content": "go on" }
        ]
    });
    let req = parse_incoming_request(&Protocol::Anthropic, &body).unwrap();

    // Anthropic round-trip：thinking block + signature 原样保留
    let (a, _) = convert_request(&req, &Protocol::Anthropic, &Protocol::Anthropic);
    let asst = a["messages"].as_array().unwrap().iter().find(|m| m["role"] == "assistant").unwrap();
    let think_block = a_thinking_block(&asst["content"]);
    assert_eq!(think_block["thinking"], "pondering");
    assert_eq!(think_block["signature"], "sig-123", "signature 透传不丢");

    // → Gemini：thought part（signature 丢，Gemini 无此概念）
    let (g, _) = convert_request(&req, &Protocol::Gemini, &Protocol::Gemini);
    let contents = g["contents"].as_array().unwrap();
    let model_turn = contents.iter().find(|c| c["role"] == "model").unwrap();
    let thought = model_turn["parts"].as_array().unwrap().iter()
        .find(|p| p.get("thought").and_then(|v| v.as_bool()).unwrap_or(false)).unwrap();
    assert_eq!(thought["text"], "pondering");
}

/// Gemini thought part（请求侧历史 turn）→ 中立 thinking block；
/// 出站 Anthropic 无 signature 降级不回传、不报错（Anthropic 回传 thinking 需有效签名）
#[test]
fn ticket06_gemini_thought_part_to_anthropic() {
    let body = json!({
        "contents": [
            { "role": "user", "parts": [{ "text": "hi" }] },
            { "role": "model", "parts": [{ "thought": true, "text": "pondering" }] },
            { "role": "user", "parts": [{ "text": "go" }] }
        ]
    });
    let req = parse_incoming_request(&Protocol::Gemini, &body).unwrap();
    // 中立层：thought part 保留为 thinking block
    assert!(req.messages.iter().any(|m| m.content.blocks().iter().any(|b|
        matches!(b, ContentBlock::Unknown(v) if v["type"] == "thinking" && v["thinking"] == "pondering"))),
        "thought part 须映射中立 thinking block");

    // 出站 Anthropic：无 signature 降级不回传、不报错
    let (a, _) = convert_request(&req, &Protocol::Anthropic, &Protocol::Anthropic);
    let think_any = a["messages"].as_array().unwrap().iter().any(|m| {
        m["content"].as_array().map(|arr| arr.iter().any(|b| b["type"] == "thinking")).unwrap_or(false)
    });
    assert!(!think_any, "无 signature thinking block 降级不回传");
}

fn a_thinking_block(content: &Value) -> &Value {
    content.as_array().unwrap().iter().find(|b| b["type"] == "thinking").unwrap()
}

// ─── ticket 09: 多模态图片双向 ───

/// OpenAI image_url(data URL / http url) → 中立 → Anthropic image.source / Gemini inlineData
#[test]
fn ticket09_openai_image_outbound() {
    let body = json!({
        "model": "gpt-x",
        "messages": [{ "role": "user", "content": [
            { "type": "text", "text": "看图" },
            { "type": "image_url", "image_url": { "url": "data:image/png;base64,QUJD" } },
            { "type": "image_url", "image_url": { "url": "https://example.com/cat.jpg" } }
        ]}]
    });
    let req = parse_incoming_request(&Protocol::OpenAI, &body).unwrap();

    // → Anthropic：base64 拆解 source + url source
    let (a, _) = convert_request(&req, &Protocol::Anthropic, &Protocol::Anthropic);
    let arr = a["messages"][0]["content"].as_array().unwrap();
    let imgs: Vec<&Value> = arr.iter().filter(|b| b["type"] == "image").collect();
    assert_eq!(imgs.len(), 2, "两张图都要保留");
    assert_eq!(imgs[0]["source"]["type"], "base64");
    assert_eq!(imgs[0]["source"]["media_type"], "image/png");
    assert_eq!(imgs[0]["source"]["data"], "QUJD");
    assert_eq!(imgs[1]["source"]["type"], "url");
    assert_eq!(imgs[1]["source"]["url"], "https://example.com/cat.jpg");

    // → Gemini：inlineData + fileData
    let (g, _) = convert_request(&req, &Protocol::Gemini, &Protocol::Gemini);
    let parts = g["contents"][0]["parts"].as_array().unwrap();
    assert_eq!(parts[0]["text"], "看图");
    assert_eq!(parts[1]["inlineData"]["mimeType"], "image/png");
    assert_eq!(parts[1]["inlineData"]["data"], "QUJD");
    assert_eq!(parts[2]["fileData"]["fileUri"], "https://example.com/cat.jpg");
}

/// Anthropic image(base64/url) → 中立 → OpenAI image_url(data URL 重组 / 原 url)
#[test]
fn ticket09_anthropic_image_to_openai() {
    let body = json!({
        "model": "claude-x", "max_tokens": 100,
        "messages": [{ "role": "user", "content": [
            { "type": "text", "text": "look" },
            { "type": "image", "source": { "type": "base64", "media_type": "image/jpeg", "data": "RGVG" } },
            { "type": "image", "source": { "type": "url", "url": "https://example.com/dog.png" } }
        ]}]
    });
    let req = parse_incoming_request(&Protocol::Anthropic, &body).unwrap();
    let (o, _) = convert_request(&req, &Protocol::OpenAI, &Protocol::OpenAI);
    let arr = o["messages"][0]["content"].as_array().expect("带图消息须用数组 content");
    let imgs: Vec<&Value> = arr.iter().filter(|b| b["type"] == "image_url").collect();
    assert_eq!(imgs.len(), 2);
    assert_eq!(imgs[0]["image_url"]["url"], "data:image/jpeg;base64,RGVG", "base64 须重组 data URL");
    assert_eq!(imgs[1]["image_url"]["url"], "https://example.com/dog.png");
}

/// Gemini inlineData/fileData → 中立 → Anthropic image source
#[test]
fn ticket09_gemini_image_to_anthropic() {
    let body = json!({
        "contents": [{ "role": "user", "parts": [
            { "text": "look" },
            { "inlineData": { "mimeType": "image/webp", "data": "V0VQ" } },
            { "fileData": { "mimeType": "image/png", "fileUri": "https://example.com/x.png" } }
        ]}]
    });
    let req = parse_incoming_request(&Protocol::Gemini, &body).unwrap();
    let (a, _) = convert_request(&req, &Protocol::Anthropic, &Protocol::Anthropic);
    let arr = a["messages"][0]["content"].as_array().unwrap();
    let imgs: Vec<&Value> = arr.iter().filter(|b| b["type"] == "image").collect();
    assert_eq!(imgs.len(), 2);
    assert_eq!(imgs[0]["source"]["media_type"], "image/webp");
    assert_eq!(imgs[0]["source"]["data"], "V0VQ");
    assert_eq!(imgs[1]["source"]["url"], "https://example.com/x.png");
}

/// 纯文本请求不回归（ticket 01 回归网已覆盖，此处补 openai 数组纯文本）
#[test]
fn ticket09_text_only_no_regression() {
    let body = json!({
        "model": "gpt-x",
        "messages": [{ "role": "user", "content": [{ "type": "text", "text": "hi" }] }]
    });
    let req = parse_incoming_request(&Protocol::OpenAI, &body).unwrap();
    assert_eq!(req.messages[0].content.as_text(), "hi");
    let (o, _) = convert_request(&req, &Protocol::OpenAI, &Protocol::OpenAI);
    assert_eq!(o["messages"][0]["content"], "hi", "纯文本数组折叠回字符串");
}

