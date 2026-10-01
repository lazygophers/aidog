//! 决策请求（/v1/systemone）单元测试：响应字段剥离（R12）+ 上游 cost 优先（R14 取值部分）。
//! 端到端（URL / model 改写 / 换平台 / 落库）在 test_integration.rs。

use super::*;

/// R12：删顶层 id / provider 与 usage.cost，保留官方字段（model / answers / usage tokens）。
#[test]
fn strips_non_official_fields_and_keeps_official() {
    let body = br#"{"id":"gen_1","provider":"openrouter","model":"jev-1.13","answers":[{"text":"yes"}],"usage":{"input_tokens":10,"output_tokens":2,"cost":0.0004}}"#;
    let (out, cost) = process_decision_response(body);
    let v: Value = serde_json::from_slice(&out).unwrap();
    assert_eq!(v.get("id"), None, "id must be stripped");
    assert_eq!(v.get("provider"), None, "provider must be stripped");
    assert_eq!(
        v.get("usage").unwrap().get("cost"),
        None,
        "usage.cost must be stripped"
    );
    assert_eq!(v.get("model").unwrap(), "jev-1.13");
    assert_eq!(
        v.get("answers").unwrap().as_array().unwrap().len(),
        1,
        "official answers preserved"
    );
    assert_eq!(v["usage"]["input_tokens"], 10);
    assert!((cost.unwrap() - 0.0004).abs() < 1e-12, "upstream cost extracted");
}

/// serde_json preserve_order：剥字段后其余字段保持上游原顺序（非字母序）。
#[test]
fn response_keeps_upstream_field_order() {
    let body = br#"{"model":"jev-1.13","id":"x","answers":[],"usage":{"output_tokens":2,"cost":0.1,"input_tokens":10},"provider":"p"}"#;
    let (out, _) = process_decision_response(body);
    assert_eq!(
        String::from_utf8(out).unwrap(),
        r#"{"model":"jev-1.13","answers":[],"usage":{"output_tokens":2,"input_tokens":10}}"#
    );
}

/// 无 usage.cost（TypeSafe 官方响应）→ cost=None，其余字段照剥。
#[test]
fn no_cost_yields_none() {
    let body = br#"{"id":"x","model":"jev-latest","answers":[],"usage":{"input_tokens":5,"output_tokens":1}}"#;
    let (out, cost) = process_decision_response(body);
    assert!(cost.is_none());
    let v: Value = serde_json::from_slice(&out).unwrap();
    assert_eq!(v.get("id"), None);
}

/// 非 JSON 2xx body：原样返回、不计费（保守，避免误杀非标准但有内容的上游）。
#[test]
fn non_json_body_passthrough() {
    let body = b"plain text";
    let (out, cost) = process_decision_response(body);
    assert_eq!(out, b"plain text");
    assert!(cost.is_none());
}

/// R7 错误 message 面向用户。
#[test]
fn kind_route_error_message_user_facing() {
    assert!(kind_route_error_message("no_decision_platform").contains("决策平台"));
    assert!(kind_route_error_message("no_chat_platform").contains("聊天平台"));
    assert_eq!(kind_route_error_message("other"), "route error: other");
}
