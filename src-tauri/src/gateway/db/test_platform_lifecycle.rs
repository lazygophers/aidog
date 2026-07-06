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
            source_protocol: None, max_retries: 2, model_mappings: vec![], env_vars: vec![],        }).await.unwrap();
        set_group_platforms(&db, auto_g.id, &[
            GroupPlatformInput { platform_id: p_del.id, priority: Some(0), weight: Some(1), level_priority: None },
            GroupPlatformInput { platform_id: p_keep.id, priority: Some(1), weight: Some(1), level_priority: None },
        ]).await.unwrap();

        // ③ p_del 的孤儿 auto 组（仅含自己），删平台后应保留为空组（前端展示空卡，用户手动清）。
        let auto_orphan = create_group(&db, CreateGroup {
            name: "del-src-orphan".into(), group_key: Some("del-src-orphan".into()),
            routing_mode: RoutingMode::Failover, auto_from_platform: p_del.id.to_string(),
            request_timeout_secs: 0, connect_timeout_secs: 0,
            source_protocol: None, max_retries: 2, model_mappings: vec![], env_vars: vec![],        }).await.unwrap();
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

        // 孤儿 auto 组保留（删平台只清关联，不连带销毁分组），组内空。
        assert!(get_group(&db, auto_orphan.id).await.unwrap().is_some(), "孤儿 auto 组应保留");
        let orphan_plats = get_group_platforms(&db, auto_orphan.id).await.unwrap();
        assert!(orphan_plats.is_empty(), "孤儿 auto 组组内应空");

        // 全表无指向已删平台的关联残留。
        let pid = p_del.id as i64;
        let stale: i64 = db.call_traced(None, std::panic::Location::caller(), move |conn| {
            Ok(conn.query_row("SELECT COUNT(*) FROM group_platform WHERE platform_id = ?1", params![pid], |r| r.get(0))?)
        }).await.unwrap();
        assert_eq!(stale, 0, "不应残留指向已删平台的 group_platform 行");
    }



    // ── 删平台保留所有分组（含孤儿 auto 组），同名 auto 组空卡前端正常展示 ──
    #[tokio::test]
    async fn delete_platform_keeps_orphan_auto_group_empty() {
        let db = test_db().await;
        let p = create_platform(&db, sample_platform("lone")).await.unwrap();
        // p 的同名 auto 组（仅含 p）。
        let g = create_group(&db, CreateGroup {
            name: "lone-auto".into(), group_key: Some("lone-auto".into()),
            routing_mode: RoutingMode::Failover, auto_from_platform: p.id.to_string(),
            request_timeout_secs: 0, connect_timeout_secs: 0,
            source_protocol: None, max_retries: 2, model_mappings: vec![], env_vars: vec![],
        }).await.unwrap();
        set_group_platforms(&db, g.id, &[
            GroupPlatformInput { platform_id: p.id, priority: Some(0), weight: Some(1), level_priority: None },
        ]).await.unwrap();
        assert_eq!(get_group_platforms(&db, g.id).await.unwrap().len(), 1);

        delete_platform(&db, p.id).await.unwrap();

        // p 软删：list 不含，get 返回 None。
        assert!(list_platforms(&db).await.unwrap().is_empty());
        assert!(get_platform(&db, p.id).await.unwrap().is_none());

        // 全表无指向 p 的关联残留。
        let pid = p.id as i64;
        let stale: i64 = db.call_traced(None, std::panic::Location::caller(), move |conn| {
            Ok(conn.query_row("SELECT COUNT(*) FROM group_platform WHERE platform_id = ?1", params![pid], |r| r.get(0))?)
        }).await.unwrap();
        assert_eq!(stale, 0, "不应残留指向已删平台的 group_platform 行");

        // 同名 auto 组保留（deleted_at=0），组内空。
        let g_row = get_group(&db, g.id).await.unwrap().expect("孤儿 auto 组应保留");
        assert_eq!(g_row.id, g.id);
        let g_plats = get_group_platforms(&db, g.id).await.unwrap();
        assert!(g_plats.is_empty(), "保留后组内应空");
    }



    // ── 一键清理失效平台：全局全删 / 分组独占删 / 分组共享仅移除关联 ──
    #[tokio::test]
    async fn purge_auto_disabled_global_deletes_all() {
        let db = test_db().await;
        // 两个 auto_disabled + 一个 enabled（应保留）。
        let p_dead1 = create_platform(&db, sample_platform("dead1")).await.unwrap();
        let p_dead2 = create_platform(&db, sample_platform("dead2")).await.unwrap();
        let p_alive = create_platform(&db, sample_platform("alive")).await.unwrap();
        // R2：auto_disabled 仅 401/403 last_error 才被一键清理删除。
        set_platform_auto_disabled(&db, p_dead1.id).await.unwrap();
        set_platform_auto_disabled(&db, p_dead2.id).await.unwrap();
        set_platform_last_error(&db, p_dead1.id, Some("HTTP 401: unauthorized".into())).await.unwrap();
        set_platform_last_error(&db, p_dead2.id, Some("HTTP 403: forbidden".into())).await.unwrap();

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
        set_platform_last_error(&db, p_a.id, Some("HTTP 401: bad key".into())).await.unwrap();
        set_platform_last_error(&db, p_b.id, Some("HTTP 403: forbidden".into())).await.unwrap();

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
        set_platform_last_error(&db, p_dead.id, Some("HTTP 401: bad key".into())).await.unwrap();
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

    /// 辅助：直接 UPDATE platform 设 expires_at（测试专用，绕过 update_platform Option 语义）。
    async fn set_expires_at(db: &Db, id: u64, expires_at: i64) {
        let pid = id as i64;
        db.call_traced(None, std::panic::Location::caller(), move |conn| {
            conn.execute(
                "UPDATE platform SET expires_at = ?1 WHERE id = ?2",
                params![expires_at, pid],
            )?;
            Ok(())
        })
        .await
        .unwrap();
    }

    /// 全局 purge：清 auto_disabled + 已过期（expires_at > 0 且 < now）平台，保留未过期的。
    #[tokio::test]
    async fn purge_global_also_deletes_expired_platforms() {
        let db = test_db().await;
        // 两个过期 + 一个未过期 + 一个 auto_disabled，均 enabled（隔离 expires_at 维度）。
        let p_expired1 = create_platform(&db, sample_platform("exp1")).await.unwrap();
        let p_expired2 = create_platform(&db, sample_platform("exp2")).await.unwrap();
        let p_future = create_platform(&db, sample_platform("future")).await.unwrap();
        let p_disabled = create_platform(&db, sample_platform("disabled")).await.unwrap();
        let now = now();
        set_expires_at(&db, p_expired1.id, now - 1000).await;
        set_expires_at(&db, p_expired2.id, now - 1).await;
        set_expires_at(&db, p_future.id, now + 86_400_000).await;
        set_platform_auto_disabled(&db, p_disabled.id).await.unwrap();
        set_platform_last_error(&db, p_disabled.id, Some("HTTP 401: unauthorized".into())).await.unwrap();

        let r = purge_auto_disabled_platforms(&db, None).await.unwrap();
        assert_eq!(r.deleted_ids.len(), 3, "应删 2 过期 + 1 auto_disabled");
        assert!(r.deleted_ids.contains(&(p_expired1.id as u64)), "过期1 应删");
        assert!(r.deleted_ids.contains(&(p_expired2.id as u64)), "过期2 应删");
        assert!(r.deleted_ids.contains(&(p_disabled.id as u64)), "auto_disabled 应删");
        assert!(get_platform(&db, p_future.id).await.unwrap().is_some(), "未过期平台应保留");
    }

    /// 分组级 purge：清 auto_disabled + 已过期平台（独占删 / 共享移关联）。
    #[tokio::test]
    async fn purge_group_also_deletes_expired_platforms() {
        let db = test_db().await;
        // p_exp_excl：仅属 g1，过期 → 分组级清理永久删。
        let p_exp_excl = create_platform(&db, sample_platform("exp-excl")).await.unwrap();
        // p_exp_shared：属 g1 + g2，过期 → 仅删 g1 关联，平台行保留。
        let p_exp_shared = create_platform(&db, sample_platform("exp-shared")).await.unwrap();
        // p_alive：属 g1，未过期 → 保留。
        let p_alive = create_platform(&db, sample_platform("alive")).await.unwrap();
        let now = now();
        set_expires_at(&db, p_exp_excl.id, now - 1000).await;
        set_expires_at(&db, p_exp_shared.id, now - 1000).await;

        let g1 = create_group(&db, sample_group("g1", vec![])).await.unwrap();
        let g2 = create_group(&db, sample_group("g2", vec![])).await.unwrap();
        set_group_platforms(&db, g1.id, &[
            GroupPlatformInput { platform_id: p_exp_excl.id, priority: Some(0), weight: Some(1), level_priority: None },
            GroupPlatformInput { platform_id: p_exp_shared.id, priority: Some(1), weight: Some(1), level_priority: None },
            GroupPlatformInput { platform_id: p_alive.id, priority: Some(2), weight: Some(1), level_priority: None },
        ]).await.unwrap();
        set_group_platforms(&db, g2.id, &[
            GroupPlatformInput { platform_id: p_exp_shared.id, priority: Some(0), weight: Some(1), level_priority: None },
        ]).await.unwrap();

        let r = purge_auto_disabled_platforms(&db, Some(g1.id)).await.unwrap();
        // 独占的过期平台永久删；共享的过期平台仅移 g1 关联。
        assert_eq!(r.deleted_ids, vec![p_exp_excl.id as u64], "独占过期平台应永久删");
        assert_eq!(r.unassigned_ids, vec![p_exp_shared.id as u64], "共享过期平台应仅移关联");
        assert!(get_platform(&db, p_exp_excl.id).await.unwrap().is_none(), "独占过期平台行应软删");
        assert!(get_platform(&db, p_exp_shared.id).await.unwrap().is_some(), "共享过期平台行应保留");
        // p_alive 仍在 g1
        let g1_plats = get_group_platforms(&db, g1.id).await.unwrap();
        assert_eq!(g1_plats.len(), 1, "g1 仅余未过期平台");
        assert_eq!(g1_plats[0].platform.id, p_alive.id);
    }

    /// update_platform_quota 写入余额和 coding_plan，再读回。
    #[tokio::test]
    async fn update_platform_quota_persists() {
        let db = test_db().await;
        let p = create_platform(&db, sample_platform("quota-test")).await.unwrap();
        update_platform_quota(&db, p.id, 12.34, r#"{"plan":"pro"}"#).await.unwrap();
        let got = get_platform(&db, p.id).await.unwrap().expect("platform exists");
        assert!((got.est_balance_remaining - 12.34).abs() < 1e-9);
        assert_eq!(got.est_coding_plan, r#"{"plan":"pro"}"#);
    }

    /// purge_old_soft_deleted_platforms 删物理行（deleted_at > 0 且超时）。
    #[tokio::test]
    async fn purge_old_soft_deleted_removes_expired_rows() {
        let db = test_db().await;
        let p = create_platform(&db, sample_platform("purge-old")).await.unwrap();
        // 软删
        delete_platform(&db, p.id).await.unwrap();
        // older_than_secs=-1 → cutoff = now+1, deleted_at < now+1 → 应删
        let n = purge_old_soft_deleted_platforms(&db, -1).await.unwrap();
        assert!(n >= 1, "should delete at least 1 old soft-deleted platform, got {n}");
        // 行已被物理删除，无法按 id 查到（get_platform 只过滤 deleted_at=0）
        let pid = p.id as i64;
        let exists: i64 = db.call_traced(None, std::panic::Location::caller(), move |c| {
            Ok(c.query_row("SELECT COUNT(*) FROM platform WHERE id = ?1", params![pid], |r| r.get(0))?)
        }).await.unwrap();
        assert_eq!(exists, 0, "row should be physically deleted");
    }

    /// purge_old_soft_deleted_platforms 不删尚未超时的软删行（very large older_than_secs）。
    #[tokio::test]
    async fn purge_old_soft_deleted_keeps_recent_rows() {
        let db = test_db().await;
        let p = create_platform(&db, sample_platform("purge-recent")).await.unwrap();
        delete_platform(&db, p.id).await.unwrap();
        // older_than_secs=9999999 → cutoff 在未来，deleted_at 不满足 < cutoff
        let n = purge_old_soft_deleted_platforms(&db, 9_999_999).await.unwrap();
        assert_eq!(n, 0, "should not delete recently soft-deleted rows");
    }

    /// set_tray_platform + get_tray_platform + clear_tray 完整流程。
    #[tokio::test]
    async fn tray_platform_set_get_clear() {
        let db = test_db().await;
        let p1 = create_platform(&db, sample_platform("tray-p1")).await.unwrap();
        let p2 = create_platform(&db, sample_platform("tray-p2")).await.unwrap();

        // 初始无 tray 平台
        assert!(get_tray_platform(&db).await.unwrap().is_none());

        // 设置 p1
        set_tray_platform(&db, p1.id, "balance").await.unwrap();
        let got = get_tray_platform(&db).await.unwrap().expect("should have tray platform");
        assert_eq!(got.id, p1.id);
        assert!(got.show_in_tray);
        assert_eq!(got.tray_display, "balance");

        // 切换到 p2（互斥：p1 应被清除）
        set_tray_platform(&db, p2.id, "coding").await.unwrap();
        let got2 = get_tray_platform(&db).await.unwrap().expect("p2 should be tray");
        assert_eq!(got2.id, p2.id);
        assert_eq!(got2.tray_display, "coding");

        // 验证 p1 已无 show_in_tray
        let p1_row = get_platform(&db, p1.id).await.unwrap().expect("p1 still exists");
        assert!(!p1_row.show_in_tray);

        // clear_tray
        clear_tray(&db).await.unwrap();
        assert!(get_tray_platform(&db).await.unwrap().is_none());
    }

    /// R2：一键清理只删 401/403 的 auto_disabled；402/429-配额（可充值恢复）保留。
    #[tokio::test]
    async fn purge_keeps_recoverable_auto_disabled() {
        let db = test_db().await;
        let p_401 = create_platform(&db, sample_platform("p401")).await.unwrap();
        let p_402 = create_platform(&db, sample_platform("p402")).await.unwrap();
        let p_429q = create_platform(&db, sample_platform("p429q")).await.unwrap();
        set_platform_auto_disabled(&db, p_401.id).await.unwrap();
        set_platform_auto_disabled(&db, p_402.id).await.unwrap();
        set_platform_auto_disabled(&db, p_429q.id).await.unwrap();
        set_platform_last_error(&db, p_401.id, Some("HTTP 401: unauthorized".into())).await.unwrap();
        set_platform_last_error(&db, p_402.id, Some("HTTP 402: 余额不足".into())).await.unwrap();
        set_platform_last_error(&db, p_429q.id, Some("HTTP 429: 已达到 Token Plan 用量上限".into())).await.unwrap();

        let r = purge_auto_disabled_platforms(&db, None).await.unwrap();
        assert_eq!(r.deleted_ids, vec![p_401.id as u64], "仅 401 被删");
        assert!(get_platform(&db, p_402.id).await.unwrap().is_some(), "402 可恢复平台应保留");
        assert!(get_platform(&db, p_429q.id).await.unwrap().is_some(), "429-配额可恢复平台应保留");
    }
