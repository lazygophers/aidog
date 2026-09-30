//! merge_json deep-merge 单元测试（随源文件 sync_settings.rs 1:1）。
use super::{MANAGED_KEY, MANAGED_SCOPE, merge_json};
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
    assert_eq!(
        base["env"]["ANTHROPIC_BASE_URL"],
        "http://127.0.0.1:9999/proxy"
    );
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
async fn read_managed_paths(db: &aidog_db::Db) -> Vec<String> {
    let v = aidog_db::get_setting(db, MANAGED_SCOPE, MANAGED_KEY)
        .await
        .unwrap()
        .unwrap_or(json!([]));
    v.as_array()
        .unwrap()
        .iter()
        .map(|x| x.as_str().unwrap().to_string())
        .collect()
}

/// write_default_claude_settings：HOME + DB 隔离下全量覆盖（用户手写字段被删）+ 幂等无写。
#[tokio::test]
async fn write_default_claude_settings_overwrites_and_idempotent() {
    use aidog_db::test_support::{HomeGuard, test_db};
    let h = HomeGuard::new();
    let db = test_db().await;
    // 预置用户配置
    let claude_dir = h.home().join(".claude");
    std::fs::create_dir_all(&claude_dir).unwrap();
    let path = claude_dir.join("settings.json");
    std::fs::write(
        &path,
        r#"{"permissions":{"allow":["Read(*)"]},"model":"opus"}"#,
    )
    .unwrap();

    let config = json!({
        "env": { "ANTHROPIC_BASE_URL": "http://127.0.0.1:9890/proxy", "ANTHROPIC_AUTH_TOKEN": "gk_x" }
    });
    super::write_default_claude_settings(&db, &config)
        .await
        .unwrap();

    let written: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(written["env"]["ANTHROPIC_AUTH_TOKEN"], "gk_x");
    // 全量覆盖：用户手写但 config 里没有的键被删掉
    assert!(written.get("permissions").is_none());
    assert!(written.get("model").is_none());
    // settings.json 不再写 marker（已迁 DB）
    assert!(written.get("_aidog_managed").is_none());

    // 幂等：再次同 config → 内容不变（命中 old==new 早退）
    let before = std::fs::read_to_string(&path).unwrap();
    super::write_default_claude_settings(&db, &config)
        .await
        .unwrap();
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

/// write_default_claude_settings：托管快照存 DB = 写入内容（默认组 config）的全部叶子。
/// 全量覆盖下用户自装条目已被删除，因此也不进快照。
/// 语义：导入 diff 排除此快照 → 同步当下零差异，仅显示同步之后用户在 CC 侧的新增/变化。
/// settings.json 不写 marker（已迁 DB）。
#[tokio::test]
async fn write_default_claude_settings_records_managed_paths() {
    use aidog_db::test_support::{HomeGuard, test_db};
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
    super::write_default_claude_settings(&db, &config)
        .await
        .unwrap();

    let written: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();

    // 全量覆盖：用户自装条目被删，只剩 config 里的
    assert!(
        written["enabledPlugins"]
            .get("user-plugin@user-market")
            .is_none()
    );
    assert!(written.get("extraKnownMarketplaces").is_none());
    assert_eq!(written["enabledPlugins"]["aidog-plugin@official"], true);

    // settings.json 不再写 marker
    assert!(written.get("_aidog_managed").is_none());

    // 托管快照在 DB：= merge 后完整快照，含 aidog 注入条目 + 用户自装条目
    let managed: Vec<String> = read_managed_paths(&db).await;
    assert!(managed.contains(&"env.ANTHROPIC_BASE_URL".to_string()));
    assert!(managed.contains(&"env.ANTHROPIC_AUTH_TOKEN".to_string()));
    assert!(managed.contains(&"enabledPlugins.aidog-plugin@official".to_string()));
    // 全量覆盖：用户自装条目已不在文件里，也不进托管集
    assert!(
        !managed
            .iter()
            .any(|p| p.contains("user-plugin@user-market"))
    );
    assert!(
        !managed
            .iter()
            .any(|p| p.starts_with("extraKnownMarketplaces."))
    );
    // 快照不含 `_aidog_` 前缀（跳过，不自引用）
    assert!(!managed.iter().any(|p| p.starts_with("_aidog_")));
}

/// write_default_claude_settings：老用户 settings.json 残留旧 `_aidog_managed` 值 →
/// 全量覆盖后自然消失（marker 数据源已迁 DB）。
#[tokio::test]
async fn write_default_claude_settings_drops_legacy_marker() {
    use aidog_db::test_support::{HomeGuard, test_db};
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
    super::write_default_claude_settings(&db, &config)
        .await
        .unwrap();

    let written: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    // 旧 marker 被清
    assert!(written.get("_aidog_managed").is_none());
    // 新字段写入
    assert_eq!(
        written["env"]["ANTHROPIC_BASE_URL"],
        "http://127.0.0.1:9001/proxy"
    );
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
    use crate::gateway::models::{CreateGroup, EnvVar, RoutingMode};
    use aidog_db::test_support::{HomeGuard, test_db};
    let h = HomeGuard::new();
    let db = test_db().await;

    let g = aidog_db::create_group(
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
            model_mappings: vec![aidog_db::models::ModelMapping {
                source_model: "claude-sonnet-5".to_string(),
                target_platform_id: 0,
                target_model: "deepseek-chat".to_string(),
                request_timeout_secs: 0,
                connect_timeout_secs: 0,
            }],
            env_vars: vec![
                EnvVar {
                    key: "CLAUDE_CODE_MAX_OUTPUT_TOKENS".to_string(),
                    value: "32000".to_string(),
                },
                // 保护字段：同名须被丢弃
                EnvVar {
                    key: "ANTHROPIC_BASE_URL".to_string(),
                    value: "http://evil.example/proxy".to_string(),
                },
                EnvVar {
                    key: "ANTHROPIC_AUTH_TOKEN".to_string(),
                    value: "leaked".to_string(),
                },
                EnvVar {
                    key: "CLAUDE_CODE_AUTO_COMPACT_WINDOW".to_string(),
                    value: "evil".to_string(),
                },
                EnvVar {
                    key: "CLAUDE_CODE_DISABLE_UNKNOWN_MODEL_WINDOW_ENFORCEMENT".to_string(),
                    value: "evil".to_string(),
                },
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
    assert_eq!(
        written["env"]["ANTHROPIC_BASE_URL"],
        "http://127.0.0.1:9911/proxy"
    );
    assert_eq!(written["env"]["ANTHROPIC_AUTH_TOKEN"], "gk_envtest");
    // 压缩窗口对齐 env 注入（组内有 registry 模型 deepseek-chat）
    let compact = written["env"]["CLAUDE_CODE_AUTO_COMPACT_WINDOW"]
        .as_str()
        .expect("CLAUDE_CODE_AUTO_COMPACT_WINDOW must be injected for non-passthrough group")
        .parse::<i64>()
        .expect("must be plain integer");
    assert!(
        (100_000..=1_000_000).contains(&compact),
        "window must clamp into CC legal range, got {compact}"
    );
    assert_eq!(
        written["env"]["CLAUDE_CODE_DISABLE_UNKNOWN_MODEL_WINDOW_ENFORCEMENT"], "1",
        "未识别模型被动压缩开关必须注入"
    );
    // 用户 env_var 里同名的两个窗口 key 已被保护清单丢弃（上面注入值仍在）
    assert_ne!(
        written["env"]["CLAUDE_CODE_DISABLE_UNKNOWN_MODEL_WINDOW_ENFORCEMENT"],
        "evil"
    );
    assert_ne!(written["env"]["CLAUDE_CODE_AUTO_COMPACT_WINDOW"], "evil");

    // 清掉这组避免污染其它测试（test_db 用内存库，但 sync 写了真实 HOME 下的文件）
    aidog_db::delete_group(&db, g.id).await.unwrap();
}

/// group_max_context_window：haiku（杂活槽）/ gpt（Codex 专用）不参与窗口计算。
/// 构造成杂活槽窗口**最宽**，取 max 时若误把它算进来就会被拉到 1048576；
/// 正确结果必须停在主对话槽（glm-4.5 / glm-4.6 均远小于 1M）。
#[tokio::test]
async fn group_max_context_window_ignores_haiku_and_gpt_slots() {
    use aidog_db::test_support::test_db;
    let db = test_db().await;

    let pm = aidog_db::models::PlatformModels {
        default: Some("glm-4.5".to_string()), // 131072
        sonnet: Some("glm-4.6".to_string()),
        opus: Some("glm-4.6".to_string()),
        haiku: Some("deepseek-chat".to_string()), // 1048576，必须被忽略
        gpt: Some("deepseek-chat".to_string()),   // 同上
        ..Default::default()
    };

    // 不写死具体数值：registry 里同名模型各平台窗口可能被更新，
    // 断言点是「结果没被 1M 的杂活槽顶上去」= 它们没参与 max。
    let got = super::group_max_context_window(&db, &[], &[&pm])
        .await
        .expect("主对话槽模型在 registry 里查得到");
    assert!(
        (131_072..1_000_000).contains(&got),
        "haiku/gpt 槽(deepseek-chat=1048576)不得参与最大值计算，got {got}"
    );
}

/// group_max_context_window 取的是最大值：宽模型在组里就按宽的算，不被窄模型拖低。
#[tokio::test]
async fn group_max_context_window_takes_widest_main_loop_model() {
    use aidog_db::test_support::test_db;
    let db = test_db().await;

    let pm = aidog_db::models::PlatformModels {
        default: Some("glm-4.5".to_string()),    // 131072
        opus: Some("deepseek-chat".to_string()), // 1048576
        ..Default::default()
    };

    let got = super::group_max_context_window(&db, &[], &[&pm])
        .await
        .expect("主对话槽模型在 registry 里查得到");
    assert!(
        got > 131_072,
        "必须取组内最宽的主对话模型窗口，不得被 glm-4.5(131072) 拖低，got {got}"
    );
}

// ── pi 模型候选：分组映射 ∪ 平台有效模型，去重；全空回落调用方传入的默认清单 ──

use super::pi_model_candidates;
use aidog_db::models::{ModelMapping, PlatformModels};

fn mapping(source: &str) -> ModelMapping {
    ModelMapping {
        source_model: source.to_string(),
        target_platform_id: 0,
        target_model: String::new(),
        request_timeout_secs: 0,
        connect_timeout_secs: 0,
    }
}

#[test]
fn pi_models_union_group_mappings_and_platform_models_deduped() {
    let models = pi_model_candidates(
        &[mapping("claude-sonnet-5"), mapping("glm-4-plus")],
        &[PlatformModels {
            default: Some("glm-4-plus".into()),
            sonnet: Some("glm-4-air".into()),
            ..PlatformModels::default()
        }],
        &["should-not-be-used".to_string()],
    );
    assert_eq!(models, vec!["claude-sonnet-5", "glm-4-plus", "glm-4-air"]);
}

#[test]
fn pi_models_fall_back_to_caller_defaults_when_group_has_none() {
    // 空 models 的 provider 在 pi 的 /model 里一个模型都选不出来，等于废配置。
    // 兜底清单由调用方查 registry 得到（gateway::proxy::default_model_ids），此处只验「用了它」。
    let fallback = vec!["claude-opus-5".to_string(), "gpt-5.5".to_string()];
    let models = pi_model_candidates(&[], &[PlatformModels::default()], &fallback);
    assert_eq!(models, fallback);
}

/// 纯 claude_code（订阅透传）组：settings.{group}.json 不注入路由 env 值，改写空串
/// 中性化（ANTHROPIC_BASE_URL / ANTHROPIC_AUTH_TOKEN = ""）—— 透传客户端自带订阅
/// OAuth，AUTH_TOKEN=group_key 会覆盖 OAuth 致上游 401；absent 会让全局文件（默认组
/// 写入）的同名 key 在 --settings 深合并时漏进来，劫走订阅流量。混合组照常注入。
#[tokio::test]
async fn do_sync_group_settings_skips_routing_env_for_pure_claude_code_group() {
    use crate::gateway::models::{CreateGroup, RoutingMode};
    use aidog_db::models::Protocol;
    use aidog_db::test_support::{HomeGuard, test_db};

    let h = HomeGuard::new();
    let db = test_db().await;

    // 纯透传组：单平台 Protocol::ClaudeCode
    let cc_platform = aidog_db::create_platform(
        &db,
        aidog_db::models::CreatePlatform {
            name: "cc-sub".to_string(),
            platform_type: Protocol::ClaudeCode,
            base_url: "https://api.anthropic.com".to_string(),
            api_key: String::new(),
            extra: String::new(),
            models: None,
            available_models: None,
            endpoints: None,
            manual_budgets: None,
            auto_group: Some(false),
            join_group_ids: None,
            expires_at: None,
            quota_source: None,
        },
    )
    .await
    .unwrap();

    let g_cc = aidog_db::create_group(
        &db,
        CreateGroup {
            name: "cc".to_string(),
            group_key: Some("gk_cc".to_string()),
            routing_mode: RoutingMode::Failover,
            auto_from_platform: String::new(),
            request_timeout_secs: 0,
            connect_timeout_secs: 0,
            source_protocol: None,
            max_retries: 2,
            model_mappings: Vec::new(),
            env_vars: Vec::new(),
        },
    )
    .await
    .unwrap();
    aidog_db::set_group_platforms(
        &db,
        g_cc.id,
        &[aidog_db::models::GroupPlatformInput {
            platform_id: cc_platform.id,
            priority: None,
            weight: None,
            level_priority: None,
        }],
    )
    .await
    .unwrap();

    // 对照组：普通平台 → 照常注入
    let normal = aidog_db::create_platform(
        &db,
        aidog_db::models::CreatePlatform {
            name: "normal".to_string(),
            platform_type: Protocol::Anthropic,
            base_url: "https://api.example.com".to_string(),
            api_key: "sk-x".to_string(),
            extra: String::new(),
            models: None,
            available_models: None,
            endpoints: None,
            manual_budgets: None,
            auto_group: Some(false),
            join_group_ids: None,
            expires_at: None,
            quota_source: None,
        },
    )
    .await
    .unwrap();
    let g_norm = aidog_db::create_group(
        &db,
        CreateGroup {
            name: "norm".to_string(),
            group_key: Some("gk_norm".to_string()),
            routing_mode: RoutingMode::Failover,
            auto_from_platform: String::new(),
            request_timeout_secs: 0,
            connect_timeout_secs: 0,
            source_protocol: None,
            max_retries: 2,
            model_mappings: Vec::new(),
            env_vars: Vec::new(),
        },
    )
    .await
    .unwrap();
    aidog_db::set_group_platforms(
        &db,
        g_norm.id,
        &[aidog_db::models::GroupPlatformInput {
            platform_id: normal.id,
            priority: None,
            weight: None,
            level_priority: None,
        }],
    )
    .await
    .unwrap();

    super::do_sync_group_settings(&db, 9912).await.unwrap();

    let cc: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(h.home().join(".aidog/settings.gk_cc.json")).unwrap(),
    )
    .unwrap();
    // 透传组：路由 env 写空串中性化（非 absent）—— --settings 与全局文件深合并时
    // absent key 会从 ~/.claude/settings.json（默认组写入）漏进来，劫走订阅流量；
    // 空串 CC 视为未设（保留订阅 OAuth，直连 api.anthropic.com 走 HTTPS_PROXY）。
    assert_eq!(
        cc["env"]["ANTHROPIC_BASE_URL"], "",
        "pure claude_code group must neutralize ANTHROPIC_BASE_URL with empty string, got: {}",
        cc["env"]
    );
    // 透传组：CC 认识真实模型，压缩窗口 env 同样不注入
    assert!(
        cc["env"].get("CLAUDE_CODE_AUTO_COMPACT_WINDOW").is_none(),
        "pure claude_code group must not set CLAUDE_CODE_AUTO_COMPACT_WINDOW, got: {}",
        cc["env"]
    );
    assert!(
        cc["env"]
            .get("CLAUDE_CODE_DISABLE_UNKNOWN_MODEL_WINDOW_ENFORCEMENT")
            .is_none(),
        "pure claude_code group must not set DISABLE_UNKNOWN_MODEL_WINDOW_ENFORCEMENT, got: {}",
        cc["env"]
    );
    assert_eq!(
        cc["env"]["ANTHROPIC_AUTH_TOKEN"], "",
        "pure claude_code group must neutralize ANTHROPIC_AUTH_TOKEN with empty string, got: {}",
        cc["env"]
    );

    let norm: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(h.home().join(".aidog/settings.gk_norm.json")).unwrap(),
    )
    .unwrap();
    // 对照：普通组照常注入
    assert_eq!(
        norm["env"]["ANTHROPIC_BASE_URL"],
        "http://127.0.0.1:9912/proxy"
    );
    assert_eq!(norm["env"]["ANTHROPIC_AUTH_TOKEN"], "gk_norm");
    // 普通组不注入 HTTPS_PROXY（那是订阅透传组的专属路由形态）
    assert!(norm["env"].get("HTTPS_PROXY").is_none());

    // 透传组：HTTPS_PROXY 注入（username=组名 group.name，密码随机非空，端口=代理端口）
    let https_proxy = cc["env"]["HTTPS_PROXY"].as_str().expect("pure cc group must set HTTPS_PROXY");
    assert!(
        https_proxy.starts_with("http://cc:") && https_proxy.ends_with("@127.0.0.1:9912"),
        "HTTPS_PROXY shape, got: {https_proxy}"
    );
    let pw = https_proxy
        .strip_prefix("http://cc:")
        .and_then(|r| r.strip_suffix("@127.0.0.1:9912"))
        .expect("parsable userinfo");
    assert!(pw.len() >= 8, "password must be non-empty random, got len {}", pw.len());

    // 二次同步密码稳定（KV 持久化；每次重新随机会让 settings 文件每轮必写）
    super::do_sync_group_settings(&db, 9912).await.unwrap();
    let cc2: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(h.home().join(".aidog/settings.gk_cc.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(cc2["env"]["HTTPS_PROXY"], cc["env"]["HTTPS_PROXY"]);

    aidog_db::delete_group(&db, g_cc.id).await.unwrap();
    aidog_db::delete_group(&db, g_norm.id).await.unwrap();
}

/// pure cc 组 HTTPS_PROXY 注入的边界：组名不 URL-safe → 不写坏配置（该组 env 无
/// HTTPS_PROXY），其余组照常同步落盘，整体返回 Err 提示改名；用户 env_vars 里的
/// HTTPS_PROXY 对 pure cc 组是托管字段，同名丢弃。
#[tokio::test]
async fn do_sync_group_settings_blocks_url_unsafe_cc_group_name() {
    use crate::gateway::models::{CreateGroup, EnvVar, RoutingMode};
    use aidog_db::models::Protocol;
    use aidog_db::test_support::{HomeGuard, test_db};

    let h = HomeGuard::new();
    let db = test_db().await;

    let mk_platform = |db: &aidog_db::Db, name: &str, pt: Protocol, api_key: &str| {
        let db = db.clone();
        let name = name.to_string();
        let api_key = api_key.to_string();
        async move {
            aidog_db::create_platform(
                &db,
                aidog_db::models::CreatePlatform {
                    name,
                    platform_type: pt,
                    base_url: "https://api.example.com".to_string(),
                    api_key,
                    extra: String::new(),
                    models: None,
                    available_models: None,
                    endpoints: None,
                    manual_budgets: None,
                    auto_group: Some(false),
                    join_group_ids: None,
                    expires_at: None,
                    quota_source: None,
                },
            )
            .await
            .unwrap()
        }
    };
    let cc_ok = mk_platform(&db, "cc-sub", Protocol::ClaudeCode, "").await;
    let cc_bad = mk_platform(&db, "cc-sub2", Protocol::ClaudeCode, "").await;
    let normal = mk_platform(&db, "normal", Protocol::Anthropic, "sk-x").await;

    let mk_group = |db: &aidog_db::Db, name: &str, key: &str, evs: Vec<EnvVar>| {
        let db = db.clone();
        let name = name.to_string();
        let key = key.to_string();
        async move {
            aidog_db::create_group(
                &db,
                CreateGroup {
                    name,
                    group_key: Some(key),
                    routing_mode: RoutingMode::Failover,
                    auto_from_platform: String::new(),
                    request_timeout_secs: 0,
                    connect_timeout_secs: 0,
                    source_protocol: None,
                    max_retries: 2,
                    model_mappings: Vec::new(),
                    env_vars: evs,
                },
            )
            .await
            .unwrap()
        }
    };
    let g_ok = mk_group(&db, "ok.cc_1", "gk_ok", vec![EnvVar {
        key: "HTTPS_PROXY".to_string(),
        value: "http://evil.example:1".to_string(),
    }])
    .await;
    aidog_db::set_group_platforms(
        &db,
        g_ok.id,
        &[aidog_db::models::GroupPlatformInput { platform_id: cc_ok.id, priority: None, weight: None, level_priority: None }],
    )
    .await
    .unwrap();
    // 组名含空格：不 URL-safe
    let g_bad = mk_group(&db, "bad name!", "gk_bad", Vec::new()).await;
    aidog_db::set_group_platforms(
        &db,
        g_bad.id,
        &[aidog_db::models::GroupPlatformInput { platform_id: cc_bad.id, priority: None, weight: None, level_priority: None }],
    )
    .await
    .unwrap();
    let g_norm = mk_group(&db, "norm", "gk_norm2", Vec::new()).await;
    aidog_db::set_group_platforms(
        &db,
        g_norm.id,
        &[aidog_db::models::GroupPlatformInput { platform_id: normal.id, priority: None, weight: None, level_priority: None }],
    )
    .await
    .unwrap();

    let err = super::do_sync_group_settings(&db, 9913).await.unwrap_err();
    assert!(err.contains("bad name!"), "error must name the offending group, got: {err}");

    // 坏名组：文件照写但无 HTTPS_PROXY（不写坏配置）
    let bad: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(h.home().join(".aidog/settings.gk_bad.json")).unwrap(),
    )
    .unwrap();
    assert!(bad["env"].get("HTTPS_PROXY").is_none());

    // 合法 cc 组：HTTPS_PROXY 注入且未被用户 env_var 覆盖
    let ok: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(h.home().join(".aidog/settings.gk_ok.json")).unwrap(),
    )
    .unwrap();
    let injected = ok["env"]["HTTPS_PROXY"].as_str().expect("must inject HTTPS_PROXY");
    assert!(
        injected.starts_with("http://ok.cc_1:") && injected.ends_with("@127.0.0.1:9913"),
        "got: {injected}"
    );

    // 普通组不被坏名组阻断，照常注入路由 env
    let norm: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(h.home().join(".aidog/settings.gk_norm2.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(norm["env"]["ANTHROPIC_BASE_URL"], "http://127.0.0.1:9913/proxy");
}

/// Desktop / 多 shell export 文案形状（票 12 UI 同源）。
#[test]
fn cc_proxy_export_line_shape() {
    let line = super::cc_proxy_export_line("my.cc_group", "pw123", 9100);
    assert_eq!(line, "export HTTPS_PROXY='http://my.cc_group:pw123@127.0.0.1:9100'");
}
