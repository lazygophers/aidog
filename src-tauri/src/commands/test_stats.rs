#![cfg(test)]
use super::*;
use crate::commands::test_harness::mock_app_with_db;
use tauri::Manager;

#[tokio::test]
async fn query_and_batch() {
    let app = mock_app_with_db().await;
    let db = app.state::<Db>();
    let q = StatsQuery {
        start: None,
        end: None,
        granularity: Some("day".into()),
        group_by: None,
        filter_group: None,
        filter_model: None,
        filter_platform: None,
    };
    let _ = stats_query(db.clone(), q.clone()).await.unwrap();
    let batch = stats_query_batch(db.clone(), vec![q.clone(), q]).await.unwrap();
    assert_eq!(batch.len(), 2);
}

#[tokio::test]
async fn settings_roundtrip_and_rebuild() {
    let app = mock_app_with_db().await;
    let db = app.state::<Db>();
    let s = stats_settings_get(db.clone()).await.unwrap();
    stats_settings_set(db.clone(), s).await.unwrap();
    stats_rebuild_from_logs(db.clone()).await.unwrap();
}
