//! 票 08 会话归组：`derive_session_key` 派生规则单测（各协议 fixture 样例 + 解析失败样例）。

use super::derive_session_key;
use axum::http::{HeaderMap, HeaderName};
use serde_json::{json, Value};

fn headers(pairs: &[(&str, &str)]) -> HeaderMap {
    let mut m = HeaderMap::new();
    for (k, v) in pairs {
        m.insert(
            HeaderName::from_bytes(k.as_bytes()).expect("valid header name"),
            v.parse().expect("valid header value"),
        );
    }
    m
}

fn derive(body: Value, hs: &[(&str, &str)]) -> String {
    derive_session_key(Some(&body), &headers(hs))
}

const UUID: &str = "3f2b8c9e-1111-2222-3333-444455556666";

#[test]
fn anthropic_metadata_user_id_session_uuid() {
    // Claude Code 形如 user_<x>_account__session_<uuid>
    let body = json!({
        "model": "claude-sonnet-4",
        "metadata": { "user_id": format!("user_abc123_account__session_{UUID}") },
    });
    assert_eq!(derive(body, &[]), UUID);
}

#[test]
fn anthropic_user_id_session_uuid_mid_string() {
    // session 段后还有别的内容时只取 UUID 字符（hex + 连字符），不带尾巴
    let body = json!({
        "metadata": { "user_id": format!("user_x_account_a_session_{UUID}_extra_suffix") },
    });
    assert_eq!(derive(body, &[]), UUID);
}

#[test]
fn anthropic_user_id_without_session_falls_through_to_empty() {
    let body = json!({ "metadata": { "user_id": "user_abc_account__nope" } });
    assert_eq!(derive(body, &[]), "");
}

#[test]
fn openai_body_session_id() {
    let body = json!({ "model": "gpt-5", "session_id": "conv-abc-123" });
    assert_eq!(derive(body, &[]), "conv-abc-123");
}

#[test]
fn openai_body_metadata_session_id() {
    let body = json!({ "metadata": { "session_id": "meta-sess-42" } });
    assert_eq!(derive(body, &[]), "meta-sess-42");
}

#[test]
fn header_x_session_id_when_body_has_no_ids() {
    let body = json!({ "model": "gpt-5" });
    assert_eq!(derive(body, &[("x-session-id", "hdr-sess-7")]), "hdr-sess-7");
}

#[test]
fn no_body_header_still_works() {
    assert_eq!(
        derive_session_key(None, &headers(&[("x-session-id", "hdr-only")])),
        "hdr-only"
    );
}

#[test]
fn parse_failures_yield_empty() {
    // metadata.user_id 是数字（解析失败样例）：非字符串 → 跳过
    assert_eq!(derive(json!({ "metadata": { "user_id": 123 } }), &[]), "");
    // session_id 空串 → 跳过且无 header 兜底 → 空
    assert_eq!(derive(json!({ "session_id": "" }), &[]), "");
    // 空 body / null body
    assert_eq!(derive_session_key(Some(&Value::Null), &headers(&[])), "");
    assert_eq!(derive_session_key(None, &headers(&[])), "");
}

#[test]
fn anthropic_takes_priority_over_openai_probes() {
    let body = json!({
        "session_id": "openai-sess",
        "metadata": { "user_id": format!("user_x_account__session_{UUID}"), "session_id": "meta-sess" },
    });
    assert_eq!(derive(body, &[]), UUID);
}

#[test]
fn long_values_truncated() {
    let long = "s".repeat(500);
    let body = json!({ "session_id": long });
    let got = derive(body, &[]);
    assert!(got.len() <= 256, "超长 session 值须截断，实际 {}", got.len());
}
