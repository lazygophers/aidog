//! Devin 纯解析 + extra 工具覆盖。
use super::*;
use serde_json::json;

#[test]
fn parse_extra_variants() {
    assert!(parse_devin_extra("").is_none());
    assert!(parse_devin_extra("not json").is_none());
    assert!(parse_devin_extra(r#"{"foo":1}"#).is_none()); // no devin
    assert!(parse_devin_extra(r#"{"devin":{}}"#).is_none()); // no org_id
    assert!(parse_devin_extra(r#"{"devin":{"org_id":""}}"#).is_none()); // empty
    assert!(parse_devin_extra(r#"{"devin":{"org_id":"  "}}"#).is_none()); // whitespace only

    let id = parse_devin_extra(r#"{"devin":{"org_id":"org-abc"}}"#).unwrap();
    assert_eq!(id, "org-abc");

    // 周围空白 trim
    let id2 = parse_devin_extra(r#"{"devin":{"org_id":"  org-xyz  "}}"#).unwrap();
    assert_eq!(id2, "org-xyz");
}

#[test]
fn quota_success_records_total_acus() {
    let q = parse_devin_quota(&json!({
        "total_acus": 1234.5,
        "acus_by_product": {"devin": 1000.0, "cascade": 234.5}
    }));
    assert!(q.success);
    assert!(q.error.is_none());
    let b = q.balance.unwrap();
    assert_eq!(b.used, Some(1234.5));
    assert_eq!(b.remaining, 0.0); // 无余额端点
    assert_eq!(b.total, None);
    assert_eq!(b.currency, "ACU");
    assert!(b.is_valid);
}

#[test]
fn quota_total_acus_as_string() {
    // 字符串数字也接受（parse_f64_field 兼容）
    let q = parse_devin_quota(&json!({"total_acus": "42"}));
    assert!(q.success);
    assert_eq!(q.balance.unwrap().used, Some(42.0));
}

#[test]
fn quota_zero_acus_still_valid() {
    let q = parse_devin_quota(&json!({"total_acus": 0.0}));
    assert!(q.success);
    assert!(q.balance.unwrap().is_valid); // 查询成功即 key 可用
}

#[test]
fn quota_missing_total_acus_fails() {
    let q = parse_devin_quota(&json!({"acus_by_product": {"devin": 1.0}}));
    assert!(!q.success);
    assert!(q.error.as_deref().unwrap().contains("total_acus"));
}
