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



    /// 缓存正确性（问题2）：setting 写后读返回新值（失效生效）；
    /// group 写后 list_groups 返回新集合（不返回陈旧缓存）。
    #[tokio::test]
    async fn hot_cache_invalidates_on_write() {
        let db = test_db().await;
        // ── setting 缓存 ──
        // 先读（不存在 → 缓存 None 槽），再写，再读必须见新值。
        assert!(get_setting(&db, "proxy", "logging").await.unwrap().is_none());
        set_setting(&db, SetSettingInput {
            scope: "proxy".into(),
            key: "logging".into(),
            value: serde_json::json!({"enabled": true}),
        }).await.unwrap();
        let v = get_setting(&db, "proxy", "logging").await.unwrap();
        assert_eq!(v, Some(serde_json::json!({"enabled": true})), "写后读见新值（缓存已失效）");
        // 改值再读。
        set_setting(&db, SetSettingInput {
            scope: "proxy".into(),
            key: "logging".into(),
            value: serde_json::json!({"enabled": false}),
        }).await.unwrap();
        assert_eq!(
            get_setting(&db, "proxy", "logging").await.unwrap(),
            Some(serde_json::json!({"enabled": false})),
        );
        // delete 后读为 None。
        delete_setting(&db, "proxy", "logging").await.unwrap();
        assert!(get_setting(&db, "proxy", "logging").await.unwrap().is_none());

        // ── group 缓存 ──
        assert_eq!(list_groups(&db).await.unwrap().len(), 0);
        let g = create_group(&db, sample_group("gc", vec![])).await.unwrap();
        // 缓存失效 → list_groups 见新建 group。
        let groups = list_groups(&db).await.unwrap();
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].name, "gc");
        // 删除后 list_groups 不再含该 group。
        force_delete_group(&db, g.id).await.unwrap();
        assert_eq!(list_groups(&db).await.unwrap().len(), 0);
    }
