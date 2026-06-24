#![cfg(test)]
use super::*;
use crate::commands::test_harness::mock_app_with_db;
use tauri::Manager;

#[tokio::test]
async fn settings_roundtrip() {
    let app = mock_app_with_db().await;
    let db = app.state::<Db>();
    let s = scheduling_settings_get(db.clone()).await.unwrap();
    scheduling_settings_set(db.clone(), s).await.unwrap();
    let s2 = scheduling_settings_get(db.clone()).await.unwrap();
    let _ = s2;
}
