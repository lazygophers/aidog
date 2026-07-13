#![cfg(test)]
use super::*;
use super::test_support::*;
use crate::gateway::db::update_extra_key;

    /// 空 extra → 写 _ui_collapsed=true → 读回应为合法 JSON 含该键。
    #[tokio::test]
    async fn ui_extra_empty_to_json_with_key() {
        let db = test_db().await;
        let p = create_platform(&db, sample_platform("ui-empty")).await.unwrap();
        update_extra_key(&db, "platform", p.id, "_ui_collapsed", serde_json::json!(true))
            .await
            .unwrap();
        let got = get_platform(&db, p.id).await.unwrap().unwrap();
        let v: serde_json::Value = serde_json::from_str(&got.extra).unwrap();
        assert_eq!(v["_ui_collapsed"], serde_json::json!(true));
    }

    /// 已有业务键（breaker / peak_hours）→ 写 _ui_* 不破坏其它键。
    #[tokio::test]
    async fn ui_extra_preserves_existing_business_keys() {
        let db = test_db().await;
        let mut input = sample_platform("ui-mix");
        input.extra = r#"{"breaker":{"failure_threshold":3,"open_secs":30,"half_open_max":1,"recovery_secs":5}}"#.to_string();
        let p = create_platform(&db, input).await.unwrap();
        update_extra_key(&db, "platform", p.id, "_ui_expand_plat", serde_json::json!(false))
            .await
            .unwrap();
        let got = get_platform(&db, p.id).await.unwrap().unwrap();
        let v: serde_json::Value = serde_json::from_str(&got.extra).unwrap();
        assert_eq!(v["_ui_expand_plat"], serde_json::json!(false));
        // 业务键完整保留（peak_hours_for 等解析仍能正常工作）
        assert_eq!(v["breaker"]["failure_threshold"], 3);
        assert_eq!(v["breaker"]["open_secs"], 30);
    }

    /// 覆盖同键值（_ui_* 二次写）：后值替换前值。
    #[tokio::test]
    async fn ui_extra_overwrite_same_key() {
        let db = test_db().await;
        let p = create_platform(&db, sample_platform("ui-ow")).await.unwrap();
        update_extra_key(&db, "platform", p.id, "_ui_collapsed", serde_json::json!(true))
            .await
            .unwrap();
        update_extra_key(&db, "platform", p.id, "_ui_collapsed", serde_json::json!(false))
            .await
            .unwrap();
        let got = get_platform(&db, p.id).await.unwrap().unwrap();
        let v: serde_json::Value = serde_json::from_str(&got.extra).unwrap();
        assert_eq!(v["_ui_collapsed"], serde_json::json!(false));
    }

    /// 表名白名单：非法表 → Err，禁注入。
    #[tokio::test]
    async fn ui_extra_rejects_unknown_table() {
        let db = test_db().await;
        let err = update_extra_key(&db, "group; DROP TABLE--", 1, "_ui_x", serde_json::json!(true))
            .await
            .unwrap_err();
        assert!(err.contains("unsupported table"), "got: {err}");
    }

    /// 当前 group 表无 extra 列：白名单拒绝（防 SQL 错误）。
    #[tokio::test]
    async fn ui_extra_group_target_currently_unsupported() {
        let db = test_db().await;
        let err = update_extra_key(&db, "group", 1, "_ui_x", serde_json::json!(true))
            .await
            .unwrap_err();
        assert!(err.contains("unsupported table 'group'"), "got: {err}");
    }
