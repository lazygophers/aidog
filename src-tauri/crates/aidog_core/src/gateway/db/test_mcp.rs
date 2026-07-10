#![cfg(test)]
use super::test_support::test_db;
use super::*;
use crate::gateway::mcp::McpServerRow;

fn row(name: &str) -> McpServerRow {
    McpServerRow {
        id: 0,
        name: name.into(),
        transport: "stdio".into(),
        command: "npx".into(),
        args_json: "[\"-y\",\"pkg\"]".into(),
        env_json: "{}".into(),
        url: String::new(),
        headers_json: "{}".into(),
        enabled_agents: "claude_code".into(),
        created_at: now(),
        updated_at: now(),
    }
}

#[tokio::test]
async fn mcp_crud_lifecycle() {
    let db = test_db().await;

    // empty
    assert!(list_mcp_servers(&db).await.unwrap().is_empty());
    assert!(get_mcp_server(&db, "x").await.unwrap().is_none());
    assert!(list_mcp_server_names(&db).await.unwrap().is_empty());

    // insert
    upsert_mcp_server(&db, &row("srv1")).await.unwrap();
    upsert_mcp_server(&db, &row("srv2")).await.unwrap();
    assert_eq!(list_mcp_servers(&db).await.unwrap().len(), 2);
    assert_eq!(list_mcp_server_names(&db).await.unwrap().len(), 2);

    // get
    let got = get_mcp_server(&db, "srv1").await.unwrap().unwrap();
    assert_eq!(got.command, "npx");

    // update (conflict path)
    let mut updated = row("srv1");
    updated.command = "uvx".into();
    upsert_mcp_server(&db, &updated).await.unwrap();
    assert_eq!(get_mcp_server(&db, "srv1").await.unwrap().unwrap().command, "uvx");

    // set enabled agents
    set_mcp_server_enabled_agents(&db, "srv1", "claude_code,codex").await.unwrap();
    assert_eq!(get_mcp_server(&db, "srv1").await.unwrap().unwrap().enabled_agents, "claude_code,codex");

    // delete
    delete_mcp_server(&db, "srv1").await.unwrap();
    assert!(get_mcp_server(&db, "srv1").await.unwrap().is_none());
    assert_eq!(list_mcp_servers(&db).await.unwrap().len(), 1);
}
