#![cfg(test)]
use super::*;
use crate::commands::test_harness::mock_app_with_db;
use tauri::Manager;

#[tokio::test]
async fn config_roundtrip_and_today() {
    let app = mock_app_with_db().await;
    let db = app.state::<Db>();
    let cfg = popover_config_get(db.clone()).await.unwrap();
    popover_config_set(cfg, db.clone()).await.unwrap();
    let _ = popover_platform_today(db.clone()).await.unwrap();
}
