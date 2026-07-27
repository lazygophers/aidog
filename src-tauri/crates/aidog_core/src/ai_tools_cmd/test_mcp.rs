#![cfg(test)]
use super::*;
use crate::gateway::db::test_support::test_db;

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

    let rows = gateway::db::list_mcp_servers(&db).await.unwrap();
    assert!(rows.is_empty());

    let info = gateway::mcp::add_server(&db, payload("srv")).await.unwrap();
    let _ = info;
    assert_eq!(gateway::db::list_mcp_servers(&db).await.unwrap().len(), 1);

    // duplicate add errs
    assert!(gateway::mcp::add_server(&db, payload("srv")).await.is_err());
    // empty name errs
    assert!(gateway::mcp::add_server(&db, payload("  ")).await.is_err());

    // delete (no enabled agents → no FS writes)
    gateway::mcp::delete_server(&db, "srv").await.unwrap();
    assert!(gateway::db::list_mcp_servers(&db).await.unwrap().is_empty());
}

#[tokio::test]
async fn set_agent_unknown_slug_errs() {
    let db = test_db().await;
    gateway::mcp::add_server(&db, payload("srv")).await.unwrap();
    assert!(gateway::mcp::McpAgent::from_slug("unknown_agent").is_none());
}
