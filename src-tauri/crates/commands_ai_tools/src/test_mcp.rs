#![cfg(test)]
use super::*;
use aidog_test_util::mock_app_with_db;
use tauri::Manager;

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
    let app = mock_app_with_db().await;
    let db = app.state::<Db>();

    assert!(mcp_list(db.clone()).await.unwrap().is_empty());

    let info = mcp_add(db.clone(), payload("srv")).await.unwrap();
    let _ = info;
    assert_eq!(mcp_list(db.clone()).await.unwrap().len(), 1);

    // duplicate add errs
    assert!(mcp_add(db.clone(), payload("srv")).await.is_err());
    // empty name errs
    assert!(mcp_add(db.clone(), payload("  ")).await.is_err());

    // delete (no enabled agents → no FS writes)
    mcp_delete(db.clone(), "srv".into()).await.unwrap();
    assert!(mcp_list(db.clone()).await.unwrap().is_empty());
}

#[tokio::test]
async fn set_agent_unknown_slug_errs() {
    let app = mock_app_with_db().await;
    let db = app.state::<Db>();
    mcp_add(db.clone(), payload("srv")).await.unwrap();
    assert!(mcp_set_agent(db.clone(), "srv".into(), "unknown_agent".into(), true).await.is_err());
}
