use crate::gateway;
use crate::logging;
use aidog_db::{self as db, Db};
use gateway::models::*;

pub async fn load_app_log_settings_from_db(db: &Db) -> logging::AppLogSettings {
    db::get_setting(db, "app", "logging")
        .await
        .ok()
        .flatten()
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or_default()
}

/// 一次性迁移：把遗留 `~/.aidog/log_settings.json` 导入 DB settings 表后删除文件。
/// 幂等：文件不存在即空操作；仅当 DB 无该 setting 时写入（不覆盖用户后续 DB 改动）。
/// app log 设置单一事实源 = DB settings 表，禁独立文件。
pub async fn migrate_log_settings_file_to_db(db: &Db) {
    let path = match dirs::home_dir() {
        Some(h) => h.join(".aidog").join("log_settings.json"),
        None => return,
    };
    if !path.exists() {
        return;
    }
    if let Ok(content) = std::fs::read_to_string(&path)
        && let Ok(settings) = serde_json::from_str::<logging::AppLogSettings>(&content)
        && db::get_setting(db, "app", "logging")
            .await
            .ok()
            .flatten()
            .is_none()
        && let Ok(value) = serde_json::to_value(&settings)
    {
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
    // 无论解析成功与否都删除：坏文件不保留，DB 已是唯一源（缺失则 default）。
    let _ = std::fs::remove_file(&path);
}

crate::tauri_command! {
    pub async fn app_log_settings_get() -> Result<logging::AppLogSettings, String> {
    let db = aidog_ctx::db();
        Ok(load_app_log_settings_from_db(db).await)
    }
}

crate::tauri_command! {
    pub async fn app_log_settings_set(settings: logging::AppLogSettings) -> Result<(), String> {
    let db = aidog_ctx::db();
        let value = serde_json::to_value(&settings).map_err(|e| e.to_string())?;
        db::set_setting(db, SetSettingInput { scope: "app".to_string(), key: "logging".to_string(), value }).await
            .map_err(|e| { tracing::error!(command = "app_log_settings_set", error = %e, "persist log settings failed"); e })?;
        Ok(())
    }
}

#[cfg(test)]
#[path = "test_app_log.rs"]
mod test_app_log;

// 前端 console 日志接入（unified-logging 票 01）：前端 shim（utils/frontendLog.ts）
// 把 console.error/warn 及未捕获异常转发到这里，与后端 tracing 落同一个
// `~/.aidog/logs/aidog.<hour>.log`。前端自身不落盘——webview 无文件系统，
// 统一经 Rust appender 才能前后端同文件按小时分割。
crate::tauri_command! {
    pub async fn app_log_frontend(level: String, message: String) -> Result<(), String> {
        // 长度截断：前端一行可能是巨型对象 dump，别让日志文件被单条撑爆
        let message = if message.chars().count() > 4000 {
            let mut m: String = message.chars().take(4000).collect();
            m.push_str("…[truncated]");
            m
        } else {
            message
        };
        match level.as_str() {
            "error" => tracing::error!(target: "frontend", "{message}"),
            "warn" => tracing::warn!(target: "frontend", "{message}"),
            "info" => tracing::info!(target: "frontend", "{message}"),
            _ => tracing::debug!(target: "frontend", "{message}"),
        }
        Ok(())
    }
}
