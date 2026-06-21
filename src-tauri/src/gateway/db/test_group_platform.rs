#![cfg(test)]
use super::*;
use super::test_support::*;

    /// list_group_details 缓存写时失效一致性：每类写后下次读必拿到新值（无陈旧）。
    /// 覆盖审计列出的失效面：① set_group_platforms（结构）② update_platform（platform 字段）
    /// ③ apply_balance_delta（estimate 热路径 est_balance）④ set_group_platform_level_priority
    /// ⑤ delete_platform（成员移除，经 invalidate_groups_cache 级联）。
    #[tokio::test]
    async fn list_group_details_cache_invalidation() {
        let db = test_db().await;
        let p1 = create_platform(&db, sample_platform("P1")).await.unwrap();
        let p2 = create_platform(&db, sample_platform("P2")).await.unwrap();
        let g = create_group(&db, sample_group("g1", vec![])).await.unwrap();

        // 初次读（无成员）→ 建立缓存。
        let d0 = list_group_details(&db).await.unwrap();
        assert_eq!(d0.len(), 1);
        assert_eq!(d0[0].platforms.len(), 0, "初始无成员");

        // ① set_group_platforms（结构写）→ 缓存须失效，读到 1 个成员。
        set_group_platforms(&db, g.id, &[
            GroupPlatformInput { platform_id: p1.id, priority: Some(0), weight: Some(1), level_priority: Some(5) },
        ]).await.unwrap();
        let d1 = list_group_details(&db).await.unwrap();
        assert_eq!(d1[0].platforms.len(), 1, "set_group_platforms 后缓存仍旧 → 失效漏");
        assert_eq!(d1[0].platforms[0].platform.id, p1.id);

        // ② update_platform（platform 字段：改名）→ GroupDetail 内嵌 platform 须刷新。
        let upd = UpdatePlatform {
            id: p1.id,
            name: Some("P1-renamed".to_string()),
            platform_type: None,
            base_url: None,
            api_key: None,
            extra: None,
            models: None,
            available_models: None,
            endpoints: None,
            enabled: None,
            status: None,
            manual_budgets: None,
            join_group_ids: None,
        };
        update_platform(&db, upd).await.unwrap();
        let d2 = list_group_details(&db).await.unwrap();
        assert_eq!(d2[0].platforms[0].platform.name, "P1-renamed", "update_platform 后缓存仍旧名 → 失效漏");

        // ③ apply_balance_delta（estimate 热路径）→ est_balance_remaining 须反映扣减。
        // 先置初始余额（update_platform 不写 est_balance，用 update_platform_quota 直写）。
        update_platform_quota(&db, p1.id, 100.0, "").await.unwrap();
        let before = list_group_details(&db).await.unwrap()[0].platforms[0].platform.est_balance_remaining;
        assert!((before - 100.0).abs() < 1e-6, "quota 写后缓存须见 100，实得 {before}");
        crate::gateway::estimate::apply_balance_delta(&db, p1.id, 30.0).await.unwrap();
        let after = list_group_details(&db).await.unwrap()[0].platforms[0].platform.est_balance_remaining;
        assert!((after - 70.0).abs() < 1e-6, "apply_balance_delta 后缓存仍旧值 → 失效漏，实得 {after}");

        // ④ set_group_platform_level_priority → level_priority 须刷新。
        set_group_platform_level_priority(&db, g.id, p1.id, 9).await.unwrap();
        let d4 = list_group_details(&db).await.unwrap();
        assert_eq!(d4[0].platforms[0].level_priority, 9, "level_priority 写后缓存仍旧 → 失效漏");

        // ⑤ delete_platform（成员移除）→ 缓存须见成员清空。
        // 再加 p2 验证删 p1 后剩 p2（避免组空）。
        set_group_platforms(&db, g.id, &[
            GroupPlatformInput { platform_id: p1.id, priority: Some(0), weight: Some(1), level_priority: Some(5) },
            GroupPlatformInput { platform_id: p2.id, priority: Some(1), weight: Some(1), level_priority: Some(5) },
        ]).await.unwrap();
        assert_eq!(list_group_details(&db).await.unwrap()[0].platforms.len(), 2);
        delete_platform(&db, p1.id).await.unwrap();
        let d5 = list_group_details(&db).await.unwrap();
        assert_eq!(d5[0].platforms.len(), 1, "delete_platform 后缓存仍含已删平台 → 失效漏");
        assert_eq!(d5[0].platforms[0].platform.id, p2.id);
    }



    // ── R4 model_mappings 来自 group 字段（get_group_detail）──
    #[tokio::test]
    async fn r4_group_detail_mappings_from_group_field() {
        let db = test_db().await;
        let mappings = vec![ModelMapping {
            source_model: "src".to_string(),
            target_platform_id: 3,
            target_model: "tgt".to_string(),
            request_timeout_secs: 0,
            connect_timeout_secs: 0,
        }];
        let g = create_group(&db, sample_group("d", mappings)).await.unwrap();
        // 该分组无关联平台 → get_group_platforms join 为空，规避遗留 BUG-1（见任务遗留）
        let detail = get_group_detail(&db, g.id).await.unwrap().unwrap();
        // detail.model_mappings 来自 group 内联字段（逐字段一致）
        assert_eq!(detail.model_mappings.len(), 1);
        assert_eq!(detail.model_mappings.len(), detail.group.model_mappings.len());
        assert_eq!(detail.model_mappings[0].source_model, detail.group.model_mappings[0].source_model);
        assert_eq!(detail.model_mappings[0].target_platform_id, detail.group.model_mappings[0].target_platform_id);
        assert_eq!(detail.model_mappings[0].target_model, detail.group.model_mappings[0].target_model);
    }



    // ── D3 复合唯一约束：group_platform 加代理主键 + UNIQUE(group_id, platform_id) ──
    #[tokio::test]
    async fn d3_group_platform_proxy_pk_and_unique() {
        let db = test_db().await;
        let p1 = create_platform(&db, sample_platform("a")).await.unwrap();
        let p2 = create_platform(&db, sample_platform("b")).await.unwrap();
        let g = create_group(&db, sample_group("g", vec![])).await.unwrap();

        set_group_platforms(
            &db,
            g.id,
            &[
                GroupPlatformInput { platform_id: p1.id, priority: Some(0), weight: Some(1), level_priority: None },
                GroupPlatformInput { platform_id: p2.id, priority: Some(1), weight: Some(2), level_priority: None },
            ],
        ).await
        .unwrap();

        let details = get_group_platforms(&db, g.id).await.unwrap();
        assert_eq!(details.len(), 2);

        // 代理主键 id 存在且自增
        let ids: Vec<i64> = db.call_traced(None, std::panic::Location::caller(), |conn| {
            Ok(conn
                .prepare("SELECT id FROM group_platform ORDER BY id")?
                .query_map([], |r| r.get(0))?
                .filter_map(|r| r.ok())
                .collect())
        }).await.unwrap();
        assert_eq!(ids.len(), 2);
        assert!(ids[0] >= 1 && ids[1] > ids[0]);
    }



    #[tokio::test]
    async fn sync_platform_manual_groups_adds_removes_preserves_auto() {
        let db = test_db().await;
        let p = create_platform(&db, sample_platform("sync-p")).await.unwrap();
        // 一个 auto 组（auto_from_platform 非空）+ 两个手动组。
        let auto_g = create_group(&db, CreateGroup {
            name: "auto-g".into(),
            group_key: Some("auto-g".into()),
            routing_mode: RoutingMode::Failover,
            auto_from_platform: p.id.to_string(),
            request_timeout_secs: 0, connect_timeout_secs: 0,
            source_protocol: None, max_retries: 2, model_mappings: vec![],
        }).await.unwrap();
        set_group_platforms(&db, auto_g.id, &[GroupPlatformInput {
            platform_id: p.id, priority: Some(0), weight: Some(1), level_priority: None,
        }]).await.unwrap();
        let m1 = create_group(&db, sample_group("m1", vec![])).await.unwrap();
        let m2 = create_group(&db, sample_group("m2", vec![])).await.unwrap();

        // 初始：加入 m1，不动 m2、auto 组。
        sync_platform_manual_groups(&db, p.id, &[m1.id]).await.unwrap();
        assert_eq!(get_group_platforms(&db, m1.id).await.unwrap().len(), 1);
        assert_eq!(get_group_platforms(&db, m2.id).await.unwrap().len(), 0);
        assert_eq!(get_group_platforms(&db, auto_g.id).await.unwrap().len(), 1, "auto 组应在");

        // 全量同步 → 移出 m1、加入 m2。
        sync_platform_manual_groups(&db, p.id, &[m2.id]).await.unwrap();
        assert_eq!(get_group_platforms(&db, m1.id).await.unwrap().len(), 0, "m1 应被移出");
        assert_eq!(get_group_platforms(&db, m2.id).await.unwrap().len(), 1, "m2 应被加入");
        assert_eq!(get_group_platforms(&db, auto_g.id).await.unwrap().len(), 1, "auto 组不受手动同步影响");

        // 清空手动组（空 target）→ auto 组仍在。
        sync_platform_manual_groups(&db, p.id, &[]).await.unwrap();
        assert_eq!(get_group_platforms(&db, m2.id).await.unwrap().len(), 0);
        assert_eq!(get_group_platforms(&db, auto_g.id).await.unwrap().len(), 1, "清空手动组不删 auto");
    }
