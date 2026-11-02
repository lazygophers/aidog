//! `tray_render` 纯数据层单测（票 I10 补的回归网 —— 此前该文件 `cfg(test)` 计数为 0）。
//!
//! 覆盖的是换宿主后**原地不动**那 246 行：排序 / 禁用跳过 / separator→gaps 映射 /
//! today_usage 各 metric / 自定义 label / 小数位 / 尾零裁剪。AppKit 渲染不在此测。

use super::{align_pad, col_width, line_gap_text, platform_item_parts, tab_stop_locs, tray_layout, trim_trailing_zeros};
use aidog_db::currency::{DisplayCurrency, DisplayCurrencySetting, build_display_currency};
use aidog_db::{Db, TrayConfig, TrayItem, set_tray_config};
use aidog_stats::DbInitTables;
use serde_json::json;

async fn test_db() -> Db {
    aidog_db::test_support::ensure_test_master_key();
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
async fn enabled_separator_becomes_gap_between_columns() {
    // 票 02：separator 不再被 clamp 禁用，不占列，落成相邻两列之间的 gap。
    let l = layout_of(vec![
        usage_item("cost", 0),
        separator_item("|", 1),
        usage_item("requests", 2),
    ])
    .await;
    assert_eq!(l.columns.len(), 2);
    assert_eq!(l.gaps, vec![Some("|".to_string())]);
}

#[tokio::test]
async fn distinct_separators_map_to_gaps_in_order() {
    let l = layout_of(vec![
        usage_item("cost", 0),
        separator_item("·", 1),
        usage_item("requests", 2),
        separator_item("｜", 3),
        usage_item("tokens", 4),
        usage_item("cache_rate", 5),
    ])
    .await;
    assert_eq!(l.columns.len(), 4);
    assert_eq!(
        l.gaps,
        vec![Some("·".to_string()), Some("｜".to_string()), None]
    );
}

#[tokio::test]
async fn leading_and_trailing_separators_are_kept() {
    // 2026-11-02 用户拍板：首尾也可放。
    let l = layout_of(vec![
        separator_item("-", 0),
        usage_item("cost", 1),
        usage_item("requests", 2),
        separator_item("|", 3),
    ])
    .await;
    assert_eq!(l.columns.len(), 2);
    assert_eq!(l.gaps, vec![None]);
    assert_eq!(l.lead_gap.as_deref(), Some("-"));
    assert_eq!(l.trail_gap.as_deref(), Some("|"));
}

#[tokio::test]
async fn separators_without_columns_yield_no_lead_or_trail() {
    let l = layout_of(vec![separator_item("|", 0), separator_item("·", 1)]).await;
    assert!(l.columns.is_empty());
    assert_eq!(l.lead_gap, None);
    assert_eq!(l.trail_gap, None);
}

#[tokio::test]
async fn separator_display_outside_presets_falls_back_to_dot() {
    let l = layout_of(vec![
        usage_item("cost", 0),
        separator_item("—", 1),
        usage_item("requests", 2),
        separator_item("", 3),
        usage_item("tokens", 4),
    ])
    .await;
    assert_eq!(
        l.gaps,
        vec![Some("·".to_string()), Some("·".to_string())]
    );
}

#[tokio::test]
async fn skipped_item_does_not_desync_gaps() {
    // platform 行不存在 → 该项被跳过；gaps 长度仍须 = columns - 1。
    let missing: TrayItem = serde_json::from_value(serde_json::json!({
        "item_type": "platform", "platform_id": 999999, "order": 2,
    }))
    .expect("item");
    let l = layout_of(vec![
        usage_item("cost", 0),
        separator_item("|", 1),
        missing,
        usage_item("requests", 3),
    ])
    .await;
    assert_eq!(l.columns.len(), 2);
    assert_eq!(l.gaps, vec![Some("|".to_string())]);
}

#[tokio::test]
async fn disabled_legacy_separator_stays_inert() {
    let mut legacy = separator_item("|", 1);
    legacy.enabled = false;
    let l = layout_of(vec![usage_item("cost", 0), legacy, usage_item("requests", 2)]).await;
    assert_eq!(l.gaps, vec![None]);
}

#[test]
fn tab_stops_include_gap_widths() {
    // 列内容宽 [20, 30, 10]，gap 宽 [4(首段前), 0, 6]：右边界 = 累加 gap+content。
    assert_eq!(
        tab_stop_locs(&[20.0, 30.0, 10.0], &[4.0, 0.0, 6.0]),
        vec![24.0, 54.0, 70.0]
    );
    // 无 gap 时与旧行为（仅 content 累加）一致。
    assert_eq!(tab_stop_locs(&[20.0, 30.0], &[0.0, 0.0]), vec![20.0, 50.0]);
}

#[tokio::test]
async fn gap_is_none_when_no_separator_between_columns() {
    let l = layout_of(vec![usage_item("cost", 0), usage_item("requests", 1)]).await;
    assert_eq!(l.gaps, vec![None]);
}

#[tokio::test]
async fn today_usage_metrics_have_expected_labels_on_empty_db() {
    // 装载段数无上限：4 项全部渲染。
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
            ("请求", "0")
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
        platform_item_parts(&platform(10.10, ""), "balance", &usd(), None),
        ("Acme".to_string(), "$10.1".to_string())
    );
    assert_eq!(
        platform_item_parts(&platform(0.0, ""), "balance", &usd(), None),
        ("Acme".to_string(), "$0".to_string())
    );
}

#[test]
fn platform_item_parts_coding_shows_remaining_percent() {
    let plan = r#"{"tiers":[{"name":"five_hour","est_utilization":25.0}]}"#;
    let (_, value) = platform_item_parts(&platform(0.0, plan), "coding", &usd(), None);
    assert_eq!(value, "75%");
}

#[test]
fn platform_item_parts_coding_clamps_over_100_utilization() {
    let plan = r#"{"tiers":[{"name":"five_hour","est_utilization":150.0}]}"#;
    let (_, value) = platform_item_parts(&platform(0.0, plan), "coding", &usd(), None);
    assert_eq!(value, "0%", "超用不显示负数");
}

#[test]
fn platform_item_parts_coding_plan_beats_balance_display() {
    // 有 coding plan 数据时，即使 display=balance 也按配额百分比显示。
    let plan = r#"{"tiers":[{"name":"five_hour","est_utilization":10.0}]}"#;
    let (_, value) = platform_item_parts(&platform(99.0, plan), "balance", &usd(), None);
    assert_eq!(value, "90%");
}

// ─── 票 I15：固定三段（今日费用 · 当前命中平台 · 高峰指示）──────

#[tokio::test]
async fn default_segments_render_three_columns() {
    let db = test_db().await;
    // 空库首读 → 出厂三段。今日费用取的就是统计页同一份 today_stats（空库 = $0）。
    let l = tray_layout(&db).await;
    let pairs: Vec<(&str, &str)> = l
        .columns
        .iter()
        .map(|c| (c.name.as_str(), c.value.as_str()))
        .collect();
    assert_eq!(pairs, vec![("花费", "$0"), ("命中", "—"), ("高峰", "平")]);
}

#[test]
fn routed_parts_fall_back_to_dash_without_traffic() {
    assert_eq!(
        super::routed_item_parts(None),
        ("命中".to_string(), "—".to_string())
    );
    assert_eq!(
        super::routed_item_parts(Some(&platform(1.0, ""))).1,
        "Acme".to_string()
    );
}

#[test]
fn peak_status_reports_multiplier_only_in_window() {
    let mut p = platform(0.0, "");
    p.extra = r#"{"peak":[{"start_hour":0,"end_hour":24,"multiplier":2.5}]}"#.to_string();
    let s = super::peak_status(&p, 0);
    assert!(s.in_peak);
    assert_eq!(s.multiplier, 2.5);
    p.extra = r#"{"peak":[{"start_hour":1,"end_hour":2,"multiplier":2.5}]}"#.to_string();
    let s = super::peak_status(&p, 0);
    assert!(!s.in_peak);
    assert_eq!(s.multiplier, 1.0);
}

#[test]
fn peak_parts_follow_routed_platform_window() {
    // 无命中平台 → 非高峰。
    assert_eq!(super::peak_item_parts(None, 0).1, "平");
    // 平台带一个 0-24 全天窗口 → 任意时刻都是高峰。
    let mut p = platform(0.0, "");
    p.extra = r#"{"peak":[{"start_hour":0,"end_hour":24,"multiplier":2.0}]}"#.to_string();
    assert_eq!(super::peak_item_parts(Some(&p), 0).1, "峰");
    // 窗口只覆盖 UTC 1:00-2:00 → epoch 0（UTC 0:00）不在窗口内。
    p.extra = r#"{"peak":[{"start_hour":1,"end_hour":2,"multiplier":2.0}]}"#.to_string();
    assert_eq!(super::peak_item_parts(Some(&p), 0).1, "平");
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

// ─── 票 06：托盘费用按显示币种 ──────────────────────────────

fn usd() -> DisplayCurrency {
    build_display_currency(&DisplayCurrencySetting::default())
}

fn cur(code: &str, rate: Option<f64>) -> DisplayCurrency {
    let mut s = DisplayCurrencySetting { code: code.to_string(), ..Default::default() };
    if let Some(r) = rate {
        s.rates.insert(code.to_string(), r);
    }
    build_display_currency(&s)
}

/// 跨层 golden：与前端 TrayConfigTab.test.tsx 共读同一份 tray_golden.json。
#[test]
fn tray_money_golden_cases_match() {
    let cases: Vec<serde_json::Value> = serde_json::from_str(include_str!("testdata/tray_golden.json")).unwrap();
    assert_eq!(cases.len(), 15);
    for c in cases {
        let code = c["code"].as_str().unwrap();
        let rate = c["rate"].as_f64().unwrap();
        let cu = if code == "USD" { usd() } else { cur(code, Some(rate)) };
        // golden 自带 symbol/decimals/rate，同时与币种表互证
        assert_eq!(cu.symbol, c["symbol"].as_str().unwrap(), "{code}");
        assert_eq!(cu.decimals as u64, c["decimals"].as_u64().unwrap(), "{code}");
        assert_eq!(cu.rate, rate, "{code}");
        let got = super::tray_money_unit(
            c["usd"].as_f64().unwrap(),
            &cu,
            c["d"].as_u64().unwrap() as usize,
            c["unit"].as_str().unwrap_or(""),
            c["pos"].as_str(),
        );
        assert_eq!(got, c["expected"].as_str().unwrap(), "{code} {}", c["usd"]);
    }
}

#[test]
fn platform_balance_uses_display_currency() {
    assert_eq!(
        platform_item_parts(&platform(10.0, ""), "balance", &cur("EUR", Some(0.5)), None),
        ("Acme".to_string(), "€5".to_string())
    );
}

// ─── 平台槽多指标（metrics / unit / decimals / coding 窗口细分）───

fn item_json(v: serde_json::Value) -> TrayItem {
    serde_json::from_value(v).expect("TrayItem from json")
}

#[test]
fn platform_metrics_legacy_config_unchanged_without_metrics() {
    // metrics=None → display 旧口径逐字不变（含 coding plan 压过 balance 的旧规则）。
    let plan = r#"{"tiers":[{"name":"five_hour","est_utilization":10.0}]}"#;
    let p = platform(99.0, plan);
    let item = item_json(json!({"item_type": "platform", "platform_id": 1, "display": "balance"}));
    assert_eq!(super::platform_item_value(&p, &item, None, &usd(), None), "90%");
    let item = item_json(json!({"item_type": "platform", "platform_id": 1, "display": "coding"}));
    assert_eq!(super::platform_item_value(&p, &item, None, &usd(), None), "90%");
    let empty_metrics =
        item_json(json!({"item_type": "platform", "platform_id": 1, "display": "balance", "metrics": []}));
    assert_eq!(super::platform_item_value(&p, &empty_metrics, None, &usd(), None), "90%");
}

#[test]
fn platform_metrics_join_with_separator() {
    let p = platform(10.10, "");
    let item = item_json(json!({
        "item_type": "platform", "platform_id": 1,
        "metrics": ["balance", "today_cost", "today_tokens"],
    }));
    // 无 per-platform today 数据 → today 两项落 0 值默认单位。
    assert_eq!(
        super::platform_item_value(&p, &item, None, &usd(), None),
        "$10.1 · $0 · 0 tok"
    );
}

#[test]
fn platform_balance_converts_original_currency_to_usd_first() {
    let bal = |ccy, c: DisplayCurrency| platform_item_parts(&platform(66.675, ""), "balance", &c, ccy).1;
    // CNY 余额 ¥66.675 → $10 → 显示 USD
    assert_eq!(bal(Some("CNY"), usd()), "$10");
    // CNY 余额 + 显示 CNY → 原值
    assert_eq!(bal(Some("CNY"), cur("CNY", None)), "¥66.67");
    // USD 余额 + 显示 JPY
    assert_eq!(bal(Some("USD"), cur("JPY", Some(150.0))), "JP¥10001");
    // 未记录 / 未知币种按 USD 处理
    assert_eq!(bal(None, usd()), "$66.67");
    assert_eq!(bal(Some("EUR"), usd()), "$66.67");
}

#[tokio::test]
async fn layout_reads_currency_setting_from_db() {
    let (db, _dir) = aidog_db::test_support::three_file_db().await;
    aidog_db::currency::set_display_currency(&db, "CNY", Default::default()).await.unwrap();
    let l = tray_layout(&db).await;
    assert_eq!(l.columns[0].value, "¥0");
}

#[test]
fn display_currency_set_triggers_tray_refresh() {
    // 命令层 set 成功后必须 emit tray-refresh（app_setup.rs:622 listener 重画托盘）
    let src = include_str!("display_currency.rs");
    assert!(src.contains(r#"emit_unit("tray-refresh")"#));
}

#[test]
fn platform_metrics_unit_and_decimals() {
    let p = platform(10.10, "");
    let item = item_json(json!({
        "item_type": "platform", "platform_id": 1,
        "metrics": ["balance", "today_cost"], "unit": " U", "decimals": 3,
    }));
    // unit 非空：$ 前缀去掉、单位作后缀；decimals 是裁尾零的上限（与 today_usage cost 同语义）。
    assert_eq!(super::platform_item_value(&p, &item, None, &usd(), None), "10.1 U · 0 U");
    let tok = item_json(json!({
        "item_type": "platform", "platform_id": 1,
        "metrics": ["today_tokens"], "unit": "k",
    }));
    assert_eq!(super::platform_item_value(&p, &tok, None, &usd(), None), "0k");
}

#[test]
fn platform_metrics_coding_window_tiers() {
    let plan = r#"{"tiers":[
        {"name":"five_hour","est_utilization":25.0},
        {"name":"weekly_limit","est_utilization":50.0}
    ]}"#;
    let p = platform(0.0, plan);
    // coding:<tier> 取指定窗口；coding 取首个窗口；缺失 tier 跳过该指标。
    let item = item_json(json!({
        "item_type": "platform", "platform_id": 1,
        "metrics": ["balance", "coding:weekly_limit", "coding:five_hour", "coding:nope"],
    }));
    assert_eq!(
        super::platform_item_value(&p, &item, None, &usd(), None),
        "$0 · 50% · 75%"
    );
    // 全部指标不可用 → "—"。
    let item = item_json(json!({
        "item_type": "platform", "platform_id": 1, "metrics": ["coding:nope"],
    }));
    assert_eq!(super::platform_item_value(&p, &item, None, &usd(), None), "—");
}

#[test]
fn platform_metrics_today_values_from_prefetched_stat() {
    let p = platform(1.0, "");
    let stat = aidog_stats::TodayPlatformStat {
        platform_id: 1,
        platform_name: "Acme".to_string(),
        tokens: 1234,
        cost: 0.5,
        requests: 7,
    };
    let item = item_json(json!({
        "item_type": "platform", "platform_id": 1,
        "metrics": ["today_cost", "today_tokens"],
    }));
    assert_eq!(
        super::platform_item_value(&p, &item, Some(&stat), &usd(), None),
        "$0.5 · 1234 tok"
    );
}

#[test]
fn platform_metrics_reset_time_known_tier() {
    use chrono::TimeZone;
    // 今天本地 10:00 起点 + five_hour(5h) cycle → 15:00 今天 → "HH:mm"。
    let today = chrono::Local::now().date_naive();
    let start = chrono::Local
        .from_local_datetime(&today.and_hms_opt(10, 0, 0).unwrap())
        .single()
        .unwrap()
        .timestamp_millis();
    let plan = format!(
        r#"{{"tiers":[{{"name":"five_hour","est_utilization":10.0,"window_start":{start}}}]}}"#
    );
    let p = platform(0.0, &plan);
    let item = item_json(json!({
        "item_type": "platform", "platform_id": 1,
        "metrics": ["reset_time:five_hour"],
    }));
    assert_eq!(super::platform_item_value(&p, &item, None, &usd(), None), "15:00");
}

#[test]
fn platform_metrics_reset_time_skips_unusable_tiers() {
    // window_start=0（无可靠周期起点）与未知 cycle 的 tier 都跳过 → "—"。
    let plan = r#"{"tiers":[
        {"name":"five_hour","est_utilization":10.0,"window_start":0},
        {"name":"mystery","est_utilization":20.0,"window_start":123}
    ]}"#;
    let p = platform(0.0, plan);
    let item = item_json(json!({
        "item_type": "platform", "platform_id": 1,
        "metrics": ["reset_time:five_hour", "reset_time:mystery"],
    }));
    assert_eq!(super::platform_item_value(&p, &item, None, &usd(), None), "—");
}

/// 今日花费 cost 段自定义 unit 通路（经 layout，空库 cost=0）+ 旧配置逐字不变。
#[tokio::test]
async fn unit_position_cost_segment_and_legacy_platform_unit() {
    let cost = |extra: serde_json::Value| {
        let mut j = json!({"item_type": "today_usage", "metric": "cost"});
        j.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
        item_json(j)
    };
    let v = |it: TrayItem| async move { layout_of(vec![it]).await.columns[0].value.clone() };
    assert_eq!(v(cost(json!({}))).await, "$0");
    assert_eq!(v(cost(json!({"unit": "元"}))).await, "0元", "无 unit_position + unit 非空 = 旧语义后置");
    assert_eq!(v(cost(json!({"unit": "元", "unit_position": "prefix"}))).await, "元0");
    assert_eq!(v(cost(json!({"unit_position": "suffix"}))).await, "0$");
    let cny = cur("CNY", Some(7.0));
    let p = platform(0.0, "");
    let stat = aidog_stats::TodayPlatformStat { platform_id: 1, platform_name: "Acme".into(), tokens: 0, cost: 0.242857142857, requests: 1 };
    let legacy = item_json(json!({"item_type": "platform", "platform_id": 1, "metrics": ["today_cost"], "unit": "元", "decimals": 2}));
    assert_eq!(super::platform_item_value(&p, &legacy, Some(&stat), &cny, None), "1.7元");
    let suffix = item_json(json!({"item_type": "platform", "platform_id": 1, "metrics": ["today_cost"], "decimals": 2, "unit_position": "suffix"}));
    assert_eq!(super::platform_item_value(&p, &suffix, Some(&stat), &cny, None), "1.7¥");
}

/// 跨层 golden：与前端 TrayConfigTab.test.tsx 共读 tray_platform_golden.json（多指标 / 余额币种 / unit）。
#[test]
fn platform_metrics_use_display_currency() {
    let cases: Vec<serde_json::Value> = serde_json::from_str(include_str!("testdata/tray_platform_golden.json")).unwrap();
    assert_eq!(cases.len(), 3);
    for c in cases {
        let p = platform(c["balance"].as_f64().unwrap(), "");
        let stat = aidog_stats::TodayPlatformStat {
            platform_id: 1,
            platform_name: "Acme".to_string(),
            tokens: 0,
            cost: c["cost"].as_f64().unwrap(),
            requests: 1,
        };
        let mut j = json!({"item_type": "platform", "platform_id": 1, "metrics": c["metrics"]});
        if let Some(u) = c["unit"].as_str() {
            j["unit"] = json!(u);
        }
        let cu = cur(c["code"].as_str().unwrap(), Some(c["rate"].as_f64().unwrap()));
        let got = super::platform_item_value(&p, &item_json(j), Some(&stat), &cu, c["balance_ccy"].as_str());
        assert_eq!(got, c["expected"].as_str().unwrap(), "{c}");
    }
}

/// 回归：两列居中，最宽单元（¥1.67 / 97%）的前+文本+后宽度越过列右边界 tab stop，
/// 下一列整体错位（截图 GLM 偏左、97% 偏右）。断言：总宽 < 列宽（不越 tab），且居中误差 ≤ 1 空格。
#[test]
fn align_pad_center_never_overshoots_tab_stop_and_is_centered() {
    // (名, 文本宽, 空格宽) 取 9pt/12pt 粗体比例字体量级；col_w = max(行1, 行2) + 5
    let cols = [
        [("花费", 18.0, 2.5), ("¥1.67", 36.0, 3.3)],
        [("GLM", 26.0, 2.5), ("97%", 34.0, 3.3)],
    ];
    for col in cols {
        let col_w = col.iter().map(|c| c.1).fold(0.0_f64, f64::max) + 5.0;
        for (name, w, sp) in col {
            for align in ["center", "right"] {
                let (lead, trail) = align_pad(false, w, sp, col_w, align);
                let total = (lead + trail) as f64 * sp + w;
                assert!(total < col_w, "{name} {align}: total {total} >= col_w {col_w}");
            }
            let (lead, trail) = align_pad(false, w, sp, col_w, "center");
            let center = lead as f64 * sp + w / 2.0;
            assert!(
                (center - col_w / 2.0).abs() <= sp + 0.01 && trail == 0,
                "{name}: center {center} vs {}",
                col_w / 2.0
            );
        }
    }
    assert_eq!(align_pad(true, 0.0, 3.0, 40.0, "center"), (0, 0));
    assert_eq!(align_pad(false, 10.0, 3.0, 40.0, "left"), (0, 0));
}

/// 模拟 AppKit 左 tab：光标跳到「大于当前位置的下一个 stop」。两行两列居中，
/// 断言 ① 第 2 列两行都从 stop0（列左缘）起步（未越 stop）② 两列最宽单元的可见间隙
/// ≤ 一个空格宽（截图 2：间隔过大）。
#[test]
fn two_column_center_layout_gap_is_tight() {
    // 行1(9pt, 空格 2.5) / 行2(12pt, 空格 3.3)；列0 花费/¥1.7，列1 GLM/97%
    let (sp1, sp2) = (2.5, 3.3);
    let cells = [[(18.0, 36.0)], [(26.0, 34.0)]]; // [列][(行1宽, 行2宽)]
    let widths: Vec<f64> = cells.iter().map(|c| col_width(c[0].0, c[0].1)).collect();
    let stops = [widths[0], widths[0] + widths[1]];
    for (row, sp) in [(0usize, sp1), (1, sp2)] {
        let mut cursor = 0.0_f64;
        let mut starts = vec![];
        for (i, c) in cells.iter().enumerate() {
            let w = if row == 0 { c[0].0 } else { c[0].1 };
            let (lead, trail) = align_pad(false, w, sp, widths[i], "center");
            if i > 0 {
                cursor = *stops.iter().find(|s| **s > cursor).unwrap();
            }
            starts.push(cursor);
            cursor += (lead + trail) as f64 * sp + w;
        }
        assert_eq!(starts[1], stops[0], "row {row}: col1 jumped past stop0");
    }
    // 最宽单元（¥1.7 在列0 行2，97% 在列1 行2）的可见间隙 = 列0 右缘空白 + 列1 左缘 lead
    let (l1, _) = align_pad(false, 34.0, sp2, widths[1], "center");
    let gap = (widths[0] - 36.0 - align_pad(false, 36.0, sp2, widths[0], "center").0 as f64 * sp2)
        + l1 as f64 * sp2;
    assert!(gap <= sp2 + 0.01, "visible gap {gap}pt > one space {sp2}pt");
    assert_eq!(align_pad(true, 0.0, 3.0, 40.0, "center"), (0, 0));
    assert_eq!(align_pad(false, 10.0, 3.0, 40.0, "left"), (0, 0));
}

// ─── 颜色「自动」模式：level 只来自 usage_color，今日花费等无 level ──────

const NOW: i64 = 1_800_000_000_000;
const HOUR: i64 = 3_600_000;

fn auto_item(json: serde_json::Value) -> TrayItem {
    let mut v = json;
    v["color"] = serde_json::json!({"mode": "auto"});
    serde_json::from_value(v).expect("TrayItem from json")
}

/// five_hour 周期已过 1h（20%）；util 与 20 比较决定进度差。
fn coding_plan_json(util: f64) -> String {
    format!(
        r#"{{"tiers":[{{"name":"five_hour","est_utilization":{util},"window_start":{}}}]}}"#,
        NOW - HOUR
    )
}

#[test]
fn balance_level_maps_days_to_usage_color_levels() {
    let item = auto_item(serde_json::json!({"item_type":"platform","platform_id":1,"display":"balance"}));
    let p = platform(10.0, "");
    assert_eq!(super::platform_item_level(&p, &item, Some(5.0), NOW), Some("green"));
    assert_eq!(super::platform_item_level(&p, &item, Some(2.0), NOW), Some("yellow"));
    assert_eq!(super::platform_item_level(&p, &item, Some(0.5), NOW), Some("red"));
    assert_eq!(super::platform_item_level(&p, &item, None, NOW), None, "无用量 -> neutral -> 无 level");
}

#[test]
fn coding_level_maps_pace_delta() {
    let item = auto_item(serde_json::json!({"item_type":"platform","platform_id":1,"display":"coding"}));
    // 时间已过 20%：util 10 -> 差 -10 绿；util 20 -> 0 黄；util 40 -> +20 红。
    assert_eq!(super::platform_item_level(&platform(0.0, &coding_plan_json(10.0)), &item, None, NOW), Some("green"));
    assert_eq!(super::platform_item_level(&platform(0.0, &coding_plan_json(20.0)), &item, None, NOW), Some("yellow"));
    assert_eq!(super::platform_item_level(&platform(0.0, &coding_plan_json(40.0)), &item, None, NOW), Some("red"));
}

#[test]
fn multi_metric_slot_takes_worst_level() {
    let item = auto_item(serde_json::json!({
        "item_type":"platform","platform_id":1,"metrics":["balance","coding","today_cost"]
    }));
    let p = platform(10.0, &coding_plan_json(40.0)); // coding 红
    assert_eq!(super::platform_item_level(&p, &item, Some(5.0), NOW), Some("red"));
    let p = platform(10.0, &coding_plan_json(10.0)); // coding 绿，余额黄
    assert_eq!(super::platform_item_level(&p, &item, Some(2.0), NOW), Some("yellow"));
}

#[test]
fn auto_color_maps_level_to_preset_and_none_to_follow() {
    let col = |level: Option<&str>| super::TrayColumn {
        name: "n".into(),
        value: "v".into(),
        color: aidog_db::TrayColor { mode: "auto".into(), value: String::new() },
        level: level.map(str::to_string),
        font_size: 9.0,
        two_line: false,
        align: "left".into(),
        align_row2: None,
    };
    let c = |l| col(l).render_color();
    assert_eq!((c(Some("red")).mode.as_str(), c(Some("red")).value.as_str()), ("preset", "red"));
    assert_eq!(c(Some("yellow")).value, "orange");
    assert_eq!(c(Some("green")).value, "green");
    assert_eq!(c(None).mode, "follow");
    // follow 老配置：有 level 也不影响色。
    let mut f = col(Some("red"));
    f.color = aidog_db::TrayColor::default();
    assert_eq!(f.render_color().mode, "follow");
}

#[tokio::test]
async fn today_cost_never_has_level_even_in_auto() {
    let it = auto_item(serde_json::json!({"item_type":"today_usage","metric":"cost","order":0}));
    let l = layout_of(vec![it]).await;
    assert_eq!(l.columns[0].color.mode, "auto");
    assert_eq!(l.columns[0].level, None);
}

#[test]
fn two_line_mode_separator_draws_on_label_row_only() {
    // 两行条目（如 coding plan 的配额行 + 重置时间行）：分隔符每边界只画一条——
    // 标签行（row 0）照画，值行（row 1）恒空（宽度仍由 tab stop 保留，列对齐不变）。
    let gaps = vec![Some(" · ".to_string())];
    let lead = Some(" - ".to_string());
    assert_eq!(line_gap_text(0, lead.as_ref(), &gaps, 0), " - ");
    assert_eq!(line_gap_text(1, lead.as_ref(), &gaps, 0), " · ");
    assert_eq!(line_gap_text(0, lead.as_ref(), &gaps, 1), "");
    assert_eq!(line_gap_text(1, lead.as_ref(), &gaps, 1), "");
    assert_eq!(line_gap_text(1, None, &gaps, 1), "");
}
