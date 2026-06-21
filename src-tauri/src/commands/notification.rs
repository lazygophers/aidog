use crate::gateway::{self, db::Db};
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


#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %crate::logging::new_trace_id()))]
pub async fn notification_settings_get(db: State<'_, Db>) -> Result<NotificationSettings, String> {
    tracing::debug!(command = "notification_settings_get", "command invoked");
    Ok(gateway::db::get_notification_settings(&db).await)
}

#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %crate::logging::new_trace_id()))]
pub async fn notification_settings_set(
    db: State<'_, Db>,
    settings: NotificationSettings,
) -> Result<(), String> {
    tracing::debug!(command = "notification_settings_set", "command invoked");
    let retention_days = settings.inbox_retention_days;
    gateway::db::set_setting(&db, SetSettingInput {
        scope: "notification".to_string(),
        key: "settings".to_string(),
        value: serde_json::to_value(&settings).map_err(|e| format!("serialize notification settings: {e}"))?,
    }).await
        .map_err(|e| { tracing::error!(command = "notification_settings_set", error = %e, "persist notification settings failed"); e })?;
    // 改保留天数即时清理一次过期收件箱（非关键路径，失败仅 warn 不阻塞保存）。
    if let Err(e) = gateway::db::cleanup_notifications(&db, retention_days).await {
        tracing::warn!(command = "notification_settings_set", error = %e, "cleanup notifications failed");
    }
    Ok(())
}

#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %crate::logging::new_trace_id()))]
pub async fn notification_inbox_list(db: State<'_, Db>, limit: Option<i64>) -> Result<Vec<Notification>, String> {
    tracing::debug!(command = "notification_inbox_list", "command invoked");
    gateway::db::list_notifications(&db, limit.unwrap_or(100)).await
}

#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %crate::logging::new_trace_id()))]
pub async fn notification_clear(db: State<'_, Db>) -> Result<(), String> {
    tracing::debug!(command = "notification_clear", "command invoked");
    gateway::db::clear_notifications(&db).await
}

#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %crate::logging::new_trace_id()))]
pub async fn notification_test(
    db: State<'_, Db>,
    app: tauri::AppHandle,
    notif_type: String,
    content: Option<String>,
) -> Result<gateway::notification::DispatchResult, String> {
    // 应用行为 key 由 dispatch 内部统一解析（取本命令 #[instrument] span 的 trace_id，
    // 与日志同口径），无需在此手动注入；vars 仅提供模板渲染所需的展示字段。
    tracing::debug!(command = "notification_test", notif_type = %notif_type, "command invoked");
    let mut vars = std::collections::HashMap::new();
    vars.insert("project".to_string(), "aidog".to_string());
    vars.insert("status".to_string(), "test".to_string());
    vars.insert("time".to_string(), chrono::Local::now().format("%H:%M:%S").to_string());
    vars.insert("session".to_string(), "test-session".to_string());
    vars.insert("group".to_string(), "test".to_string());
    let db_arc = std::sync::Arc::new(db.inner().clone());
    Ok(gateway::notification::dispatch(&db_arc, Some(&app), None, &notif_type, content.as_deref(), &vars).await)
}

/// 仅测 TTS 通道（绕过 dispatch，按当前 settings.tts_backend 播报 text）。
#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %crate::logging::new_trace_id()))]
pub async fn notification_test_tts(
    db: State<'_, Db>,
    app: tauri::AppHandle,
    text: String,
) -> Result<(), String> {
    tracing::debug!(command = "notification_test_tts", "command invoked");
    let db_arc = std::sync::Arc::new(db.inner().clone());
    let settings = gateway::db::get_notification_settings(&db_arc).await;
    gateway::notification::speak(Some(&app), settings.tts_backend, &text);
    Ok(())
}

/// 仅测系统弹窗通道（绕过 dispatch，直接调 tauri-plugin-notification）。
#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %crate::logging::new_trace_id()))]
pub async fn notification_test_popup(
    app: tauri::AppHandle,
    title: String,
    body: String,
) -> Result<(), String> {
    tracing::debug!(command = "notification_test_popup", "command invoked");
    gateway::notification::show_popup(&app, &title, &body);
    Ok(())
}

/// 仅测系统提示音通道（跨平台 spawn system beep）。
#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %crate::logging::new_trace_id()))]
pub async fn notification_test_beep() -> Result<(), String> {
    tracing::debug!(command = "notification_test_beep", "command invoked");
    gateway::notification::play_beep();
    Ok(())
}
