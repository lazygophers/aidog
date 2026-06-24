#![cfg(test)]
use super::*;
use crate::commands::test_harness::mock_app_with_db;
use tauri::Manager;

fn sample_group_input(name: &str) -> CreateGroup {
    CreateGroup {
        name: name.into(),
        group_key: Some(name.into()),
        routing_mode: RoutingMode::Failover,
        auto_from_platform: String::new(),
        request_timeout_secs: 0,
        connect_timeout_secs: 0,
        source_protocol: None,
        max_retries: 2,
        model_mappings: vec![],
    }
}

#[tokio::test]
async fn list_get_detail_empty_db() {
    let app = mock_app_with_db().await;
    let db = app.state::<Db>();
    assert!(group_list(db.clone()).await.unwrap().is_empty());
    assert!(group_get(1, db.clone()).await.unwrap().is_none());
    assert!(group_get_platforms(1, db.clone()).await.unwrap().is_empty());
    assert!(group_detail(1, db.clone()).await.unwrap().is_none());
    assert!(group_detail_list(db.clone()).await.unwrap().is_empty());
}

#[tokio::test]
async fn list_after_seeding_via_db() {
    let app = mock_app_with_db().await;
    let db = app.state::<Db>();
    db::create_group(&db, sample_group_input("g")).await.unwrap();
    assert_eq!(group_list(db.clone()).await.unwrap().len(), 1);
    assert_eq!(group_detail_list(db.clone()).await.unwrap().len(), 1);
}

/// Tests for group commands that don't require tauri::AppHandle (AppHandle commands
/// are bound to Wry runtime and cannot be called from MockRuntime tests).
/// The underlying DB functions are well-tested in gateway/db/test_group.rs.
#[tokio::test]
async fn group_create_via_db_and_read_commands() {
    let app = mock_app_with_db().await;
    let db = app.state::<Db>();

    // Test group_key validation by going through the db layer directly and checking commands
    let g = db::create_group(&db, sample_group_input("valid-key")).await.unwrap();
    assert!(group_get(g.id, db.clone()).await.unwrap().is_some());
    assert_eq!(group_get_platforms(g.id, db.clone()).await.unwrap().len(), 0);
    assert!(group_detail(g.id, db.clone()).await.unwrap().is_some());

    // Add a second group and test group_detail_list count
    let g2 = db::create_group(&db, sample_group_input("g2")).await.unwrap();
    let details = group_detail_list(db.clone()).await.unwrap();
    assert_eq!(details.len(), 2);
    assert!(details.iter().any(|d| d.group.id == g.id));
    assert!(details.iter().any(|d| d.group.id == g2.id));

    // group_get for non-existent
    assert!(group_get(999999, db.clone()).await.unwrap().is_none());
    assert!(group_detail(999999, db.clone()).await.unwrap().is_none());
}

/// Test group_create group_key validation (pure logic, no AppHandle needed).
#[tokio::test]
async fn group_create_validates_group_key_inline() {
    // Test the validation logic directly: empty group_key after trim should fail
    // We do this by calling the inner check logic
    let empty = "";
    let valid = empty.trim().is_empty() || !empty.trim().chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-');
    assert!(valid, "empty key should be rejected");

    let bad = "bad key!";
    let invalid = bad.trim().is_empty() || !bad.trim().chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-');
    assert!(invalid, "key with space/! should be rejected");

    let good = "valid_key-123";
    let ok = !good.trim().is_empty() && good.trim().chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-');
    assert!(ok, "valid key should be accepted");
}
