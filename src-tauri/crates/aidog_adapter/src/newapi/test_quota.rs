//! New API 纯解析 + URL/extra 工具覆盖。
use super::*;
use serde_json::json;

#[test]
fn instance_root_strips_version_suffix() {
    assert_eq!(
        newapi_instance_root("https://x.com/v1"),
        "https://x.com"
    );
    assert_eq!(
        newapi_instance_root("https://x.com/v1/"),
        "https://x.com"
    );
    assert_eq!(newapi_instance_root("https://x.com/v22"), "https://x.com");
    // no version suffix → unchanged
    assert_eq!(newapi_instance_root("https://x.com/api"), "https://x.com/api");
    assert_eq!(newapi_instance_root("https://x.com"), "https://x.com");
    // "v" alone or "vx" not stripped
    assert_eq!(newapi_instance_root("https://x.com/vx"), "https://x.com/vx");
}

#[test]
fn parse_extra_variants() {
    assert!(parse_newapi_extra("").is_none());
    assert!(parse_newapi_extra("not json").is_none());
    assert!(parse_newapi_extra(r#"{"foo":1}"#).is_none()); // no newapi
    assert!(parse_newapi_extra(r#"{"newapi":{}}"#).is_none()); // no key
    assert!(parse_newapi_extra(r#"{"newapi":{"balance_api_key":""}}"#).is_none()); // empty key

    let (base, key) =
        parse_newapi_extra(r#"{"newapi":{"balance_base_url":"https://b.com","balance_api_key":"k"}}"#)
            .unwrap();
    assert_eq!(base, "https://b.com");
    assert_eq!(key, "k");

    // missing base_url defaults empty
    let (base2, key2) = parse_newapi_extra(r#"{"newapi":{"balance_api_key":"k2"}}"#).unwrap();
    assert_eq!(base2, "");
    assert_eq!(key2, "k2");
}

#[test]
fn user_balance_success() {
    let q = parse_newapi_user_balance(&json!({
        "success": true,
        "data": {"id": 42, "quota": 1000000.0, "used_quota": 500000.0}
    }));
    let b = q.balance.unwrap();
    assert_eq!(b.remaining, 2.0);
    assert_eq!(b.used, Some(1.0));
    assert_eq!(b.total, Some(3.0));
    assert_eq!(q.newapi_user_id.as_deref(), Some("42"));
    assert!(b.is_valid);
}

#[test]
fn user_balance_id_as_string() {
    let q = parse_newapi_user_balance(&json!({
        "success": true,
        "data": {"id": "u-7", "quota": 0.0}
    }));
    assert_eq!(q.newapi_user_id.as_deref(), Some("u-7"));
    assert!(!q.balance.unwrap().is_valid); // remaining 0 → invalid
}

#[test]
fn user_balance_failure_paths() {
    let nf = parse_newapi_user_balance(&json!({"success": false, "message": "denied"}));
    assert!(!nf.success);
    assert_eq!(nf.error.as_deref(), Some("denied"));

    let no_data = parse_newapi_user_balance(&json!({"success": true}));
    assert!(!no_data.success);
}

#[test]
fn limited_token_scales_and_omits_zero() {
    let q = limited_token_quota(1500000.0, 500000.0, 1000000.0);
    let b = q.balance.unwrap();
    assert_eq!(b.remaining, 2.0);
    assert_eq!(b.used, Some(1.0));
    assert_eq!(b.total, Some(3.0));

    // zero granted/used → None
    let q2 = limited_token_quota(0.0, 0.0, 0.0);
    let b2 = q2.balance.unwrap();
    assert_eq!(b2.total, None);
    assert_eq!(b2.used, None);
    assert!(!b2.is_valid);
}
