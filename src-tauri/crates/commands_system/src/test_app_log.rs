#![cfg(test)]
use super::*;
use aidog_test_util::mock_app_with_db;
use tauri::Manager;

#[tokio::test]
async fn app_log_settings_roundtrip() {
    let app = mock_app_with_db().await;
    let db = app.state::<Db>();
    let s = app_log_settings_get(db.clone()).await.unwrap();
    app_log_settings_set(s, db.clone()).await.unwrap();
    let _ = app_log_settings_get(db.clone()).await.unwrap();
}

#[tokio::test]
async fn load_settings_helper() {
    let app = mock_app_with_db().await;
    let db = app.state::<Db>();
    let _ = load_app_log_settings_from_db(&db).await;
}
