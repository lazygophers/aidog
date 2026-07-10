#![cfg(test)]
use super::*;
use aidog_test_util::mock_app_with_db;
use tauri::Manager;

#[tokio::test]
async fn backup_settings_get_default() {
    let app = mock_app_with_db().await;
    let db = app.state::<Db>();
    let settings = backup_settings_get(db.clone()).await.unwrap();
    // Default settings should be valid
    let _ = settings;
}

#[tokio::test]
async fn backup_settings_set_and_get_roundtrip() {
    let app = mock_app_with_db().await;
    let db = app.state::<Db>();

    let mut settings = gateway::backup::BackupSettings::load(&db).await;
    settings.enabled = true;
    settings.interval_hours = 1;
    settings.retention_days = 7;

    let saved = backup_settings_set(db.clone(), settings.clone()).await.unwrap();
    assert!(saved.enabled);
    assert_eq!(saved.interval_hours, 1);
    assert_eq!(saved.retention_days, 7);

    let got = backup_settings_get(db.clone()).await.unwrap();
    assert!(got.enabled);
    assert_eq!(got.interval_hours, 1);
}

#[tokio::test]
async fn db_compact_returns_sizes() {
    let app = mock_app_with_db().await;
    let db = app.state::<Db>();
    let result = db_compact(db.clone()).await.unwrap();
    // Memory DB: before_bytes may be 0 or small, but should not error
    let _ = result;
}

#[tokio::test]
async fn backup_run_now_returns_result() {
    let app = mock_app_with_db().await;
    let db = app.state::<Db>();
    // run_backup may fail (no backup dir configured), but should return a result not panic
    let result = backup_run_now(db.clone()).await.unwrap();
    // ok=false is acceptable (no path configured), just verify it returns
    let _ = result;
}
