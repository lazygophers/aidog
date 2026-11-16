//! 插件管理单元测试（随 plugins.rs 1:1）。全部落 tempdir，不碰真实 ~/.claude、
//! ~/.claude/plugins、~/.codex，不调用真实 codex CLI。
use super::*;
use serde_json::json;

#[test]
fn claude_installed_path_under_guarded_home() {
    let g = aidog_db::test_support::HomeGuard::new();
    let p = claude_installed_path().unwrap();
    assert_eq!(p, g.home().join(".claude/plugins/installed_plugins.json"));
}

#[test]
fn claude_views_defaults_on_and_honors_explicit_false() {
    let installed = json!({
        "a@mkt": [{"installPath": "/cache/a", "version": "1.0.0", "lastUpdated": "2026-01-01T00:00:00.000Z"}],
        "b@mkt": []
    });
    // a 显式 false；b 缺键 → 缺省启用、未配置
    let cfg = json!({"enabledPlugins": {"a@mkt": false}, "env": {"X": "1"}});
    let views = claude_views(&installed, &cfg);
    assert_eq!(views.len(), 2, "got: {views:?}");
    let a = views.iter().find(|v| v.id == "a@mkt").unwrap();
    assert_eq!(a.version, "1.0.0");
    assert_eq!(a.install_path, "/cache/a");
    assert_eq!(a.last_updated, "2026-01-01T00:00:00.000Z");
    assert!(!a.enabled);
    assert!(a.configured);
    let b = views.iter().find(|v| v.id == "b@mkt").unwrap();
    assert!(b.enabled);
    assert!(!b.configured);
    assert_eq!(b.version, ""); // 空 installs 数组 → 字段缺省空串
}

#[test]
fn apply_toggle_creates_map_keeps_siblings_and_overrides() {
    let mut cfg = json!({"env": {"X": "1"}});
    apply_toggle(&mut cfg, "a@mkt", true).unwrap();
    assert_eq!(cfg["enabledPlugins"]["a@mkt"], json!(true));
    assert_eq!(cfg["env"]["X"], json!("1"));
    apply_toggle(&mut cfg, "a@mkt", false).unwrap();
    assert_eq!(cfg["enabledPlugins"]["a@mkt"], json!(false));
    assert!(apply_toggle(&mut cfg, "  ", true).is_err());
    // enabledPlugins 被外部写成非对象时重建
    let mut bad = json!({"enabledPlugins": "junk"});
    apply_toggle(&mut bad, "a@mkt", true).unwrap();
    assert_eq!(bad["enabledPlugins"]["a@mkt"], json!(true));
}

/// DB 空（或显式 `{}`）时回落内置 oss 预设：插件开关整份写回 DB，回落保证预设既有键
/// （env / enabledPlugins 等）不因一次开关操作被 `{enabledPlugins:...}` 小对象覆盖丢失。
#[tokio::test]
async fn load_claude_raw_falls_back_to_preset_so_toggle_cannot_wipe_it() {
    let db = aidog_db::test_support::test_db().await;
    let mut cfg = load_claude_raw(&db).await;
    assert!(
        cfg.get("env").is_some(),
        "empty DB must fall back to bundled oss preset, got: {cfg}"
    );
    apply_toggle(&mut cfg, "a@mkt", false).unwrap();
    aidog_db::set_setting(
        &db,
        aidog_db::models::SetSettingInput {
            scope: "global".to_string(),
            key: "claude_code".to_string(),
            value: cfg,
        },
    )
    .await
    .unwrap();
    let stored = aidog_db::get_setting(&db, "global", "claude_code")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        stored["enabledPlugins"]["a@mkt"],
        json!(false),
        "toggle must be recorded"
    );
    assert!(
        stored.get("env").is_some(),
        "preset keys must survive a plugin toggle, got: {stored}"
    );
}

#[test]
fn parse_codex_list_matches_cli_contract() {
    // 本机 codex-cli 0.168.0 `codex plugin list --json` 实测输出（隔离 HOME），
    // 前置 stderr WARNING 行证明容错。
    let raw = concat!(
        "WARNING: proceeding, even though we could not create PATH aliases: Refusing to create helper binaries under temporary dir\n",
        r#"{
  "installed": [
    {
      "pluginId": "hello@testmkt",
      "name": "hello",
      "marketplaceName": "testmkt",
      "version": "0.1.0",
      "installed": true,
      "enabled": true,
      "source": {"source": "local", "path": "/tmp/mkt/hello"},
      "marketplaceSource": {"sourceType": "local", "source": "/tmp/mkt"},
      "installPolicy": "AVAILABLE",
      "authPolicy": "ON_INSTALL"
    }
  ],
  "available": []
}"#,
    );
    let list = parse_codex_list(raw).unwrap();
    assert_eq!(list.len(), 1, "got: {list:?}");
    let p = &list[0];
    assert_eq!(p.plugin_id, "hello@testmkt");
    assert_eq!(p.name, "hello");
    assert_eq!(p.marketplace_name, "testmkt");
    assert_eq!(p.version, "0.1.0");
    assert!(p.installed);
    assert!(p.enabled);

    let empty = parse_codex_list("{\n  \"installed\": [],\n  \"available\": []\n}").unwrap();
    assert!(empty.is_empty());
}

#[test]
fn parse_codex_marketplaces_matches_cli_contract() {
    let raw = concat!(
        "WARNING: proceeding\n",
        r#"{
  "marketplaces": [
    {
      "name": "testmkt",
      "root": "/tmp/mkt",
      "marketplaceSource": {"sourceType": "local", "source": "/tmp/mkt"}
    }
  ]
}"#,
    );
    let list = parse_codex_marketplaces(raw).unwrap();
    assert_eq!(list.len(), 1, "got: {list:?}");
    assert_eq!(list[0].name, "testmkt");
    assert_eq!(list[0].source, "/tmp/mkt");
    assert_eq!(list[0].root, "/tmp/mkt");
}
