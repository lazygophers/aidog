//! domain 层全流程测试：用 HOME 隔离（指向 tempdir），覆盖 scan/import/set_agent/
//! add/delete/resync/update 的 FS 写入路径。HOME 进程全局 → 串行锁。
// ENV_LOCK 必须跨 await 持有：HOME/CODEX_HOME 是进程全局状态，整个异步测试期间都需独占，
// 提前 drop 会让并行测试线程交叉改 HOME → 串话。故有意保留 std Mutex 守卫跨 await。
#![allow(clippy::await_holding_lock)]
use super::*;
use crate::gateway::db::test_support::{test_db, HomeGuard};
use std::collections::BTreeMap;

fn payload(name: &str) -> McpUpdatePayload {
    let mut env = BTreeMap::new();
    env.insert("API_KEY".to_string(), "secret123".to_string());
    McpUpdatePayload {
        name: name.into(),
        transport: "stdio".into(),
        command: "npx".into(),
        args: vec!["-y".into(), "pkg".into()],
        env,
        url: String::new(),
        headers: Default::default(),
    }
}

#[tokio::test]
async fn add_validation_and_lifecycle() {
    let _home = HomeGuard::new();
    let db = test_db().await;

    // 空名拒绝
    assert!(add_server(&db, payload("  ")).await.is_err());
    // 正常添加（enabled_agents 空 → 不写 FS）
    let info = add_server(&db, payload("srv")).await.unwrap();
    assert_eq!(info.name, "srv");
    // 重名拒绝
    assert!(add_server(&db, payload("srv")).await.is_err());
}

#[tokio::test]
async fn set_agent_writes_and_removes_config() {
    let home = HomeGuard::new();
    let db = test_db().await;
    add_server(&db, payload("srv")).await.unwrap();

    // 启用 claude-code → 写 ~/.claude.json
    set_agent_enabled(&db, "srv", McpAgent::ClaudeCode, true)
        .await
        .unwrap();
    let claude_json = home.dir.path().join(".claude.json");
    let content = std::fs::read_to_string(&claude_json).unwrap();
    assert!(content.contains("srv"));
    assert!(content.contains("secret123")); // DB 原值写入，非脱敏

    // 启用 codex（stdio 支持）→ 写 config.toml
    set_agent_enabled(&db, "srv", McpAgent::Codex, true)
        .await
        .unwrap();
    let toml_path = home.dir.path().join(".codex/config.toml");
    assert!(std::fs::read_to_string(&toml_path).unwrap().contains("srv"));

    // 禁用 claude → 从 ~/.claude.json 移除
    set_agent_enabled(&db, "srv", McpAgent::ClaudeCode, false)
        .await
        .unwrap();
    let content2 = std::fs::read_to_string(&claude_json).unwrap();
    assert!(!content2.contains("\"srv\""));

    // 未知 server 报错
    assert!(
        set_agent_enabled(&db, "nope", McpAgent::ClaudeCode, true)
            .await
            .is_err()
    );
}

#[tokio::test]
async fn set_agent_transport_incompatible_errs() {
    let _home = HomeGuard::new();
    let db = test_db().await;
    // http transport — codex 仅支持 stdio
    let mut p = payload("httpsrv");
    p.transport = "http".into();
    p.command = String::new();
    p.url = "https://mcp.example.com".into();
    add_server(&db, p).await.unwrap();
    assert!(
        set_agent_enabled(&db, "httpsrv", McpAgent::Codex, true)
            .await
            .is_err()
    );
}

#[tokio::test]
async fn delete_removes_from_agents_and_db() {
    let home = HomeGuard::new();
    let db = test_db().await;
    add_server(&db, payload("srv")).await.unwrap();
    set_agent_enabled(&db, "srv", McpAgent::ClaudeCode, true)
        .await
        .unwrap();

    delete_server(&db, "srv").await.unwrap();
    let claude_json = home.dir.path().join(".claude.json");
    let content = std::fs::read_to_string(&claude_json).unwrap();
    assert!(!content.contains("\"srv\""));
    assert!(db::get_mcp_server(&db, "srv").await.unwrap().is_none());
}

#[tokio::test]
async fn scan_finds_imported_and_unimported() {
    let _home = HomeGuard::new();
    let db = test_db().await;

    // 在 claude 配置写一个 server（直接经 backend），DB 无 → already_imported=false
    add_server(&db, payload("known")).await.unwrap();
    set_agent_enabled(&db, "known", McpAgent::ClaudeCode, true)
        .await
        .unwrap();

    let items = scan_all(&db).await.unwrap();
    let known = items.iter().find(|i| i.name == "known").unwrap();
    assert!(known.already_imported);
    assert!(known.found_in_agents.contains(&"claude-code".to_string()));
    // env 脱敏
    assert_eq!(known.env.get("API_KEY").unwrap(), "***");
}

#[tokio::test]
async fn import_takes_source_real_values() {
    let _home = HomeGuard::new();
    let db = test_db().await;

    // 先在 claude 配置植入真实 server（经独立 db）
    let seed = test_db().await;
    add_server(&seed, payload("imp")).await.unwrap();
    set_agent_enabled(&seed, "imp", McpAgent::ClaudeCode, true)
        .await
        .unwrap();

    // 前端传脱敏 env，import 应从 source agent 取真实值
    let mut masked_env = BTreeMap::new();
    masked_env.insert("API_KEY".to_string(), "***".to_string());
    let report = import_items(
        &db,
        vec![McpImportPayload {
            name: "imp".into(),
            source_agent: "claude-code".into(),
            transport: "stdio".into(),
            command: "npx".into(),
            args: vec![],
            env: masked_env,
            url: String::new(),
            headers: Default::default(),
        }],
    )
    .await
    .unwrap();
    assert_eq!(report.imported, vec!["imp".to_string()]);
    let row = db::get_mcp_server(&db, "imp").await.unwrap().unwrap();
    assert!(row.env_json.contains("secret123")); // 真实值，非 ***

    // 未知 source agent → skipped
    let r2 = import_items(
        &db,
        vec![McpImportPayload {
            name: "x".into(),
            source_agent: "bogus".into(),
            transport: "stdio".into(),
            command: "c".into(),
            args: vec![],
            env: Default::default(),
            url: String::new(),
            headers: Default::default(),
        }],
    )
    .await
    .unwrap();
    assert_eq!(r2.skipped, vec!["x".to_string()]);
}

#[tokio::test]
async fn resync_rewrites_enabled_agents() {
    let home = HomeGuard::new();
    let db = test_db().await;
    add_server(&db, payload("srv")).await.unwrap();
    set_agent_enabled(&db, "srv", McpAgent::ClaudeCode, true)
        .await
        .unwrap();

    // 污染：清空 claude.json
    let claude_json = home.dir.path().join(".claude.json");
    std::fs::write(&claude_json, "{}").unwrap();

    let count = resync_all(&db).await.unwrap();
    assert!(count >= 1);
    let content = std::fs::read_to_string(&claude_json).unwrap();
    assert!(content.contains("srv")); // 重写恢复
}

#[tokio::test]
async fn update_rename_and_masked_merge() {
    let home = HomeGuard::new();
    let db = test_db().await;
    add_server(&db, payload("old")).await.unwrap();
    set_agent_enabled(&db, "old", McpAgent::ClaudeCode, true)
        .await
        .unwrap();

    // 改名 + env 用 *** 占位（应 merge 回旧明文）
    let mut env = BTreeMap::new();
    env.insert("API_KEY".to_string(), "***".to_string());
    let p = McpUpdatePayload {
        name: "new".into(),
        transport: "stdio".into(),
        command: "npx".into(),
        args: vec!["-y".into()],
        env,
        url: String::new(),
        headers: Default::default(),
    };
    let info = update_server(&db, "old", p).await.unwrap();
    assert_eq!(info.name, "new");

    // 旧名 DB 删除，旧名 agent 配置移除，新名写入
    assert!(db::get_mcp_server(&db, "old").await.unwrap().is_none());
    let row = db::get_mcp_server(&db, "new").await.unwrap().unwrap();
    assert!(row.env_json.contains("secret123")); // *** merge 回旧明文

    let claude_json = home.dir.path().join(".claude.json");
    let content = std::fs::read_to_string(&claude_json).unwrap();
    assert!(content.contains("new"));
    assert!(!content.contains("\"old\""));
}

#[tokio::test]
async fn update_transport_switch_drops_unsupported_agent() {
    let home = HomeGuard::new();
    let db = test_db().await;
    add_server(&db, payload("srv")).await.unwrap();
    // 启用 codex（stdio ok）
    set_agent_enabled(&db, "srv", McpAgent::Codex, true)
        .await
        .unwrap();

    // 切到 http → codex 不支持 → drop
    let p = McpUpdatePayload {
        name: "srv".into(),
        transport: "http".into(),
        command: String::new(),
        args: vec![],
        env: Default::default(),
        url: "https://mcp.example.com".into(),
        headers: Default::default(),
    };
    update_server(&db, "srv", p).await.unwrap();
    let row = db::get_mcp_server(&db, "srv").await.unwrap().unwrap();
    assert!(!row.enabled_agents.contains("codex"));

    // codex config 应已移除
    let toml_path = home.dir.path().join(".codex/config.toml");
    if let Ok(c) = std::fs::read_to_string(&toml_path) {
        assert!(!c.contains("srv"));
    }

    // 未知 old name → err
    let bad = McpUpdatePayload {
        name: "z".into(),
        transport: "stdio".into(),
        command: "c".into(),
        args: vec![],
        env: Default::default(),
        url: String::new(),
        headers: Default::default(),
    };
    assert!(update_server(&db, "ghost", bad).await.is_err());
}

// ─── parse_pasted_json（纯解析，无 FS）───
#[test]
fn parse_pasted_with_wrapper() {
    let json = r#"{"mcpServers":{"fs":{"command":"npx","args":["-y","pkg"],"env":{"K":"v"}}}}"#;
    let out = parse_pasted_json(json).unwrap();
    assert_eq!(out.len(), 1);
    let (name, cfg) = &out[0];
    assert_eq!(name, "fs");
    assert_eq!(cfg.command, "npx");
    assert_eq!(cfg.args, vec!["-y", "pkg"]);
    assert_eq!(cfg.env.get("K").map(String::as_str), Some("v"));
}

#[test]
fn parse_pasted_bare_map_and_http() {
    let json = r#"{"remote":{"type":"http","url":"https://x.dev","headers":{"A":"b"}}}"#;
    let out = parse_pasted_json(json).unwrap();
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].0, "remote");
    assert_eq!(out[0].1.url, "https://x.dev");
    assert_eq!(out[0].1.transport.as_str(), "http");
}

#[test]
fn parse_pasted_invalid() {
    assert!(parse_pasted_json("not json").is_err());
    assert!(parse_pasted_json("[]").is_err()); // 非 object
    assert!(parse_pasted_json(r#"{"command":"npx"}"#).is_err()); // 无名 entry → 空
}
