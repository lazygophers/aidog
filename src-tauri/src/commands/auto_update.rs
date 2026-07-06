//! 自动检查更新开关（settings KV scope="app" key="auto_update_enabled"，JSON bool，默认 true）。
//! 仅 gate 启动期 daily throttled check；手动按钮 (about 页) 不 gate。

use crate::gateway::{db::{self, Db}, models::SetSettingInput};
use tauri::State;

/// 读 auto_update_enabled；缺失/解析失败默认 true（不打扰存量用户）。
pub(crate) async fn load_auto_update_enabled(db: &Db) -> bool {
    match db::get_setting(db, "app", "auto_update_enabled").await {
        Ok(Some(v)) => v.as_bool().unwrap_or(true),
        _ => true,
    }
}

#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %crate::logging::new_trace_id()))]
pub async fn get_auto_update_enabled(db: State<'_, Db>) -> Result<bool, String> {
    tracing::debug!(command = "get_auto_update_enabled", "command invoked");
    Ok(load_auto_update_enabled(&db).await)
}

#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %crate::logging::new_trace_id()))]
pub async fn set_auto_update_enabled(enabled: bool, db: State<'_, Db>) -> Result<(), String> {
    tracing::debug!(command = "set_auto_update_enabled", enabled, "command invoked");
    db::set_setting(&db, SetSettingInput {
        scope: "app".to_string(),
        key: "auto_update_enabled".to_string(),
        value: serde_json::Value::Bool(enabled),
    })
    .await
    .map_err(|e| { tracing::error!(command = "set_auto_update_enabled", error = %e, "persist auto_update_enabled failed"); e })?;
    Ok(())
}

#[cfg(test)]
#[path = "test_auto_update.rs"]
mod test_auto_update;
