use crate::gateway;

// 命名说明（票 T6 决定不改）：`model_price_sync` 现在同步的是整个 registry
// （platform.json 品牌/端点 + 逐平台模型条目），不只是价格，名字确实偏窄。
// 仍保留 `model_price_*` 前缀，因为改名要连坐的不是一两处：
// 与它成组的 `price_sync_settings_get/set` 读写的 setting key 是 **持久化在用户 DB 里的**
// `price_sync`（见 `gateway::price_sync::get_sync_settings`），改名要么留孤儿 key，
// 要么再写一条数据迁移；`PriceSyncSettings`（承载 fallback 单价，确实是价格语义）与
// `PriceSyncResult` 又是 ts-rs 导出的前端契约。只改 command 名不动这些，反而落成
// 「registry_sync 命令 + price_sync 模块 + PriceSyncResult 返回值」的更差局部不一致。
// 真要改就整组一起改（含 setting key 迁移），那超出本票范围，另起票。
//
// 旧的 5 个 `model_price` 表查询命令（list / count / search / list_filtered / count_filtered）
// 与预览命令 `model_price_resolve` 随该表一并删除（票 T6）：表已 DROP，前端无消费方，
// 模型清单与价格改由 `model_entry_list` / `model_entry_get` / `model_info_snapshot` 提供。

crate::tauri_command! {
pub async fn model_price_sync() -> Result<gateway::models::PriceSyncResult, String> {
    let db = aidog_ctx::db();
    gateway::price_sync::sync_registry(db).await
        .map_err(|e| { tracing::error!(command = "model_price_sync", error = %e, "registry sync failed"); e })
}
}

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
