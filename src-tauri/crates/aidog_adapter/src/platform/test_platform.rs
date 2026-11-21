//! 平台适配器框架门禁测试（issue 68）。

use super::error::UpstreamErrorClass;
use super::hooks;
use crate::types::ChatRequest;
use aidog_db::models::Protocol;
use serde_json::json;

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
