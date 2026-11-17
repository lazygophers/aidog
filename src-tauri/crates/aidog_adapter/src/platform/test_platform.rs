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
