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
        color: TrayColor { mode: "custom".to_string(), value: "#ff8800".to_string() },
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
