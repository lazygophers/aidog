#![cfg(test)]
use super::*;
use super::test_support::*;
use rusqlite::params;

    // ── R9 软删除：delete 后 deleted_at>0；list 不含；get 返回 None ──
    #[tokio::test]
    async fn r9_soft_delete_platform() {
        let db = test_db().await;
        let p = create_platform(&db, sample_platform("del")).await.unwrap();
        assert_eq!(list_platforms(&db).await.unwrap().len(), 1);

        delete_platform(&db, p.id).await.unwrap();

        // list 不返回已删行
        assert_eq!(list_platforms(&db).await.unwrap().len(), 0);
        // get 返回 None
        assert!(get_platform(&db, p.id).await.unwrap().is_none());

        // 行仍存在且 deleted_at > 0（物理保留）
        let pid = p.id as i64;
        let deleted_at: i64 = db.call_traced(None, std::panic::Location::caller(), move |conn| {
            Ok(conn.query_row("SELECT deleted_at FROM platform WHERE id = ?1", params![pid], |r| r.get(0))?)
        }).await.unwrap();
        assert!(deleted_at > 0, "deleted_at should be set, got {deleted_at}");
    }



    // ── 删平台只清关联，不连带销毁手动组与有其他成员的 auto 组 ──
    #[tokio::test]
    async fn delete_platform_preserves_groups_with_other_members() {
        let db = test_db().await;
        let p_del = create_platform(&db, sample_platform("del-src")).await.unwrap();
        let p_keep = create_platform(&db, sample_platform("keep")).await.unwrap();

        // ① 手动组：同时含待删平台与另一存活平台。
        let m = create_group(&db, sample_group("m-shared", vec![])).await.unwrap();
        set_group_platforms(&db, m.id, &[
            GroupPlatformInput { platform_id: p_del.id, priority: Some(0), weight: Some(1), level_priority: None },
            GroupPlatformInput { platform_id: p_keep.id, priority: Some(1), weight: Some(1), level_priority: None },
        ]).await.unwrap();

        // ② p_del 的 auto 组，用户额外把 p_keep 也拖了进来。
        let auto_g = create_group(&db, CreateGroup {
            name: "del-src-auto".into(), group_key: Some("del-src-auto".into()),
            routing_mode: RoutingMode::Failover, auto_from_platform: p_del.id.to_string(),
            request_timeout_secs: 0, connect_timeout_secs: 0,
            source_protocol: None, max_retries: 2, model_mappings: vec![],
        }).await.unwrap();
        set_group_platforms(&db, auto_g.id, &[
            GroupPlatformInput { platform_id: p_del.id, priority: Some(0), weight: Some(1), level_priority: None },
            GroupPlatformInput { platform_id: p_keep.id, priority: Some(1), weight: Some(1), level_priority: None },
        ]).await.unwrap();

        // ③ p_del 的孤儿 auto 组（仅含自己），删平台后应被清掉。
        let auto_orphan = create_group(&db, CreateGroup {
            name: "del-src-orphan".into(), group_key: Some("del-src-orphan".into()),
            routing_mode: RoutingMode::Failover, auto_from_platform: p_del.id.to_string(),
            request_timeout_secs: 0, connect_timeout_secs: 0,
            source_protocol: None, max_retries: 2, model_mappings: vec![],
        }).await.unwrap();
        set_group_platforms(&db, auto_orphan.id, &[
            GroupPlatformInput { platform_id: p_del.id, priority: Some(0), weight: Some(1), level_priority: None },
        ]).await.unwrap();

        delete_platform(&db, p_del.id).await.unwrap();

        // 手动组仍在，仅剩存活平台，悬空关联已清除。
        assert!(get_group(&db, m.id).await.unwrap().is_some(), "手动组不应被删");
        let m_plats = get_group_platforms(&db, m.id).await.unwrap();
        assert_eq!(m_plats.len(), 1, "手动组仅余存活平台");
        assert_eq!(m_plats[0].platform.id, p_keep.id);

        // 含其他成员的 auto 组保留，只剩 p_keep。
        assert!(get_group(&db, auto_g.id).await.unwrap().is_some(), "有其他成员的 auto 组不应被删");
        let a_plats = get_group_platforms(&db, auto_g.id).await.unwrap();
        assert_eq!(a_plats.len(), 1);
        assert_eq!(a_plats[0].platform.id, p_keep.id);

        // 孤儿 auto 组（删后无成员）被删。
        assert!(get_group(&db, auto_orphan.id).await.unwrap().is_none(), "孤儿 auto 组应被删");

        // 全表无指向已删平台的关联残留。
        let pid = p_del.id as i64;
        let stale: i64 = db.call_traced(None, std::panic::Location::caller(), move |conn| {
            Ok(conn.query_row("SELECT COUNT(*) FROM group_platform WHERE platform_id = ?1", params![pid], |r| r.get(0))?)
        }).await.unwrap();
        assert_eq!(stale, 0, "不应残留指向已删平台的 group_platform 行");
    }



    // ── 一键清理失效平台：全局全删 / 分组独占删 / 分组共享仅移除关联 ──
    #[tokio::test]
    async fn purge_auto_disabled_global_deletes_all() {
        let db = test_db().await;
        // 两个 auto_disabled + 一个 enabled（应保留）。
        let p_dead1 = create_platform(&db, sample_platform("dead1")).await.unwrap();
        let p_dead2 = create_platform(&db, sample_platform("dead2")).await.unwrap();
        let p_alive = create_platform(&db, sample_platform("alive")).await.unwrap();
        set_platform_auto_disabled(&db, p_dead1.id).await.unwrap();
        set_platform_auto_disabled(&db, p_dead2.id).await.unwrap();

        let r = purge_auto_disabled_platforms(&db, None).await.unwrap();
        assert_eq!(r.deleted_ids.len(), 2, "全局应删两个 auto_disabled");
        assert!(r.unassigned_ids.is_empty(), "全局模式不应有 unassigned");
        assert!(r.deleted_ids.contains(&(p_dead1.id as u64)));
        assert!(r.deleted_ids.contains(&(p_dead2.id as u64)));

        // 已删平台行仍存在但 deleted_at>0；enabled 平台未动。
        assert!(get_platform(&db, p_dead1.id).await.unwrap().is_none());
        assert!(get_platform(&db, p_dead2.id).await.unwrap().is_none());
        assert!(get_platform(&db, p_alive.id).await.unwrap().is_some(), "enabled 平台不应被删");
    }



    #[tokio::test]
    async fn purge_auto_disabled_group_exclusive_deletes_shared_unassigns() {
        let db = test_db().await;
        // 平台 A：仅属 g1，auto_disabled → 分组级清理应永久删。
        let p_a = create_platform(&db, sample_platform("a-exclusive")).await.unwrap();
        // 平台 B：属 g1 + g2，auto_disabled → 分组级清理 g1 应仅移除 g1 关联，平台行保留、g2 关联保留。
        let p_b = create_platform(&db, sample_platform("b-shared")).await.unwrap();
        set_platform_auto_disabled(&db, p_a.id).await.unwrap();
        set_platform_auto_disabled(&db, p_b.id).await.unwrap();

        let g1 = create_group(&db, sample_group("g1", vec![])).await.unwrap();
        let g2 = create_group(&db, sample_group("g2", vec![])).await.unwrap();
        set_group_platforms(&db, g1.id, &[
            GroupPlatformInput { platform_id: p_a.id, priority: Some(0), weight: Some(1), level_priority: None },
            GroupPlatformInput { platform_id: p_b.id, priority: Some(1), weight: Some(1), level_priority: None },
        ]).await.unwrap();
        set_group_platforms(&db, g2.id, &[
            GroupPlatformInput { platform_id: p_b.id, priority: Some(0), weight: Some(1), level_priority: None },
        ]).await.unwrap();

        let r = purge_auto_disabled_platforms(&db, Some(g1.id)).await.unwrap();
        // 独占本分组的 A 被永久删；共享的 B 仅从 g1 移除关联。
        assert_eq!(r.deleted_ids, vec![p_a.id as u64], "独占本分组的 A 应永久删");
        assert_eq!(r.unassigned_ids, vec![p_b.id as u64], "共享的 B 应仅移除本分组关联");

        // A 已软删；B 行仍在。
        assert!(get_platform(&db, p_a.id).await.unwrap().is_none(), "A 应被软删");
        assert!(get_platform(&db, p_b.id).await.unwrap().is_some(), "B 行应保留");

        // g1 成员已空（A 删 + B 移除）；g2 仍保留 B。
        let g1_plats = get_group_platforms(&db, g1.id).await.unwrap();
        assert!(g1_plats.is_empty(), "g1 应无成员残留");
        let g2_plats = get_group_platforms(&db, g2.id).await.unwrap();
        assert_eq!(g2_plats.len(), 1, "g2 应仍含 B");
        assert_eq!(g2_plats[0].platform.id, p_b.id);

        // 全表无指向已删平台 A 的关联残留（delete_platform 清所有 group_platform）。
        let pid_a = p_a.id as i64;
        let stale_a: i64 = db.call_traced(None, std::panic::Location::caller(), move |conn| {
            Ok(conn.query_row("SELECT COUNT(*) FROM group_platform WHERE platform_id = ?1", params![pid_a], |r| r.get(0))?)
        }).await.unwrap();
        assert_eq!(stale_a, 0, "A 不应残留任何 group_platform 行");
    }



    #[tokio::test]
    async fn purge_auto_disabled_group_skips_enabled() {
        let db = test_db().await;
        // 本分组含一个 enabled 平台 + 一个 auto_disabled 平台。
        let p_alive = create_platform(&db, sample_platform("alive-g")).await.unwrap();
        let p_dead = create_platform(&db, sample_platform("dead-g")).await.unwrap();
        set_platform_auto_disabled(&db, p_dead.id).await.unwrap();
        let g = create_group(&db, sample_group("g", vec![])).await.unwrap();
        set_group_platforms(&db, g.id, &[
            GroupPlatformInput { platform_id: p_alive.id, priority: Some(0), weight: Some(1), level_priority: None },
            GroupPlatformInput { platform_id: p_dead.id, priority: Some(1), weight: Some(1), level_priority: None },
        ]).await.unwrap();

        let r = purge_auto_disabled_platforms(&db, Some(g.id)).await.unwrap();
        assert_eq!(r.deleted_ids, vec![p_dead.id as u64], "仅 auto_disabled 应被删");
        assert!(r.unassigned_ids.is_empty());

        // enabled 平台行 + 分组关联保留。
        assert!(get_platform(&db, p_alive.id).await.unwrap().is_some());
        let g_plats = get_group_platforms(&db, g.id).await.unwrap();
        assert_eq!(g_plats.len(), 1, "g 应仅余 enabled 平台");
        assert_eq!(g_plats[0].platform.id, p_alive.id);
    }



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
