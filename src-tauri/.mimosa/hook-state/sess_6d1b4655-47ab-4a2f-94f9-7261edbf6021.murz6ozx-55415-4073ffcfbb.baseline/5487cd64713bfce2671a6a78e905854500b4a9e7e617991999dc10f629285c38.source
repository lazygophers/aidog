//! tray.rs 模型单测（原 models.rs `popover_config_model_tests`）。

use super::*;

#[test]
fn legacy_item_without_trend_fields_deserializes() {
    // 旧配置（无 scope / scope_ref / time_window）必须反序列化成功并取默认值。
    let json = r#"{"id":"popover-today_cost","item_type":"today_cost","visible":true,"order":2}"#;
    let item: PopoverItem = serde_json::from_str(json).expect("legacy item must deserialize");
    assert_eq!(item.item_type, "today_cost");
    assert_eq!(item.scope, "overall");
    assert!(item.scope_ref.is_none());
    assert_eq!(item.time_window, "7d");
    // 旧配置无 row/size/color → serde default 兜底。
    assert_eq!(item.row, 0);
    assert_eq!(item.size, "m");
    assert_eq!(item.color.mode, "follow");
    assert_eq!(item.color.value, "");
}

#[test]
fn cost_trend_item_roundtrips() {
    let item = PopoverItem {
        id: "popover-trend-1".to_string(),
        item_type: "cost_trend".to_string(),
        visible: true,
        order: 0,
        scope: "group".to_string(),
        scope_ref: Some("gk_abc".to_string()),
        time_window: "30d".to_string(),
        row: 2,
        size: "l".to_string(),
        color: TrayColor {
            mode: "custom".to_string(),
            value: "#ff8800".to_string(),
        },
    };
    let json = serde_json::to_string(&item).unwrap();
    let back: PopoverItem = serde_json::from_str(&json).unwrap();
    assert_eq!(back.scope, "group");
    assert_eq!(back.scope_ref.as_deref(), Some("gk_abc"));
    assert_eq!(back.time_window, "30d");
    assert_eq!(back.row, 2);
    assert_eq!(back.size, "l");
    assert_eq!(back.color.mode, "custom");
    assert_eq!(back.color.value, "#ff8800");
}

#[test]
fn legacy_config_without_new_fields_deserializes() {
    let json = r#"{"items":[{"id":"a","item_type":"proxy_status","visible":true,"order":0}]}"#;
    let cfg: PopoverConfig = serde_json::from_str(json).expect("legacy config must deserialize");
    assert_eq!(cfg.items.len(), 1);
    assert_eq!(cfg.items[0].scope, "overall");
    assert_eq!(cfg.items[0].time_window, "7d");
    // 旧配置无 rows → 空 vec；item 新字段取默认。
    assert!(cfg.rows.is_empty());
    assert_eq!(cfg.items[0].row, 0);
    assert_eq!(cfg.items[0].size, "m");
    assert_eq!(cfg.items[0].color.mode, "follow");
}

#[test]
fn config_with_rows_roundtrips() {
    // 含二维布局新字段的完整配置往返。
    let json = r#"{
        "items":[{"id":"a","item_type":"today_cost","visible":true,"order":0,"row":0,"size":"s","color":{"mode":"preset","value":"green"}}],
        "rows":[{"cols":2},{"cols":3}]
    }"#;
    let cfg: PopoverConfig = serde_json::from_str(json).expect("config with rows must deserialize");
    assert_eq!(cfg.rows.len(), 2);
    assert_eq!(cfg.rows[0].cols, 2);
    assert_eq!(cfg.rows[1].cols, 3);
    assert_eq!(cfg.items[0].size, "s");
    assert_eq!(cfg.items[0].color.mode, "preset");
    assert_eq!(cfg.items[0].color.value, "green");

    // 序列化回去再读，字段保真。
    let s = serde_json::to_string(&cfg).unwrap();
    let back: PopoverConfig = serde_json::from_str(&s).unwrap();
    assert_eq!(back.rows[0].cols, 2);
    assert_eq!(back.items[0].size, "s");
}

#[test]
fn row_meta_without_cols_defaults_to_one() {
    // rows 项缺 cols → default_cols=1。
    let json = r#"{"items":[],"rows":[{}]}"#;
    let cfg: PopoverConfig = serde_json::from_str(json).expect("row without cols must deserialize");
    assert_eq!(cfg.rows[0].cols, 1);
}

#[test]
fn default_config_populates_new_fields() {
    let cfg = PopoverConfig::default();
    // 默认配置各 item row=order（各占一行），size="m"，color follow。
    for (i, item) in cfg.items.iter().enumerate() {
        assert_eq!(item.row, i as i32);
        assert_eq!(item.size, "m");
        assert_eq!(item.color.mode, "follow");
    }
    assert!(cfg.rows.is_empty());
}

// ─── 票 I15：二维网格 → 最多 3 段的迁移规则 ───────────────

/// 造一个 enabled 的数据项（item_type 由调用方给）。
fn it(item_type: &str, order: i32) -> TrayItem {
    let mut i = segment("platform", None, order);
    i.item_type = item_type.to_string();
    i
}

#[test]
fn clamp_keeps_first_three_and_disables_rest_without_deleting() {
    let mut cfg = TrayConfig {
        separator: "  ".to_string(),
        items: vec![
            it("platform", 0),
            it("today_usage", 1),
            it("platform", 2),
            it("today_usage", 3),
            it("platform", 4),
        ],
    };
    assert!(clamp_to_segments(&mut cfg));
    // 一项都没丢。
    assert_eq!(cfg.items.len(), 5);
    let enabled: Vec<bool> = cfg.items.iter().map(|i| i.enabled).collect();
    assert_eq!(enabled, vec![true, true, true, false, false]);
}

#[test]
fn clamp_uses_order_not_array_index() {
    let mut cfg = TrayConfig {
        separator: "  ".to_string(),
        items: vec![
            it("platform", 9),
            it("today_usage", 0),
            it("platform", 1),
            it("today_usage", 2),
        ],
    };
    clamp_to_segments(&mut cfg);
    // order 0/1/2 三项留下，order 9 那项被关掉（数组第 0 个）。
    assert!(!cfg.items[0].enabled);
    assert!(cfg.items[1].enabled && cfg.items[2].enabled && cfg.items[3].enabled);
}

#[test]
fn clamp_disables_separators_but_keeps_their_fields() {
    let mut sep = it("separator", 1);
    sep.display = "·".to_string();
    let mut two_line = it("platform", 0);
    two_line.line_mode = "two".to_string();
    two_line.align_row2 = Some("right".to_string());
    let mut cfg = TrayConfig {
        separator: "  ".to_string(),
        items: vec![two_line, sep, it("platform", 2)],
    };
    assert!(clamp_to_segments(&mut cfg));
    assert!(!cfg.items[1].enabled, "separator 不再是可选段");
    assert_eq!(cfg.items[1].display, "·", "字段原样保留，可降级回旧版本");
    // 二维排布字段（line_mode / align_row2）一律不动。
    assert_eq!(cfg.items[0].line_mode, "two");
    assert_eq!(cfg.items[0].align_row2.as_deref(), Some("right"));
    assert!(cfg.items[0].enabled && cfg.items[2].enabled);
}

#[test]
fn clamp_is_idempotent_and_reports_no_change_when_already_clamped() {
    let mut cfg = TrayConfig::default_segments();
    assert!(!clamp_to_segments(&mut cfg), "出厂三段本就合规，不该被改");
    assert_eq!(cfg.items.len(), 3);
    assert!(cfg.items.iter().all(|i| i.enabled));
}

#[test]
fn default_segments_are_cost_routed_peak() {
    let cfg = TrayConfig::default_segments();
    let kinds: Vec<&str> = cfg.items.iter().map(|i| i.item_type.as_str()).collect();
    assert_eq!(kinds, vec!["today_usage", "routed_platform", "peak"]);
    assert_eq!(cfg.items[0].metric.as_deref(), Some("cost"));
    assert_eq!(cfg.items.len(), TRAY_MAX_SEGMENTS);
}
