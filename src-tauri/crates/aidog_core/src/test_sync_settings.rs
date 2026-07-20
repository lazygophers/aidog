//! merge_json deep-merge 单元测试（随源文件 sync_settings.rs 1:1）。
use super::{merge_json, MARKER_MANAGED, MANAGED_KEY, MANAGED_SCOPE};
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

    /// 读 DB `setting` 表里的 managed_paths 快照（test helper：unwrap + 数组化）。
    async fn read_managed_paths(db: &crate::gateway::db::Db) -> Vec<String> {
        let v = crate::gateway::db::get_setting(db, MANAGED_SCOPE, MANAGED_KEY)
            .await
            .unwrap()
            .unwrap_or(json!([]));
        v.as_array()
            .unwrap()
            .iter()
            .map(|x| x.as_str().unwrap().to_string())
            .collect()
    }

    /// write_default_claude_settings：HOME + DB 隔离下首次写 + deep merge 保留用户字段 + 幂等无写。
    #[tokio::test]
    async fn write_default_claude_settings_merges_and_idempotent() {
        use crate::gateway::db::test_support::{HomeGuard, test_db};
        let h = HomeGuard::new();
        let db = test_db().await;
        // 预置用户配置
        let claude_dir = h.home().join(".claude");
        std::fs::create_dir_all(&claude_dir).unwrap();
        let path = claude_dir.join("settings.json");
        std::fs::write(&path, r#"{"permissions":{"allow":["Read(*)"]},"model":"opus"}"#).unwrap();

        let config = json!({
            "env": { "ANTHROPIC_BASE_URL": "http://127.0.0.1:9876/proxy", "ANTHROPIC_AUTH_TOKEN": "gk_x" }
        });
        super::write_default_claude_settings(&db, &config).await.unwrap();

        let written: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(written["env"]["ANTHROPIC_AUTH_TOKEN"], "gk_x");
        assert_eq!(written["permissions"]["allow"][0], "Read(*)"); // 用户字段保留
        assert_eq!(written["model"], "opus");
        // settings.json 不再写 marker（已迁 DB）
        assert!(written.get(MARKER_MANAGED).is_none());

        // 幂等：再次同 config → 内容不变（命中 old==new 早退）
        let before = std::fs::read_to_string(&path).unwrap();
        super::write_default_claude_settings(&db, &config).await.unwrap();
        assert_eq!(before, std::fs::read_to_string(&path).unwrap());
    }

    /// collect_leaf_paths：嵌套 object 递归到叶子 dot-path，跳过 `_aidog_` 内部 marker。
    #[test]
    fn collect_leaf_paths_nested_and_skips_aidog() {
        let v = json!({
            "env": { "ANTHROPIC_BASE_URL": "x", "ANTHROPIC_AUTH_TOKEN": "y" },
            "statusLine": { "type": "command", "command": "z" },
            "enabledPlugins": { "a@m": true },
            "language": "zh-Hans",
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

    /// write_default_claude_settings：托管快照存 DB = merge 后完整 base 的全部叶子。
    /// 既含 aidog 注入字段，**也含 merge 后保留的用户自装条目**（plugins/marketplaces/hooks）。
    /// 语义：导入 diff 排除此快照 → 同步当下零差异（含用户自装项），仅显示同步之后的新增/变化。
    /// settings.json 不写 marker（已迁 DB）。
    #[tokio::test]
    async fn write_default_claude_settings_records_managed_paths() {
        use crate::gateway::db::test_support::{HomeGuard, test_db};
        let h = HomeGuard::new();
        let db = test_db().await;
        let claude_dir = h.home().join(".claude");
        std::fs::create_dir_all(&claude_dir).unwrap();
        let path = claude_dir.join("settings.json");
        // 用户预置：自装一个插件 + 一个 marketplace
        std::fs::write(
            &path,
            r#"{"enabledPlugins":{"user-plugin@user-market":true},"extraKnownMarketplaces":{"user-market":{"source":{"repo":"u/m","source":"github"}}}}"#,
        )
        .unwrap();

        let config = json!({
            "env": { "ANTHROPIC_BASE_URL": "http://127.0.0.1:9000/proxy", "ANTHROPIC_AUTH_TOKEN": "gk" },
            "enabledPlugins": { "aidog-plugin@official": true }
        });
        super::write_default_claude_settings(&db, &config).await.unwrap();

        let written: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();

        // 用户自装条目保留（union merge）
        assert_eq!(
            written["enabledPlugins"]["user-plugin@user-market"],
            true
        );
        assert_eq!(written["enabledPlugins"]["aidog-plugin@official"], true);

        // settings.json 不再写 marker
        assert!(written.get(MARKER_MANAGED).is_none());

        // 托管快照在 DB：= merge 后完整快照，含 aidog 注入条目 + 用户自装条目
        let managed: Vec<String> = read_managed_paths(&db).await;
        assert!(managed.contains(&"env.ANTHROPIC_BASE_URL".to_string()));
        assert!(managed.contains(&"env.ANTHROPIC_AUTH_TOKEN".to_string()));
        assert!(managed.contains(&"enabledPlugins.aidog-plugin@official".to_string()));
        // 新语义：用户自装条目也进托管集 → 导入 diff 当下零差异
        assert!(managed.contains(&"enabledPlugins.user-plugin@user-market".to_string()));
        assert!(managed.contains(&"extraKnownMarketplaces.user-market.source.repo".to_string()));
        assert!(managed.contains(&"extraKnownMarketplaces.user-market.source.source".to_string()));
        // 快照不含 `_aidog_` 前缀（跳过，不自引用）
        assert!(!managed.iter().any(|p| p.starts_with("_aidog_")));
    }

    /// write_default_claude_settings：老用户 settings.json 残留旧 `_aidog_managed` 值 →
    /// 再次 sync 后被显式 remove（连旧值清），marker 数据源已迁 DB。
    #[tokio::test]
    async fn write_default_claude_settings_strips_legacy_marker() {
        use crate::gateway::db::test_support::{HomeGuard, test_db};
        let h = HomeGuard::new();
        let db = test_db().await;
        let claude_dir = h.home().join(".claude");
        std::fs::create_dir_all(&claude_dir).unwrap();
        let path = claude_dir.join("settings.json");
        // 老用户文件：含旧 marker 值（历史遗留）
        std::fs::write(
            &path,
            r#"{"env":{"ANTHROPIC_BASE_URL":"http://old/proxy"},"_aidog_managed":["env.ANTHROPIC_BASE_URL","env.ANTHROPIC_AUTH_TOKEN"]}"#,
        )
        .unwrap();

        let config = json!({
            "env": { "ANTHROPIC_BASE_URL": "http://127.0.0.1:9001/proxy", "ANTHROPIC_AUTH_TOKEN": "gk2" }
        });
        super::write_default_claude_settings(&db, &config).await.unwrap();

        let written: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        // 旧 marker 被清
        assert!(written.get(MARKER_MANAGED).is_none());
        // 新字段写入
        assert_eq!(written["env"]["ANTHROPIC_BASE_URL"], "http://127.0.0.1:9001/proxy");
        assert_eq!(written["env"]["ANTHROPIC_AUTH_TOKEN"], "gk2");

        // DB 重新建立托管快照（sync 当下重算覆盖，不读旧 settings 值）
        let managed: Vec<String> = read_managed_paths(&db).await;
        assert!(managed.contains(&"env.ANTHROPIC_BASE_URL".to_string()));
        assert!(managed.contains(&"env.ANTHROPIC_AUTH_TOKEN".to_string()));
    }

    /// do_sync_group_settings：用户 env_vars 注入 settings.{group}.json env block；
    /// aidog 强写的 ANTHROPIC_BASE_URL / ANTHROPIC_AUTH_TOKEN 不被覆盖（保护字段过滤）。
    #[tokio::test]
    async fn do_sync_group_settings_merges_user_env_and_protects_routing_keys() {
        use crate::gateway::db::test_support::{HomeGuard, test_db};
        use crate::gateway::models::{CreateGroup, EnvVar, RoutingMode};
        let h = HomeGuard::new();
        let db = test_db().await;

        let g = crate::gateway::db::create_group(
            &db,
            CreateGroup {
                name: "env-test".to_string(),
                group_key: Some("gk_envtest".to_string()),
                routing_mode: RoutingMode::Failover,
                auto_from_platform: String::new(),
                request_timeout_secs: 0,
                connect_timeout_secs: 0,
                source_protocol: None,
                max_retries: 2,
                model_mappings: Vec::new(),
                env_vars: vec![
                    EnvVar { key: "CLAUDE_CODE_MAX_OUTPUT_TOKENS".to_string(), value: "32000".to_string() },
                    // 保护字段：同名须被丢弃
                    EnvVar { key: "ANTHROPIC_BASE_URL".to_string(), value: "http://evil.example/proxy".to_string() },
                    EnvVar { key: "ANTHROPIC_AUTH_TOKEN".to_string(), value: "leaked".to_string() },
                ],
            },
        )
        .await
        .unwrap();

        super::do_sync_group_settings(&db, 9911).await.unwrap();

        let written: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(h.home().join(".aidog/settings.gk_envtest.json")).unwrap(),
        )
        .unwrap();

        // 用户自定义变量注入
        assert_eq!(written["env"]["CLAUDE_CODE_MAX_OUTPUT_TOKENS"], "32000");
        // aidog 强写的 proxy 路由字段未被用户覆盖
        assert_eq!(written["env"]["ANTHROPIC_BASE_URL"], "http://127.0.0.1:9911/proxy");
        assert_eq!(written["env"]["ANTHROPIC_AUTH_TOKEN"], "gk_envtest");

        // 清掉这组避免污染其它测试（test_db 用内存库，但 sync 写了真实 HOME 下的文件）
        crate::gateway::db::delete_group(&db, g.id).await.unwrap();
    }
