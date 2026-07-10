#![cfg(test)]
//! hooks 命令端到端覆盖：HOME 隔离（tempdir）下走 generate_hook_scripts / enabled_hook_events /
//! seed_default_templates / inject_hooks / remove_hooks / set/get default_hooks_enabled /
//! build_notify_hooks_fragment。transitively 覆盖 do_sync_group_settings + codex config 写入。
use aidog_core::hooks::*;
use aidog_core::sync_settings::do_sync_group_settings;
#[allow(unused_imports)]
use aidog_core::shared::*;
use aidog_core::gateway::{self, db::Db};
use aidog_core::gateway::models::*;
use crate::commands::test_harness::mock_app_with_db;
use aidog_core::gateway::db::test_support::HomeGuard;
use tauri::Manager;

#[tokio::test]
async fn enabled_events_default_when_per_event_empty() {
    let _h = HomeGuard::new();
    let app = mock_app_with_db().await;
    let db = app.state::<Db>();
    // 未配 per_event → 回退默认精选 ON 集
    let events = enabled_hook_events(&db).await;
    assert!(!events.is_empty());
    assert!(events.iter().any(|e| e == "Stop"));
}

#[tokio::test]
async fn seed_templates_fills_then_idempotent() {
    let _h = HomeGuard::new();
    let app = mock_app_with_db().await;
    let db = app.state::<Db>();
    seed_default_templates(&db).await.unwrap();
    let settings = gateway::db::get_notification_settings(&db).await;
    let tc = settings.per_type.get("task_complete").unwrap();
    assert!(!tc.template.trim().is_empty());
    // 二次调用幂等（不 panic）
    seed_default_templates(&db).await.unwrap();
}

#[tokio::test]
async fn generate_scripts_writes_files() {
    let _h = HomeGuard::new();
    let invoker = gateway::scripts::ScriptInvoker::from_setting(Some("python3"));
    let paths = generate_hook_scripts(invoker).unwrap();
    assert!(!paths.complete.is_empty());
    assert!(!paths.event_notify.is_empty());
    // 脚本目录应存在生成文件
    let scripts_dir = aidog_core::shared::aidog_scripts_dir().unwrap();
    assert!(scripts_dir.join(gateway::hooks::SCRIPT_COMPLETE).exists());
    assert!(scripts_dir.join(gateway::hooks::SCRIPT_EVENT_NOTIFY).exists());
}

#[tokio::test]
async fn build_fragment_returns_hooks_object() {
    let _h = HomeGuard::new();
    let app = mock_app_with_db().await;
    let db = app.state::<Db>();
    let frag = build_notify_hooks_fragment(db.clone()).await.unwrap();
    assert!(frag.is_object());
}

// inject_hooks / remove_hooks / set_default_hooks_enabled 签名硬绑 tauri::AppHandle(Wry runtime)，
// 无法用 MockRuntime AppHandle 调用（tauri command 不泛型化 runtime）。改直调其等价内部链路：
// 基线 claude_code 配置注入/剥离 hooks + do_sync_group_settings 物化，覆盖 sync_settings 大块。

#[tokio::test]
async fn claude_code_inject_remove_and_sync() {
    let _h = HomeGuard::new();
    let app = mock_app_with_db().await;
    let db = app.state::<Db>();

    // 建一个分组使 do_sync_group_settings 有物化目标
    gateway::db::create_group(&db, gateway::models::CreateGroup {
        name: "g".into(),
        group_key: Some("gk_abc".into()),
        routing_mode: RoutingMode::Failover,
        auto_from_platform: String::new(),
        request_timeout_secs: 0,
        connect_timeout_secs: 0,
        source_protocol: None,
        max_retries: 2,
        model_mappings: vec![], env_vars: vec![],    }).await.unwrap();

    // 注入 claude_code hooks（基线配置）
    let invoker = gateway::scripts::ScriptInvoker::from_setting(Some("python3"));
    let scripts = generate_hook_scripts(invoker).unwrap();
    let mut config = serde_json::json!({});
    let events = enabled_hook_events(&db).await;
    gateway::hooks::inject_claude_code_hooks(&mut config, &scripts, &events);
    assert!(config.get("hooks").is_some());
    gateway::db::set_setting(&db, SetSettingInput {
        scope: "global".into(), key: "claude_code".into(), value: config,
    }).await.unwrap();

    // 物化到 settings.{group}.json（默认 port 9876）
    let synced = do_sync_group_settings(&db, 9876).await.unwrap();
    assert!(!synced.is_empty());

    // 剥离 hooks
    let mut cfg = gateway::db::get_setting(&db, "global", "claude_code").await.unwrap().unwrap();
    gateway::hooks::remove_claude_code_hooks(&mut cfg);
    assert!(!gateway::hooks::hooks_marker_enabled(&cfg));
}

#[tokio::test]
async fn default_hooks_enabled_get_set() {
    let _h = HomeGuard::new();
    let app = mock_app_with_db().await;
    let db = app.state::<Db>();
    // get：无基线配置 → 回退编译内默认（defaults/settings.json）
    let _ = get_default_hooks_enabled(db.clone()).await.unwrap();
    // 写 marker enabled=true 后再读
    let mut config = serde_json::json!({});
    if let Some(obj) = config.as_object_mut() {
        obj.insert(gateway::hooks::MARKER_HOOKS.into(), serde_json::json!({"enabled": true}));
    }
    gateway::db::set_setting(&db, SetSettingInput {
        scope: "global".into(), key: "claude_code".into(), value: config,
    }).await.unwrap();
    assert!(get_default_hooks_enabled(db.clone()).await.unwrap());
}

#[tokio::test]
async fn codex_notify_inject_remove() {
    let _h = HomeGuard::new();
    let invoker = gateway::scripts::ScriptInvoker::from_setting(Some("python3"));
    let scripts = generate_hook_scripts(invoker).unwrap();

    let mut config = gateway::codex::codex_config_read().unwrap();
    gateway::hooks::inject_codex_notify(&mut config, &scripts.complete);
    assert!(config.get("notify").is_some());
    gateway::codex::codex_config_write(config).unwrap();

    let mut cfg = gateway::codex::codex_config_read().unwrap();
    gateway::hooks::remove_codex_notify(&mut cfg);
    assert!(cfg.get("notify").is_none());
}

#[tokio::test]
async fn unknown_client_errors() {
    assert!(gateway::hooks::HookClient::from_str("bogus").is_err());
    assert!(gateway::hooks::HookClient::from_str("claude_code").is_ok());
    assert!(gateway::hooks::HookClient::from_str("codex").is_ok());
}
