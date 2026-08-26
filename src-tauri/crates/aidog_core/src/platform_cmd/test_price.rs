#![cfg(test)]
use super::*;
use aidog_db::test_support::test_db;

/// aidog_core 不能 dev-dep aidog_test_util（后者依赖 aidog_core，会成环），
/// 故不经 tauri::State 走 command 包装层，直测 command 转发的 gateway:: 函数
/// （command 本身只是薄转发 + tracing，逻辑等价）。
#[tokio::test]
async fn price_crud_and_resolve() {
    let db = test_db().await;

    // seed via gateway upsert
    let pd = serde_json::json!({"input_cost_per_token": 3e-6, "output_cost_per_token": 6e-6}).to_string();
    aidog_db::upsert_model_price(&db, "claude", "github", &pd, None, None, None).await.unwrap();

    assert_eq!(aidog_db::count_model_prices(&db).await.unwrap(), 1);
    assert_eq!(aidog_db::list_model_prices(&db, 10, 0).await.unwrap().len(), 1);
    assert_eq!(aidog_db::search_model_prices(&db, "claude", 10).await.unwrap().len(), 1);

    let f = aidog_db::filtered_list_model_prices(&db, Some("cl"), Some("github"), 10, 0).await.unwrap();
    assert_eq!(f.len(), 1);
    assert_eq!(aidog_db::filtered_count_model_prices(&db, Some("cl"), None).await.unwrap(), 1);

    // 计费解析已改查 model_entry（票 T4）：model_price 里的行不再参与，
    // 落到 bundled registry 的 (anthropic, claude-opus-4-6) 条目上。
    let settings = gateway::price_sync::get_sync_settings(&db).await;
    let r = aidog_db::resolve_price(&db, "anthropic", "claude-opus-4-6", settings.fallback_input_price, settings.fallback_output_price, 0, 0, false).await.unwrap();
    assert_eq!(r.price.source, "model_entry");
    assert!(r.price.input_cost_per_token > 0.0);
    assert!(!r.peak_applied);
}

#[tokio::test]
async fn price_sync_settings_roundtrip() {
    let db = test_db().await;
    let s = gateway::price_sync::get_sync_settings(&db).await;
    gateway::price_sync::save_sync_settings(&db, &s).await;
}
