#![cfg(test)]
//! `model_price` 表 CRUD。计费解析已迁至 `test_price_resolve.rs`（票 T4）。
use super::*;
use super::test_support::test_db;

fn pd_basic() -> String {
    serde_json::json!({
        "input_cost_per_token": 3e-6,
        "output_cost_per_token": 1.5e-5,
        "cache_read_input_token_cost": 3e-7,
        "max_output_tokens": 8192
    })
    .to_string()
}

#[tokio::test]
async fn upsert_get_list_count_search() {
    let db = test_db().await;
    upsert_model_price(&db, "claude-sonnet-4", "github", &pd_basic(), Some(200000), Some(8192), Some(200000))
        .await
        .unwrap();
    // upsert again → update path
    upsert_model_price(&db, "claude-sonnet-4", "github", &pd_basic(), Some(200000), Some(8192), Some(200000))
        .await
        .unwrap();
    upsert_model_price(&db, "gpt-4o", "github", &pd_basic(), None, None, None)
        .await
        .unwrap();

    assert_eq!(count_model_prices(&db).await.unwrap(), 2);

    let list = list_model_prices(&db, 10, 0).await.unwrap();
    assert_eq!(list.len(), 2);
    // input_price converted to $/M
    let claude = list.iter().find(|m| m.model_name == "claude-sonnet-4").unwrap();
    assert!((claude.input_price.unwrap() - 3.0).abs() < 1e-9);

    let got = get_model_price(&db, "gpt-4o").await.unwrap();
    assert!(got.is_some());
    assert!(get_model_price(&db, "missing").await.unwrap().is_none());

    let found = search_model_prices(&db, "claude", 10).await.unwrap();
    assert_eq!(found.len(), 1);
}

#[tokio::test]
async fn get_model_price_prefers_manual_over_github() {
    let db = test_db().await;
    upsert_model_price(&db, "m", "github", &pd_basic(), None, None, None).await.unwrap();
    let manual_pd = serde_json::json!({"input_cost_per_token": 9e-6}).to_string();
    upsert_model_price(&db, "m", "manual", &manual_pd, None, None, None).await.unwrap();
    let got = get_model_price(&db, "m").await.unwrap().unwrap();
    assert_eq!(got.source, "manual");
}

#[tokio::test]
async fn filtered_list_and_count() {
    let db = test_db().await;
    upsert_model_price(&db, "alpha", "github", &pd_basic(), None, None, None).await.unwrap();
    upsert_model_price(&db, "beta", "manual", &pd_basic(), None, None, None).await.unwrap();
    upsert_model_price(&db, "alphabeta", "github", &pd_basic(), None, None, None).await.unwrap();

    // no filter
    assert_eq!(filtered_count_model_prices(&db, None, None).await.unwrap(), 3);
    // query filter
    let q = filtered_list_model_prices(&db, Some("alpha"), None, 10, 0).await.unwrap();
    assert_eq!(q.len(), 2);
    assert_eq!(filtered_count_model_prices(&db, Some("alpha"), None).await.unwrap(), 2);
    // source filter
    let s = filtered_list_model_prices(&db, None, Some("manual"), 10, 0).await.unwrap();
    assert_eq!(s.len(), 1);
    assert_eq!(filtered_count_model_prices(&db, None, Some("manual")).await.unwrap(), 1);
    // both + empty strings ignored ("alpha"/"alphabeta" both github)
    let b = filtered_list_model_prices(&db, Some("alpha"), Some("github"), 10, 0).await.unwrap();
    assert_eq!(b.len(), 2);
    let empty = filtered_list_model_prices(&db, Some(""), Some(""), 10, 0).await.unwrap();
    assert_eq!(empty.len(), 3);
    assert_eq!(filtered_count_model_prices(&db, Some(""), Some("")).await.unwrap(), 3);
}
