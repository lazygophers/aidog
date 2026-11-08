//! agent_config 一键卸载单元测试（随源文件 agent_config.rs 1:1）。
//!
//! HOME / CODEX_HOME / PI_CODING_AGENT_DIR 全部指向 tempdir（HomeGuard），
//! 不碰真家目录、不真实外呼。用户内容以独立键预置，断言「卸载后 aidog 段
//! 消失、用户内容逐项保留」与「dry_run 不落盘不改 DB」。

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use aidog_db::test_support::{HomeGuard, test_db};
use aidog_db::Db;
use serde_json::json;

fn home() -> PathBuf {
    std::env::var("HOME").unwrap().into()
}

fn codex_home() -> PathBuf {
    std::env::var("CODEX_HOME").unwrap().into()
}

fn pi_dir() -> PathBuf {
    std::env::var("PI_CODING_AGENT_DIR").unwrap().into()
}

fn jv(path: &Path) -> serde_json::Value {
    serde_json::from_str(&std::fs::read_to_string(path).unwrap())
        .unwrap_or_else(|e| panic!("parse {}: {e}", path.display()))
}

fn mcp_payload(name: &str) -> crate::gateway::mcp::McpUpdatePayload {
    crate::gateway::mcp::McpUpdatePayload {
        name: name.to_string(),
        transport: "stdio".into(),
        command: format!("{name}-cmd"),
        args: vec!["mcp".into()],
        env: Default::default(),
        url: String::new(),
        headers: Default::default(),
    }
}

/// 构造「aidog 全量注入 + 用户自有内容混杂」的现场（走真实 install 路径写）。
async fn seed_all(db: &Db) {
    let h = home();
    // 用户先有自己的 .claude.json（ANTHROPIC_BASE_URL 将被 install 覆盖 → 触发备份）
    std::fs::write(
        h.join(".claude.json"),
        serde_json::to_string_pretty(&json!({
            "ANTHROPIC_BASE_URL": "https://u.example",
            "projects": {"p1": "keep"},
        }))
        .unwrap(),
    )
    .unwrap();
    // 用户自己的 ~/.claude/config.json
    std::fs::create_dir_all(h.join(".claude")).unwrap();
    std::fs::write(
        h.join(".claude/config.json"),
        serde_json::to_string_pretty(&json!({
            "primaryApiKey": "user-key",
            "theme": "dark",
        }))
        .unwrap(),
    )
    .unwrap();

    // mcp：aidog 自装（claude + codex）+ 用户条目（仅 claude）
    crate::gateway::mcp::add_server(db, mcp_payload("aidog"))
        .await
        .unwrap();
    crate::gateway::mcp::set_agent_enabled(db, "aidog", crate::gateway::mcp::McpAgent::ClaudeCode, true)
        .await
        .unwrap();
    crate::gateway::mcp::set_agent_enabled(db, "aidog", crate::gateway::mcp::McpAgent::Codex, true)
        .await
        .unwrap();
    crate::gateway::mcp::add_server(db, mcp_payload("user-srv"))
        .await
        .unwrap();
    crate::gateway::mcp::set_agent_enabled(db, "user-srv", crate::gateway::mcp::McpAgent::ClaudeCode, true)
        .await
        .unwrap();

    // coding tools 默认写（覆盖用户已有键 → 触发备份）
    crate::ai_tools_cmd::coding_tools::ensure_default_coding_tools_settings(db)
        .await
        .unwrap();

    // export base url（覆盖用户已有值 → 触发备份）
    crate::sync_settings::export_claude_config_with_db(db, 8080)
        .await
        .unwrap();

    // codex 默认 profile + 用户键 + aidog notify
    crate::gateway::codex::write_default_profile_to_config(8080).unwrap();
    let mut cfg = crate::gateway::codex::codex_config_read().unwrap();
    cfg.as_object_mut().unwrap().insert("userKey".into(), json!(42));
    let notify_cmd = h.join(".aidog/scripts").join(aidog_hooks::SCRIPT_COMPLETE);
    aidog_hooks::inject_codex_notify(&mut cfg, notify_cmd.to_string_lossy().as_ref());
    crate::gateway::codex::codex_config_write(cfg).unwrap();

    // codex 分组 profile（aidog 生成）+ 用户自己的 *.config.toml（非 aidog）
    crate::gateway::codex::write_group_profile("g1", 8080).unwrap();
    std::fs::write(codex_home().join("my.config.toml"), "model_provider = \"other\"\n").unwrap();

    // pi：aidog-g1 + 用户 provider
    std::fs::create_dir_all(pi_dir()).unwrap();
    std::fs::write(
        pi_dir().join("models.json"),
        serde_json::to_string_pretty(&json!({
            "providers": {
                "aidog-g1": {"apiKey": "k", "baseUrl": "http://127.0.0.1:9890/pi/v1", "api": "openai-completions"},
                "userp": {"apiKey": "u", "baseUrl": "https://user.example/v1", "api": "openai-completions"}
            }
        }))
        .unwrap(),
    )
    .unwrap();
    std::fs::write(
        pi_dir().join("settings.json"),
        serde_json::to_string_pretty(&json!({
            "defaultProvider": "aidog-g1",
            "userKey": 1,
        }))
        .unwrap(),
    )
    .unwrap();

    // ~/.aidog 分组 settings 文件 + hook 脚本 + 用户自己的脚本
    std::fs::create_dir_all(h.join(".aidog")).unwrap();
    std::fs::write(h.join(".aidog/settings.foo.json"), json!({"group": "foo"}).to_string()).unwrap();
    crate::hooks::generate_hook_scripts(crate::gateway::scripts::ScriptInvoker::Python3).unwrap();
    std::fs::write(h.join(".aidog/scripts/user-thing.py"), "# user script\n").unwrap();

    // 托管叶子（默认组 settings.json + DB 快照）
    std::fs::write(
        h.join(".claude/settings.json"),
        serde_json::to_string_pretty(&json!({
            "env": {"FOO": "x", "BAR": "y"},
            "model": "opus",
            "userKey": 2,
        }))
        .unwrap(),
    )
    .unwrap();
    aidog_db::set_setting(
        db,
        aidog_db::models::SetSettingInput {
            scope: crate::sync_settings::MANAGED_SCOPE.into(),
            key: crate::sync_settings::MANAGED_KEY.into(),
            value: json!(["env.FOO", "model"]),
        },
    )
    .await
    .unwrap();
}

/// 递归快照 HOME 下全部文件内容（dry_run 前后对比）。
fn snapshot_home() -> BTreeMap<String, String> {
    fn walk(dir: &Path, out: &mut BTreeMap<String, String>) {
        for e in std::fs::read_dir(dir).into_iter().flatten().flatten() {
            let p = e.path();
            if p.is_dir() {
                walk(&p, out);
            } else if let Ok(c) = std::fs::read_to_string(&p) {
                out.insert(p.to_string_lossy().into_owned(), c);
            }
        }
    }
    let mut out = BTreeMap::new();
    walk(&home(), &mut out);
    out
}

/// 卸载后：aidog 注入全消失、用户内容逐项保留、有备份的原值恢复。
#[tokio::test]
async fn uninstall_removes_aidog_and_keeps_user_content() {
    let _h = HomeGuard::new();
    let db = test_db().await;
    seed_all(&db).await;
    let h = home();

    let actions = super::run_uninstall(&db).await.unwrap();
    assert!(!actions.is_empty(), "应有卸载动作");
    assert!(
        actions.iter().all(|a| a.error.is_none()),
        "单面不应失败: {actions:?}"
    );

    // ~/.claude.json：aidog mcp 条目消失、用户条目/键保留、备份恢复
    let cj = jv(&h.join(".claude.json"));
    assert!(
        cj.get("mcpServers").and_then(|m| m.get("aidog")).is_none(),
        "aidog mcp 条目应消失"
    );
    assert_eq!(cj["mcpServers"]["user-srv"]["command"], "user-srv-cmd", "用户 mcp 条目保留");
    assert_eq!(
        cj["ANTHROPIC_BASE_URL"], "https://u.example",
        "install 覆盖前有备份 → 恢复用户原值"
    );
    assert!(cj.get("hasCompletedOnboarding").is_none(), "无备份 → 删 aidog 注入键");
    assert_eq!(cj["projects"]["p1"], "keep");

    // ~/.claude/config.json：primaryApiKey 恢复用户原值，其余保留
    let cc = jv(&h.join(".claude/config.json"));
    assert_eq!(cc["primaryApiKey"], "user-key", "primaryApiKey 恢复用户原值");
    assert_eq!(cc["theme"], "dark");

    // ~/.codex/config.toml：aidog 三处标识 + notify 消失，用户键保留
    let cx = crate::gateway::codex::codex_config_read().unwrap();
    assert!(
        cx.get("mcp_servers").and_then(|m| m.get("aidog")).is_none(),
        "codex mcp 条目应消失"
    );
    assert!(cx.get("model_provider").is_none(), "aidog 默认 profile 删除");
    assert!(cx.get("model_providers").and_then(|p| p.get("aidog")).is_none());
    assert!(cx.get("notify").is_none(), "aidog notify 删除");
    assert_eq!(cx["userKey"], 42);

    // pi：aidog- provider 与我们的 defaultProvider 消失，用户内容保留
    let pm = jv(&pi_dir().join("models.json"));
    assert!(pm["providers"].get("aidog-g1").is_none());
    assert_eq!(pm["providers"]["userp"]["apiKey"], "u");
    let ps = jv(&pi_dir().join("settings.json"));
    assert!(ps.get("defaultProvider").is_none(), "我们的 defaultProvider 删除");
    assert_eq!(ps["userKey"], 1);

    // 分组 settings 文件 / codex profile / hook 脚本
    assert!(!h.join(".aidog/settings.foo.json").exists());
    assert!(!codex_home().join("g1.config.toml").exists(), "aidog 生成的 profile 删除");
    assert_eq!(
        std::fs::read_to_string(codex_home().join("my.config.toml")).unwrap(),
        "model_provider = \"other\"\n",
        "用户自己的 *.config.toml 保留"
    );
    assert!(!h.join(".aidog/scripts").join(aidog_hooks::SCRIPT_COMPLETE).exists());
    assert!(!h.join(".aidog/scripts").join(aidog_hooks::SCRIPT_EVENT_NOTIFY).exists());
    assert!(h.join(".aidog/scripts/user-thing.py").exists(), "用户脚本保留");

    // 默认组 settings.json：托管叶子删、用户叶子留
    let st = jv(&h.join(".claude/settings.json"));
    assert!(st.get("model").is_none(), "托管叶子删除");
    assert!(st.get("env").and_then(|e| e.get("FOO")).is_none(), "托管叶子 env.FOO 删除");
    assert_eq!(st["env"]["BAR"], "y", "非托管叶子保留");
    assert_eq!(st["userKey"], 2);

    // DB：aidog mcp 行 / 托管快照 / 备份行全清
    assert!(
        crate::gateway::mcp::store::get_mcp_server(&db, "aidog")
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        aidog_db::get_setting(&db, crate::sync_settings::MANAGED_SCOPE, crate::sync_settings::MANAGED_KEY)
            .await
            .unwrap()
            .is_none()
    );
    for k in [
        super::KEY_CLAUDE_BASE_URL,
        super::KEY_PRIMARY_API_KEY,
        super::KEY_ONBOARDING,
    ] {
        assert!(
            aidog_db::get_setting(&db, super::BACKUP_SCOPE, k).await.unwrap().is_none(),
            "备份 {k} 应清"
        );
    }
}

/// dry_run：清单覆盖各写入面；文件与 DB 均不动。
#[tokio::test]
async fn dry_run_lists_actions_without_touching_files() {
    let _h = HomeGuard::new();
    let db = test_db().await;
    seed_all(&db).await;

    let before = snapshot_home();
    let actions = super::plan_actions(&db).await.unwrap();
    assert!(actions.len() >= 10, "清单应覆盖各写入面: {actions:?}");
    for surface in [
        "mcp",
        "claude_env",
        "claude_tools",
        "claude_settings",
        "group_settings",
        "codex_profile",
        "codex_notify",
        "pi",
        "hooks",
    ] {
        assert!(actions.iter().any(|a| a.surface == surface), "缺 {surface} 面: {actions:?}");
    }
    assert_eq!(snapshot_home(), before, "dry_run 不得改文件");
    assert!(
        aidog_db::get_setting(&db, crate::sync_settings::MANAGED_SCOPE, crate::sync_settings::MANAGED_KEY)
            .await
            .unwrap()
            .is_some(),
        "dry_run 不得清 DB 托管快照"
    );
    assert!(
        aidog_db::get_setting(&db, super::BACKUP_SCOPE, super::KEY_CLAUDE_BASE_URL)
            .await
            .unwrap()
            .is_some(),
        "install 覆盖用户 ANTHROPIC_BASE_URL 前应已备份"
    );
}

/// 幂等：第二次运行动作清单为空（恢复后的用户原值不再命中 aidog 指纹）。
#[tokio::test]
async fn uninstall_is_idempotent() {
    let _h = HomeGuard::new();
    let db = test_db().await;
    seed_all(&db).await;
    super::run_uninstall(&db).await.unwrap();
    let second = super::plan_actions(&db).await.unwrap();
    assert!(second.is_empty(), "二次清单应为空: {second:?}");
}

/// 只有用户自己的内容 → 清单为空（指纹不误伤用户配置）。
#[tokio::test]
async fn user_owned_config_produces_no_actions() {
    let _h = HomeGuard::new();
    let db = test_db().await;
    let h = home();
    std::fs::create_dir_all(h.join(".claude")).unwrap();
    std::fs::create_dir_all(codex_home()).unwrap();
    std::fs::write(
        h.join(".claude.json"),
        serde_json::to_string_pretty(&json!({
            "ANTHROPIC_BASE_URL": "https://mine.example/v1",
            "mcpServers": {"mine": {"command": "x"}},
        }))
        .unwrap(),
    )
    .unwrap();
    std::fs::write(
        h.join(".claude/config.json"),
        json!({"primaryApiKey": "custom"}).to_string(),
    )
    .unwrap();
    crate::gateway::codex::codex_config_write(json!({
        "model_provider": "other",
        "notify": ["/usr/bin/user-notify"],
        "mcp_servers": {"mine": {"command": "x"}},
    }))
    .unwrap();
    std::fs::create_dir_all(pi_dir()).unwrap();
    std::fs::write(
        pi_dir().join("models.json"),
        json!({"providers": {"mine": {"apiKey": "m"}}}).to_string(),
    )
    .unwrap();
    std::fs::write(
        pi_dir().join("settings.json"),
        json!({"defaultProvider": "mine"}).to_string(),
    )
    .unwrap();

    let actions = super::plan_actions(&db).await.unwrap();
    assert!(actions.is_empty(), "用户自有内容不应产生卸载动作: {actions:?}");
}
