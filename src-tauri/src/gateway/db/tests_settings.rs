#![cfg(test)]
use super::*;
use super::test_support::*;



    // ── setting 软删除 + upsert ──
    #[tokio::test]
    async fn setting_upsert_and_soft_delete() {
        let db = test_db().await;
        set_setting(&db, SetSettingInput {
            scope: "proxy".to_string(),
            key: "logging".to_string(),
            value: serde_json::json!({"enabled": true}),
        }).await.unwrap();
        assert_eq!(list_setting_keys(&db, "proxy").await.unwrap(), vec!["logging".to_string()]);
        let v = get_setting(&db, "proxy", "logging").await.unwrap().unwrap();
        assert_eq!(v["enabled"], serde_json::json!(true));

        delete_setting(&db, "proxy", "logging").await.unwrap();
        assert!(get_setting(&db, "proxy", "logging").await.unwrap().is_none());
        assert_eq!(list_setting_keys(&db, "proxy").await.unwrap().len(), 0);
    }



    // ─── Tray Config ───────────────────────────────────────

    /// TrayConfig serde 往返：写入后读回各字段一致（separator/items 颜色三态/字号/line_mode/排序）。
    #[tokio::test]
    async fn tray_config_serde_roundtrip() {
        let db = test_db().await;
        let cfg = TrayConfig {
            separator: " | ".to_string(),
            items: vec![
                TrayItem {
                    item_type: "platform".to_string(),
                    platform_id: Some(7),
                    display: "coding".to_string(),
                    metric: None,
                    label: None,
decimals: None,
                    color: TrayColor { mode: "preset".to_string(), value: "green".to_string() },
                    font_size: 11.0,
                    line_mode: "two".to_string(),
                    align: "left".to_string(),
                    align_row2: None,
                    enabled: true,
                    order: 0,
                },
                TrayItem {
                    item_type: "today_usage".to_string(),
                    platform_id: None,
                    display: "balance".to_string(),
                    metric: Some("tokens".to_string()),
                    label: None,
decimals: None,
                    color: TrayColor { mode: "custom".to_string(), value: "#ff8800".to_string() },
                    font_size: 9.0,
                    line_mode: "single".to_string(),
                    align: "left".to_string(),
                    align_row2: None,
                    enabled: false,
                    order: 1,
                },
            ],
        };
        set_tray_config(&db, &cfg).await.unwrap();
        let got = get_tray_config(&db).await.unwrap().expect("config present");
        assert_eq!(got.separator, " | ");
        assert_eq!(got.items.len(), 2);
        assert_eq!(got.items[0].item_type, "platform");
        assert_eq!(got.items[0].platform_id, Some(7));
        assert_eq!(got.items[0].display, "coding");
        assert_eq!(got.items[0].color.mode, "preset");
        assert_eq!(got.items[0].color.value, "green");
        assert!((got.items[0].font_size - 11.0).abs() < 1e-9);
        assert_eq!(got.items[0].line_mode, "two");
        assert!(got.items[0].enabled);
        assert_eq!(got.items[1].line_mode, "single");
        assert_eq!(got.items[1].item_type, "today_usage");
        assert_eq!(got.items[1].metric.as_deref(), Some("tokens"));
        assert_eq!(got.items[1].color.mode, "custom");
        assert_eq!(got.items[1].color.value, "#ff8800");
        assert!(!got.items[1].enabled);
        assert_eq!(got.items[1].order, 1);
    }



    /// 迁移：无 tray config 且无旧 show_in_tray 平台 → 生成空配置并持久化（避免重复迁移）。
    #[tokio::test]
    async fn tray_config_migrate_empty() {
        let db = test_db().await;
        // 首次读取触发迁移；无旧平台 → 空 items。
        let cfg = get_tray_config(&db).await.unwrap().expect("migrated config");
        assert_eq!(cfg.items.len(), 0);
        // 已持久化：settings 中应存在 tray/config。
        assert!(get_setting(&db, "tray", "config").await.unwrap().is_some());
    }



    /// 迁移：旧 show_in_tray=1 平台 → 生成默认 platform item（保留 tray_display）。
    #[tokio::test]
    async fn tray_config_migrate_from_legacy_platform() {
        let db = test_db().await;
        let p = create_platform(&db, sample_platform("legacy")).await.unwrap();
        set_tray_platform(&db, p.id, "coding").await.unwrap();

        let cfg = get_tray_config(&db).await.unwrap().expect("migrated config");
        assert_eq!(cfg.items.len(), 1, "应从旧平台生成 1 个 platform item");
        let item = &cfg.items[0];
        assert_eq!(item.item_type, "platform");
        assert_eq!(item.platform_id, Some(p.id));
        assert_eq!(item.display, "coding");
        assert!(item.enabled);
    }



    /// 迁移：旧全局 layout=two_line → 各 item line_mode="two"；缺 line_mode 字段时按 serde default "single"。
    #[tokio::test]
    async fn tray_config_migrate_legacy_layout() {
        let db = test_db().await;
        // 模拟旧版本写入：含全局 layout 字段，item 无 line_mode 字段。
        let legacy = serde_json::json!({
            "layout": "two_line",
            "separator": "  ",
            "items": [
                { "item_type": "platform", "platform_id": 3, "display": "balance",
                  "color": { "mode": "follow", "value": "" }, "font_size": 9.0,
                  "enabled": true, "order": 0 }
            ]
        });
        set_setting(&db, SetSettingInput {
            scope: "tray".to_string(),
            key: "config".to_string(),
            value: legacy,
        }).await.unwrap();

        let cfg = get_tray_config(&db).await.unwrap().expect("config present");
        assert_eq!(cfg.items.len(), 1);
        // 旧全局 two_line → item line_mode="two"。
        assert_eq!(cfg.items[0].line_mode, "two");
    }



    /// serde default：缺 line_mode 字段 → "two"（default_line_mode）。
    #[tokio::test]
    async fn tray_item_line_mode_serde_default() {
        let raw = serde_json::json!({
            "item_type": "platform", "platform_id": 1, "display": "balance",
            "color": { "mode": "follow", "value": "" }, "font_size": 9.0,
            "enabled": true, "order": 0
        });
        let item: TrayItem = serde_json::from_value(raw).unwrap();
        assert_eq!(item.line_mode, "two");
    }
