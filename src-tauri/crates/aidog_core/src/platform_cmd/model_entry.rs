//! 模型信息中枢查询命令（model-info 票 T2）。
//!
//! 数据源是 `model_entry` / `platform_preset` 两表，DB 空时读取层自动回落编译期内置 registry。
//! 三条命令覆盖 SPEC 的两个 tab：平台维度用 `model_entry_list(platform_code)`，
//! 模型维度与首屏用 `model_info_snapshot`（聚合行 + 全部平台预设一次拿全，前端不做二次 RPC 拼装）。

use crate::gateway;

crate::tauri_command! {
pub async fn model_entry_list( platform_code: Option<String>) -> Result<Vec<gateway::models::ModelEntry>, String> {
    let db = aidog_ctx::db();
    tracing::debug!(command = "model_entry_list", platform_code = ?platform_code, "command invoked");
    aidog_db::list_model_entries(&db, platform_code.as_deref()).await
}
}

crate::tauri_command! {
pub async fn model_entry_get( platform_code: String, model_id: String) -> Result<Option<gateway::models::ModelEntry>, String> {
    let db = aidog_ctx::db();
    tracing::debug!(command = "model_entry_get", platform_code = %platform_code, model_id = %model_id, "command invoked");
    aidog_db::get_model_entry(&db, &platform_code, &model_id).await
}
}

crate::tauri_command! {
pub async fn model_info_snapshot() -> Result<gateway::models::ModelInfoSnapshot, String> {
    let db = aidog_ctx::db();
    aidog_db::model_info_snapshot(&db).await
}
}
