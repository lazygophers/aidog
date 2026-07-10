//! coding_tools 设置单元测试（随源文件 coding_tools.rs 1:1）。
use serde_json::json;

    /// CodingToolsSettings 默认 = 两开关 false（UI 显示关，功能与开关解耦）。
    #[test]
    fn coding_tools_settings_default_is_off() {
        let s = super::CodingToolsSettings::default();
        assert!(!s.apply_to_claude_plugin, "default apply_to_claude_plugin must be false");
        assert!(!s.skip_claude_onboarding, "default skip_claude_onboarding must be false");
    }

    /// CodingToolsSettings 反序列化时缺失字段也走默认 false。
    #[test]
    fn coding_tools_settings_deserialize_missing_fields_defaults_false() {
        let s: super::CodingToolsSettings = serde_json::from_value(json!({})).unwrap();
        assert!(!s.apply_to_claude_plugin);
        assert!(!s.skip_claude_onboarding);
    }

    /// ensure_default_coding_tools_settings：DB 无记录 → 写文件但**不落 DB 记录**。
    ///
    /// 语义：功能（文件写入）与开关（DB 记录）解耦。无记录代表用户未操作，
    /// 默认行为写文件让功能生效，但 UI get 返 false（开关显示关）。
    /// HOME 隔离（tempdir）下，文件写入落到临时目录，单测只验 DB 不落记录这一不变量，
    /// 杜绝污染真实 ~/.claude。
    #[tokio::test]
    async fn ensure_default_coding_tools_settings_no_record_no_db_write() {
        use aidog_core::gateway::db::test_support::HomeGuard;
        let _h = HomeGuard::new();
        let db = crate::Db::new(":memory:").await.expect("open memory db");
        db.init_tables().await.expect("init tables");

        // 首次：无记录
        super::ensure_default_coding_tools_settings(&db).await.expect("ensure first run");

        // 关键断言：ensure 不落 DB 记录（功能与开关解耦）
        let rec = aidog_core::gateway::db::get_setting(&db, "global", "coding_tools_settings")
            .await
            .expect("query setting");
        assert!(rec.is_none(), "ensure must NOT create DB record when none existed");
    }

    /// ensure_default_coding_tools_settings：DB 有记录 → 不改写。
    ///
    /// 用户 toggle 过（任意值）→ ensure 完全尊重 DB，不强制默认写。
    /// 这里验证 ensure 看到记录后 DB 值原样保留。
    #[tokio::test]
    async fn ensure_default_coding_tools_settings_respects_existing_record() {
        let db = crate::Db::new(":memory:").await.expect("open memory db");
        db.init_tables().await.expect("init tables");

        // 预置：用户把两开关都开了（DB true,true）
        let user_value = serde_json::json!({
            "apply_to_claude_plugin": true,
            "skip_claude_onboarding": true,
        });
        aidog_core::gateway::db::set_setting(&db, crate::SetSettingInput {
            scope: "global".to_string(),
            key: "coding_tools_settings".to_string(),
            value: user_value,
        }).await.expect("seed record");

        // 调 ensure：应尊重已有记录
        super::ensure_default_coding_tools_settings(&db).await.expect("ensure with record");

        let rec = aidog_core::gateway::db::get_setting(&db, "global", "coding_tools_settings")
            .await
            .expect("query setting")
            .expect("record must still exist");
        let s: super::CodingToolsSettings = serde_json::from_value(rec).unwrap();
        assert!(s.apply_to_claude_plugin, "user's true must be preserved");
        assert!(s.skip_claude_onboarding, "user's true must be preserved");
    }

    /// load_coding_tools_settings: DB 无记录时返回默认值。
    #[tokio::test]
    async fn load_coding_tools_settings_no_record_returns_default() {
        let db = crate::Db::new(":memory:").await.expect("open memory db");
        db.init_tables().await.expect("init tables");
        let s = super::load_coding_tools_settings(&db).await;
        assert!(!s.apply_to_claude_plugin);
        assert!(!s.skip_claude_onboarding);
    }

    /// load_coding_tools_settings: DB 有记录时返回正确值。
    #[tokio::test]
    async fn load_coding_tools_settings_with_record() {
        let db = crate::Db::new(":memory:").await.expect("open memory db");
        db.init_tables().await.expect("init tables");
        aidog_core::gateway::db::set_setting(&db, crate::SetSettingInput {
            scope: "global".to_string(),
            key: "coding_tools_settings".to_string(),
            value: serde_json::json!({
                "apply_to_claude_plugin": true,
                "skip_claude_onboarding": false,
            }),
        }).await.expect("seed record");
        let s = super::load_coding_tools_settings(&db).await;
        assert!(s.apply_to_claude_plugin);
        assert!(!s.skip_claude_onboarding);
    }

    /// default helper fns.
    #[test]
    fn default_helpers() {
        assert!(!super::coding_tools_default_apply());
        assert!(!super::coding_tools_default_skip());
    }
