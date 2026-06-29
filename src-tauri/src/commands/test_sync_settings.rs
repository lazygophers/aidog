//! merge_json deep-merge 单元测试（随源文件 sync_settings.rs 1:1）。
use super::{merge_json, MARKER_MANAGED};
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

    /// collect_leaf_paths：嵌套 object 递归到叶子 dot-path，跳过 `_aidog_` 内部 marker。
    #[test]
    fn collect_leaf_paths_nested_and_skips_aidog() {
        let v = json!({
            "env": { "ANTHROPIC_BASE_URL": "x", "ANTHROPIC_AUTH_TOKEN": "y" },
            "statusLine": { "type": "command", "command": "z" },
            "enabledPlugins": { "a@m": true },
            "language": "zh-CN",
            "_aidog_statusline": { "enabled": true }
        });
        let mut out = Vec::new();
        super::collect_leaf_paths(&v, "", &mut out);
        assert!(out.contains(&"env.ANTHROPIC_BASE_URL".to_string()));
        assert!(out.contains(&"env.ANTHROPIC_AUTH_TOKEN".to_string()));
        assert!(out.contains(&"statusLine.type".to_string()));
        assert!(out.contains(&"statusLine.command".to_string()));
        assert!(out.contains(&"enabledPlugins.a@m".to_string()));
        assert!(out.contains(&"language".to_string()));
        // 内部 marker 不入托管集
        assert!(!out.iter().any(|p| p.starts_with("_aidog_")));
    }

    /// collect_leaf_paths 叶子粒度契约（与前端比对一致，防泄漏）：
    /// - 数组 = 单叶子（不展开索引）→ `hooks.Stop` 整体一个 path（前端 1 层展开后
    ///   `managed.has("hooks.Stop")` 直接命中）。
    /// - 深层 object 递归到标量叶子 → `extraKnownMarketplaces.x.source.repo`（前端把
    ///   `extraKnownMarketplaces.x` 当 1 层子节点，须靠 `isFullyManaged` 子树全叶子 ∈
    ///   managed 命中排除）。
    #[test]
    fn collect_leaf_paths_arrays_are_single_leaf_objects_recurse() {
        let v = json!({
            "hooks": {
                "Stop": [ { "hooks": [ { "type": "command", "command": "aidog-notify.py" } ] } ]
            },
            "extraKnownMarketplaces": {
                "ccplugin-market": { "source": { "repo": "x/y", "source": "github" }, "skipLfs": true }
            }
        });
        let mut out = Vec::new();
        super::collect_leaf_paths(&v, "", &mut out);
        // 数组整体一个叶子，不展开索引
        assert!(out.contains(&"hooks.Stop".to_string()));
        assert!(!out.iter().any(|p| p.starts_with("hooks.Stop.")));
        // 深层 object 递归到标量
        assert!(out.contains(&"extraKnownMarketplaces.ccplugin-market.source.repo".to_string()));
        assert!(out.contains(&"extraKnownMarketplaces.ccplugin-market.source.source".to_string()));
        assert!(out.contains(&"extraKnownMarketplaces.ccplugin-market.skipLfs".to_string()));
    }

    /// write_default_claude_settings：写入 `_aidog_managed` marker，含注入字段叶子 path，
    /// 且不含用户自加的 enabledPlugins 条目（保留用户条目、仅 aidog 自身条目入托管集）。
    #[tokio::test]
    async fn write_default_claude_settings_records_managed_paths() {
        use crate::gateway::db::test_support::HomeGuard;
        let h = HomeGuard::new();
        let claude_dir = h.home().join(".claude");
        std::fs::create_dir_all(&claude_dir).unwrap();
        let path = claude_dir.join("settings.json");
        // 用户预置：自装一个插件
        std::fs::write(
            &path,
            r#"{"enabledPlugins":{"user-plugin@user-market":true}}"#,
        )
        .unwrap();

        let config = json!({
            "env": { "ANTHROPIC_BASE_URL": "http://127.0.0.1:9000/proxy", "ANTHROPIC_AUTH_TOKEN": "gk" },
            "enabledPlugins": { "aidog-plugin@official": true }
        });
        super::write_default_claude_settings(&config).unwrap();

        let written: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();

        // 用户自装条目保留（union merge）
        assert_eq!(
            written["enabledPlugins"]["user-plugin@user-market"],
            true
        );
        assert_eq!(written["enabledPlugins"]["aidog-plugin@official"], true);

        // 托管 marker：含 aidog 注入条目，不含用户自加条目
        let managed: Vec<String> = written[MARKER_MANAGED]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().to_string())
            .collect();
        assert!(managed.contains(&"env.ANTHROPIC_BASE_URL".to_string()));
        assert!(managed.contains(&"env.ANTHROPIC_AUTH_TOKEN".to_string()));
        assert!(managed.contains(&"enabledPlugins.aidog-plugin@official".to_string()));
        // 用户自加条目不进托管集 → 导入 diff 能列出
        assert!(!managed.contains(&"enabledPlugins.user-plugin@user-market".to_string()));
    }
