#![cfg(test)]
use super::*;
use crate::commands::test_harness::mock_app_with_db;
use tauri::Manager;

#[tokio::test]
async fn price_crud_and_resolve() {
    let app = mock_app_with_db().await;
    let db = app.state::<Db>();

    // seed via gateway upsert
    let pd = serde_json::json!({"input_cost_per_token": 3e-6, "output_cost_per_token": 6e-6}).to_string();
    gateway::db::upsert_model_price(&db, "claude", "github", &pd, None, None, None).await.unwrap();

    assert_eq!(model_price_count(db.clone()).await.unwrap(), 1);
    assert_eq!(model_price_list(db.clone(), 10, 0).await.unwrap().len(), 1);
    assert_eq!(model_price_search(db.clone(), "claude".into(), 10).await.unwrap().len(), 1);

    let f = model_price_list_filtered(db.clone(), Some("cl".into()), Some("github".into()), 10, 0).await.unwrap();
    assert_eq!(f.len(), 1);
    assert_eq!(model_price_count_filtered(db.clone(), Some("cl".into()), None).await.unwrap(), 1);

    let r = model_price_resolve(db.clone(), "claude".into(), "anthropic".into(), Some(0)).await.unwrap();
    assert_eq!(r.source, "top_level");
    // resolve with None input_tokens
    let r2 = model_price_resolve(db.clone(), "claude".into(), "anthropic".into(), None).await.unwrap();
    assert!(r2.input_cost_per_token > 0.0);
}

#[tokio::test]
async fn price_sync_settings_roundtrip() {
    let app = mock_app_with_db().await;
    let db = app.state::<Db>();
    let s = price_sync_settings_get(db.clone()).await.unwrap();
    price_sync_settings_set(db.clone(), s).await.unwrap();
}
