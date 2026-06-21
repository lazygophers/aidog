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
