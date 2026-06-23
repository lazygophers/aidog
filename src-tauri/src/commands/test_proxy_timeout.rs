#![cfg(test)]
use super::*;
use crate::commands::test_harness::mock_app_with_db;
use tauri::Manager;

#[tokio::test]
async fn timeout_roundtrip() {
    let app = mock_app_with_db().await;
    let db = app.state::<Db>();
    let s = proxy_timeout_get(db.clone()).await.unwrap();
    proxy_timeout_set(db.clone(), s).await.unwrap();
    let _ = proxy_timeout_get(db.clone()).await.unwrap();
}
