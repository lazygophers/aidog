#![cfg(test)]
use super::*;
use super::test_support::*;


    // ── R4 / D4 model_mappings 内联 JSON 往返 ──
    #[tokio::test]
    async fn r4_group_model_mappings_inline_roundtrip() {
        let db = test_db().await;
        let mappings = vec![
            ModelMapping {
                source_model: "claude-sonnet-4".to_string(),
                target_platform_id: 42,
                target_model: "glm-4-plus".to_string(),
                request_timeout_secs: 30,
                connect_timeout_secs: 5,
            },
            ModelMapping {
                source_model: "claude-haiku".to_string(),
                target_platform_id: 7,
                target_model: "glm-4-air".to_string(),
                request_timeout_secs: 0,
                connect_timeout_secs: 0,
            },
        ];
        let g = create_group(&db, sample_group("mm", mappings)).await.unwrap();

        let fetched = get_group(&db, g.id).await.unwrap().unwrap();
        assert_eq!(fetched.model_mappings.len(), 2);
        assert_eq!(fetched.model_mappings[0].source_model, "claude-sonnet-4");
        // target_platform_id 为 u64
        let tpid: u64 = fetched.model_mappings[0].target_platform_id;
        assert_eq!(tpid, 42);
        assert_eq!(fetched.model_mappings[0].target_model, "glm-4-plus");
        assert_eq!(fetched.model_mappings[0].request_timeout_secs, 30);
        assert_eq!(fetched.model_mappings[1].target_platform_id, 7);
    }



    /// group max_retries 持久化往返
    #[tokio::test]
    async fn group_max_retries_roundtrip() {
        let db = test_db().await;
        let mut input = sample_group("mr", vec![]);
        input.max_retries = 5;
        let g = create_group(&db, input).await.unwrap();
        assert_eq!(g.max_retries, 5);
        let fetched = get_group(&db, g.id).await.unwrap().unwrap();
        assert_eq!(fetched.max_retries, 5);

        let upd = update_group(&db, UpdateGroup {
            id: g.id, name: None, routing_mode: None,
            request_timeout_secs: 0, connect_timeout_secs: 0, source_protocol: None,
            max_retries: Some(0), model_mappings: vec![], env_vars: vec![], is_default: None,
        }).await.unwrap();
        assert_eq!(upd.max_retries, 0);
        assert_eq!(get_group(&db, g.id).await.unwrap().unwrap().max_retries, 0);
    }



    /// group is_default 单选：set_default_group 同时清零其它 + 置目标；None 清零全部。
    #[tokio::test]
    async fn group_set_default_single_select() {
        let db = test_db().await;
        let g1 = create_group(&db, sample_group("d1", vec![])).await.unwrap();
        let g2 = create_group(&db, sample_group("d2", vec![])).await.unwrap();
        assert!(!get_group(&db, g1.id).await.unwrap().unwrap().is_default);
        assert!(!get_group(&db, g2.id).await.unwrap().unwrap().is_default);

        set_default_group(&db, Some(g1.id)).await.unwrap();
        assert!(get_group(&db, g1.id).await.unwrap().unwrap().is_default);
        assert!(!get_group(&db, g2.id).await.unwrap().unwrap().is_default);

        // 切换默认到 g2 → g1 自动清零（单选）
        set_default_group(&db, Some(g2.id)).await.unwrap();
        assert!(!get_group(&db, g1.id).await.unwrap().unwrap().is_default);
        assert!(get_group(&db, g2.id).await.unwrap().unwrap().is_default);

        // 取消默认 → 全部清零
        set_default_group(&db, None).await.unwrap();
        assert!(!get_group(&db, g1.id).await.unwrap().unwrap().is_default);
        assert!(!get_group(&db, g2.id).await.unwrap().unwrap().is_default);
    }

    // ── list_groups / create / update / delete ──
    #[tokio::test]
    async fn list_groups_empty_on_fresh_db() {
        let db = test_db().await;
        let groups = list_groups(&db).await.unwrap();
        assert!(groups.is_empty());
    }

    #[tokio::test]
    async fn create_and_list_groups() {
        let db = test_db().await;
        let g1 = create_group(&db, sample_group("group-a", vec![])).await.unwrap();
        let g2 = create_group(&db, sample_group("group-b", vec![])).await.unwrap();
        let groups = list_groups(&db).await.unwrap();
        assert_eq!(groups.len(), 2);
        let names: Vec<&str> = groups.iter().map(|g| g.name.as_str()).collect();
        assert!(names.contains(&"group-a"));
        assert!(names.contains(&"group-b"));
        assert_ne!(g1.id, g2.id);
    }

    #[tokio::test]
    async fn get_group_returns_none_for_missing() {
        let db = test_db().await;
        let result = get_group(&db, 99999).await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn update_group_changes_name() {
        let db = test_db().await;
        let g = create_group(&db, sample_group("original", vec![])).await.unwrap();
        let updated = update_group(&db, UpdateGroup {
            id: g.id,
            name: Some("renamed".to_string()),
            routing_mode: None,
            request_timeout_secs: 0,
            connect_timeout_secs: 0,
            source_protocol: None,
            max_retries: None,
            model_mappings: vec![], env_vars: vec![],            is_default: None,
        }).await.unwrap();
        assert_eq!(updated.name, "renamed");
        let fetched = get_group(&db, g.id).await.unwrap().unwrap();
        assert_eq!(fetched.name, "renamed");
    }

    #[tokio::test]
    async fn update_group_changes_routing_mode() {
        let db = test_db().await;
        let g = create_group(&db, sample_group("routing-test", vec![])).await.unwrap();
        let updated = update_group(&db, UpdateGroup {
            id: g.id,
            name: None,
            routing_mode: Some(RoutingMode::LoadBalance),
            request_timeout_secs: 0,
            connect_timeout_secs: 0,
            source_protocol: None,
            max_retries: None,
            model_mappings: vec![], env_vars: vec![],            is_default: None,
        }).await.unwrap();
        assert!(matches!(updated.routing_mode, RoutingMode::LoadBalance));
    }

    #[tokio::test]
    async fn update_group_timeouts() {
        let db = test_db().await;
        let g = create_group(&db, sample_group("timeout-test", vec![])).await.unwrap();
        let updated = update_group(&db, UpdateGroup {
            id: g.id,
            name: None,
            routing_mode: None,
            request_timeout_secs: 120,
            connect_timeout_secs: 15,
            source_protocol: None,
            max_retries: None,
            model_mappings: vec![], env_vars: vec![],            is_default: None,
        }).await.unwrap();
        assert_eq!(updated.request_timeout_secs, 120);
        assert_eq!(updated.connect_timeout_secs, 15);
    }

    #[tokio::test]
    async fn delete_group_non_auto() {
        let db = test_db().await;
        let g = create_group(&db, sample_group("to-delete", vec![])).await.unwrap();
        assert!(get_group(&db, g.id).await.unwrap().is_some());
        delete_group(&db, g.id).await.unwrap();
        assert!(get_group(&db, g.id).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn delete_group_missing_returns_error() {
        let db = test_db().await;
        let result = delete_group(&db, 99999).await;
        assert!(result.is_err());
    }

    // ── reorder_groups ──
    #[tokio::test]
    async fn reorder_groups_updates_sort_order() {
        let db = test_db().await;
        let g1 = create_group(&db, sample_group("r1", vec![])).await.unwrap();
        let g2 = create_group(&db, sample_group("r2", vec![])).await.unwrap();
        let g3 = create_group(&db, sample_group("r3", vec![])).await.unwrap();
        // Reverse order: g3, g1, g2
        reorder_groups(&db, &[g3.id, g1.id, g2.id]).await.unwrap();
        let groups = list_groups(&db).await.unwrap();
        // After reorder, sort_order should be set. Check that g3 comes first.
        assert_eq!(groups[0].id, g3.id);
        assert_eq!(groups[1].id, g1.id);
        assert_eq!(groups[2].id, g2.id);
    }

    #[tokio::test]
    async fn reorder_groups_empty_list_noop() {
        let db = test_db().await;
        reorder_groups(&db, &[]).await.unwrap();
    }

    // ── reorder_platforms ──
    #[tokio::test]
    async fn reorder_platforms_works() {
        let db = test_db().await;
        let p1 = create_platform(&db, sample_platform("plat-a")).await.unwrap();
        let p2 = create_platform(&db, sample_platform("plat-b")).await.unwrap();
        reorder_platforms(&db, &[p2.id, p1.id]).await.unwrap();
        // No panic = success (sort_order updated in DB)
    }

    // ── reorder_group_platforms ──
    #[tokio::test]
    async fn reorder_group_platforms_works() {
        let db = test_db().await;
        let g = create_group(&db, sample_group("gp-reorder", vec![])).await.unwrap();
        let p1 = create_platform(&db, sample_platform("p-reorder-1")).await.unwrap();
        let p2 = create_platform(&db, sample_platform("p-reorder-2")).await.unwrap();
        set_group_platforms(&db, g.id, &[
            GroupPlatformInput { platform_id: p1.id, priority: Some(1), weight: Some(1), level_priority: None },
            GroupPlatformInput { platform_id: p2.id, priority: Some(2), weight: Some(1), level_priority: None },
        ]).await.unwrap();
        reorder_group_platforms(&db, g.id, &[p2.id, p1.id]).await.unwrap();
        // Verify group platforms exist in DB
        let gps = get_group_platforms(&db, g.id).await.unwrap();
        assert_eq!(gps.len(), 2);
    }

    // ── set_group_platform_level_priority ──
    #[tokio::test]
    async fn set_group_platform_level_priority_updates() {
        let db = test_db().await;
        let g = create_group(&db, sample_group("lp-group", vec![])).await.unwrap();
        let p = create_platform(&db, sample_platform("lp-platform")).await.unwrap();
        set_group_platforms(&db, g.id, &[
            GroupPlatformInput { platform_id: p.id, priority: Some(1), weight: Some(1), level_priority: None },
        ]).await.unwrap();
        set_group_platform_level_priority(&db, g.id, p.id, 7).await.unwrap();
        // No error = success
    }

    #[tokio::test]
    async fn set_group_platform_level_priority_clamped() {
        let db = test_db().await;
        let g = create_group(&db, sample_group("lp-clamp", vec![])).await.unwrap();
        let p = create_platform(&db, sample_platform("lp-clamp-p")).await.unwrap();
        set_group_platforms(&db, g.id, &[
            GroupPlatformInput { platform_id: p.id, priority: Some(1), weight: Some(1), level_priority: None },
        ]).await.unwrap();
        // Out-of-range values should be clamped
        set_group_platform_level_priority(&db, g.id, p.id, 100).await.unwrap();
        set_group_platform_level_priority(&db, g.id, p.id, -5).await.unwrap();
    }

    // ── move_group_platform ──
    #[tokio::test]
    async fn move_group_platform_between_groups() {
        let db = test_db().await;
        let g1 = create_group(&db, sample_group("from-group", vec![])).await.unwrap();
        let g2 = create_group(&db, sample_group("to-group", vec![])).await.unwrap();
        let p = create_platform(&db, sample_platform("movable-plat")).await.unwrap();
        set_group_platforms(&db, g1.id, &[
            GroupPlatformInput { platform_id: p.id, priority: Some(1), weight: Some(1), level_priority: None },
        ]).await.unwrap();

        move_group_platform(&db, p.id, g1.id, g2.id).await.unwrap();

        let g1_plats = get_group_platforms(&db, g1.id).await.unwrap();
        let g2_plats = get_group_platforms(&db, g2.id).await.unwrap();
        assert!(g1_plats.is_empty(), "platform should be removed from source group");
        assert_eq!(g2_plats.len(), 1, "platform should be added to target group");
    }

    // ── create_group with auto group_key ──
    #[tokio::test]
    async fn create_group_auto_generates_group_key() {
        let db = test_db().await;
        let mut input = sample_group("auto-key", vec![]);
        input.group_key = None; // Let it auto-generate
        let g = create_group(&db, input).await.unwrap();
        assert!(g.group_key.starts_with("gk_"), "auto-generated key should start with gk_: {}", g.group_key);
        assert!(g.group_key.len() > 3);
    }

    #[tokio::test]
    async fn create_group_with_source_protocol() {
        let db = test_db().await;
        let mut input = sample_group("with-proto", vec![]);
        input.source_protocol = Some("openai".to_string());
        let g = create_group(&db, input).await.unwrap();
        assert_eq!(g.source_protocol, "openai");
    }
