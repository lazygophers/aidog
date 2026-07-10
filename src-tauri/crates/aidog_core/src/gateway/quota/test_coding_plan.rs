//! Coding Plan 纯解析覆盖。
use super::*;
use serde_json::json;

fn tier<'a>(q: &'a PlatformQuota, name: &str) -> &'a QuotaTier {
    q.coding_plan
        .as_ref()
        .unwrap()
        .tiers
        .iter()
        .find(|t| t.name == name)
        .unwrap_or_else(|| panic!("tier {name} missing"))
}

#[test]
fn kimi_five_hour_and_weekly() {
    let q = parse_kimi_coding_plan(&json!({
        "limits": [
            {"detail": {"limit": 100.0, "remaining": 40.0, "resetTime": "2026-06-23T10:00:00Z"}}
        ],
        "usage": {"limit": 1000.0, "remaining": 250.0, "resetTime": 1750000000000_i64}
    }));
    let fh = tier(&q, "five_hour");
    assert!((fh.utilization - 60.0).abs() < 1e-6);
    assert_eq!(fh.limit, Some(100.0));
    assert_eq!(fh.remaining, Some(40.0));
    assert_eq!(fh.resets_at.as_deref(), Some("2026-06-23T10:00:00Z"));

    let wk = tier(&q, "weekly_limit");
    assert!((wk.utilization - 75.0).abs() < 1e-6);
    assert!(wk.resets_at.is_some()); // millis → iso
}

#[test]
fn kimi_zero_limit_no_div_by_zero() {
    let q = parse_kimi_coding_plan(&json!({
        "limits": [{"detail": {"limit": 0.0, "remaining": 0.0}}]
    }));
    assert_eq!(tier(&q, "five_hour").utilization, 0.0);
}

#[test]
fn zhipu_units_classify_and_level() {
    let q = parse_zhipu_coding_plan(&json!({
        "success": true,
        "data": {
            "level": "pro",
            "limits": [
                {"type": "TOKENS_LIMIT", "unit": 3, "percentage": 20.0, "nextResetTime": 1750000000000_i64},
                {"type": "TOKENS_LIMIT", "unit": 6, "percentage": 55.0, "nextResetTime": 1750100000000_i64},
                {"type": "TIME_LIMIT", "percentage": 30.0, "usage": 100.0, "remaining": 70.0, "nextResetTime": 1750200000000_i64}
            ]
        }
    }));
    assert_eq!(q.coding_plan.as_ref().unwrap().level.as_deref(), Some("pro"));
    assert_eq!(tier(&q, "five_hour").utilization, 20.0);
    assert_eq!(tier(&q, "weekly_limit").utilization, 55.0);
    let mcp = tier(&q, "mcp_monthly");
    // utilization（已用%）= percentage 字段，与 TOKENS_LIMIT 同口径
    assert_eq!(mcp.utilization, 30.0);
    assert_eq!(mcp.limit, Some(100.0));
    assert_eq!(mcp.remaining, Some(70.0));
    // 顺序: five_hour, weekly_limit, mcp_monthly
    let names: Vec<_> = q
        .coding_plan
        .as_ref()
        .unwrap()
        .tiers
        .iter()
        .map(|t| t.name.clone())
        .collect();
    assert_eq!(names, vec!["five_hour", "weekly_limit", "mcp_monthly"]);
}

#[test]
fn zhipu_mcp_unused_is_zero_utilization() {
    // 回归: 用户实测 mcp 月用量 0% 已用 → percentage=0 → utilization=0（剩余 100%）。
    // 旧实现误用 currentValue/usage 推算，把 0% 已用算成 100% 已用。
    let q = parse_zhipu_coding_plan(&json!({
        "success": true,
        "data": {
            "limits": [
                {"type": "TIME_LIMIT", "percentage": 0.0, "usage": 200.0, "remaining": 200.0, "nextResetTime": 1750200000000_i64}
            ]
        }
    }));
    let mcp = tier(&q, "mcp_monthly");
    assert_eq!(mcp.utilization, 0.0);
}

#[test]
fn zhipu_unclassified_fills_slots() {
    let q = parse_zhipu_coding_plan(&json!({
        "data": {
            "limits": [
                {"type": "TOKENS_LIMIT", "percentage": 10.0, "nextResetTime": 200_i64},
                {"type": "TOKENS_LIMIT", "percentage": 20.0, "nextResetTime": 100_i64}
            ]
        }
    }));
    // sorted by reset asc → reset=100 first → five_hour=20%, weekly=10%
    assert_eq!(tier(&q, "five_hour").utilization, 20.0);
    assert_eq!(tier(&q, "weekly_limit").utilization, 10.0);
}

#[test]
fn zhipu_success_false_and_missing_data() {
    let err = parse_zhipu_coding_plan(&json!({"success": false, "msg": "bad key"}));
    assert!(!err.success);
    assert_eq!(err.error.as_deref(), Some("bad key"));

    let no_data = parse_zhipu_coding_plan(&json!({"success": true}));
    assert!(!no_data.success);
}

#[test]
fn minimax_general_model_buckets() {
    let q = parse_minimax_coding_plan(&json!({
        "base_resp": {"status_code": 0, "status_msg": "ok"},
        "model_remains": [
            {"model_name": "other", "current_interval_remaining_percent": 99.0},
            {
                "model_name": "general",
                "current_interval_remaining_percent": 70.0,
                "end_time": 1750000000000_i64,
                "current_weekly_status": 1,
                "current_weekly_remaining_percent": 40.0,
                "weekly_end_time": 1750100000000_i64
            }
        ]
    }));
    assert!((tier(&q, "five_hour").utilization - 30.0).abs() < 1e-6);
    assert!((tier(&q, "weekly_limit").utilization - 60.0).abs() < 1e-6);
}

#[test]
fn minimax_weekly_skipped_when_status_not_one() {
    let q = parse_minimax_coding_plan(&json!({
        "model_remains": [{
            "model_name": "general",
            "current_interval_remaining_percent": 80.0,
            "current_weekly_status": 0,
            "current_weekly_remaining_percent": 50.0
        }]
    }));
    assert_eq!(q.coding_plan.as_ref().unwrap().tiers.len(), 1);
    assert_eq!(q.coding_plan.as_ref().unwrap().tiers[0].name, "five_hour");
}

#[test]
fn minimax_error_base_resp() {
    let q = parse_minimax_coding_plan(&json!({
        "base_resp": {"status_code": 1004, "status_msg": "auth failed"}
    }));
    assert!(!q.success);
    assert!(q.error.unwrap().contains("1004"));
}

/// minimax: model_remains 无 general 模型 → empty tiers
#[test]
fn minimax_no_general_model_empty_tiers() {
    let q = parse_minimax_coding_plan(&json!({
        "model_remains": [
            {"model_name": "other", "current_interval_remaining_percent": 50.0}
        ]
    }));
    assert!(q.success);
    assert_eq!(q.coding_plan.as_ref().unwrap().tiers.len(), 0);
}

/// minimax: empty model_remains → empty tiers (no panic)
#[test]
fn minimax_empty_model_remains() {
    let q = parse_minimax_coding_plan(&json!({"model_remains": []}));
    assert!(q.success);
    assert_eq!(q.coding_plan.as_ref().unwrap().tiers.len(), 0);
}

/// kimi: empty limits and no usage → empty tiers (no panic)
#[test]
fn kimi_empty_body_returns_empty_tiers() {
    let q = parse_kimi_coding_plan(&json!({}));
    assert!(q.success);
    assert_eq!(q.coding_plan.as_ref().unwrap().tiers.len(), 0);
}

/// kimi: resetTime as integer millis in limits[].detail
#[test]
fn kimi_reset_time_millis_in_detail() {
    let q = parse_kimi_coding_plan(&json!({
        "limits": [{"detail": {"limit": 100.0, "remaining": 80.0, "resetTime": 1750000000000_i64}}]
    }));
    let fh = tier(&q, "five_hour");
    assert!(fh.resets_at.is_some(), "millis resetTime should convert to ISO");
}

/// zhipu: success=true but data.limits is empty → empty tiers
#[test]
fn zhipu_empty_limits_returns_empty_tiers() {
    let q = parse_zhipu_coding_plan(&json!({
        "success": true,
        "data": {"limits": []}
    }));
    assert!(q.success);
    assert_eq!(q.coding_plan.as_ref().unwrap().tiers.len(), 0);
}

/// zhipu: TOKENS_LIMIT unit=3 and unit=6 with unclassified (not 3 or 6)
#[test]
fn zhipu_unclassified_token_limit_uses_slot_fill() {
    // Three TOKENS_LIMIT items: unit=3 (five_hour), unit=6 (weekly), unit=99 (ignored/other)
    let q = parse_zhipu_coding_plan(&json!({
        "success": true,
        "data": {
            "limits": [
                {"type": "TOKENS_LIMIT", "unit": 3, "percentage": 10.0, "nextResetTime": 300_i64},
                {"type": "TOKENS_LIMIT", "unit": 6, "percentage": 50.0, "nextResetTime": 600_i64},
                {"type": "TOKENS_LIMIT", "unit": 99, "percentage": 70.0, "nextResetTime": 100_i64}
            ]
        }
    }));
    // unit=3 → five_hour, unit=6 → weekly
    assert_eq!(tier(&q, "five_hour").utilization, 10.0);
    assert_eq!(tier(&q, "weekly_limit").utilization, 50.0);
}
