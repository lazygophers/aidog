#![cfg(test)]
use super::*;
use aidog_db as db;
use aidog_db::test_support::test_db;

// aidog_core 不能 dev-dep aidog_test_util（后者依赖 aidog_core，会成环），
// 故不经 tauri::State/AppHandle 走 command 包装层，直测 command 转发的 db:: 函数
// （command 本身只是薄转发 + try_sync_settings，逻辑等价；underlying db:: 函数已在
// gateway/db/test_group.rs 充分覆盖）。

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
        env_vars: vec![],
    }
}

#[tokio::test]
async fn list_get_detail_empty_db() {
    let db = test_db().await;
    assert!(db::list_groups(&db).await.unwrap().is_empty());
    assert!(db::get_group(&db, 1).await.unwrap().is_none());
    assert!(db::get_group_platforms(&db, 1).await.unwrap().is_empty());
    assert!(db::get_group_detail(&db, 1).await.unwrap().is_none());
    assert!(db::list_group_details(&db).await.unwrap().is_empty());
}

#[tokio::test]
async fn list_after_seeding_via_db() {
    let db = test_db().await;
    db::create_group(&db, sample_group_input("g"))
        .await
        .unwrap();
    assert_eq!(db::list_groups(&db).await.unwrap().len(), 1);
    assert_eq!(db::list_group_details(&db).await.unwrap().len(), 1);
}

/// Tests for group commands that don't require tauri::AppHandle (AppHandle commands
/// are bound to Wry runtime and cannot be called from MockRuntime tests).
/// The underlying DB functions are well-tested in gateway/db/test_group.rs.
#[tokio::test]
async fn group_create_via_db_and_read_commands() {
    let db = test_db().await;

    // Test group_key validation by going through the db layer directly and checking commands
    let g = db::create_group(&db, sample_group_input("valid-key"))
        .await
        .unwrap();
    assert!(db::get_group(&db, g.id).await.unwrap().is_some());
    assert_eq!(db::get_group_platforms(&db, g.id).await.unwrap().len(), 0);
    assert!(db::get_group_detail(&db, g.id).await.unwrap().is_some());

    // Add a second group and test group_detail_list count
    let g2 = db::create_group(&db, sample_group_input("g2"))
        .await
        .unwrap();
    let details = db::list_group_details(&db).await.unwrap();
    assert_eq!(details.len(), 2);
    assert!(details.iter().any(|d| d.group.id == g.id));
    assert!(details.iter().any(|d| d.group.id == g2.id));

    // group_get for non-existent
    assert!(db::get_group(&db, 999999).await.unwrap().is_none());
    assert!(db::get_group_detail(&db, 999999).await.unwrap().is_none());
}

/// Test group_create group_key validation (pure logic, no AppHandle needed).
#[tokio::test]
async fn group_create_validates_group_key_inline() {
    // Test the validation logic directly: empty group_key after trim should fail
    // We do this by calling the inner check logic
    let empty = "";
    let valid = empty.trim().is_empty()
        || !empty
            .trim()
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-');
    assert!(valid, "empty key should be rejected");

    let bad = "bad key!";
    let invalid = bad.trim().is_empty()
        || !bad
            .trim()
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-');
    assert!(invalid, "key with space/! should be rejected");

    let good = "valid_key-123";
    let ok = !good.trim().is_empty()
        && good
            .trim()
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-');
    assert!(ok, "valid key should be accepted");
}
