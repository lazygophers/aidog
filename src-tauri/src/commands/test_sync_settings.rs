//! merge_json deep-merge 单元测试（随源文件 sync_settings.rs 1:1）。
use super::merge_json;
use serde_json::json;

    #[test]
    fn merge_json_deep_merges_and_preserves_user_keys() {
        // 用户已有全局配置（含 aidog 不管的 permissions / 自定义 statusLine）
        let mut base = json!({
            "permissions": { "allow": ["Read(*)"] },
            "env": { "MY_OTHER_VAR": "keep" },
            "model": "claude-opus",
            "statusLine": { "type": "command", "command": "user-script" }
        });
        // aidog 注入（默认组的 config）
        let overlay = json!({
            "env": {
                "ANTHROPIC_BASE_URL": "http://127.0.0.1:9999/proxy",
                "ANTHROPIC_AUTH_TOKEN": "gk_abc"
            },
            "statusLine": { "type": "command", "command": "aidog-script" }
        });
        merge_json(&mut base, &overlay);

        // aidog 字段覆盖
        assert_eq!(base["env"]["ANTHROPIC_BASE_URL"], "http://127.0.0.1:9999/proxy");
        assert_eq!(base["env"]["ANTHROPIC_AUTH_TOKEN"], "gk_abc");
        assert_eq!(base["statusLine"]["command"], "aidog-script");
        // 用户其它字段保留
        assert_eq!(base["permissions"]["allow"][0], "Read(*)");
        assert_eq!(base["env"]["MY_OTHER_VAR"], "keep");
        assert_eq!(base["model"], "claude-opus");
    }

    /// merge_json 显式 null 删除 base 同键（用于取消默认时清理 aidog 字段）。
    #[test]
    fn merge_json_null_deletes_key() {
        let mut base = json!({ "env": { "AIDOG_KEY": "x", "keep": "y" } });
        let overlay = json!({ "env": { "AIDOG_KEY": null } });
        merge_json(&mut base, &overlay);
        assert!(base["env"].get("AIDOG_KEY").is_none());
        assert_eq!(base["env"]["keep"], "y");
    }

    /// overlay 标量直接覆盖 base object。
    #[test]
    fn merge_json_scalar_overwrites_object() {
        let mut base = json!({ "a": { "nested": 1 } });
        merge_json(&mut base, &json!({ "a": "scalar" }));
        assert_eq!(base["a"], "scalar");
    }

    /// base 非 object 时被升级为 object 再合并。
    #[test]
    fn merge_json_upgrades_non_object_base() {
        let mut base = json!("string");
        merge_json(&mut base, &json!({ "k": "v" }));
        assert_eq!(base["k"], "v");
    }

    /// write_default_claude_settings：HOME 隔离下首次写 + deep merge 保留用户字段 + 幂等无写。
    #[tokio::test]
    async fn write_default_claude_settings_merges_and_idempotent() {
        use crate::gateway::db::test_support::HomeGuard;
        let h = HomeGuard::new();
        // 预置用户配置
        let claude_dir = h.home().join(".claude");
        std::fs::create_dir_all(&claude_dir).unwrap();
        let path = claude_dir.join("settings.json");
        std::fs::write(&path, r#"{"permissions":{"allow":["Read(*)"]},"model":"opus"}"#).unwrap();

        let config = json!({
            "env": { "ANTHROPIC_BASE_URL": "http://127.0.0.1:9876/proxy", "ANTHROPIC_AUTH_TOKEN": "gk_x" }
        });
        super::write_default_claude_settings(&config).unwrap();

        let written: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(written["env"]["ANTHROPIC_AUTH_TOKEN"], "gk_x");
        assert_eq!(written["permissions"]["allow"][0], "Read(*)"); // 用户字段保留
        assert_eq!(written["model"], "opus");

        // 幂等：再次同 config → 内容不变（命中 old==new 早退）
        let before = std::fs::read_to_string(&path).unwrap();
        super::write_default_claude_settings(&config).unwrap();
        assert_eq!(before, std::fs::read_to_string(&path).unwrap());
    }
