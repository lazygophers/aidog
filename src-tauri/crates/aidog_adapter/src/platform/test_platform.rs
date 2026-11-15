//! 平台适配器框架门禁测试（issue 68）。

use super::error::UpstreamErrorClass;
use super::registry::KimiAdapter;
use super::{hooks, PlatformAdapter};
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

/// 家族示例（issue 68）：KimiAdapter 覆盖 coding_plan 注入差异，只登记在 kimi 上；
/// 产出与基座的 Kimi 协议臂一致（注入条件同源：端点 coding_plan flag）。
#[test]
fn kimi_adapter_coding_plan_matches_base_kimi_arm() {
    let pa = super::adapter_for("kimi");
    let mut body = json!({ "model": "kimi-for-coding" });
    pa.inject_coding_plan(&mut body, &Protocol::Kimi);
    assert!(
        body.get("prompt_cache_key").is_some(),
        "kimi 未注入 prompt_cache_key"
    );
    // 基座默认按平台协议分派：platform=Kimi 同样注入；非 Kimi 协议不注入。
    let mut via_base = json!({ "model": "kimi-for-coding" });
    super::adapter_for("anthropic").inject_coding_plan(&mut via_base, &Protocol::Kimi);
    assert!(via_base.get("prompt_cache_key").is_some());
    let mut non_kimi = json!({ "model": "m" });
    super::adapter_for("anthropic").inject_coding_plan(&mut non_kimi, &Protocol::Anthropic);
    assert!(non_kimi.get("prompt_cache_key").is_none());
    // 家族适配器与基座 Kimi 臂产出同形状（除时间窗口随机段外的键集合一致）。
    let mut member = json!({ "model": "k3" });
    KimiAdapter.inject_coding_plan(&mut member, &Protocol::Kimi);
    assert_eq!(
        member.as_object().unwrap().keys().collect::<Vec<_>>(),
        via_base.as_object().unwrap().keys().collect::<Vec<_>>()
    );
}

/// 行为零变化门禁（review 必改 1）：kimi_coding / kimi_en 不注入 prompt_cache_key。
/// 迁移前 headers.rs 仅对 Protocol::Kimi 注入，KimiCoding / KimiEn 走 no-op 臂；
/// KimiAdapter 只登记在 kimi 上，这两个 code 走基座（协议臂 no-op）。
#[test]
fn kimi_coding_and_kimi_en_do_not_inject_prompt_cache_key() {
    for (code, proto) in [
        ("kimi_coding", Protocol::KimiCoding),
        ("kimi_en", Protocol::KimiEn),
    ] {
        let mut body = json!({ "model": "kimi-for-coding" });
        super::adapter_for(code).inject_coding_plan(&mut body, &proto);
        assert!(
            body.get("prompt_cache_key").is_none(),
            "code {code} 不应注入 prompt_cache_key（仅 Protocol::Kimi 注入）"
        );
    }
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
