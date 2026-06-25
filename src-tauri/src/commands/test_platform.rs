#![cfg(test)]
use super::*;
use crate::commands::test_harness::mock_app_with_db;
use tauri::Manager;

fn sample_create(name: &str, auto_group: Option<bool>, join: Option<Vec<u64>>) -> CreatePlatform {
    CreatePlatform {
        name: name.into(),
        platform_type: Protocol::Anthropic,
        base_url: "https://example.invalid".into(),
        api_key: "k".into(),
        extra: String::new(),
        models: None,
        available_models: None,
        endpoints: None,
        manual_budgets: None,
        auto_group,
        join_group_ids: join, default_level_priority: None,
    }
}

#[tokio::test]
async fn create_list_get_update_delete_flow() {
    let app = mock_app_with_db().await;
    let db = app.state::<crate::gateway::db::Db>();

    // create with auto_group
    let p = platform_create(sample_create("P1", Some(true), None), db.clone()).await.unwrap();
    assert_eq!(p.name, "P1");

    // list (balance_level computed path)
    let list = platform_list(db.clone()).await.unwrap();
    assert_eq!(list.len(), 1);
    assert!(!list[0].balance_level.is_empty());

    // get found + not found
    assert!(platform_get(p.id, db.clone()).await.unwrap().is_some());
    assert!(platform_get(999999, db.clone()).await.unwrap().is_none());

    // update
    let upd = UpdatePlatform {
        id: p.id,
        name: Some("P1-renamed".into()),
        platform_type: None,
        base_url: None,
        api_key: None,
        extra: None,
        models: None,
        available_models: None,
        endpoints: None,
        enabled: None,
        status: None,
        manual_budgets: None,
        join_group_ids: Some(vec![]),
    };
    let p2 = platform_update(upd, db.clone()).await.unwrap();
    assert_eq!(p2.name, "P1-renamed");

    // reorder (single)
    platform_reorder(vec![p.id], db.clone()).await.unwrap();

    // delete
    platform_delete(p.id, db.clone()).await.unwrap();
    assert!(platform_get(p.id, db.clone()).await.unwrap().is_none());
}

#[tokio::test]
async fn create_without_auto_group_and_join_groups() {
    let app = mock_app_with_db().await;
    let db = app.state::<crate::gateway::db::Db>();
    // no auto group + empty join
    let p = platform_create(sample_create("NA", Some(false), Some(vec![])), db.clone()).await.unwrap();
    assert!(p.id > 0);
}

#[tokio::test]
async fn ensure_auto_group_idempotent() {
    let app = mock_app_with_db().await;
    let db = app.state::<crate::gateway::db::Db>();
    // create without auto group, then ensure
    let p = platform_create(sample_create("E1", Some(false), None), db.clone()).await.unwrap();
    platform_ensure_auto_group(p.id, db.clone()).await.unwrap();
    // second call is a no-op (already has auto group)
    platform_ensure_auto_group(p.id, db.clone()).await.unwrap();
    // missing platform errs
    assert!(platform_ensure_auto_group(999999, db.clone()).await.is_err());
}

#[tokio::test]
async fn purge_disabled_returns_result() {
    let app = mock_app_with_db().await;
    let db = app.state::<crate::gateway::db::Db>();
    // no disabled platforms → empty result, global scope
    let res = platform_purge_disabled(None, db.clone()).await.unwrap();
    assert!(res.deleted_ids.is_empty());
}

#[tokio::test]
async fn tray_config_and_today_stats() {
    let app = mock_app_with_db().await;
    let db = app.state::<crate::gateway::db::Db>();
    // default tray config (no config yet)
    let cfg = tray_config_get(db.clone()).await.unwrap();
    let _ = cfg;
    // today stats
    let stats = tray_today_stats(db.clone()).await.unwrap();
    let _ = stats;
}
