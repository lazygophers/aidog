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
async fn self_install_writes_both_agents_and_preserves_existing_entries() {
    let home = HomeGuard::new();
    let db = test_db().await;
    // 用户既有条目必须保留（merge 语义：只动 `aidog` 这一条）。
    let claude_json =
        seed_claude_json(home.dir.path(), serde_json::json!({ "user-srv": { "command": "npx", "args": ["-y", "pkg"] } }));

    let report = self_install(&db).await.unwrap();
    assert_eq!(report.name, "aidog");
    assert!(report.command.contains("aidog-kernel"), "command={}", report.command);
    assert_eq!(report.agents, vec!["claude-code".to_string(), "codex".to_string()]);

    // DB 行 + 两个 agent 全启用。
    let row = gateway::mcp::store::get_mcp_server(&db, "aidog").await.unwrap().expect("row exists");
    let enabled = row.enabled_set();
    assert!(enabled.contains(&gateway::mcp::McpAgent::ClaudeCode));
    assert!(enabled.contains(&gateway::mcp::McpAgent::Codex));

    // ~/.claude.json：mcpServers.aidog 存在、env 带 AIDOG_MCP_TOOLS_JSON、用户条目仍在。
    let v: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&claude_json).unwrap()).unwrap();
    let entry = &v["mcpServers"]["aidog"];
    assert_eq!(entry["command"], "aidog-kernel", "entry={entry}");
    assert_eq!(entry["args"], serde_json::json!(["mcp"]));
    let env_json = entry["env"]["AIDOG_MCP_TOOLS_JSON"].as_str().expect("env json");
    let parsed: crate::mcp_tools::McpToolsSettings = serde_json::from_str(env_json).unwrap();
    assert_eq!(parsed, crate::mcp_tools::McpToolsSettings::default());
    assert!(
        v["mcpServers"]["user-srv"]["command"] == "npx",
        "用户既有条目不得被覆盖, v={v}"
    );

    // ~/.codex/config.toml：[mcp_servers.aidog] + 同一 env。
    let codex_toml = home.dir.path().join(".codex/config.toml");
    let content = std::fs::read_to_string(&codex_toml).unwrap();
    assert!(content.contains("[mcp_servers.aidog]"), "content={content}");
    assert!(content.contains("AIDOG_MCP_TOOLS_JSON"), "content={content}");

    // 幂等：再装一次不报错、文件仍合法。
    self_install(&db).await.unwrap();
    let v2: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&claude_json).unwrap()).unwrap();
    assert!(v2["mcpServers"]["user-srv"].is_object(), "二次安装后用户条目仍在");
}

#[tokio::test]
async fn self_install_updates_existing_aidog_entry() {
    // 已有同名条目（此前一键装过）→ 更新而非报错。
    let _home = HomeGuard::new();
    let db = test_db().await;
    self_install(&db).await.unwrap();
    // 第二次直接走 update_server 分支。
    let report = self_install(&db).await.unwrap();
    assert_eq!(report.name, "aidog");
    let row = gateway::mcp::store::get_mcp_server(&db, "aidog").await.unwrap().unwrap();
    assert!(row.enabled_set().contains(&gateway::mcp::McpAgent::Codex));
}
