use super::*;
use serde_json::json;

#[test]
fn anchors_in_maps_bailian_budgets() {
    let doc = json!({
        "protocols": {
            "bailian_coding": {
                "plan_quotas": [
                    { "id": "pro", "budgets": [
                        { "kind": "rolling", "unit": "count", "amount": 6000, "window_hours": 5, "window_unit": "hour" },
                        { "kind": "rolling", "unit": "count", "amount": 45000, "window_unit": "week" },
                        { "kind": "rolling", "unit": "count", "amount": 90000, "window_unit": "month" },
                        { "kind": "rolling", "unit": "token", "amount": 1e9, "window_unit": "week" }
                    ]}
                ]
            }
        }
    });
    let anchors = anchors_in(&doc, "bailian_coding");
    assert_eq!(anchors.len(), 3, "token 型 budget 不造锚点");
    let five = &anchors[0];
    assert_eq!(five.name, "five_hour");
    assert!(five.has_base);
    assert!((five.limit - 6000.0).abs() < 1e-9);
    assert_eq!(five.unit, "prompt_count");
    assert_eq!(anchors[1].name, "weekly_limit");
    assert_eq!(anchors[2].name, "monthly");
    // 每请求增量：100/6000
    let mut t = five.clone();
    super::super::apply_tier_delta(&mut t, 1, 0.0);
    assert!((t.est_utilization - 100.0 / 6000.0).abs() < 1e-12);
}

#[test]
fn anchors_in_missing_or_noncount() {
    let doc = json!({ "protocols": { "qianfan_coding": {} } });
    assert!(anchors_in(&doc, "qianfan_coding").is_empty());
    assert!(anchors_in(&doc, "never_exists").is_empty());
    // 全 token 型 / 非整小时窗口 → 无锚点
    let doc2 = json!({ "protocols": { "x": { "plan_quotas": [
        { "budgets": [{ "unit": "token", "amount": 100, "window_unit": "week" }] },
        { "budgets": [{ "unit": "count", "amount": 100, "window_hours": 3, "window_unit": "hour" }] }
    ]}}});
    assert!(anchors_in(&doc2, "x").is_empty());
}

// bundled registry（编译期 include）的 bailian_coding 带 plan_quotas → 默认入口可取到
#[test]
fn default_anchors_from_bundled_registry() {
    let anchors = default_plan_quota_anchors("bailian_coding");
    assert!(
        anchors
            .iter()
            .any(|t| t.name == "five_hour" && (t.limit - 6000.0).abs() < 1e-9)
    );
}
