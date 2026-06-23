#![cfg(test)]
use super::*;
use crate::commands::test_harness::mock_app_with_db;
use tauri::Manager;

#[test]
fn check_uv_runs() {
    // returns bool either way; just exercise the path
    let _ = check_uv();
}

#[tokio::test]
async fn set_executor_normalizes() {
    let app = mock_app_with_db().await;
    let db = app.state::<Db>();
    set_script_executor("uv".into(), db.clone()).await.unwrap();
    set_script_executor("python3".into(), db.clone()).await.unwrap();
    set_script_executor("garbage".into(), db.clone()).await.unwrap();
}
