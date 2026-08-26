use crate::gateway;
use aidog_db::Db;
use tauri::State;


crate::tauri_command! {
pub async fn model_price_list(db: State<'_, Db>, limit: u32, offset: u32) -> Result<Vec<gateway::models::ModelPriceSummary>, String> {
    tracing::debug!(command = "model_price_list", limit, offset, "command invoked");
    aidog_db::list_model_prices(&db, limit, offset).await
}
}

crate::tauri_command! {
pub async fn model_price_count(db: State<'_, Db>) -> Result<u32, String> {
    aidog_db::count_model_prices(&db).await
}
}

crate::tauri_command! {
pub async fn model_price_search(db: State<'_, Db>, query: String, limit: u32) -> Result<Vec<gateway::models::ModelPriceSummary>, String> {
    tracing::debug!(command = "model_price_search", query = %query, limit, "command invoked");
    aidog_db::search_model_prices(&db, &query, limit).await
}
}

crate::tauri_command! {
pub async fn model_price_list_filtered(
    db: State<'_, Db>,
    query: Option<String>,
    source: Option<String>,
    limit: u32,
    offset: u32,
) -> Result<Vec<gateway::models::ModelPriceSummary>, String> {
    tracing::debug!(command = "model_price_list_filtered", limit, offset, "command invoked");
    aidog_db::filtered_list_model_prices(&db, query.as_deref(), source.as_deref(), limit, offset).await
}
}

crate::tauri_command! {
pub async fn model_price_count_filtered(
    db: State<'_, Db>,
    query: Option<String>,
    source: Option<String>,
) -> Result<u32, String> {
    aidog_db::filtered_count_model_prices(&db, query.as_deref(), source.as_deref()).await
}
}

crate::tauri_command! {
pub async fn model_price_resolve(
    db: State<'_, Db>,
    model_name: String,
    platform_type: String,
    input_tokens: Option<i64>,
) -> Result<gateway::models::ResolvedPrice, String> {
    let input_tokens = input_tokens.unwrap_or(0);
    tracing::debug!(command = "model_price_resolve", model_name = %model_name, platform_type = %platform_type, input_tokens, "command invoked");
    let settings = gateway::price_sync::get_sync_settings(&db).await;
    // 预览口径与计费链一致：先判高峰（preset 默认窗口，无 platform_id 上下文故不读用户覆盖），
    // 再按 (platform_type, model_name) 查 model_entry。
    let now_ms = aidog_db::now();
    let windows = gateway::peak_hours::default_peak_hours(&platform_type);
    let is_peak = gateway::peak_hours::is_in_peak_window(&windows, now_ms, &model_name);
    aidog_db::resolve_price(&db, &platform_type, &model_name, settings.fallback_input_price, settings.fallback_output_price, input_tokens, now_ms, is_peak)
        .await
        .map(|r| r.price)
}
}

crate::tauri_command! {
pub async fn model_price_sync(db: State<'_, Db>) -> Result<gateway::models::PriceSyncResult, String> {
    gateway::price_sync::sync_registry(&db).await
        .map_err(|e| { tracing::error!(command = "model_price_sync", error = %e, "registry sync failed"); e })
}
}

crate::tauri_command! {
pub async fn price_sync_settings_get(db: State<'_, Db>) -> Result<gateway::models::PriceSyncSettings, String> {
    Ok(gateway::price_sync::get_sync_settings(&db).await)
}
}

crate::tauri_command! {
pub async fn price_sync_settings_set(db: State<'_, Db>, settings: gateway::models::PriceSyncSettings) -> Result<(), String> {
    gateway::price_sync::save_sync_settings(&db, &settings).await;
    Ok(())
}
}

#[cfg(test)]
#[path = "test_price.rs"]
mod test_price;
