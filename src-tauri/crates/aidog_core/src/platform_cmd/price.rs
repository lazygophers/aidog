use crate::gateway;

// 价格设置命令。`model_price_sync`（registry 远程同步）2026-10-21 删除：registry 只读
// 编译期内置，启动灌种（gateway::price_sync::seed_registry_from_bundled），无手动触发面。
// `price_sync_settings_get/set` 保留：setting key `price_sync` 持久化在用户 DB（改 key
// 要数据迁移，不值），现承载兜底单价（fallback_input/output_price）与诊断戳
// registry_last_updated。

crate::tauri_command! {
pub async fn price_sync_settings_get() -> Result<gateway::models::PriceSyncSettings, String> {
    let db = aidog_ctx::db();
    Ok(gateway::price_sync::get_sync_settings(db).await)
}
}

crate::tauri_command! {
pub async fn price_sync_settings_set( settings: gateway::models::PriceSyncSettings) -> Result<(), String> {
    let db = aidog_ctx::db();
    gateway::price_sync::save_sync_settings(db, &settings).await;
    Ok(())
}
}

#[cfg(test)]
#[path = "test_price.rs"]
mod test_price;
