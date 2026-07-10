//! 纯解析覆盖：每平台 JSON body → PlatformQuota。
use super::*;
use serde_json::json;

#[test]
fn deepseek_sums_balance_infos() {
    let q = parse_deepseek_balance(&json!({
        "is_available": true,
        "balance_infos": [
            {"total_balance": "10.5", "currency": "CNY"},
            {"total_balance": 4.5, "currency": "CNY"}
        ]
    }));
    let b = q.balance.unwrap();
    assert!((b.remaining - 15.0).abs() < 1e-9);
    assert_eq!(b.currency, "CNY");
    assert!(b.is_valid);
}

#[test]
fn deepseek_unavailable_and_empty() {
    let q = parse_deepseek_balance(&json!({"is_available": false}));
    let b = q.balance.unwrap();
    assert_eq!(b.remaining, 0.0);
    assert!(!b.is_valid);
}

#[test]
fn stepfun_reads_balance() {
    let q = parse_stepfun_balance(&json!({"balance": 88.0}));
    assert_eq!(q.balance.unwrap().remaining, 88.0);
    // missing field → 0
    let q2 = parse_stepfun_balance(&json!({}));
    assert_eq!(q2.balance.unwrap().remaining, 0.0);
}

#[test]
fn siliconflow_cn_vs_com_currency() {
    let q = parse_siliconflow_balance(&json!({"data": {"totalBalance": "12.0"}}), true);
    let b = q.balance.unwrap();
    assert_eq!(b.remaining, 12.0);
    assert_eq!(b.currency, "CNY");

    let q2 = parse_siliconflow_balance(&json!({"data": {"totalBalance": 9.0}}), false);
    assert_eq!(q2.balance.unwrap().currency, "USD");
}

#[test]
fn siliconflow_missing_data_errs() {
    let q = parse_siliconflow_balance(&json!({"nope": 1}), true);
    assert!(!q.success);
    assert!(q.balance.is_none());
}

#[test]
fn openrouter_remaining_is_credits_minus_usage() {
    let q = parse_openrouter_balance(&json!({
        "data": {"total_credits": 100.0, "total_usage": 30.0}
    }));
    let b = q.balance.unwrap();
    assert_eq!(b.remaining, 70.0);
    assert_eq!(b.total, Some(100.0));
    assert_eq!(b.used, Some(30.0));
    assert!(b.is_valid);
}

#[test]
fn openrouter_no_data_wrapper_and_negative_invalid() {
    // flat body (no data wrapper)
    let q = parse_openrouter_balance(&json!({"total_credits": 5.0, "total_usage": 10.0}));
    let b = q.balance.unwrap();
    assert_eq!(b.remaining, -5.0);
    assert!(!b.is_valid);
}

#[test]
fn novita_scales_by_10000() {
    let q = parse_novita_balance(&json!({"availableBalance": 50000.0}));
    let b = q.balance.unwrap();
    assert_eq!(b.remaining, 5.0);
    assert_eq!(b.currency, "USD");
    assert!(b.is_valid);

    let q2 = parse_novita_balance(&json!({}));
    assert_eq!(q2.balance.unwrap().remaining, 0.0);
}
