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
            max_retries: Some(0), model_mappings: vec![], is_default: None,
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
