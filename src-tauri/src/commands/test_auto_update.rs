#![cfg(test)]
use super::*;
use crate::commands::test_harness::mock_app_with_db;
use tauri::Manager;

#[tokio::test]
async fn defaults_true_when_unset() {
    let app = mock_app_with_db().await;
    let db = app.state::<Db>();
    let v = get_auto_update_enabled(db.clone()).await.unwrap();
    assert!(v, "auto_update_enabled defaults true when missing");
}

#[tokio::test]
async fn roundtrip_persists() {
    let app = mock_app_with_db().await;
    let db = app.state::<Db>();
    set_auto_update_enabled(false, db.clone()).await.unwrap();
    assert!(!get_auto_update_enabled(db.clone()).await.unwrap(), "false persists");
    set_auto_update_enabled(true, db.clone()).await.unwrap();
    assert!(get_auto_update_enabled(db.clone()).await.unwrap(), "true persists");
}

#[tokio::test]
async fn corrupt_value_falls_back_to_true() {
    let app = mock_app_with_db().await;
    let db = app.state::<Db>();
    // 直接写非 bool JSON，load 路径应兜底 true
    db::set_setting(&db, SetSettingInput {
        scope: "app".into(),
        key: "auto_update_enabled".into(),
        value: serde_json::Value::String("garbage".into()),
    }).await.unwrap();
    assert!(get_auto_update_enabled(db.clone()).await.unwrap(), "non-bool falls back to true");
}
