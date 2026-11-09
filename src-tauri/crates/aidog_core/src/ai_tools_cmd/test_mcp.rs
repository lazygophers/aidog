#![cfg(test)]
use super::*;
use aidog_db::test_support::{HomeGuard, test_db};

/// aidog_core 不能 dev-dep aidog_test_util（后者依赖 aidog_core，会成环），
/// 故不经 tauri::State/AppHandle 走 command 包装层，直测 command 转发的 gateway:: 函数
/// （command 本身只是薄转发 + tracing，逻辑等价）。
fn payload(name: &str) -> gateway::mcp::McpUpdatePayload {
    gateway::mcp::McpUpdatePayload {
        name: name.into(),
        transport: "stdio".into(),
        command: "npx".into(),
        args: vec!["-y".into(), "pkg".into()],
        env: Default::default(),
        url: String::new(),
        headers: Default::default(),
    }
}

#[tokio::test]
async fn list_add_delete() {
    let db = test_db().await;

    let rows = aidog_mcp::store::list_mcp_servers(&db).await.unwrap();
    assert!(rows.is_empty());

    let info = gateway::mcp::add_server(&db, payload("srv")).await.unwrap();
    let _ = info;
    assert_eq!(
        aidog_mcp::store::list_mcp_servers(&db).await.unwrap().len(),
        1
    );

    // duplicate add errs
    assert!(gateway::mcp::add_server(&db, payload("srv")).await.is_err());
    // empty name errs
    assert!(gateway::mcp::add_server(&db, payload("  ")).await.is_err());

    // delete (no enabled agents → no FS writes)
    gateway::mcp::delete_server(&db, "srv").await.unwrap();
    assert!(
        aidog_mcp::store::list_mcp_servers(&db)
            .await
            .unwrap()
            .is_empty()
    );
}

#[tokio::test]
async fn set_agent_unknown_slug_errs() {
    let db = test_db().await;
    gateway::mcp::add_server(&db, payload("srv")).await.unwrap();
    assert!(gateway::mcp::McpAgent::from_slug("unknown_agent").is_none());
}

/// 预置一份带既有条目的 ~/.claude.json，返回文件路径。
fn seed_claude_json(home: &std::path::Path, extra: serde_json::Value) -> std::path::PathBuf {
    let path = home.join(".claude.json");
    std::fs::write(
        &path,
        serde_json::to_string_pretty(&serde_json::json!({ "mcpServers": extra })).unwrap(),
    )
    .unwrap();
    path
}

#[tokio::test]
async fn self_install_writes_http_entry_for_both_agents_and_replaces_old_stdio() {
    let home = HomeGuard::new();
    let db = test_db().await;
    // 用户既有条目必须保留（merge 语义：只动 `aidog` 这一条）。
    let claude_json =
        seed_claude_json(home.dir.path(), serde_json::json!({ "user-srv": { "command": "npx", "args": ["-y", "pkg"] } }));

    // 升级前遗留：同名 stdio 条目（DB 行 + 两个 agent 配置文件）。
    let stdio = gateway::mcp::McpUpdatePayload {
        name: "aidog".into(),
        transport: "stdio".into(),
        command: "aidog-kernel".into(),
        args: vec!["mcp".into()],
        env: std::collections::BTreeMap::from([("AIDOG_MCP_TOOLS_JSON".into(), "{}".into())]),
        url: String::new(),
        headers: Default::default(),
    };
    gateway::mcp::add_server(&db, stdio).await.unwrap();
    gateway::mcp::set_agent_enabled(&db, "aidog", gateway::mcp::McpAgent::ClaudeCode, true).await.unwrap();
    gateway::mcp::set_agent_enabled(&db, "aidog", gateway::mcp::McpAgent::Codex, true).await.unwrap();

    // 默认分组决定 Bearer。
    let g1 = aidog_db::create_group(&db, aidog_db::test_support::sample_group("g1", vec![])).await.unwrap();
    let g2 = aidog_db::create_group(&db, aidog_db::test_support::sample_group("g2", vec![])).await.unwrap();
    aidog_db::set_default_group(&db, Some(g2.id)).await.unwrap();
    let _ = g1;

    let report = self_install(&db).await.unwrap();
    assert_eq!(report.name, "aidog");
    assert_eq!(report.url, "http://127.0.0.1:9890/mcp");
    assert_eq!(report.agents, vec!["claude-code".to_string(), "codex".to_string()]);

    // DB 行：http，无 command/env。
    let row = gateway::mcp::store::get_mcp_server(&db, "aidog").await.unwrap().expect("row exists");
    assert_eq!(row.transport, "http");
    assert!(row.command.is_empty());
    assert!(!row.env_json.contains("AIDOG_MCP_TOOLS_JSON"));
    let enabled = row.enabled_set();
    assert!(enabled.contains(&gateway::mcp::McpAgent::ClaudeCode));
    assert!(enabled.contains(&gateway::mcp::McpAgent::Codex));

    // ~/.claude.json：type http + url + Authorization，旧 stdio 字段消失，用户条目仍在。
    let v: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&claude_json).unwrap()).unwrap();
    let entry = &v["mcpServers"]["aidog"];
    assert_eq!(entry["type"], "http", "entry={entry}");
    assert_eq!(entry["url"], "http://127.0.0.1:9890/mcp");
    assert_eq!(entry["headers"]["Authorization"], format!("Bearer {}", g2.group_key));
    assert!(entry.get("command").is_none() && entry.get("env").is_none(), "旧 stdio 字段须被替换掉: {entry}");
    assert!(v["mcpServers"]["user-srv"]["command"] == "npx", "用户既有条目不得被覆盖, v={v}");

    // ~/.codex/config.toml：[mcp_servers.aidog] url + http_headers，无 command。
    let content = std::fs::read_to_string(home.dir.path().join(".codex/config.toml")).unwrap();
    let t: toml::Value = toml::from_str(&content).unwrap();
    let c = &t["mcp_servers"]["aidog"];
    assert_eq!(c["url"].as_str(), Some("http://127.0.0.1:9890/mcp"), "content={content}");
    assert_eq!(c["http_headers"]["Authorization"].as_str(), Some(format!("Bearer {}", g2.group_key).as_str()));
    assert!(c.get("command").is_none(), "content={content}");

    // 幂等：再装一次不报错、文件仍合法。
    self_install(&db).await.unwrap();
    let v2: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&claude_json).unwrap()).unwrap();
    assert!(v2["mcpServers"]["user-srv"].is_object(), "二次安装后用户条目仍在");
}

#[tokio::test]
async fn self_install_without_any_group_is_explicit_error() {
    let _home = HomeGuard::new();
    let db = test_db().await;
    let err = self_install(&db).await.unwrap_err();
    assert!(err.contains("分组"), "err={err}");
}

#[tokio::test]
async fn self_install_falls_back_to_first_group_when_no_default() {
    let home = HomeGuard::new();
    let db = test_db().await;
    let g = aidog_db::create_group(&db, aidog_db::test_support::sample_group("only", vec![])).await.unwrap();
    self_install(&db).await.unwrap();
    let v: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(home.dir.path().join(".claude.json")).unwrap()).unwrap();
    assert_eq!(v["mcpServers"]["aidog"]["headers"]["Authorization"], format!("Bearer {}", g.group_key));
}

#[tokio::test]
async fn self_install_updates_existing_aidog_entry() {
    // 已有同名条目（此前一键装过）→ 更新而非报错。
    let _home = HomeGuard::new();
    let db = test_db().await;
    aidog_db::create_group(&db, aidog_db::test_support::sample_group("g", vec![])).await.unwrap();
    self_install(&db).await.unwrap();
    // 第二次直接走 update_server 分支。
    let report = self_install(&db).await.unwrap();
    assert_eq!(report.name, "aidog");
    let row = gateway::mcp::store::get_mcp_server(&db, "aidog").await.unwrap().unwrap();
    assert!(row.enabled_set().contains(&gateway::mcp::McpAgent::Codex));
}
