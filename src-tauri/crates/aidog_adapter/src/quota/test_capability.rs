//! quota 能力派生测试：registry 脚本变体 `returns` 声明 → `QuotaCapability`。
use super::*;
use aidog_db::registry::{QuotaScriptReturns, QuotaScriptVariant};

fn variant(returns: QuotaScriptReturns) -> QuotaScriptVariant {
    QuotaScriptVariant {
        id: "official".into(),
        name: [("en-US", "Official"), ("zh-Hans", "官方")]
            .into_iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect(),
        requires: Vec::new(),
        returns,
        script: "return { success: true }".into(),
    }
}

#[test]
fn derives_from_returns_declaration() {
    let cap = capability_for_variant(&variant(QuotaScriptReturns {
        balance: true,
        coding_plan: true,
        mcp: false,
        tiers: vec!["five_hour".into(), "weekly_limit".into()],
    }));
    assert!(cap.supports_balance);
    assert!(cap.supports_coding_plan);
    assert!(!cap.supports_mcp_query);
    assert_eq!(cap.tier_names, ["five_hour", "weekly_limit"]);
    assert!(cap.custom_query_supported);
}

#[test]
fn empty_returns_still_supports_custom_query() {
    let cap = capability_for_variant(&variant(QuotaScriptReturns::default()));
    assert!(!cap.supports_balance && !cap.supports_coding_plan && !cap.supports_mcp_query);
    assert!(cap.tier_names.is_empty());
    assert!(cap.custom_query_supported);
}
