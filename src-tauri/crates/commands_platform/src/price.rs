use aidog_core::gateway::{self, db::Db};
#[allow(unused_imports)]
use aidog_core::logging;
#[allow(unused_imports)]
use gateway::models::*;
#[allow(unused_imports)]
use tauri::State;
#[allow(unused_imports)]
use serde_json::Value;
#[allow(unused_imports)]
use std::sync::Arc;
#[allow(unused_imports)]
use tauri::Manager;


#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %aidog_core::logging::new_trace_id()))]
pub async fn model_price_list(db: State<'_, Db>, limit: u32, offset: u32) -> Result<Vec<gateway::models::ModelPriceSummary>, String> {
    tracing::debug!(command = "model_price_list", limit, offset, "command invoked");
    gateway::db::list_model_prices(&db, limit, offset).await
}

#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %aidog_core::logging::new_trace_id()))]
pub async fn model_price_count(db: State<'_, Db>) -> Result<u32, String> {
    tracing::debug!(command = "model_price_count", "command invoked");
    gateway::db::count_model_prices(&db).await
}

#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %aidog_core::logging::new_trace_id()))]
pub async fn model_price_search(db: State<'_, Db>, query: String, limit: u32) -> Result<Vec<gateway::models::ModelPriceSummary>, String> {
    tracing::debug!(command = "model_price_search", query = %query, limit, "command invoked");
    gateway::db::search_model_prices(&db, &query, limit).await
}

#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %aidog_core::logging::new_trace_id()))]
pub async fn model_price_list_filtered(
    db: State<'_, Db>,
    query: Option<String>,
    source: Option<String>,
    limit: u32,
    offset: u32,
) -> Result<Vec<gateway::models::ModelPriceSummary>, String> {
    tracing::debug!(command = "model_price_list_filtered", limit, offset, "command invoked");
    gateway::db::filtered_list_model_prices(&db, query.as_deref(), source.as_deref(), limit, offset).await
}

#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %aidog_core::logging::new_trace_id()))]
pub async fn model_price_count_filtered(
    db: State<'_, Db>,
    query: Option<String>,
    source: Option<String>,
) -> Result<u32, String> {
    tracing::debug!(command = "model_price_count_filtered", "command invoked");
    gateway::db::filtered_count_model_prices(&db, query.as_deref(), source.as_deref()).await
}

#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %aidog_core::logging::new_trace_id()))]
pub async fn model_price_resolve(
    db: State<'_, Db>,
    model_name: String,
    platform_type: String,
    input_tokens: Option<i64>,
) -> Result<gateway::models::ResolvedPrice, String> {
    let input_tokens = input_tokens.unwrap_or(0);
    tracing::debug!(command = "model_price_resolve", model_name = %model_name, platform_type = %platform_type, input_tokens, "command invoked");
    let settings = gateway::price_sync::get_sync_settings(&db).await;
    gateway::db::resolve_price(&db, &model_name, &platform_type, settings.fallback_input_price, settings.fallback_output_price, input_tokens).await
}

#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %aidog_core::logging::new_trace_id()))]
pub async fn model_price_sync(db: State<'_, Db>) -> Result<gateway::models::PriceSyncResult, String> {
    tracing::debug!(command = "model_price_sync", "command invoked");
    gateway::price_sync::sync_github_prices(&db).await
        .map_err(|e| { tracing::error!(command = "model_price_sync", error = %e, "model price sync failed"); e })
}

#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %aidog_core::logging::new_trace_id()))]
pub async fn price_sync_settings_get(db: State<'_, Db>) -> Result<gateway::models::PriceSyncSettings, String> {
    tracing::debug!(command = "price_sync_settings_get", "command invoked");
    Ok(gateway::price_sync::get_sync_settings(&db).await)
}

#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %aidog_core::logging::new_trace_id()))]
pub async fn price_sync_settings_set(db: State<'_, Db>, settings: gateway::models::PriceSyncSettings) -> Result<(), String> {
    tracing::debug!(command = "price_sync_settings_set", "command invoked");
    gateway::price_sync::save_sync_settings(&db, &settings).await;
    Ok(())
}

#[cfg(test)]
#[path = "test_price.rs"]
mod test_price;
