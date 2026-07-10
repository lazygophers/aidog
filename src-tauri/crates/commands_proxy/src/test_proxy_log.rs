#![cfg(test)]
use super::*;
use aidog_test_util::mock_app_with_db;
use tauri::Manager;

#[tokio::test]
async fn list_count_get_clear_flow() {
    let app = mock_app_with_db().await;
    let db = app.state::<Db>();

    assert_eq!(proxy_log_count(db.clone()).await.unwrap(), 0);
    assert!(proxy_log_list(db.clone(), 10, 0).await.unwrap().is_empty());
    assert!(proxy_log_get("none".into(), db.clone()).await.unwrap().is_none());

    let filter = ProxyLogFilter::default();
    assert!(proxy_log_list_filtered(db.clone(), filter.clone(), 10, 0).await.unwrap().is_empty());
    assert_eq!(proxy_log_count_filtered(db.clone(), filter).await.unwrap(), 0);

    proxy_log_clear(db.clone()).await.unwrap();
}

#[tokio::test]
async fn usage_stats_endpoints() {
    let app = mock_app_with_db().await;
    let db = app.state::<Db>();

    let _ = platform_usage_stats(1, db.clone()).await.unwrap();
    let _ = group_usage_stats("gk".into(), db.clone()).await.unwrap();
    assert!(all_group_usage_stats(db.clone()).await.unwrap().is_empty());
    assert!(all_platform_usage_stats(db.clone()).await.unwrap().is_empty());
    assert!(get_last_test_result(1, db.clone()).await.unwrap().is_none());
}

#[tokio::test]
async fn log_settings_roundtrip() {
    let app = mock_app_with_db().await;
    let db = app.state::<Db>();

    // default
    let _ = proxy_log_settings_get(db.clone()).await.unwrap();
    // set with retention cleanup branches exercised
    let settings = ProxyLogSettings {
        retention_days: 30,
        user_request_retention_days: 7,
        upstream_request_retention_days: 7,
        ..Default::default()
    };
    proxy_log_settings_set(db.clone(), settings).await.unwrap();
    let got = proxy_log_settings_get(db.clone()).await.unwrap();
    assert_eq!(got.retention_days, 30);
}
