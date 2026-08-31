//! Devin extra 工具覆盖（解析语义等价用例在 `quota/test_special_scripts.rs` 跑 registry 脚本）。
use super::*;

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
