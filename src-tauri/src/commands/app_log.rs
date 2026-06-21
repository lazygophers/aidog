use crate::gateway::{self, db::{self, Db}};
#[allow(unused_imports)]
use crate::logging;
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


pub(crate) async fn load_app_log_settings_from_db(db: &Db) -> logging::AppLogSettings {
    db::get_setting(db, "app", "logging").await
        .ok()
        .flatten()
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or_default()
}

/// 一次性迁移：把遗留 `~/.aidog/log_settings.json` 导入 DB settings 表后删除文件。
/// 幂等：文件不存在即空操作；仅当 DB 无该 setting 时写入（不覆盖用户后续 DB 改动）。
/// app log 设置单一事实源 = DB settings 表，禁独立文件。
pub(crate) async fn migrate_log_settings_file_to_db(db: &Db) {
    let path = match dirs::home_dir() {
        Some(h) => h.join(".aidog").join("log_settings.json"),
        None => return,
    };
    if !path.exists() {
        return;
    }
    if let Ok(content) = std::fs::read_to_string(&path) {
        if let Ok(settings) = serde_json::from_str::<logging::AppLogSettings>(&content) {
            if db::get_setting(db, "app", "logging").await.ok().flatten().is_none() {
                if let Ok(value) = serde_json::to_value(&settings) {
                    let _ = db::set_setting(
                        db,
                        SetSettingInput {
                            scope: "app".to_string(),
                            key: "logging".to_string(),
                            value,
                        },
                    )
                    .await;
                }
            }
        }
    }
    // 无论解析成功与否都删除：坏文件不保留，DB 已是唯一源（缺失则 default）。
    let _ = std::fs::remove_file(&path);
}

#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %crate::logging::new_trace_id()))]
pub async fn app_log_settings_get(db: State<'_, Db>) -> Result<logging::AppLogSettings, String> {
    tracing::debug!(command = "app_log_settings_get", "command invoked");
    Ok(load_app_log_settings_from_db(&db).await)
}

#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %crate::logging::new_trace_id()))]
pub async fn app_log_settings_set(settings: logging::AppLogSettings, db: State<'_, Db>) -> Result<(), String> {
    tracing::debug!(command = "app_log_settings_set", "command invoked");
    let value = serde_json::to_value(&settings).map_err(|e| e.to_string())?;
    db::set_setting(&db, SetSettingInput { scope: "app".to_string(), key: "logging".to_string(), value }).await
        .map_err(|e| { tracing::error!(command = "app_log_settings_set", error = %e, "persist log settings failed"); e })?;
    Ok(())
}
