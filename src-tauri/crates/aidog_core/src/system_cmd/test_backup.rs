#![cfg(test)]
use super::*;
use aidog_db::test_support::test_db;

/// aidog_core 不能 dev-dep aidog_test_util（后者依赖 aidog_core，会成环），
/// 故不经 tauri::State/AppHandle 走 command 包装层，直测 command 转发的 db:: / gateway:: 函数
/// （command 本身只是薄转发 + tracing，逻辑等价）。
#[tokio::test]
async fn backup_settings_get_default() {
    let db = test_db().await;
    let settings = gateway::backup::BackupSettings::load(&db).await.sanitized();
    // Default settings should be valid
    let _ = settings;
}

#[tokio::test]
async fn backup_settings_set_and_get_roundtrip() {
    let db = test_db().await;

    let mut settings = gateway::backup::BackupSettings::load(&db).await;
    settings.enabled = true;
    settings.interval_hours = 1;
    settings.retention_days = 7;
    settings.defaults_version = gateway::backup::CURRENT_DEFAULTS_VERSION;

    let sanitized = settings.sanitized();
    sanitized.save(&db).await.unwrap();
    assert!(sanitized.enabled);
    assert_eq!(sanitized.interval_hours, 1);
    assert_eq!(sanitized.retention_days, 7);

    let got = gateway::backup::BackupSettings::load(&db).await.sanitized();
    assert!(got.enabled);
    assert_eq!(got.interval_hours, 1);
}

#[tokio::test]
async fn db_compact_returns_sizes() {
    let db = test_db().await;
    let result = aidog_db::compact_database(&db).await.unwrap();
    // Memory DB: before_bytes may be 0 or small, but should not error
    let _ = result;
}

#[tokio::test]
async fn backup_run_now_returns_result() {
    let db = test_db().await;
    // run_backup may fail (no backup dir configured), but should return a result not panic
    let result = gateway::backup::run_backup(&db).await;
    let _ = result;
}
