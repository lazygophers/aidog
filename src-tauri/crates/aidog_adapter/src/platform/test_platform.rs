//! 平台适配器框架门禁测试（issue 68）。

use super::error::UpstreamErrorClass;
use super::hooks;
use crate::types::{ChatRequest, ChatStreamEvent};
use aidog_db::models::Protocol;
use serde_json::{json, Value};

/// 门禁（issue 68）：registry 全量平台（含无内置默认端点的 devin / newapi / passthrough）
/// 都有适配器注册项；收集全量缺失后一次报错，不逐条 panic。
#[test]
fn every_registry_platform_has_adapter_registration() {
    let missing: Vec<&str> = aidog_db::bundled_platform_files()
        .iter()
        .map(|(code, _)| *code)
        .filter(|code| super::lookup(code).is_none())
        .collect();
    assert!(
        missing.is_empty(),
        "registry 平台缺适配器注册项，共 {} 个: {:?}",
        missing.len(),
        missing
    );
}

/// Kimi 家族（issue 76 / 119）：coding_plan 注入按平台协议分派，kimi 与 kimi_coding 注入
/// prompt_cache_key（chat.md「对于 Kimi Code Plan，此字段为必填」，issue 119 扩入 KimiCoding 臂），
/// 产出同形状（注入条件同源：端点 coding_plan flag）。
#[test]
fn kimi_adapter_coding_plan_matches_base_kimi_arm() {
    let pa = super::adapter_for("kimi");
    let mut body = json!({ "model": "kimi-for-coding" });
    pa.inject_coding_plan(&mut body, &Protocol::Kimi, &Protocol::OpenAI);
    assert!(
        body.get("prompt_cache_key").is_some(),
        "kimi 未注入 prompt_cache_key"
    );
    // 基座默认按平台协议分派：platform=Kimi 同样注入；非 Kimi 协议不注入。
    let mut via_base = json!({ "model": "kimi-for-coding" });
    super::adapter_for("anthropic").inject_coding_plan(&mut via_base, &Protocol::Kimi, &Protocol::OpenAI);
    assert!(via_base.get("prompt_cache_key").is_some());
    let mut non_kimi = json!({ "model": "m" });
    super::adapter_for("anthropic").inject_coding_plan(&mut non_kimi, &Protocol::Anthropic, &Protocol::OpenAI);
    assert!(non_kimi.get("prompt_cache_key").is_none());
    // 家族适配器与基座 Kimi 臂产出同形状（除时间窗口随机段外的键集合一致）。
    let mut member = json!({ "model": "k3" });
    super::adapter_for("kimi_coding").inject_coding_plan(&mut member, &Protocol::Kimi, &Protocol::OpenAI);
    assert_eq!(
        member.as_object().unwrap().keys().collect::<Vec<_>>(),
        via_base.as_object().unwrap().keys().collect::<Vec<_>>()
    );
    // issue 119: preserve explicit keys; converted requests without session metadata get per-request keys.
    let adapter = super::adapter_for("kimi_coding");
    let mut supplied = json!({ "model": "k3", "prompt_cache_key": "session-42" });
    adapter.inject_coding_plan(&mut supplied, &Protocol::KimiCoding, &Protocol::OpenAI);
    assert_eq!(supplied["prompt_cache_key"], "session-42");

    let request = json!({
        "model": "k3",
        "metadata": { "session_id": "session-42" },
        "messages": [{ "role": "user", "content": "hello" }]
    });
    let mut first = request.clone();
    let mut later = request;
    adapter.inject_coding_plan(&mut first, &Protocol::KimiCoding, &Protocol::OpenAI);
    adapter.inject_coding_plan(&mut later, &Protocol::KimiCoding, &Protocol::OpenAI);
    assert_eq!(first["prompt_cache_key"], later["prompt_cache_key"]);

    let mut other_session = json!({ "model": "k3", "metadata": { "session_id": "session-43" } });
    adapter.inject_coding_plan(&mut other_session, &Protocol::KimiCoding, &Protocol::OpenAI);
    assert_ne!(first["prompt_cache_key"], other_session["prompt_cache_key"]);
}

/// review R2-2（issue 119）：anthropic wire 的 Kimi coding_plan 端点不注入 prompt_cache_key
/// （messages.md 未列该参数，缓存走 `cache_control`；chat.md「必填」仅限 Chat Completions）。
/// https://platform.kimi.com/docs/api/messages.md
#[test]
fn kimi_coding_plan_no_inject_on_anthropic_wire() {
    let mut body = json!({ "model": "kimi-for-coding" });
    super::adapter_for("kimi_coding").inject_coding_plan(
        &mut body,
        &Protocol::KimiCoding,
        &Protocol::Anthropic,
    );
    assert!(
        body.get("prompt_cache_key").is_none(),
        "anthropic wire 不应注入 prompt_cache_key: {body}"
    );
}

/// 行为零变化门禁（review 必改 1）：kimi_en 不注入 prompt_cache_key。
/// 迁移前 headers.rs 仅对 Protocol::Kimi 注入，KimiEn 走 no-op 臂；kimi_coding 由 issue 119
/// 按官方「必填」证据扩为注入（`Protocol::KimiCoding` 臂）。kimi_en 非编码套餐仍不注入（issue 76）。
#[test]
fn kimi_coding_and_kimi_en_do_not_inject_prompt_cache_key() {
    let mut body = json!({ "model": "kimi-for-coding" });
    super::adapter_for("kimi_en").inject_coding_plan(&mut body, &Protocol::KimiEn, &Protocol::OpenAI);
    assert!(
        body.get("prompt_cache_key").is_none(),
        "kimi_en 不应注入 prompt_cache_key（仅 Protocol::Kimi / KimiCoding 注入）"
    );
}

/// review R2-1（issue 119）：kimi_coding openai wire 多轮工具历史携带 reasoning_content。
/// 官方错误参考 400「thinking is enabled but reasoning_content is missing in assistant
/// tool call message … 这是 Kimi Code 特有字段，启用思维链时必须携带」；Chat Completions
/// 文档以 `thinking.keep="all"` 配置保留历史 reasoning_content。
/// anthropic 客户端回灌的 thinking 块在 to_openai 被跳过，基座产出缺 reasoning_content 的
/// assistant tool-call 消息 → 上游 400。仅 kimi_coding 改写（Kimi Code 专属校验），
/// kimi / kimi_en 维持基座形状。
/// https://www.kimi.com/code/docs/kimi-code/error-reference.html
#[test]
fn kimi_coding_openai_tool_history_carries_reasoning_content() {
    let chat_req: ChatRequest = serde_json::from_value(json!({
        "model": "kimi-k3",
        "max_tokens": 64,
        "messages": [
            { "role": "user", "content": "北京天气?" },
            { "role": "assistant", "content": [
                { "type": "thinking", "thinking": "需要调用天气工具", "signature": "sig" },
                { "type": "tool_use", "id": "tu1", "name": "get_weather", "input": { "city": "北京" } }
            ]},
            { "role": "user", "content": [
                { "type": "tool_result", "tool_use_id": "tu1", "content": "晴" }
            ]}
        ]
    }))
    .unwrap();
    let (body, _) = super::adapter_for("kimi_coding").build_request(
        &chat_req,
        &Protocol::OpenAI,
        &Protocol::KimiCoding,
    );
    let asst = body["messages"]
        .as_array()
        .unwrap()
        .iter()
        .find(|m| m["role"] == "assistant" && m.get("tool_calls").is_some_and(|v| !v.is_null()))
        .expect("转换应产出 assistant tool-call 消息");
    assert_eq!(
        asst["reasoning_content"],
        "需要调用天气工具",
        "assistant tool-call 历史应携带 reasoning_content: {asst}"
    );
    // 家族其他成员（kimi）维持基座形状：不写 reasoning_content（Kimi Code 专属校验）
    let (base_body, _) = super::adapter_for("kimi").build_request(
        &chat_req,
        &Protocol::OpenAI,
        &Protocol::Kimi,
    );
    let base_asst = base_body["messages"]
        .as_array()
        .unwrap()
        .iter()
        .find(|m| m["role"] == "assistant" && m.get("tool_calls").is_some_and(|v| !v.is_null()))
        .unwrap();
    assert!(
        base_asst.get("reasoning_content").is_none(),
        "kimi 标准平台不应写 reasoning_content: {base_asst}"
    );
}

/// 基座默认实现与协议构件逐字节等价（issue 68：初始映射与现状等价的单元面）。
#[test]
fn generic_adapter_delegates_to_protocol_components() {
    let pa = super::adapter_for("deepseek");
    let chat_req: ChatRequest = serde_json::from_value(json!({
        "model": "deepseek-v4",
        "max_tokens": 64,
        "messages": [{ "role": "user", "content": "hi" }]
    }))
    .unwrap();
    let (body, path) = pa.build_request(&chat_req, &Protocol::OpenAI, &Protocol::DeepSeek);
    let (body2, path2) =
        crate::converter::convert_request(&chat_req, &Protocol::OpenAI, &Protocol::DeepSeek);
    assert_eq!(path, "/chat/completions");
    assert_eq!(body, body2);
    assert_eq!(path, path2);
    // 同 wire family 非流式响应 → None 透传（与 converter::convert_response 一致）。
    let resp = json!({ "id": "x", "choices": [] });
    assert_eq!(
        pa.convert_response(&resp, &Protocol::OpenAI, &Protocol::OpenAICompletions, "m"),
        None
    );
    // 流式单帧解析与 converter::parse_sse 一致（anthropic ping 帧透传事件）。
    let frame = json!({ "type": "ping" });
    assert_eq!(
        format!("{:?}", pa.parse_sse_frame(&frame, &Protocol::Anthropic)),
        format!("{:?}", crate::converter::parse_sse(&frame, &Protocol::Anthropic))
    );
}

/// 默认错误体分类与 usage 抽取（issue 68：接口④⑤与原实现等价的单元面）。
#[test]
fn error_and_usage_defaults_match_legacy_helpers() {
    let pa = super::adapter_for("newapi");
    assert_eq!(
        pa.classify_error(404, r#"{"error":{"message":"model not found"}}"#),
        UpstreamErrorClass::Model
    );
    assert_eq!(
        pa.classify_error(404, "Not found the model kimi-for-coding or Permission denied"),
        UpstreamErrorClass::Unknown
    );
    assert_eq!(pa.classify_error(500, "boom"), UpstreamErrorClass::Service);
    assert_eq!(pa.classify_error(400, "bad request"), UpstreamErrorClass::Unknown);
    assert_eq!(
        pa.classify_error(429, r#"{"error":{"type":"rate_limit_error"}}"#),
        UpstreamErrorClass::Service
    );
    let (i, o, c, cw, cw1h, tools) =
        pa.extract_usage(r#"{"usage":{"input_tokens":5,"output_tokens":7}}"#);
    assert_eq!((i, o, c, cw, cw1h, tools.len()), (5, 7, 0, 0, 0, 0));
    // cache 五形态：anthropic cache_read 臂
    let (i, _o, c, _cw, _cw1h, _t) = pa.extract_usage(
        r#"{"usage":{"input_tokens":10,"output_tokens":2,"cache_read_input_tokens":9}}"#,
    );
    assert_eq!((i, c), (10, 9));
    // 禁用路径不动：devin / passthrough 等无内置端点平台同样有默认实现
    for code in ["devin", "newapi", "passthrough", "claude_code"] {
        assert!(super::lookup(code).is_some(), "code {code} 应有注册项");
    }
}

/// MiniMax 成员沿用原有 OpenAI wire 禁思考映射；本地 endpoint URL 不触发官方域名特殊分支。
#[test]
fn minimax_members_disable_thinking_with_native_parameter() {
    for code in ["minimax", "minimax_en", "minimax_coding"] {
        let adapter = super::adapter_for(code);
        let mut body = json!({ "model": "MiniMax-M3" });
        adapter.apply_disable_thinking(
            &mut body,
            true,
            &Protocol::OpenAI,
            "http://127.0.0.1:31234/custom/v1",
            false,
        );
        assert_eq!(body["thinking"]["type"], "disabled", "{code}");
        assert!(body.get("reasoning_effort").is_none(), "{code}");

        let mut forced = json!({ "model": "MiniMax-M3.1-Flash" });
        adapter.apply_disable_thinking(
            &mut forced,
            true,
            &Protocol::OpenAI,
            "http://127.0.0.1:31234/custom/v1",
            true,
        );
        assert!(forced.get("thinking").is_none(), "{code}");
        assert!(forced.get("reasoning_effort").is_none(), "{code}");
    }
}

/// Token Plan has no documented coding_plan payload additions; retain the family no-op on both wires.
#[test]
fn minimax_coding_plan_keeps_family_noop() {
    for wire in [Protocol::OpenAI, Protocol::Anthropic] {
        let mut body = json!({ "model": "MiniMax-M3", "messages": [] });
        let original = body.clone();
        super::adapter_for("minimax_coding").inject_coding_plan(
            &mut body,
            &Protocol::MinimaxCoding,
            &wire,
        );
        assert_eq!(body, original, "{wire:?}");
    }
}

/// Synthetic base_resp input: code 2022 is ambiguous, so do not infer HTTP status semantics or model-only failure.
#[test]
fn minimax_synthetic_base_resp_stays_unclassified() {
    let adapter = super::adapter_for("minimax");
    let body = r#"{"base_resp":{"status_code":2022,"status_msg":"模型不存在或无权使用"}}"#;

    assert_eq!(
        adapter.classify_error(400, body),
        UpstreamErrorClass::Unknown
    );
    assert_eq!(
        adapter.classify_error(400, body),
        super::adapter_for("deepseek").classify_error(400, body)
    );
}

/// MiniMax family inherits protocol conversion, response, stream, error, and usage behavior from the framework.
#[test]
fn minimax_family_preserves_generic_protocol_behavior() {
    let input: ChatRequest = serde_json::from_value(json!({
        "model": "MiniMax-M3",
        "max_tokens": 128,
        "messages": [{ "role": "user", "content": "hello" }]
    }))
    .unwrap();
    let generic = super::adapter_for("deepseek");
    let completion = json!({
        "id": "cmpl-1", "model": "MiniMax-M3",
        "choices": [{ "index": 0, "message": { "role": "assistant", "content": "hi" }, "finish_reason": "stop" }]
    });
    let cached_completion = json!({
        "id": "cmpl-1", "choices": [{ "index": 0, "delta": { "content": "hi" }, "finish_reason": null }],
        "usage": { "prompt_tokens": 10, "completion_tokens": 2,
            "prompt_tokens_details": { "cached_tokens": 4 } }
    });
    let error_body = r#"{"error":{"type":"invalid_request_error","message":"bad request"}}"#;
    let anthropic_message = json!({
        "id": "msg_1", "type": "message", "role": "assistant", "model": "MiniMax-M3",
        "content": [{ "type": "text", "text": "hi" }], "stop_reason": "end_turn",
        "usage": { "input_tokens": 5, "output_tokens": 3 }
    });
    let anthropic_delta = json!({
        "type": "content_block_delta", "index": 0,
        "delta": { "type": "text_delta", "text": "hi" }
    });
    let anthropic_error = r#"{"type":"error","error":{"type":"invalid_request_error","message":"bad request"}}"#;
    let anthropic_usage = r#"{"usage":{"input_tokens":5,"output_tokens":3,"cache_read_input_tokens":2,"cache_creation_input_tokens":1}}"#;

    for (code, platform) in [
        ("minimax", Protocol::MiniMax),
        ("minimax_en", Protocol::MiniMaxEn),
        ("minimax_coding", Protocol::MinimaxCoding),
    ] {
        let adapter = super::adapter_for(code);
        for wire in [Protocol::OpenAI, Protocol::Anthropic] {
            let (body, path) = adapter.build_request(&input, &wire, &platform);
            let (base_body, base_path) = generic.build_request(&input, &wire, &platform);
            assert_eq!((body, path), (base_body, base_path), "{code}/{wire:?}");
        }
        let converted = adapter
            .convert_response(&completion, &Protocol::OpenAI, &Protocol::Anthropic, "MiniMax-M3")
            .expect("OpenAI response should convert to Anthropic");
        assert_eq!(
            converted,
            generic
                .convert_response(&completion, &Protocol::OpenAI, &Protocol::Anthropic, "MiniMax-M3")
                .unwrap(),
            "{code} non-stream response"
        );
        assert_eq!(converted["content"][0]["text"], "hi");

        let anthropic_converted = adapter
            .convert_response(&anthropic_message, &Protocol::Anthropic, &Protocol::OpenAI, "MiniMax-M3")
            .expect("Anthropic response should convert to OpenAI");
        assert_eq!(anthropic_converted["choices"][0]["message"]["content"], "hi", "{code} Anthropic non-stream");
        assert_eq!(
            anthropic_converted,
            generic.convert_response(&anthropic_message, &Protocol::Anthropic, &Protocol::OpenAI, "MiniMax-M3").unwrap(),
            "{code} Anthropic non-stream parity"
        );

        let anthropic_events = adapter.parse_sse_frame(&anthropic_delta, &Protocol::Anthropic);
        assert_eq!(
            format!("{anthropic_events:?}"),
            format!("{:?}", generic.parse_sse_frame(&anthropic_delta, &Protocol::Anthropic)),
            "{code} Anthropic stream parity"
        );
        assert!(format!("{anthropic_events:?}").contains("hi"), "{code} Anthropic stream content: {anthropic_events:?}");

        assert_eq!(
            adapter.classify_error(400, anthropic_error),
            UpstreamErrorClass::Unknown,
            "{code} Anthropic error classification"
        );
        assert_eq!(
            adapter.classify_error(400, anthropic_error),
            generic.classify_error(400, anthropic_error),
            "{code} Anthropic error"
        );
        assert_eq!(
            adapter.extract_usage(anthropic_usage),
            (5, 3, 2, 1, 0, Vec::new()),
            "{code} Anthropic usage"
        );
        assert_eq!(
            adapter.extract_usage(anthropic_usage),
            generic.extract_usage(anthropic_usage),
            "{code} Anthropic usage parity"
        );

        let events = adapter.parse_sse_frame(&cached_completion, &Protocol::OpenAI);
        assert_eq!(
            format!("{events:?}"),
            format!("{:?}", generic.parse_sse_frame(&cached_completion, &Protocol::OpenAI)),
            "{code} stream"
        );
        assert!(format!("{events:?}").contains("Delta"), "{code} stream content: {events:?}");
        assert_eq!(
            adapter.classify_error(400, error_body),
            UpstreamErrorClass::Unknown,
            "{code} error"
        );
        assert_eq!(
            adapter.classify_error(400, error_body),
            generic.classify_error(400, error_body),
            "{code} error parity"
        );
        assert_eq!(
            adapter.extract_usage(&cached_completion.to_string()),
            (6, 2, 4, 0, 0, Vec::new()),
            "{code} cached usage"
        );
        assert_eq!(
            adapter.extract_usage(&cached_completion.to_string()),
            generic.extract_usage(&cached_completion.to_string()),
            "{code} usage parity"
        );
    }
}

/// 差异钩子默认实现 = 搬家前的通用行为（body 层 max_tokens 裁剪按 wire 协议键名）。
#[test]
fn hook_defaults_cap_max_tokens_by_wire_key() {
    let pa = super::adapter_for("anthropic");
    let mut body = json!({ "max_tokens": 999 });
    assert_eq!(pa.cap_max_tokens(&mut body, Some(100), &Protocol::Anthropic), Some((999, 100)));
    assert_eq!(body.get("max_tokens"), Some(&json!(100)));
    let mut resp_body = json!({ "max_output_tokens": 999 });
    let pa2 = super::adapter_for("openai");
    assert_eq!(
        pa2.cap_max_tokens(&mut resp_body, Some(100), &Protocol::OpenAIResponses),
        Some((999, 100))
    );
    assert_eq!(resp_body.get("max_output_tokens"), Some(&json!(100)));
    let _ = hooks::is_official_openai_host("https://api.openai.com/v1");
}

/// issue 75 回归护栏：GLM 家族强制思考模型（registry `thinking_toggleable: false`）路径与基座一致，
/// 只剥禁用参数、不写任何禁用值；非 GLM 的 OpenAI 兼容平台仍走基座 `chat_template_kwargs`。
#[test]
fn glm_family_forced_thinking_and_non_glm_unchanged() {
    let url = "https://open.bigmodel.cn/api/paas/v4/chat/completions";
    for code in ["glm", "glm_en", "glm_coding", "glm_coding_en"] {
        let mut body = json!({ "disable_thinking": true, "thinking": { "type": "disabled" } });
        super::adapter_for(code).apply_disable_thinking(&mut body, true, &Protocol::OpenAI, url, true);
        assert_eq!(body, json!({}), "{code} 强制思考应只剥禁用参数: {body}");
    }
    let mut body = json!({ "disable_thinking": true });
    super::adapter_for("deepseek").apply_disable_thinking(
        &mut body,
        true,
        &Protocol::OpenAI,
        "https://api.deepseek.com/v1/chat/completions",
        false,
    );
    assert_eq!(body, json!({ "chat_template_kwargs": { "enable_thinking": false } }));
}

/// issue 76 跟进：tool_choice 降级只命中 kimi / kimi_en 的受影响模型（kimi-k2.6 /
/// kimi-k2.7-code / kimi-k2.7-code-highspeed，后者与 kimi-k2.7-code 参数约束相同）的 openai wire；
/// K3 系列、kimi_coding、anthropic wire、非 "required" 值一律不动。
/// https://platform.kimi.com/docs/api/models-overview.md
#[test]
fn kimi_family_adjust_tool_choice_scope() {
    let kimi = super::adapter_for("kimi");
    for model in ["kimi-k2.6", "kimi-k2.7-code", "kimi-k2.7-code-highspeed"] {
        let mut body = json!({ "model": model, "tool_choice": "required" });
        kimi.adjust_tool_choice(&mut body, &Protocol::OpenAI, model);
        assert_eq!(body.get("tool_choice"), Some(&json!("auto")), "{model}: {body}");
    }
    // K3 系列接受 "required"，不降级。
    let mut body = json!({ "model": "kimi-k3", "tool_choice": "required" });
    kimi.adjust_tool_choice(&mut body, &Protocol::OpenAI, "kimi-k3");
    assert_eq!(body.get("tool_choice"), Some(&json!("required")), "{body}");
    // kimi_coding 不降级（成员 issue 119：编码套餐文档未列 tool_choice 约束）。
    let mut body = json!({ "model": "kimi-k2.6", "tool_choice": "required" });
    super::adapter_for("kimi_coding").adjust_tool_choice(&mut body, &Protocol::OpenAI, "kimi-k2.6");
    assert_eq!(body.get("tool_choice"), Some(&json!("required")), "{body}");
    // anthropic wire 不在范围。
    let mut body = json!({ "model": "kimi-k2.6", "tool_choice": "required" });
    kimi.adjust_tool_choice(&mut body, &Protocol::Anthropic, "kimi-k2.6");
    assert_eq!(body.get("tool_choice"), Some(&json!("required")), "{body}");
    // 非 "required" 值（auto / none / 指名工具对象）不动。
    let mut body = json!({ "tool_choice": "none" });
    kimi.adjust_tool_choice(&mut body, &Protocol::OpenAI, "kimi-k2.6");
    assert_eq!(body.get("tool_choice"), Some(&json!("none")), "{body}");
    let mut body = json!({ "tool_choice": { "type": "function", "function": { "name": "f" } } });
    kimi.adjust_tool_choice(&mut body, &Protocol::OpenAI, "kimi-k2.6");
    assert!(body.get("tool_choice").and_then(|v| v.get("function")).is_some(), "{body}");
    // 默认钩子 no-op：无 tool_choice 键的 body 不受影响。
    let mut body = json!({ "model": "kimi-k2.6" });
    kimi.adjust_tool_choice(&mut body, &Protocol::OpenAI, "kimi-k2.6");
    assert!(body.get("tool_choice").is_none(), "{body}");
}

/// Bailian 家族（issue 78）：openai 端点禁思考写官方顶层 `enable_thinking: false`
/// （DashScope Chat Completions 文档只列顶层参数，「设为 false：模型直接回复」，
/// https://help.aliyun.com/zh/model-studio/qwen-api-via-openai-chat-completions
/// https://help.aliyun.com/zh/model-studio/deep-thinking ）；客户端带来的旧
/// `enable_thinking` / `thinking` / `reasoning_effort` / vLLM 式
/// `chat_template_kwargs.enable_thinking` 一并剥除。anthropic 端点与强制思考路径同基座。
#[test]
fn bailian_family_openai_disable_thinking_writes_enable_thinking() {
    for code in ["bailian", "bailian_en", "bailian_coding", "bailian_coding_en"] {
        let adapter = super::adapter_for(code);
        let url = "http://127.0.0.1:31234/compatible-mode/v1";
        let mut body = json!({
            "model": "qwen3.8-max",
            "enable_thinking": true,
            "thinking": { "type": "enabled" },
            "reasoning_effort": "high",
            "chat_template_kwargs": { "enable_thinking": true },
            "generationConfig": { "thinkingConfig": { "thinkingBudget": 0 } },
        });
        adapter.apply_disable_thinking(&mut body, true, &Protocol::OpenAI, url, false);
        assert_eq!(body["enable_thinking"], json!(false), "{code}");
        assert!(body.get("thinking").is_none(), "{code}: {body}");
        assert!(body.get("reasoning_effort").is_none(), "{code}: {body}");
        assert!(
            body.get("chat_template_kwargs")
                .and_then(|c| c.get("enable_thinking"))
                .is_none(),
            "{code}: {body}"
        );
        assert!(
            body.get("generationConfig")
                .and_then(|g| g.get("thinkingConfig"))
                .is_none(),
            "{code}: {body}"
        );

        // 不禁思考：只剥 aidog 私有字段与禁用形参数语义，不写禁用参数、不动客户端旧值。
        let mut keep = json!({
            "model": "qwen3.8-max",
            "disable_thinking": false,
            "enable_thinking": true,
        });
        adapter.apply_disable_thinking(&mut keep, false, &Protocol::OpenAI, url, false);
        assert_eq!(keep["enable_thinking"], json!(true), "{code}: {keep}");
        assert!(keep.get("disable_thinking").is_none(), "{code}: {keep}");

        // 强制思考模型（仅思考模式，registry thinking_toggleable: false）：只剥禁用形参数、
        // 不写任何禁用参数（同基座 forced_thinking 路径）。
        let mut forced = json!({ "model": "deepseek-r1", "thinking": { "type": "disabled" } });
        adapter.apply_disable_thinking(&mut forced, true, &Protocol::OpenAI, url, true);
        assert!(forced.get("thinking").is_none(), "{code}: {forced}");
        assert!(forced.get("enable_thinking").is_none(), "{code}: {forced}");

        // anthropic 端点走基座：thinking.type=disabled（与官方 enabled/disabled 词表同形）。
        let mut anth = json!({ "model": "qwen3.8-max" });
        adapter.apply_disable_thinking(
            &mut anth,
            true,
            &Protocol::Anthropic,
            "http://127.0.0.1:31234/apps/anthropic",
            false,
        );
        assert_eq!(anth["thinking"]["type"], "disabled", "{code}: {anth}");
    }
}

/// Bailian 家族错误分类沿用基座：404 model_not_found + 「does not exist」归模型类；
/// 「Model can not be found.」不命中基座特征词落 Unknown（调用方按模型类处理，
/// platform/error.rs「无法判定：调用方按 Model 处理」），不扩展映射。
/// https://help.aliyun.com/zh/model-studio/error-code
#[test]
fn bailian_family_error_classification_tracks_base() {
    let adapter = super::adapter_for("bailian");
    assert_eq!(
        adapter.classify_error(
            404,
            r#"{"error":{"code":"model_not_found","message":"The model qwen3.8-max does not exist."}}"#,
        ),
        UpstreamErrorClass::Model
    );
    assert_eq!(
        adapter.classify_error(
            404,
            r#"{"code":"ModelNotFound","message":"Model can not be found."}"#,
        ),
        UpstreamErrorClass::Unknown
    );
    // 429 按状态码直接归服务类（Throttling.RateQuota）。
    assert_eq!(
        adapter.classify_error(
            429,
            r#"{"error":{"code":"Throttling.RateQuota","message":"Requests rate limit exceeded."}}"#,
        ),
        UpstreamErrorClass::Service
    );
}

/// 覆写只作用于家族成员：非成员第三方 OpenAI 兼容端点仍写基座 vLLM 式
/// `chat_template_kwargs.enable_thinking`。
#[test]
fn bailian_override_scoped_to_family_members() {
    let mut other = json!({ "model": "m" });
    super::adapter_for("aihubmix").apply_disable_thinking(
        &mut other,
        true,
        &Protocol::OpenAI,
        "http://127.0.0.1:31234/v1",
        false,
    );
    assert_eq!(other["chat_template_kwargs"]["enable_thinking"], json!(false));
    assert!(other.get("enable_thinking").is_none());
}

/// coding_plan 注入保持 no-op：官方接入页只要求换 base_url 与 API Key，无出站附加字段
/// （https://help.aliyun.com/zh/model-studio/claude-code ）。
#[test]
fn bailian_coding_plan_keeps_noop() {
    for (code, platform) in [
        ("bailian_coding", Protocol::BailianCoding),
        ("bailian_coding_en", Protocol::BailianCodingEn),
    ] {
        for wire in [Protocol::OpenAI, Protocol::Anthropic] {
            let mut body = json!({ "model": "qwen3.8-max", "messages": [] });
            let original = body.clone();
            super::adapter_for(code).inject_coding_plan(&mut body, &platform, &wire);
            assert_eq!(body, original, "{code} {wire:?}");
        }
    }
}

/// 火山方舟家族（issue 79 / 127 / 128）：openai 端点禁思考写官方顶层 `thinking: {"type": "disabled"}`
/// （方舟 Chat API 文档「disabled：关闭思考模式」；BytePlus 文档同形示例 `"type":"disabled"` /
/// `"type":"enabled"`，https://docs.byteplus.com/en/docs/ModelArk/text-generation ）；客户端带来的
/// 旧 `enable_thinking` / `reasoning_effort` / vLLM 式 `chat_template_kwargs.enable_thinking`
/// 一并剥除。anthropic 端点与 openai_responses 端点同基座（anthropic 臂 thinking.type=disabled；
/// responses 臂 reasoning.effort=none，官方词表「none: Turn off reasoning」可受，
/// https://docs.byteplus.com/en/docs/ModelArk/deep-thinking ）。
#[test]
fn ark_family_disable_thinking_all_wires() {
    for code in ["doubao", "byteplus"] {
        let adapter = super::adapter_for(code);
        let url = "http://127.0.0.1:31234/api/plan/v3";
        let mut body = json!({
            "model": "doubao-seed-2.1",
            "enable_thinking": true,
            "thinking": { "type": "enabled" },
            "reasoning_effort": "high",
            "chat_template_kwargs": { "enable_thinking": true },
            "generationConfig": { "thinkingConfig": { "thinkingBudget": 0 } },
        });
        adapter.apply_disable_thinking(&mut body, true, &Protocol::OpenAI, url, false);
        assert_eq!(body["thinking"], json!({ "type": "disabled" }), "{code}");
        assert!(body.get("enable_thinking").is_none(), "{code}: {body}");
        assert!(body.get("reasoning_effort").is_none(), "{code}: {body}");
        assert!(
            body.get("chat_template_kwargs")
                .and_then(|c| c.get("enable_thinking"))
                .is_none(),
            "{code}: {body}"
        );
        assert!(
            body.get("generationConfig")
                .and_then(|g| g.get("thinkingConfig"))
                .is_none(),
            "{code}: {body}"
        );

        // 不禁思考：只剥 aidog 私有字段，不写禁用参数、不动客户端旧值。
        let mut keep = json!({
            "model": "doubao-seed-2.1",
            "disable_thinking": false,
            "enable_thinking": true,
        });
        adapter.apply_disable_thinking(&mut keep, false, &Protocol::OpenAI, url, false);
        assert_eq!(keep["enable_thinking"], json!(true), "{code}: {keep}");
        assert!(keep.get("disable_thinking").is_none(), "{code}: {keep}");

        // 强制思考模型（仅思考模式，registry thinking_toggleable: false）：只剥禁用形参数、
        // 不写任何禁用参数（同基座 forced_thinking 路径）。
        let mut forced = json!({ "model": "doubao-seed-2.1", "thinking": { "type": "disabled" } });
        adapter.apply_disable_thinking(&mut forced, true, &Protocol::OpenAI, url, true);
        assert!(forced.get("thinking").is_none(), "{code}: {forced}");

        // openai_responses 端点走基座：reasoning.effort=none（官方词表 none 关闭思考）。
        let mut resp = json!({ "model": "doubao-seed-2.1", "reasoning": { "effort": "high" } });
        adapter.apply_disable_thinking(
            &mut resp,
            true,
            &Protocol::OpenAIResponses,
            "http://127.0.0.1:31234/api/plan/v3/responses",
            false,
        );
        assert_eq!(resp["reasoning"]["effort"], "none", "{code}: {resp}");

        // anthropic 端点走基座：thinking.type=disabled（方舟 anthropic 兼容端点同词表）。
        let mut anth = json!({ "model": "doubao-seed-2.1" });
        adapter.apply_disable_thinking(
            &mut anth,
            true,
            &Protocol::Anthropic,
            "http://127.0.0.1:31234/api/plan",
            false,
        );
        assert_eq!(anth["thinking"]["type"], "disabled", "{code}: {anth}");
    }
}

/// 火山方舟家族错误分类沿用基座：404 InvalidEndpointOrModel.NotFound 的 message
/// 「The model or endpoint … does not exist」命中基座 model + does not exist 特征归模型类；
/// 401 AuthenticationError / 429 RateLimitExceeded.EndpointRPMExceeded 按状态码归服务类。
/// 错误体统一 JSON 形状未证实（错误码页只列表格，无 JSON 示例）；BytePlus 错误码表逐条同形
/// （404 NotFound.InvalidEndpointOrModel.NotFound、401 AuthenticationError、
/// 429 RateLimitExceeded.EndpointRPMExceeded），
/// https://docs.byteplus.com/en/docs/ModelArk/error-codes
#[test]
fn ark_family_error_classification_tracks_base() {
    for code in ["doubao", "byteplus"] {
        let adapter = super::adapter_for(code);
        assert_eq!(
            adapter.classify_error(
                404,
                r#"{"error":{"code":"InvalidEndpointOrModel.NotFound","message":"The model or endpoint ep-2026 does not exist or you do not have access to it."}}"#,
            ),
            UpstreamErrorClass::Model,
            "{code}"
        );
        assert_eq!(
            adapter.classify_error(
                401,
                r#"{"error":{"code":"AuthenticationError","message":"The API key or AK/SK in the request is missing or invalid."}}"#,
            ),
            UpstreamErrorClass::Service,
            "{code}"
        );
        assert_eq!(
            adapter.classify_error(
                429,
                r#"{"error":{"code":"RateLimitExceeded.EndpointRPMExceeded","message":"Endpoint RPM limit exceeded."}}"#,
            ),
            UpstreamErrorClass::Service,
            "{code}"
        );
    }
}

/// 覆写只作用于家族成员：非成员第三方 OpenAI 兼容端点仍写基座 vLLM 式
/// `chat_template_kwargs.enable_thinking`。
#[test]
fn ark_override_scoped_to_family_members() {
    let mut other = json!({ "model": "m" });
    super::adapter_for("aihubmix").apply_disable_thinking(
        &mut other,
        true,
        &Protocol::OpenAI,
        "http://127.0.0.1:31234/v1",
        false,
    );
    assert_eq!(other["chat_template_kwargs"]["enable_thinking"], json!(false));
    assert!(other.get("thinking").is_none());
}

/// coding_plan 注入保持 no-op：官方接入页只要求设 base_url 与 API Key，
/// 无出站附加字段（火山 https://docs.volcengine.com/docs/ark/ai-tools-claude-code?lang=zh ；
/// BytePlus quick start 只设 base_url 与 API key，
/// https://docs.byteplus.com/en/docs/ModelArk/coding-plan ）。
#[test]
fn ark_coding_plan_keeps_noop() {
    for (platform, wire) in [
        (Protocol::Doubao, Protocol::Anthropic),
        (Protocol::Doubao, Protocol::OpenAI),
        (Protocol::BytePlus, Protocol::Anthropic),
        (Protocol::BytePlus, Protocol::OpenAI),
        (Protocol::BytePlus, Protocol::OpenAIResponses),
    ] {
        let mut body = json!({ "model": "doubao-seed-2.1", "messages": [] });
        let original = body.clone();
        super::adapter_for("byteplus").inject_coding_plan(&mut body, &platform, &wire);
        assert_eq!(body, original, "{platform:?} {wire:?}");
    }
}

/// usage 抽取沿用基座 openai 构件：`prompt_tokens_details.cached_tokens` 从 input 扣除、
/// `completion_tokens_details.reasoning_tokens` 并入 output（方舟 Chat API 字段名与基座一致，
/// https://www.volcengine.com/docs/82379/1494384 ；BytePlus 同名
/// completion_tokens_details.reasoning_tokens，
/// https://docs.byteplus.com/en/docs/ModelArk/deep-thinking ）。
#[test]
fn ark_usage_extraction_tracks_base() {
    let body = r#"{
        "usage": {
            "prompt_tokens": 100,
            "completion_tokens": 50,
            "total_tokens": 150,
            "prompt_tokens_details": { "cached_tokens": 40 },
            "completion_tokens_details": { "reasoning_tokens": 20 }
        }
    }"#;
    for code in ["doubao", "byteplus"] {
        let adapter = super::adapter_for(code);
        assert_eq!(adapter.extract_usage(body), (60, 50, 40, 0, 0, vec![]), "{code}");
    }
    // BytePlus Messages API（anthropic wire）usage 为 Anthropic 标准四件
    // （cache_creation_input_tokens / cache_read_input_tokens，示例 cache_read=139），
    // https://docs.byteplus.com/en/docs/ModelArk/messages-api
    let anth = super::adapter_for("byteplus").extract_usage(
        r#"{"usage":{"input_tokens":31,"output_tokens":33,"cache_creation_input_tokens":0,"cache_read_input_tokens":139}}"#,
    );
    assert_eq!(anth, (31, 33, 139, 0, 0, vec![]));
}

/// BytePlus 成员（issue 128）三协议全链路等价家族基座：请求构造 / 非流式响应 / 流式事件
/// / 错误分类 / usage 抽取（接口①②③④⑤），base_url 取本地假上游地址（127.0.0.1），
/// 出站永不读 registry 内置端点。对照件 = GENERIC 平台 deepseek（同基座构件）。
#[test]
fn byteplus_full_pipeline_matches_family_base() {
    let input: ChatRequest = serde_json::from_value(json!({
        "model": "doubao-seed-2.1",
        "max_tokens": 128,
        "messages": [{ "role": "user", "content": "hello" }]
    }))
    .unwrap();
    let adapter = super::adapter_for("byteplus");
    let generic = super::adapter_for("deepseek");
    let platform = Protocol::BytePlus;

    // ① 请求构造：三协议路径与 body 同基座构件（本地假上游 URL 不参与转换）。
    for wire in [Protocol::OpenAI, Protocol::Anthropic, Protocol::OpenAIResponses] {
        let (body, path) = adapter.build_request(&input, &wire, &platform);
        let (base_body, base_path) = generic.build_request(&input, &wire, &platform);
        assert_eq!((body, path), (base_body, base_path), "{wire:?}");
    }
    assert_eq!(
        adapter
            .build_request(&input, &Protocol::OpenAI, &platform)
            .1,
        "/chat/completions"
    );

    // ② 非流式响应：openai wire → anthropic 客户端、anthropic wire → openai 客户端。
    let completion = json!({
        "id": "cmpl-1", "model": "doubao-seed-2.1",
        "choices": [{ "index": 0, "message": { "role": "assistant", "content": "hi" }, "finish_reason": "stop" }]
    });
    let converted = adapter
        .convert_response(&completion, &Protocol::OpenAI, &Protocol::Anthropic, "doubao-seed-2.1")
        .expect("openai 非流式应可转换");
    assert_eq!(converted["content"][0]["text"], "hi");
    assert_eq!(
        converted,
        generic
            .convert_response(&completion, &Protocol::OpenAI, &Protocol::Anthropic, "doubao-seed-2.1")
            .unwrap()
    );
    let message = json!({
        "id": "msg_1", "type": "message", "role": "assistant", "model": "doubao-seed-2.1",
        "content": [{ "type": "text", "text": "hi" }], "stop_reason": "end_turn",
        "usage": { "input_tokens": 31, "output_tokens": 33 }
    });
    let anth_converted = adapter
        .convert_response(&message, &Protocol::Anthropic, &Protocol::OpenAI, "doubao-seed-2.1")
        .expect("anthropic 非流式应可转换");
    assert_eq!(anth_converted["choices"][0]["message"]["content"], "hi");

    // ③ 流式事件：openai delta 帧按 BytePlus 官方分帧（reasoning_content 与 content 各一帧，
    // https://docs.byteplus.com/en/docs/ModelArk/streaming-output 示例）与 anthropic 帧同基座构件。
    let reasoning_frame = json!({
        "choices": [{ "index": 0, "delta": { "reasoning_content": "thinking" }, "usage": null }]
    });
    let content_frame = json!({
        "choices": [{ "index": 0, "delta": { "content": "hi" }, "usage": null }]
    });
    for (frame, marker) in [(&reasoning_frame, "thinking"), (&content_frame, "hi")] {
        let openai_events = adapter.parse_sse_frame(frame, &Protocol::OpenAI);
        assert_eq!(
            format!("{openai_events:?}"),
            format!("{:?}", generic.parse_sse_frame(frame, &Protocol::OpenAI))
        );
        assert!(format!("{openai_events:?}").contains(marker), "{marker}");
    }
    let anth_delta = json!({
        "type": "content_block_delta", "index": 0,
        "delta": { "type": "text_delta", "text": "hi" }
    });
    let anth_events = adapter.parse_sse_frame(&anth_delta, &Protocol::Anthropic);
    assert_eq!(
        format!("{anth_events:?}"),
        format!("{:?}", generic.parse_sse_frame(&anth_delta, &Protocol::Anthropic))
    );

    // ④ 错误分类见 ark_family_error_body（404 模型类 / 401 / 429 服务类），此处补 anthropic
    // wire 错误形状同基座。
    assert_eq!(
        adapter.classify_error(
            400,
            r#"{"type":"error","error":{"type":"invalid_request_error","message":"bad request"}}"#,
        ),
        UpstreamErrorClass::Unknown
    );
}

/// 百度千帆家族（issue 80）：openai 端点禁思考写官方顶层 `thinking: {"type": "disabled"}`
/// （文本生成 API「思考模式类型，默认值为 disabled」，`reasoning_effort` 词表无 none，
/// https://cloud.baidu.com/doc/qianfan-api/s/3m7of64lb ）；客户端带来的旧 `enable_thinking`
/// / `reasoning_effort` / vLLM 式 `chat_template_kwargs.enable_thinking` 一并剥除。
/// anthropic 端点同基座 anthropic 臂（MaaS Anthropic 兼容 API，
/// https://cloud.baidu.com/doc/qianfan/s/empm33i0q ）。
#[test]
fn qianfan_family_openai_endpoint_disables_thinking() {
    for code in ["qianfan", "qianfan_coding"] {
        let adapter = super::adapter_for(code);
        let url = "http://127.0.0.1:31234/v2";
        let mut body = json!({
            "model": "deepseek-v4-pro",
            "enable_thinking": true,
            "thinking": { "type": "enabled" },
            "reasoning_effort": "high",
            "chat_template_kwargs": { "enable_thinking": true },
            "generationConfig": { "thinkingConfig": { "thinkingBudget": 0 } },
        });
        adapter.apply_disable_thinking(&mut body, true, &Protocol::OpenAI, url, false);
        assert_eq!(body["thinking"], json!({ "type": "disabled" }), "{code}");
        assert!(body.get("enable_thinking").is_none(), "{code}: {body}");
        assert!(body.get("reasoning_effort").is_none(), "{code}: {body}");
        assert!(
            body.get("chat_template_kwargs")
                .and_then(|c| c.get("enable_thinking"))
                .is_none(),
            "{code}: {body}"
        );
        assert!(
            body.get("generationConfig")
                .and_then(|g| g.get("thinkingConfig"))
                .is_none(),
            "{code}: {body}"
        );

        // 不禁思考：只剥 aidog 私有字段，不写禁用参数、不动客户端旧值。
        let mut keep = json!({
            "model": "deepseek-v4-pro",
            "disable_thinking": false,
            "enable_thinking": true,
        });
        adapter.apply_disable_thinking(&mut keep, false, &Protocol::OpenAI, url, false);
        assert_eq!(keep["enable_thinking"], json!(true), "{code}: {keep}");
        assert!(keep.get("disable_thinking").is_none(), "{code}: {keep}");

        // 强制思考模型（仅支持开启不可关闭，registry thinking_toggleable: false）：
        // 只剥禁用形参数、不写任何禁用参数（同基座 forced_thinking 路径）。
        let mut forced = json!({ "model": "deepseek-v4-pro", "thinking": { "type": "disabled" } });
        adapter.apply_disable_thinking(&mut forced, true, &Protocol::OpenAI, url, true);
        assert!(forced.get("thinking").is_none(), "{code}: {forced}");

        // anthropic 端点走基座：thinking.type=disabled。
        let mut anth = json!({ "model": "deepseek-v4-pro" });
        adapter.apply_disable_thinking(
            &mut anth,
            true,
            &Protocol::Anthropic,
            "http://127.0.0.1:31234/anthropic",
            false,
        );
        assert_eq!(anth["thinking"]["type"], "disabled", "{code}: {anth}");
    }
}

/// 百度千帆家族错误分类沿用基座：错误体 `{"error":{code,message,type},"id"}`。
/// 401 invalid_model「The model does not exist …」按状态码归服务类（模型不存在挂 401，
/// 基座 401 先于业务码，不覆盖，见 QianfanFamilyAdapter doc）；400 invalid_model
/// 「model is empty」落 Unknown（调用方按模型类处理）；429 限流按状态码归服务类。
/// https://cloud.baidu.com/doc/qianfan-api/s/Om9b4yj3w
#[test]
fn qianfan_family_error_body() {
    let adapter = super::adapter_for("qianfan");
    assert_eq!(
        adapter.classify_error(
            401,
            r#"{"error":{"code":"invalid_model","message":"The model does not exist or you do not have access to it.","type":"invalid_request_error"},"id":"as-x"}"#,
        ),
        UpstreamErrorClass::Service
    );
    assert_eq!(
        adapter.classify_error(
            400,
            r#"{"error":{"code":"invalid_model","message":"model is empty","type":"invalid_request_error"},"id":"as-x"}"#,
        ),
        UpstreamErrorClass::Unknown
    );
    assert_eq!(
        adapter.classify_error(
            429,
            r#"{"error":{"code":"rpm_rate_limit_exceeded","message":"Rate limit reached for RPM","type":"rate_limit_exceeded"},"id":"as-x"}"#,
        ),
        UpstreamErrorClass::Service
    );
    // Coding Plan 专属码同按状态码归服务类（qianfan_coding）。
    assert_eq!(
        super::adapter_for("qianfan_coding").classify_error(
            429,
            r#"{"error":{"code":"coding_plan_hour_quota_exceeded","message":"Coding Plan hour quota has been exceeded","type":"quota_exceeded"},"id":"as-x"}"#,
        ),
        UpstreamErrorClass::Service
    );
}

/// usage 抽取沿用基座 openai 构件：`prompt_tokens_details.cached_tokens` 从 input 扣除、
/// `completion_tokens_details.reasoning_tokens` 并入 output（千帆 Chat API 字段名与基座一致，
/// https://cloud.baidu.com/doc/qianfan-api/s/3m7of64lb ）。
#[test]
fn qianfan_family_usage_extraction() {
    let adapter = super::adapter_for("qianfan");
    let body = r#"{
        "usage": {
            "prompt_tokens": 100,
            "completion_tokens": 50,
            "total_tokens": 150,
            "prompt_tokens_details": { "cached_tokens": 40, "search_tokens": 10 },
            "completion_tokens_details": { "reasoning_tokens": 20 }
        }
    }"#;
    assert_eq!(adapter.extract_usage(body), (60, 50, 40, 0, 0, vec![]));
}

/// coding_plan 注入保持 no-op：官方 Claude Code 接入页（Coding Plan）只要求设
/// ANTHROPIC_BASE_URL 与专属 API Key，无出站附加字段
/// （https://cloud.baidu.com/doc/qianfan/s/0mn2mnemj ）。
#[test]
fn qianfan_family_coding_plan_keeps_noop() {
    for (platform, wire) in [
        (Protocol::QianFan, Protocol::Anthropic),
        (Protocol::QianFan, Protocol::OpenAI),
        (Protocol::QianfanCoding, Protocol::OpenAI),
        (Protocol::QianfanCoding, Protocol::Anthropic),
    ] {
        let mut body = json!({ "model": "deepseek-v4-pro", "messages": [] });
        let original = body.clone();
        super::adapter_for("qianfan_coding").inject_coding_plan(&mut body, &platform, &wire);
        assert_eq!(body, original, "{platform:?} {wire:?}");
    }
}

/// 覆写只作用于家族成员：非成员第三方 OpenAI 兼容端点仍写基座 vLLM 式
/// `chat_template_kwargs.enable_thinking`。
#[test]
fn qianfan_override_scoped_to_family_members() {
    let mut other = json!({ "model": "m" });
    super::adapter_for("aihubmix").apply_disable_thinking(
        &mut other,
        true,
        &Protocol::OpenAI,
        "http://127.0.0.1:31234/v1",
        false,
    );
    assert_eq!(other["chat_template_kwargs"]["enable_thinking"], json!(false));
    assert!(other.get("thinking").is_none());
}

/// SiliconFlow 家族（issue 82）：openai 端点禁思考写官方顶层 `enable_thinking: false`
/// （API 文档「enable_thinking boolean 在推理模式与非推理模式之间切换」，无 thinking 对象、
/// 无 chat_template_kwargs），写 false 前剥客户端旧 `thinking` / `reasoning_effort` /
/// `enable_thinking` / chat_template_kwargs / generationConfig；anthropic 端点官方
/// `thinking.type=disabled` 与基座同形，不覆盖；强制思考路径只剥不写（基座）。
#[test]
fn siliconflow_members_disable_thinking_with_native_parameter() {
    for code in ["siliconflow", "siliconflow_en"] {
        let adapter = super::adapter_for(code);
        let mut body = json!({
            "model": "deepseek-ai/DeepSeek-V3.2",
            "enable_thinking": true,
            "reasoning_effort": "high",
            "thinking": {"type": "enabled"},
            "chat_template_kwargs": {"enable_thinking": true},
        });
        adapter.apply_disable_thinking(
            &mut body,
            true,
            &Protocol::OpenAI,
            "http://127.0.0.1:31234/v1",
            false,
        );
        assert_eq!(body["enable_thinking"], json!(false), "{code}");
        assert!(body.get("thinking").is_none(), "{code}");
        assert!(body.get("reasoning_effort").is_none(), "{code}");
        assert!(
            body["chat_template_kwargs"].get("enable_thinking").is_none(),
            "{code}"
        );

        // 禁思考关：不写任何参数（官方默认值未证实，aidog 不注入）。
        let mut keep = json!({ "model": "m", "enable_thinking": true });
        adapter.apply_disable_thinking(
            &mut keep,
            false,
            &Protocol::OpenAI,
            "http://127.0.0.1:31234/v1",
            false,
        );
        assert_eq!(keep["enable_thinking"], json!(true), "{code}");

        // 强制思考模型（registry thinking_toggleable: false）：只剥禁用形参数、不写 false。
        let mut forced = json!({ "model": "deepseek-ai/DeepSeek-R1", "thinking": {"type": "disabled"} });
        adapter.apply_disable_thinking(
            &mut forced,
            true,
            &Protocol::OpenAI,
            "http://127.0.0.1:31234/v1",
            true,
        );
        assert!(forced.get("thinking").is_none(), "{code}");
        assert!(forced.get("enable_thinking").is_none(), "{code}");

        // anthropic wire：官方 thinking 对象与基座同形。
        let mut ant = json!({ "model": "m" });
        adapter.apply_disable_thinking(
            &mut ant,
            true,
            &Protocol::Anthropic,
            "http://127.0.0.1:31234",
            false,
        );
        assert_eq!(ant["thinking"]["type"], "disabled", "{code}");
    }
}

/// SiliconFlow 家族其余接口继承协议构件：请求构造 / 响应转换 / 流式 / 错误分类 / usage
/// 与通用适配逐字节等价；顶层 `{"code":20012,"message":"Model does not exist..."}`
/// （官方错误示例，无 error 包裹）按基座特征归模型类。
#[test]
fn siliconflow_family_preserves_generic_protocol_behavior() {
    let input: ChatRequest = serde_json::from_value(json!({
        "model": "deepseek-ai/DeepSeek-V3.2",
        "max_tokens": 64,
        "messages": [{ "role": "user", "content": "hi" }]
    }))
    .unwrap();
    let generic = super::adapter_for("deepseek");
    let completion = json!({
        "id": "cmpl-1", "model": "deepseek-ai/DeepSeek-V3.2",
        "choices": [{ "index": 0, "message": { "role": "assistant", "content": "ok" }, "finish_reason": "stop" }],
        "usage": { "prompt_tokens": 10, "completion_tokens": 2, "total_tokens": 12,
            "prompt_tokens_details": { "cached_tokens": 4 },
            "prompt_cache_hit_tokens": 4, "prompt_cache_miss_tokens": 6 }
    });
    let delta = json!({
        "id": "cmpl-1", "choices": [{ "index": 0, "delta": { "reasoning_content": "think", "content": "ok" } }]
    });
    let sf_error = r#"{"code":20012,"message":"Model does not exist. Please check it carefully.","data":null}"#;
    let anthropic_message = json!({
        "id": "msg_1", "type": "message", "role": "assistant", "model": "m",
        "content": [{ "type": "text", "text": "ok" }], "stop_reason": "end_turn",
        "usage": { "input_tokens": 5, "output_tokens": 3,
            "cache_read_input_tokens": 2, "cache_creation_input_tokens": 1 }
    });

    for (code, platform) in [
        ("siliconflow", Protocol::SiliconFlow),
        ("siliconflow_en", Protocol::SiliconFlowEn),
    ] {
        let adapter = super::adapter_for(code);
        for wire in [Protocol::OpenAI, Protocol::Anthropic] {
            let (body, path) = adapter.build_request(&input, &wire, &platform);
            let (base_body, base_path) = generic.build_request(&input, &wire, &platform);
            assert_eq!((body, path), (base_body, base_path), "{code}/{wire:?}");
        }
        let converted = adapter
            .convert_response(&completion, &Protocol::OpenAI, &Protocol::Anthropic, "m")
            .expect("OpenAI response should convert to Anthropic");
        assert_eq!(
            converted,
            generic
                .convert_response(&completion, &Protocol::OpenAI, &Protocol::Anthropic, "m")
                .unwrap(),
            "{code} non-stream parity"
        );
        assert_eq!(converted["content"][0]["text"], "ok", "{code}");

        let events = adapter.parse_sse_frame(&delta, &Protocol::OpenAI);
        assert_eq!(
            format!("{events:?}"),
            format!("{:?}", generic.parse_sse_frame(&delta, &Protocol::OpenAI)),
            "{code} stream parity"
        );
        assert!(format!("{events:?}").contains("ok"), "{code} stream content: {events:?}");

        let anthropic_converted = adapter
            .convert_response(&anthropic_message, &Protocol::Anthropic, &Protocol::OpenAI, "m")
            .expect("Anthropic response should convert to OpenAI");
        assert_eq!(
            anthropic_converted,
            generic
                .convert_response(&anthropic_message, &Protocol::Anthropic, &Protocol::OpenAI, "m")
                .unwrap(),
            "{code} anthropic non-stream parity"
        );

        // 官方错误示例（顶层 code/message，无 error 包裹）：模型不存在归模型类。
        assert_eq!(
            adapter.classify_error(400, sf_error),
            UpstreamErrorClass::Model,
            "{code} top-level code/message model error"
        );
        assert_eq!(
            adapter.classify_error(429, r#"{"code":30002,"message":"rate limit exceeded","data":null}"#),
            UpstreamErrorClass::Service,
            "{code} 429 rate limit"
        );

        // usage：cached_tokens 与 prompt_cache_hit_tokens 同源（基座取其一并从 input 扣除）。
        assert_eq!(
            adapter.extract_usage(&completion.to_string()),
            generic.extract_usage(&completion.to_string()),
            "{code} usage parity"
        );
        let usage = adapter.extract_usage(&completion.to_string());
        assert_eq!(usage.0, 6, "{code} input = prompt - cache hit");
        assert_eq!(usage.2, 4, "{code} cache read");
    }
}

/// StepFun 家族（issue 83）：openai wire 官方没有关闭思考值，禁思考不写基座第三方参数；
/// anthropic wire 的 `thinking` 对象认受性未证实，按票面保留框架默认（`thinking.type=disabled`），
/// 同时剥官方 `output_config.effort`；不相关输出配置保留。
#[test]
fn stepfun_members_disable_thinking_strips_without_injection() {
    for code in ["stepfun", "stepfun_en"] {
        let adapter = super::adapter_for(code);
        let mut body = json!({
            "model": "step-5-preview",
            "reasoning_effort": "high",
            "thinking": {"type": "enabled"},
            "output_config": {"effort": "high"},
            "chat_template_kwargs": {"enable_thinking": true},
        });
        adapter.apply_disable_thinking(
            &mut body,
            true,
            &Protocol::OpenAI,
            "https://api.stepfun.com/v1",
            false,
        );
        assert!(body.get("reasoning_effort").is_none(), "{code}");
        assert!(body.get("thinking").is_none(), "{code}");
        // output_config 是 Messages 端点参数，openai wire 不碰它。
        assert_eq!(body["output_config"]["effort"], "high", "{code}");
        assert!(
            body["chat_template_kwargs"].get("enable_thinking").is_none(),
            "{code}"
        );
        // 官方无关闭形态：不注入任何禁思考参数（无 enable_thinking / chat_template_kwargs 注入）。
        assert!(body.get("enable_thinking").is_none(), "{code}");

        // anthropic wire：`thinking` 对象认受性未证实 → 保持框架默认（thinking.type=disabled），
        // 仅额外剥官方 output_config.effort 思考档位。
        let mut ant = json!({
            "model": "step-5-preview",
            "thinking": {"type": "enabled"},
            "output_config": {"effort": "low", "format": {"type": "json"}},
        });
        adapter.apply_disable_thinking(
            &mut ant,
            true,
            &Protocol::Anthropic,
            "https://api.stepfun.com/step_plan",
            false,
        );
        assert_eq!(ant["thinking"]["type"], "disabled", "{code}");
        assert_eq!(ant["output_config"]["format"]["type"], "json", "{code}");
        assert!(ant["output_config"].get("effort").is_none(), "{code}");

        // anthropic wire 禁思考关：不剥 output_config（基座行为）。
        let mut keep_ant = json!({ "output_config": {"effort": "high"} });
        adapter.apply_disable_thinking(
            &mut keep_ant,
            false,
            &Protocol::Anthropic,
            "https://api.stepfun.com/step_plan",
            false,
        );
        assert_eq!(keep_ant["output_config"]["effort"], "high", "{code}");

        // 禁思考关：不剥客户端思考参数（仅剥内部 disable_thinking 标记，基座行为）。
        let mut keep = json!({ "model": "m", "reasoning_effort": "high" });
        adapter.apply_disable_thinking(
            &mut keep,
            false,
            &Protocol::OpenAI,
            "https://api.stepfun.com/v1",
            false,
        );
        assert_eq!(keep["reasoning_effort"], "high", "{code}");
        assert!(keep.get("disable_thinking").is_none(), "{code}");

        // 强制思考模型：只剥禁用形参数、不写任何参数（基座）。
        let mut forced = json!({ "model": "step-3.5-flash-2603", "thinking": {"type": "disabled"} });
        adapter.apply_disable_thinking(
            &mut forced,
            true,
            &Protocol::OpenAI,
            "https://api.stepfun.com/v1",
            true,
        );
        assert!(forced.get("thinking").is_none(), "{code}");
        assert!(forced.get("reasoning_effort").is_none(), "{code}");
    }
}

/// StepFun 家族其余接口继承协议构件：请求构造 / 响应转换 / 流式 / 错误分类 / usage
/// 与通用适配逐字节等价；官方错误码（400 模型不存在 / 401 认证 / 429 限频）按基座
/// 特征归类。https://platform.stepfun.com/docs/zh/api-reference/error-codes.md
#[test]
fn stepfun_family_preserves_generic_protocol_behavior() {
    let input: ChatRequest = serde_json::from_value(json!({
        "model": "step-5-preview",
        "max_tokens": 64,
        "messages": [{ "role": "user", "content": "hi" }]
    }))
    .unwrap();
    let generic = super::adapter_for("deepseek");
    let completion = json!({
        "id": "cmpl-1", "model": "step-5-preview",
        "choices": [{ "index": 0, "message": { "role": "assistant", "content": "ok" }, "finish_reason": "stop" }],
        "usage": { "prompt_tokens": 90, "completion_tokens": 24, "total_tokens": 114 }
    });
    let delta = json!({
        "id": "cmpl-1", "choices": [{ "index": 0, "delta": { "reasoning_content": "think", "content": "ok" } }]
    });
    let anthropic_message = json!({
        "id": "msg_1", "type": "message", "role": "assistant", "model": "m",
        "content": [{ "type": "text", "text": "ok" }], "stop_reason": "end_turn",
        "usage": { "input_tokens": 5, "output_tokens": 3 }
    });

    for (code, platform) in [
        ("stepfun", Protocol::StepFun),
        ("stepfun_en", Protocol::StepFunEn),
    ] {
        let adapter = super::adapter_for(code);
        for wire in [Protocol::OpenAI, Protocol::Anthropic] {
            let (body, path) = adapter.build_request(&input, &wire, &platform);
            let (base_body, base_path) = generic.build_request(&input, &wire, &platform);
            assert_eq!((body, path), (base_body, base_path), "{code}/{wire:?}");
        }
        let converted = adapter
            .convert_response(&completion, &Protocol::OpenAI, &Protocol::Anthropic, "m")
            .expect("OpenAI response should convert to Anthropic");
        assert_eq!(
            converted,
            generic
                .convert_response(&completion, &Protocol::OpenAI, &Protocol::Anthropic, "m")
                .unwrap(),
            "{code} non-stream parity"
        );
        assert_eq!(converted["content"][0]["text"], "ok", "{code}");

        let events = adapter.parse_sse_frame(&delta, &Protocol::OpenAI);
        assert_eq!(
            format!("{events:?}"),
            format!("{:?}", generic.parse_sse_frame(&delta, &Protocol::OpenAI)),
            "{code} stream parity"
        );
        assert!(format!("{events:?}").contains("ok"), "{code} stream content: {events:?}");

        let anthropic_converted = adapter
            .convert_response(&anthropic_message, &Protocol::Anthropic, &Protocol::OpenAI, "m")
            .expect("Anthropic response should convert to OpenAI");
        assert_eq!(
            anthropic_converted,
            generic
                .convert_response(&anthropic_message, &Protocol::Anthropic, &Protocol::OpenAI, "m")
                .unwrap(),
            "{code} anthropic non-stream parity"
        );

        // 错误归类：模型不存在归模型类；401 / 429 按状态码归服务类。
        assert_eq!(
            adapter.classify_error(
                400,
                r#"{"error":{"message":"模型不存在或无权限","type":"request_params_invalid"}}"#,
            ),
            UpstreamErrorClass::Model,
            "{code} model error"
        );
        assert_eq!(
            adapter.classify_error(401, r#"{"error":{"message":"invalid api key"}}"#),
            UpstreamErrorClass::Service,
            "{code} 401"
        );
        assert_eq!(
            adapter.classify_error(429, r#"{"error":{"message":"rate limit exceeded"}}"#),
            UpstreamErrorClass::Service,
            "{code} 429"
        );

        // usage：无缓存字段时基座构件直取 prompt/completion。
        assert_eq!(
            adapter.extract_usage(&completion.to_string()),
            generic.extract_usage(&completion.to_string()),
            "{code} usage parity"
        );
        let usage = adapter.extract_usage(&completion.to_string());
        assert_eq!(usage.0, 90, "{code} input tokens");
        assert_eq!(usage.1, 24, "{code} output tokens");
    }
}

/// StepFun 默认 `reasoning_format=general` 用 reasoning 字段，缓存计数来自官方 cached_tokens 字段。
#[test]
fn stepfun_general_reasoning_and_cached_usage_are_supported() {
    let adapter = super::adapter_for("stepfun");
    let completion = json!({
        "id": "cmpl-stepfun",
        "model": "step-5-preview",
        "choices": [{
            "index": 0,
            "message": {"role": "assistant", "content": "answer", "reasoning": "thought"},
            "finish_reason": "stop"
        }],
        "usage": {
            "prompt_tokens": 10,
            "completion_tokens": 3,
            "total_tokens": 13,
            "prompt_tokens_details": {"cached_tokens": 4}
        }
    });
    let converted = adapter
        .convert_response(&completion, &Protocol::OpenAI, &Protocol::Anthropic, "step-5-preview")
        .unwrap();
    assert!(converted["content"].as_array().unwrap().iter().any(|block| {
        block["type"] == "thinking" && block["thinking"] == "thought"
    }));
    assert!(converted["content"].as_array().unwrap().iter().any(|block| {
        block["type"] == "text" && block["text"] == "answer"
    }));

    let (input, output, cache_read, cache_write, _, _) =
        adapter.extract_usage(&completion.to_string());
    assert_eq!((input, output, cache_read, cache_write), (6, 3, 4, 0));

    let delta = json!({
        "id": "cmpl-stepfun",
        "choices": [{"index": 0, "delta": {"reasoning": "thought"}}]
    });
    let events = adapter.parse_sse_frame(&delta, &Protocol::OpenAI);
    assert!(format!("{events:?}").contains("thought"), "{events:?}");
}

/// 本地假上游（禁真实外呼）：分块循环完整读取请求至 Content-Length 声明的 body 末尾
/// （不受单次 read 上限影响），服务端断言请求行路径与 body JSON 后回写预置响应并关闭连接。
async fn tencent_serve_one(
    response: Vec<u8>,
    expect_path: &'static str,
    assert_body: impl FnOnce(&Value) + Send + 'static,
) -> (String, tokio::task::JoinHandle<()>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let task = tokio::spawn(async move {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let (mut stream, _) = listener.accept().await.unwrap();
        let mut buf = Vec::new();
        let mut chunk = [0u8; 4096];
        let header_end = loop {
            let n = stream.read(&mut chunk).await.unwrap();
            assert!(n > 0, "upstream closed before headers complete");
            buf.extend_from_slice(&chunk[..n]);
            if let Some(pos) = tencent_find(&buf, b"\r\n\r\n") {
                break pos;
            }
        };
        let head = String::from_utf8_lossy(&buf[..header_end]).to_string();
        let path = head
            .lines()
            .next()
            .and_then(|l| l.split(' ').nth(1))
            .unwrap_or("")
            .to_string();
        let content_length: usize = head
            .lines()
            .find(|l| l.to_ascii_lowercase().starts_with("content-length:"))
            .and_then(|l| l.split(':').nth(1))
            .and_then(|v| v.trim().parse().ok())
            .unwrap_or(0);
        let body_start = header_end + 4;
        while buf.len() < body_start + content_length {
            let n = stream.read(&mut chunk).await.unwrap();
            assert!(n > 0, "upstream closed before body complete");
            buf.extend_from_slice(&chunk[..n]);
        }
        let body: Value = serde_json::from_slice(&buf[body_start..body_start + content_length])
            .expect("upstream request body should be valid JSON");
        assert_eq!(path, expect_path, "upstream request path");
        assert_body(&body);
        stream.write_all(&response).await.unwrap();
    });
    (addr.to_string(), task)
}

fn tencent_find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|w| w == needle)
}

fn tencent_json_response(response: &str) -> Vec<u8> {
    format!(
        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{response}",
        response.len()
    )
    .into_bytes()
}

/// SSE 响应：frames 各元素为完整 `data: ...\n\n` 帧，`[DONE]` 由调用方拼在末尾。
fn tencent_sse_response(frames: &[String]) -> Vec<u8> {
    let body = frames.join("");
    format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )
    .into_bytes()
}

/// 真实生产 HTTP client 往返：用 reqwest POST 序列化适配器构造的 body，读取完整响应。
async fn tencent_roundtrip(addr: &str, path: &str, body: &Value) -> (u16, Vec<u8>) {
    let response = reqwest::Client::new()
        .post(format!("http://{addr}{path}"))
        .json(body)
        .send()
        .await
        .unwrap();
    let status = response.status().as_u16();
    (status, response.bytes().await.unwrap().to_vec())
}

/// SSE 帧切分：按 `\n\n` 拆帧、剥 `data: ` 前缀、跳过 `[DONE]`，解析成 JSON。
fn tencent_sse_frames(raw_body: &str) -> Vec<Value> {
    raw_body
        .split("\n\n")
        .filter_map(|frame| frame.lines().find_map(|line| line.strip_prefix("data: ")))
        .filter(|data| data.trim() != "[DONE]")
        .map(|data| serde_json::from_str(data).unwrap())
        .collect()
}

/// 验证三个家族成员的 OpenAI 与 Anthropic 协议都走本地假上游的完整 HTTP 往返
/// （服务端对请求行路径与请求体 JSON 断言），并按平台已证实的缓存字段抽取 usage。
#[tokio::test]
async fn tencent_family_protocols_use_local_upstream_and_extract_usage() {
    let req: ChatRequest = serde_json::from_value(json!({
        "model": "tc-code-latest",
        "max_tokens": 64,
        "messages": [{ "role": "user", "content": "hello" }]
    }))
    .unwrap();
    let generic = super::adapter_for("deepseek");
    for code in ["tencent_coding", "tencent_tokenhub", "tencent_tokenhub_intl"] {
        let adapter = super::adapter_for(code);
        assert!(super::lookup(code).is_some(), "{code} registration");
        for wire in [Protocol::OpenAI, Protocol::Anthropic] {
            let platform = match code {
                "tencent_coding" => Protocol::TencentCoding,
                "tencent_tokenhub" => Protocol::TencentTokenhub,
                _ => Protocol::TencentTokenhubIntl,
            };
            let (body, path) = adapter.build_request(&req, &wire, &platform);
            let (base_body, base_path) = generic.build_request(&req, &wire, &platform);
            assert_eq!((body.clone(), path.clone()), (base_body, base_path), "{code}/{wire:?}");
            let (response, expect_path, expected) = if wire == Protocol::OpenAI {
                (
                    tencent_json_response(
                        r#"{"id":"cmpl-local","model":"tc-code-latest","choices":[],"usage":{"prompt_tokens":100,"completion_tokens":20,"total_tokens":120,"prompt_tokens_details":{"cached_tokens":30}}}"#,
                    ),
                    "/chat/completions",
                    (70, 20, 30, 0),
                )
            } else {
                (
                    tencent_json_response(
                        r#"{"id":"msg_local","type":"message","role":"assistant","model":"tc-code-latest","content":[],"stop_reason":"end_turn","usage":{"input_tokens":100,"output_tokens":20,"cache_read_input_tokens":30,"cache_creation_input_tokens":5}}"#,
                    ),
                    "/v1/messages",
                    (100, 20, 30, 5),
                )
            };
            let body_for_assert = body.clone();
            let (addr, upstream) = tencent_serve_one(response, expect_path, move |received| {
                assert_eq!(received["model"].as_str(), Some("tc-code-latest"), "{code} upstream model");
                assert_eq!(received["messages"][0]["role"].as_str(), Some("user"), "{code} upstream messages");
                assert!(received["messages"][0].get("content").is_some(), "{code} upstream content: {received}");
                assert_eq!(received, &body_for_assert, "{code} wire body == build_request output");
            })
            .await;
            let (status, raw) = tencent_roundtrip(&addr, expect_path, &body).await;
            upstream.await.unwrap();
            assert_eq!(status, 200, "{code}/{wire:?}");
            let upstream_body = String::from_utf8_lossy(&raw);
            let (input, output, cache_read, cache_write, _, _) = adapter.extract_usage(&upstream_body);
            assert_eq!((input, output, cache_read, cache_write), expected, "{code}/{wire:?}");
        }
    }
}

/// 禁思考逐成员作用域（官方取证核对 2026-11-23）：TokenHub 两成员 openai wire 写平台标准
/// `thinking: {"type":"disabled"}`（135872 思考模式参数表「标准方式」+ 工具调用节「需显式传入
/// `thinking: {"type": "disabled"}` 后使用」），剥开启型参数；Coding Plan（`tencent_coding`）
/// 官方文档（130092）只给专属 Key / Base URL 与动态模型库、未列任何思考参数——未证实范围
/// 按票面保持框架默认（第三方端点 `chat_template_kwargs.enable_thinking=false`），
/// 成员 issue 137 取证后再收窄。断言经本地假上游完整 HTTP 往返、服务端对线上 body JSON 判定。
#[tokio::test]
async fn tencent_disable_thinking_reaches_wire_per_member_scope() {
    let mut body = json!({
        "model": "tc-code-latest",
        "thinking": { "type": "enabled", "budget_tokens": 8192 },
        "thinking_budget": 4096,
        "reasoning_effort": "high"
    });
    let response = tencent_json_response(
        r#"{"id":"cmpl-local","model":"tc-code-latest","choices":[],"usage":{"prompt_tokens":1,"completion_tokens":1,"total_tokens":2}}"#,
    );
    for (code, expected_thinking, expected_ctk) in [
        ("tencent_tokenhub", Some("disabled"), None),
        ("tencent_tokenhub_intl", Some("disabled"), None),
        ("tencent_coding", None, Some(false)),
    ] {
        let adapter = super::adapter_for(code);
        let mut outbound = body.clone();
        adapter.apply_disable_thinking(
            &mut outbound,
            true,
            &Protocol::OpenAI,
            "http://127.0.0.1:1/v1",
            false,
        );
        let (addr, upstream) = tencent_serve_one(response.clone(), "/chat/completions", move |received| {
            let thinking_type = received
                .get("thinking")
                .and_then(|t| t.get("type"))
                .and_then(Value::as_str)
                .map(str::to_string);
            assert_eq!(
                thinking_type.as_deref(),
                expected_thinking,
                "{code} upstream thinking.type: {received}"
            );
            assert_eq!(
                received
                    .get("chat_template_kwargs")
                    .and_then(|c| c.get("enable_thinking"))
                    .and_then(Value::as_bool),
                expected_ctk,
                "{code} upstream chat_template_kwargs.enable_thinking: {received}"
            );
        })
        .await;
        let (status, _) = tencent_roundtrip(&addr, "/chat/completions", &outbound).await;
        upstream.await.unwrap();
        assert_eq!(status, 200, "{code}");
    }
    // 客户端侧同判：TokenHub 成员开启型参数被剥净；family 其它接口不涉。
    let adapter = super::adapter_for("tencent_tokenhub");
    adapter.apply_disable_thinking(&mut body, true, &Protocol::OpenAI, "http://127.0.0.1:1/v1", false);
    assert_eq!(body["thinking"]["type"], "disabled");
    assert!(body.get("thinking_budget").is_none(), "{body}");
    assert!(body.get("reasoning_effort").is_none(), "{body}");
}

/// 两端点流式（本地假上游完整 SSE 往返）：openai wire `data: {chunk}` 帧流
/// （delta.reasoning_content 先于 content、`data: [DONE]` 结束、末帧 usage）与
/// anthropic wire 标准事件链（message_start / content_block_delta / message_delta /
/// message_stop），经基座构件逐帧解析并从 openai 末帧 usage 抽取用量。
#[tokio::test]
async fn tencent_family_stream_frames_parse_via_local_upstream() {
    let adapter = super::adapter_for("tencent_tokenhub");
    let stream_req: ChatRequest = serde_json::from_value(json!({
        "model": "hy3",
        "max_tokens": 32,
        "stream": true,
        "messages": [{ "role": "user", "content": "hello" }]
    }))
    .unwrap();
    let (mut openai_request, openai_path) =
        adapter.build_request(&stream_req, &Protocol::OpenAI, &Protocol::TencentTokenhub);
    super::hooks::apply_field_passthrough(
        &mut openai_request,
        &json!({ "stream_options": { "include_usage": true } }),
        &Protocol::OpenAI,
        "http://127.0.0.1",
    );
    let openai_request_for_assert = openai_request.clone();
    assert_eq!(openai_path, "/chat/completions");
    assert_eq!(openai_request["stream"], true);
    let openai_frames = [
        format!(
            "data: {}\n\n",
            r#"{"id":"c1","choices":[{"index":0,"delta":{"role":"assistant"}}]}"#
        ),
        format!(
            "data: {}\n\n",
            r#"{"id":"c1","choices":[{"index":0,"delta":{"reasoning_content":"think"}}]}"#
        ),
        format!(
            "data: {}\n\n",
            r#"{"id":"c1","choices":[{"index":0,"delta":{"content":"ok"}}]}"#
        ),
        format!(
            "data: {}\n\n",
            r#"{"id":"c1","choices":[{"index":0,"delta":{},"finish_reason":"stop"}],"usage":{"prompt_tokens":90,"completion_tokens":24,"total_tokens":114}}"#
        ),
        "data: [DONE]\n\n".to_string(),
    ];
    let (addr, upstream) = tencent_serve_one(
        tencent_sse_response(&openai_frames),
        "/chat/completions",
        move |received| {
            assert_eq!(
                received["stream_options"]["include_usage"],
                json!(true),
                "openai stream request should request usage: {received}"
            );
            assert_eq!(
                received,
                &openai_request_for_assert,
                "openai wire body == build_request output"
            );
        },
    )
    .await;
    let (status, raw) = tencent_roundtrip(&addr, "/chat/completions", &openai_request).await;
    upstream.await.unwrap();
    assert_eq!(status, 200);
    let raw_body = String::from_utf8_lossy(&raw);
    let events: Vec<ChatStreamEvent> = tencent_sse_frames(&raw_body)
        .iter()
        .flat_map(|frame| adapter.parse_sse_frame(frame, &Protocol::OpenAI))
        .collect();
    assert!(
        events.iter().any(|e| matches!(e, ChatStreamEvent::ReasoningDelta { text } if text == "think")),
        "openai reasoning event: {events:?}"
    );
    assert!(
        events.iter().any(|e| matches!(e, ChatStreamEvent::Delta { text } if text == "ok")),
        "openai content event: {events:?}"
    );
    assert!(
        events.iter().any(|e| matches!(e, ChatStreamEvent::Stop { .. })),
        "openai stop event: {events:?}"
    );
    assert!(
        events.iter().any(|e| matches!(e, ChatStreamEvent::Usage { usage }
            if usage.prompt_tokens == Some(90) && usage.completion_tokens == Some(24))),
        "openai stream usage event: {events:?}"
    );

    let anthropic_frames = [
        "event: message_start\ndata: {\"type\":\"message_start\",\"message\":{\"id\":\"msg_1\",\"type\":\"message\",\"role\":\"assistant\",\"model\":\"hy3\",\"content\":[],\"usage\":{\"input_tokens\":12,\"output_tokens\":0}}}\n\n".to_string(),
        "event: content_block_start\ndata: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"text\"}}\n\n".to_string(),
        "event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"ok\"}}\n\n".to_string(),
        "event: content_block_stop\ndata: {\"type\":\"content_block_stop\",\"index\":0}\n\n".to_string(),
        "event: message_delta\ndata: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"end_turn\"},\"usage\":{\"output_tokens\":6}}\n\n".to_string(),
        "event: message_stop\ndata: {\"type\":\"message_stop\"}\n\n".to_string(),
    ];
    let (anthropic_request, anthropic_path) =
        adapter.build_request(&stream_req, &Protocol::Anthropic, &Protocol::TencentTokenhub);
    assert_eq!(anthropic_path, "/v1/messages");
    assert_eq!(anthropic_request["stream"], true);
    let anthropic_request_for_assert = anthropic_request.clone();
    let (addr, upstream) = tencent_serve_one(
        tencent_sse_response(&anthropic_frames),
        "/v1/messages",
        move |received| {
            assert_eq!(
                received,
                &anthropic_request_for_assert,
                "anthropic wire body == build_request output"
            );
        },
    )
    .await;
    let (status, raw) = tencent_roundtrip(&addr, "/v1/messages", &anthropic_request).await;
    upstream.await.unwrap();
    assert_eq!(status, 200);
    let raw_body = String::from_utf8_lossy(&raw);
    let events: Vec<ChatStreamEvent> = tencent_sse_frames(&raw_body)
        .iter()
        .flat_map(|frame| adapter.parse_sse_frame(frame, &Protocol::Anthropic))
        .collect();
    assert!(
        events.iter().any(|e| matches!(e, ChatStreamEvent::Delta { text } if text == "ok")),
        "anthropic text event: {events:?}"
    );
    let mut merged = crate::types::Usage::default();
    for event in &events {
        if let ChatStreamEvent::Usage { usage } = event {
            merged.merge_partial(usage);
        }
    }
    assert_eq!(
        (merged.prompt_tokens, merged.completion_tokens),
        (Some(12), Some(6)),
        "anthropic partial usage events (message_start input + message_delta output) merged: {events:?}"
    );
}

// ── issue 85：小米 MiMo 家族（xiaomi_mimo / xiaomi_mimo_coding / xiaomi_mimo_coding_en）──
// 家族取证（官方文档核对日期 2026-11-23）见 aidog_adapter platform::registry
// `MimoFamilyAdapter` 模块注释。本节全部走本地假上游，禁真实外呼。

fn mimo_platform(code: &str) -> Protocol {
    match code {
        "xiaomi_mimo" => Protocol::XiaomiMimo,
        "xiaomi_mimo_coding" => Protocol::XiaomiMimoCoding,
        _ => Protocol::XiaomiMimoCodingEn,
    }
}

/// 三成员 openai/anthropic 两 wire 经本地假上游完整 HTTP 往返（服务端读满
/// Content-Length、对请求行路径与请求体 JSON 断言），usage 按官方字段抽取：
/// openai `prompt_tokens_details.cached_tokens`（从 input 扣除）；anthropic
/// `cache_read_input_tokens`（官方无 cache_creation 字段，读不到即 0）。
#[tokio::test]
async fn mimo_family_protocols_use_local_upstream_and_extract_usage() {
    let req: ChatRequest = serde_json::from_value(json!({
        "model": "mimo-v2.6-pro",
        "max_tokens": 64,
        "messages": [{ "role": "user", "content": "hello" }]
    }))
    .unwrap();
    let generic = super::adapter_for("deepseek");
    for code in ["xiaomi_mimo", "xiaomi_mimo_coding", "xiaomi_mimo_coding_en"] {
        let adapter = super::adapter_for(code);
        assert!(super::lookup(code).is_some(), "{code} registration");
        let platform = mimo_platform(code);
        for wire in [Protocol::OpenAI, Protocol::Anthropic] {
            let (body, path) = adapter.build_request(&req, &wire, &platform);
            let (base_body, base_path) = generic.build_request(&req, &wire, &platform);
            assert_eq!((body.clone(), path.clone()), (base_body, base_path), "{code}/{wire:?}");
            let (response, expect_path, expected) = if wire == Protocol::OpenAI {
                (
                    tencent_json_response(
                        r#"{"id":"cmpl-local","model":"mimo-v2.6-pro","choices":[],"usage":{"prompt_tokens":100,"completion_tokens":20,"total_tokens":120,"completion_tokens_details":{"reasoning_tokens":5},"prompt_tokens_details":{"cached_tokens":30}}}"#,
                    ),
                    "/chat/completions",
                    (70, 20, 30, 0),
                )
            } else {
                (
                    tencent_json_response(
                        r#"{"id":"msg_local","type":"message","role":"assistant","model":"mimo-v2.6-pro","content":[],"stop_reason":"end_turn","usage":{"input_tokens":100,"output_tokens":20,"cache_read_input_tokens":30}}"#,
                    ),
                    "/v1/messages",
                    (100, 20, 30, 0),
                )
            };
            let body_for_assert = body.clone();
            let (addr, upstream) = tencent_serve_one(response, expect_path, move |received| {
                assert_eq!(received["model"].as_str(), Some("mimo-v2.6-pro"), "{code} upstream model");
                assert_eq!(received["messages"][0]["role"].as_str(), Some("user"), "{code} upstream messages");
                assert_eq!(received, &body_for_assert, "{code} wire body == build_request output");
            })
            .await;
            let (status, raw) = tencent_roundtrip(&addr, expect_path, &body).await;
            upstream.await.unwrap();
            assert_eq!(status, 200, "{code}/{wire:?}");
            let upstream_body = String::from_utf8_lossy(&raw);
            let (input, output, cache_read, cache_write, _, _) = adapter.extract_usage(&upstream_body);
            assert_eq!((input, output, cache_read, cache_write), expected, "{code}/{wire:?}");
        }
    }
}

/// 禁思考逐成员作用域（官方取证核对 2026-11-23）：`xiaomi_mimo` openai wire 写平台官方
/// `thinking: {"type":"disabled"}`（deep-thinking.md），剥开启型参数（`thinking` /
/// `reasoning_effort` 等）但**不剥 temperature**（官方思考模式传入会被静默覆盖为 1.0，
/// 不报错，与 Kimi 报错语义不同）；套餐两成员官方文档未列 API 参数（订阅页），保持框架
/// 默认 `chat_template_kwargs.enable_thinking=false`，成员 issue 取证后收窄。anthropic
/// wire 三成员同基座标准 `thinking.type=disabled`（官方参数表 enabled/disabled）。断言经
/// 本地假上游完整 HTTP 往返、服务端对线上 body JSON 判定。
#[tokio::test]
async fn mimo_disable_thinking_reaches_wire_per_member_scope() {
    let mut body = json!({
        "model": "mimo-v2.6-pro",
        "thinking": { "type": "enabled" },
        "reasoning_effort": "high",
        "temperature": 0.7
    });
    let response = tencent_json_response(
        r#"{"id":"cmpl-local","model":"mimo-v2.6-pro","choices":[],"usage":{"prompt_tokens":1,"completion_tokens":1,"total_tokens":2}}"#,
    );
    for (code, expected_thinking, expected_ctk) in [
        ("xiaomi_mimo", Some("disabled"), None),
        ("xiaomi_mimo_coding", None, Some(false)),
        ("xiaomi_mimo_coding_en", None, Some(false)),
    ] {
        let adapter = super::adapter_for(code);
        let mut outbound = body.clone();
        adapter.apply_disable_thinking(
            &mut outbound,
            true,
            &Protocol::OpenAI,
            "http://127.0.0.1:1/v1",
            false,
        );
        let (addr, upstream) = tencent_serve_one(response.clone(), "/chat/completions", move |received| {
            let thinking_type = received
                .get("thinking")
                .and_then(|t| t.get("type"))
                .and_then(Value::as_str)
                .map(str::to_string);
            assert_eq!(
                thinking_type.as_deref(),
                expected_thinking,
                "{code} upstream thinking.type: {received}"
            );
            assert_eq!(
                received
                    .get("chat_template_kwargs")
                    .and_then(|c| c.get("enable_thinking"))
                    .and_then(Value::as_bool),
                expected_ctk,
                "{code} upstream chat_template_kwargs.enable_thinking: {received}"
            );
        })
        .await;
        let (status, _) = tencent_roundtrip(&addr, "/chat/completions", &outbound).await;
        upstream.await.unwrap();
        assert_eq!(status, 200, "{code}");
    }
    // 客户端侧同判：标准平台成员开启型参数被剥净、temperature 保留、thinking 写 disabled。
    let adapter = super::adapter_for("xiaomi_mimo");
    adapter.apply_disable_thinking(&mut body, true, &Protocol::OpenAI, "http://127.0.0.1:1/v1", false);
    assert_eq!(body["thinking"]["type"], "disabled");
    assert!(body.get("reasoning_effort").is_none(), "{body}");
    assert_eq!(body["temperature"], json!(0.7), "temperature 官方静默覆盖非报错，不剥: {body}");
    let anthropic_body = json!({ "model": "mimo-v2.6-pro", "thinking": { "type": "enabled" } });
    for code in ["xiaomi_mimo", "xiaomi_mimo_coding", "xiaomi_mimo_coding_en"] {
        let mut b = anthropic_body.clone();
        super::adapter_for(code).apply_disable_thinking(
            &mut b,
            true,
            &Protocol::Anthropic,
            "http://127.0.0.1:1/anthropic",
            false,
        );
        assert_eq!(b["thinking"]["type"], "disabled", "{code}: {b}");
    }
}

/// 多轮工具历史携带 reasoning_content（官方 400 校验）逐成员作用域：anthropic 客户端回灌
/// 的 thinking 块在 openai 转换中被跳过，`xiaomi_mimo` 按 ChatRequest thinking 块取回补写；
/// 套餐成员官方文档未列该行为，不外推、不补写。
#[tokio::test]
async fn mimo_openai_tool_history_reasoning_content_per_member_scope() {
    let req: ChatRequest = serde_json::from_value(json!({
        "model": "mimo-v2.6-pro",
        "max_tokens": 64,
        "messages": [
            { "role": "user", "content": "北京天气?" },
            { "role": "assistant", "content": [
                { "type": "thinking", "thinking": "需要调用天气工具", "signature": "sig" },
                { "type": "tool_use", "id": "tu1", "name": "get_weather", "input": { "city": "北京" } }
            ]},
            { "role": "user", "content": [ { "type": "tool_result", "tool_use_id": "tu1", "content": "晴" } ] }
        ]
    }))
    .unwrap();
    for (code, expected) in [
        ("xiaomi_mimo", Some("需要调用天气工具")),
        ("xiaomi_mimo_coding", None),
        ("xiaomi_mimo_coding_en", None),
    ] {
        let adapter = super::adapter_for(code);
        let (body, _) = adapter.build_request(&req, &Protocol::OpenAI, &mimo_platform(code));
        let asst = body["messages"]
            .as_array()
            .unwrap()
            .iter()
            .find(|m| m["role"] == "assistant" && m.get("tool_calls").is_some_and(|v| !v.is_null()))
            .expect("出站应含 assistant tool-call 消息");
        assert_eq!(
            asst.get("reasoning_content").and_then(Value::as_str),
            expected,
            "{code} assistant tool-call 历史 reasoning_content: {asst}"
        );
    }
}

/// 两端点流式（本地假上游完整 SSE 往返）：openai wire `data: {chunk}` 帧流
///（delta.reasoning_content → delta.content、末帧 choices 空 + 顶层 usage、`data: [DONE]`
/// 结束）与 anthropic wire 标准事件链（message_start / content_block_delta thinking_delta /
/// message_delta / message_stop），经基座构件逐帧解析并抽取用量。
#[tokio::test]
async fn mimo_family_stream_frames_parse_via_local_upstream() {
    let adapter = super::adapter_for("xiaomi_mimo");
    let stream_req: ChatRequest = serde_json::from_value(json!({
        "model": "mimo-v2.6-pro",
        "max_tokens": 32,
        "stream": true,
        "messages": [{ "role": "user", "content": "hello" }]
    }))
    .unwrap();
    let (openai_request, openai_path) =
        adapter.build_request(&stream_req, &Protocol::OpenAI, &Protocol::XiaomiMimo);
    let openai_request_for_assert = openai_request.clone();
    assert_eq!(openai_path, "/chat/completions");
    assert_eq!(openai_request["stream"], true);
    let openai_frames = [
        format!(
            "data: {}\n\n",
            r#"{"id":"c1","choices":[{"index":0,"delta":{"role":"assistant","reasoning_content":"think"}}]}"#
        ),
        format!(
            "data: {}\n\n",
            r#"{"id":"c1","choices":[{"index":0,"delta":{"content":"ok"}}]}"#
        ),
        format!(
            "data: {}\n\n",
            r#"{"id":"c1","choices":[],"usage":{"prompt_tokens":61,"completion_tokens":467,"total_tokens":528,"prompt_tokens_details":{"cached_tokens":0}}}"#
        ),
        "data: [DONE]\n\n".to_string(),
    ];
    let (addr, upstream) = tencent_serve_one(
        tencent_sse_response(&openai_frames),
        "/chat/completions",
        move |received| {
            assert_eq!(
                received,
                &openai_request_for_assert,
                "openai wire body == build_request output"
            );
        },
    )
    .await;
    let (status, raw) = tencent_roundtrip(&addr, "/chat/completions", &openai_request).await;
    upstream.await.unwrap();
    assert_eq!(status, 200);
    let raw_body = String::from_utf8_lossy(&raw);
    let events: Vec<ChatStreamEvent> = tencent_sse_frames(&raw_body)
        .iter()
        .flat_map(|frame| adapter.parse_sse_frame(frame, &Protocol::OpenAI))
        .collect();
    assert!(
        events.iter().any(|e| matches!(e, ChatStreamEvent::ReasoningDelta { text } if text == "think")),
        "openai reasoning event: {events:?}"
    );
    assert!(
        events.iter().any(|e| matches!(e, ChatStreamEvent::Delta { text } if text == "ok")),
        "openai content event: {events:?}"
    );
    assert!(
        events.iter().any(|e| matches!(e, ChatStreamEvent::Usage { usage }
            if usage.prompt_tokens == Some(61) && usage.completion_tokens == Some(467))),
        "openai stream usage event: {events:?}"
    );

    let anthropic_frames = [
        "event: message_start\ndata: {\"type\":\"message_start\",\"message\":{\"id\":\"msg_1\",\"type\":\"message\",\"role\":\"assistant\",\"model\":\"mimo-v2.6-pro\",\"content\":[],\"usage\":{\"input_tokens\":12,\"output_tokens\":0}}}\n\n".to_string(),
        "event: content_block_start\ndata: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"thinking\",\"thinking\":\"\"}}\n\n".to_string(),
        "event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"thinking_delta\",\"thinking\":\"plan\"}}\n\n".to_string(),
        "event: content_block_stop\ndata: {\"type\":\"content_block_stop\",\"index\":0}\n\n".to_string(),
        "event: content_block_start\ndata: {\"type\":\"content_block_start\",\"index\":1,\"content_block\":{\"type\":\"text\",\"text\":\"\"}}\n\n".to_string(),
        "event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":1,\"delta\":{\"type\":\"text_delta\",\"text\":\"ok\"}}\n\n".to_string(),
        "event: message_delta\ndata: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"end_turn\"},\"usage\":{\"output_tokens\":6}}\n\n".to_string(),
        "event: message_stop\ndata: {\"type\":\"message_stop\"}\n\n".to_string(),
    ];
    let (anthropic_request, anthropic_path) =
        adapter.build_request(&stream_req, &Protocol::Anthropic, &Protocol::XiaomiMimo);
    assert_eq!(anthropic_path, "/v1/messages");
    assert_eq!(anthropic_request["stream"], true);
    let anthropic_request_for_assert = anthropic_request.clone();
    let (addr, upstream) = tencent_serve_one(
        tencent_sse_response(&anthropic_frames),
        "/v1/messages",
        move |received| {
            assert_eq!(
                received,
                &anthropic_request_for_assert,
                "anthropic wire body == build_request output"
            );
        },
    )
    .await;
    let (status, raw) = tencent_roundtrip(&addr, "/v1/messages", &anthropic_request).await;
    upstream.await.unwrap();
    assert_eq!(status, 200);
    let raw_body = String::from_utf8_lossy(&raw);
    let events: Vec<ChatStreamEvent> = tencent_sse_frames(&raw_body)
        .iter()
        .flat_map(|frame| adapter.parse_sse_frame(frame, &Protocol::Anthropic))
        .collect();
    assert!(
        events.iter().any(|e| matches!(e, ChatStreamEvent::Delta { text } if text == "ok")),
        "anthropic text event: {events:?}"
    );
    let mut merged = crate::types::Usage::default();
    for event in &events {
        if let ChatStreamEvent::Usage { usage } = event {
            merged.merge_partial(usage);
        }
    }
    assert_eq!(
        (merged.prompt_tokens, merged.completion_tokens),
        (Some(12), Some(6)),
        "anthropic partial usage events (message_start input + message_delta output) merged: {events:?}"
    );
}

/// 错误分类（错误码页仅有状态码表格、无 JSON 错误体形状示例——未证实，不伪造模型类
/// 契约）：401 / 429 按状态码归服务类；400 中性错误体按基座 Unknown（调用方按模型类
/// 处理）。
#[test]
fn mimo_error_classification_status_code_only() {
    let adapter = super::adapter_for("xiaomi_mimo");
    // 官方未给 JSON 错误体示例，body 形状用基座通用读法可解析的最小形态。
    assert_eq!(
        adapter.classify_error(401, r#"{"error":{"message":"invalid api key"}}"#),
        UpstreamErrorClass::Service
    );
    assert_eq!(
        adapter.classify_error(429, r#"{"error":{"message":"rate limit exceeded"}}"#),
        UpstreamErrorClass::Service
    );
    assert_eq!(
        adapter.classify_error(400, r#"{"error":{"message":"bad request"}}"#),
        super::error::classify_upstream_error(400, r#"{"error":{"message":"bad request"}}"#),
        "400 未证实文案不扩展特征，同基座"
    );
}
