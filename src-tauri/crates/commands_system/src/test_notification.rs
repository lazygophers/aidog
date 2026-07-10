#![cfg(test)]
use super::*;
use aidog_test_util::mock_app_with_db;
use tauri::Manager;

#[tokio::test]
async fn settings_inbox_clear() {
    let app = mock_app_with_db().await;
    let db = app.state::<Db>();

    let s = notification_settings_get(db.clone()).await.unwrap();
    notification_settings_set(db.clone(), s).await.unwrap();

    assert!(notification_inbox_list(db.clone(), Some(10)).await.unwrap().is_empty());
    assert!(notification_inbox_list(db.clone(), None).await.unwrap().is_empty());
    notification_clear(db.clone()).await.unwrap();
}
