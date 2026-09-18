//! `tray_render` 纯数据层单测（票 I10 补的回归网 —— 此前该文件 `cfg(test)` 计数为 0）。
//!
//! 覆盖的是换宿主后**原地不动**那 246 行：排序 / 禁用跳过 / separator→gaps 映射 /
//! today_usage 各 metric / 自定义 label / 小数位 / 尾零裁剪。AppKit 渲染不在此测。

use super::{platform_item_parts, tray_layout, trim_trailing_zeros};
use aidog_db::{Db, TrayConfig, TrayItem, set_tray_config};
use aidog_stats::DbInitTables;

async fn test_db() -> Db {
    let db = Db::new(":memory:").await.expect("open memory db");
    db.init_tables().await.expect("init tables");
    db
}

/// 造一个 today_usage item（不需要 platform 行，聚焦布局逻辑本身）。
fn usage_item(metric: &str, order: i32) -> TrayItem {
    serde_json::from_value(serde_json::json!({
        "item_type": "today_usage",
        "metric": metric,
        "order": order,
    }))
    .expect("TrayItem from json")
}

fn separator_item(text: &str, order: i32) -> TrayItem {
    serde_json::from_value(serde_json::json!({
        "item_type": "separator",
        "display": text,
        "order": order,
    }))
    .expect("TrayItem from json")
}

async fn layout_of(items: Vec<TrayItem>) -> super::TrayLayout {
    let db = test_db().await;
    set_tray_config(
        &db,
        &TrayConfig {
            separator: "  ".to_string(),
            items,
        },
    )
    .await
    .expect("set tray config");
    tray_layout(&db).await
}

#[tokio::test]
async fn empty_config_yields_empty_layout() {
    let l = layout_of(vec![]).await;
    assert!(l.columns.is_empty());
    assert!(l.gaps.is_empty());
}

#[tokio::test]
async fn items_render_in_order_field_not_vec_order() {
    let l = layout_of(vec![usage_item("cost", 2), usage_item("requests", 1)]).await;
    let names: Vec<&str> = l.columns.iter().map(|c| c.name.as_str()).collect();
    assert_eq!(names, vec!["请求", "花费"]);
}

#[tokio::test]
async fn disabled_items_are_skipped() {
    let mut off = usage_item("cost", 0);
    off.enabled = false;
    let l = layout_of(vec![off, usage_item("requests", 1)]).await;
    assert_eq!(l.columns.len(), 1);
    assert_eq!(l.columns[0].name, "请求");
}

#[tokio::test]
async fn separator_becomes_gap_not_column() {
    let l = layout_of(vec![
        usage_item("cost", 0),
        separator_item("|", 1),
        usage_item("requests", 2),
    ])
    .await;
    assert_eq!(l.columns.len(), 2, "separator 不占列");
    assert_eq!(l.gaps, vec![Some("|".to_string())]);
}

#[tokio::test]
async fn separator_with_empty_display_falls_back_to_middot() {
    let l = layout_of(vec![
        usage_item("cost", 0),
        separator_item("", 1),
        usage_item("requests", 2),
    ])
    .await;
    assert_eq!(l.gaps, vec![Some("·".to_string())]);
}

#[tokio::test]
async fn gap_is_none_when_no_separator_between_columns() {
    let l = layout_of(vec![usage_item("cost", 0), usage_item("requests", 1)]).await;
    assert_eq!(l.gaps, vec![None]);
}

#[tokio::test]
async fn leading_separator_does_not_produce_a_gap() {
    // 首列之前的 separator 无处可挂，应被丢弃而不是造出越界的 gap。
    let l = layout_of(vec![separator_item("|", 0), usage_item("cost", 1)]).await;
    assert_eq!(l.columns.len(), 1);
    assert!(l.gaps.is_empty());
}

#[tokio::test]
async fn today_usage_metrics_have_expected_labels_on_empty_db() {
    let l = layout_of(vec![
        usage_item("tokens", 0),
        usage_item("cache_rate", 1),
        usage_item("cost", 2),
        usage_item("requests", 3),
    ])
    .await;
    let pairs: Vec<(&str, &str)> = l
        .columns
        .iter()
        .map(|c| (c.name.as_str(), c.value.as_str()))
        .collect();
    assert_eq!(
        pairs,
        vec![
            ("今日", "0 tok"),
            ("Cache", "0%"),
            ("花费", "$0"),
            ("请求", "0"),
        ]
    );
}

#[tokio::test]
async fn unknown_metric_falls_back_to_tokens() {
    let l = layout_of(vec![usage_item("nope", 0)]).await;
    assert_eq!(l.columns[0].name, "今日");
}

#[tokio::test]
async fn custom_label_overrides_generated_name() {
    let mut it = usage_item("cost", 0);
    it.label = Some("💰".to_string());
    let l = layout_of(vec![it]).await;
    assert_eq!(l.columns[0].name, "💰");
}

#[tokio::test]
async fn platform_item_without_matching_row_is_skipped() {
    let it: TrayItem = serde_json::from_value(serde_json::json!({
        "item_type": "platform",
        "platform_id": 9999,
        "display": "balance",
        "order": 0,
    }))
    .expect("TrayItem from json");
    let l = layout_of(vec![it]).await;
    assert!(l.columns.is_empty(), "取数失败的平台项不生成列");
}

#[tokio::test]
async fn line_mode_two_sets_two_line_flag() {
    let mut a = usage_item("cost", 0);
    a.line_mode = "two".to_string();
    let mut b = usage_item("requests", 1);
    b.line_mode = "single".to_string();
    let l = layout_of(vec![a, b]).await;
    assert!(l.columns[0].two_line);
    assert!(!l.columns[1].two_line);
}

fn platform(balance: f64, coding_plan: &str) -> aidog_db::Platform {
    serde_json::from_value(serde_json::json!({
        "id": 1,
        "name": "Acme",
        "platform_type": "anthropic",
        "base_url": "https://a.example.com",
        "api_key": "sk-test",
        "extra": "{}",
        "models": {},
        "available_models": [],
        "est_balance_remaining": balance,
        "est_coding_plan": coding_plan,
        "created_at": 0,
        "updated_at": 0,
    }))
    .expect("Platform from json")
}

#[test]
fn platform_item_parts_balance_trims_zeros() {
    assert_eq!(
        platform_item_parts(&platform(10.10, ""), "balance"),
        ("Acme".to_string(), "$10.1".to_string())
    );
    assert_eq!(
        platform_item_parts(&platform(0.0, ""), "balance"),
        ("Acme".to_string(), "$0".to_string())
    );
}

#[test]
fn platform_item_parts_coding_shows_remaining_percent() {
    let plan = r#"{"tiers":[{"name":"five_hour","est_utilization":25.0}]}"#;
    let (_, value) = platform_item_parts(&platform(0.0, plan), "coding");
    assert_eq!(value, "75%");
}

#[test]
fn platform_item_parts_coding_clamps_over_100_utilization() {
    let plan = r#"{"tiers":[{"name":"five_hour","est_utilization":150.0}]}"#;
    let (_, value) = platform_item_parts(&platform(0.0, plan), "coding");
    assert_eq!(value, "0%", "超用不显示负数");
}

#[test]
fn platform_item_parts_coding_plan_beats_balance_display() {
    // 有 coding plan 数据时，即使 display=balance 也按配额百分比显示。
    let plan = r#"{"tiers":[{"name":"five_hour","est_utilization":10.0}]}"#;
    let (_, value) = platform_item_parts(&platform(99.0, plan), "balance");
    assert_eq!(value, "90%");
}

#[test]
fn trim_trailing_zeros_cases() {
    assert_eq!(trim_trailing_zeros("10.10"), "10.1");
    assert_eq!(trim_trailing_zeros("0.00"), "0");
    assert_eq!(trim_trailing_zeros("965.80"), "965.8");
    assert_eq!(trim_trailing_zeros("12"), "12", "无小数点原样返回");
    assert_eq!(trim_trailing_zeros("1.000"), "1");
    assert_eq!(trim_trailing_zeros("0.00001"), "0.00001");
}
