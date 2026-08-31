//! New API extra 工具覆盖（两步查询 / instance_root / user-self 解析的等价用例在
//! `quota/test_special_scripts.rs` 跑 registry 脚本）。
use super::*;

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
