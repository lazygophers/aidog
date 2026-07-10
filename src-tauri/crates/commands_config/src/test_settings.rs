#![cfg(test)]
use super::*;
use aidog_test_util::mock_app_with_db;
use tauri::Manager;

#[tokio::test]
async fn get_delete_list_settings() {
    let app = mock_app_with_db().await;
    let db = app.state::<Db>();

    // not set yet
    assert!(settings_get("scope1".into(), "k1".into(), db.clone()).await.unwrap().is_none());
    assert!(settings_list("scope1".into(), db.clone()).await.unwrap().is_empty());

    // seed via db
    db::set_setting(&db, SetSettingInput {
        scope: "scope1".into(),
        key: "k1".into(),
        value: serde_json::json!({"v": 1}),
    }).await.unwrap();

    assert!(settings_get("scope1".into(), "k1".into(), db.clone()).await.unwrap().is_some());
    assert_eq!(settings_list("scope1".into(), db.clone()).await.unwrap().len(), 1);

    settings_delete("scope1".into(), "k1".into(), db.clone()).await.unwrap();
    assert!(settings_get("scope1".into(), "k1".into(), db.clone()).await.unwrap().is_none());
}
