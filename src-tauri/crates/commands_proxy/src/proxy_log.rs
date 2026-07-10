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


use gateway::models::{ProxyLog, ProxyLogSummary, ProxyLogSettings, ProxyLogFilter};

#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %aidog_core::logging::new_trace_id()))]
pub async fn proxy_log_list(db: State<'_, Db>, limit: u32, offset: u32) -> Result<Vec<ProxyLogSummary>, String> {
    tracing::debug!(command = "proxy_log_list", limit, offset, "command invoked");
    gateway::db::list_proxy_logs(&db, limit, offset).await
}

#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %aidog_core::logging::new_trace_id()))]
pub async fn proxy_log_list_filtered(
    db: State<'_, Db>,
    filter: ProxyLogFilter,
    limit: u32,
    offset: u32,
) -> Result<Vec<ProxyLogSummary>, String> {
    tracing::debug!(command = "proxy_log_list_filtered", limit, offset, "command invoked");
    gateway::db::filtered_list_proxy_logs(&db, &filter, limit, offset).await
}

#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %aidog_core::logging::new_trace_id()))]
pub async fn proxy_log_count_filtered(
    db: State<'_, Db>,
    filter: ProxyLogFilter,
) -> Result<u32, String> {
    tracing::debug!(command = "proxy_log_count_filtered", "command invoked");
    gateway::db::filtered_count_proxy_logs(&db, &filter).await
}

#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %aidog_core::logging::new_trace_id()))]
pub async fn proxy_log_get(id: String, db: State<'_, Db>) -> Result<Option<ProxyLog>, String> {
    tracing::debug!(command = "proxy_log_get", id = %id, "command invoked");
    gateway::db::get_proxy_log(&db, &id).await
}

#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %aidog_core::logging::new_trace_id()))]
pub async fn proxy_log_clear(db: State<'_, Db>) -> Result<(), String> {
    tracing::debug!(command = "proxy_log_clear", "command invoked");
    gateway::db::clear_proxy_logs(&db).await
}

#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %aidog_core::logging::new_trace_id()))]
pub async fn proxy_log_count(db: State<'_, Db>) -> Result<u32, String> {
    tracing::debug!(command = "proxy_log_count", "command invoked");
    gateway::db::count_proxy_logs(&db).await
}

#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %aidog_core::logging::new_trace_id()))]
pub async fn platform_usage_stats(platform_id: u64, db: State<'_, Db>) -> Result<gateway::models::PlatformUsageStats, String> {
    tracing::debug!(command = "platform_usage_stats", platform_id, "command invoked");
    gateway::db::get_platform_usage_stats(&db, platform_id).await
}

#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %aidog_core::logging::new_trace_id()))]
pub async fn group_usage_stats(group_key: String, db: State<'_, Db>) -> Result<gateway::models::PlatformUsageStats, String> {
    tracing::debug!(command = "group_usage_stats", group_key = %group_key, "command invoked");
    gateway::db::get_group_usage_stats(&db, &group_key).await
}

#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %aidog_core::logging::new_trace_id()))]
pub async fn all_group_usage_stats(db: State<'_, Db>) -> Result<std::collections::HashMap<String, gateway::models::PlatformUsageStats>, String> {
    tracing::debug!(command = "all_group_usage_stats", "command invoked");
    gateway::db::get_all_group_usage_stats(&db).await
}

#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %aidog_core::logging::new_trace_id()))]
pub async fn all_platform_usage_stats(db: State<'_, Db>) -> Result<std::collections::HashMap<u64, gateway::models::PlatformUsageStats>, String> {
    tracing::debug!(command = "all_platform_usage_stats", "command invoked");
    gateway::db::platform_usage_stats_all(&db).await
}

#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %aidog_core::logging::new_trace_id()))]
pub async fn get_last_test_result(platform_id: u64, db: State<'_, Db>) -> Result<Option<gateway::models::LastTestResult>, String> {
    tracing::debug!(command = "get_last_test_result", platform_id, "command invoked");
    gateway::db::get_last_test_result(&db, platform_id).await
}

#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %aidog_core::logging::new_trace_id()))]
pub async fn proxy_log_settings_get(db: State<'_, Db>) -> Result<ProxyLogSettings, String> {
    tracing::debug!(command = "proxy_log_settings_get", "command invoked");
    let val = gateway::db::get_setting(&db, "proxy", "logging").await
        .ok()
        .flatten()
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or_default();
    Ok(val)
}

#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %aidog_core::logging::new_trace_id()))]
pub async fn proxy_log_settings_set(db: State<'_, Db>, settings: ProxyLogSettings) -> Result<(), String> {
    tracing::debug!(command = "proxy_log_settings_set", "command invoked");
    let value = serde_json::to_value(&settings)
        .map_err(|e| format!("serialize log settings: {e}"))?;
    gateway::db::set_setting(&db, gateway::models::SetSettingInput {
        scope: "proxy".into(),
        key: "logging".into(),
        value,
    }).await
        .map_err(|e| { tracing::error!(command = "proxy_log_settings_set", error = %e, "persist log settings failed"); e })?;
    run_retention_cleanup(&db, &settings).await;
    Ok(())
}

/// 跑 4 步 retention 清理链（user/upstream fields + retention_days + purge tombstone）。
/// 每步 `tracing::warn!` 容错（单步失败不阻塞其余）。settings_set 与 cleanup_expired 共用。
async fn run_retention_cleanup(db: &State<'_, Db>, settings: &ProxyLogSettings) {
    // Run field-level cleanup for user/upstream request data
    if let Err(e) = gateway::db::cleanup_user_request_fields(db, settings.user_request_retention_days).await {
        tracing::warn!(command = "proxy_log_cleanup", error = %e, "cleanup user_request fields failed");
    }
    if let Err(e) = gateway::db::cleanup_upstream_request_fields(db, settings.upstream_request_retention_days).await {
        tracing::warn!(command = "proxy_log_cleanup", error = %e, "cleanup upstream_request fields failed");
    }
    // Delete entire log rows older than overall retention (hard delete → physical row removal)
    if settings.retention_days > 0 {
        if let Err(e) = gateway::db::cleanup_proxy_logs(db, settings.retention_days).await {
            tracing::warn!(command = "proxy_log_cleanup", error = %e, "cleanup proxy_logs failed");
        }
    }
    // 清积压 tombstone（本次 cleanup 前历史软删残留）+ incremental_vacuum 回收 free pages。
    // 软删→硬删迁移期一次性清旧 tombstone；日常 retention_days 已硬删则此步为 no-op + 回收。
    if let Err(e) = gateway::db::purge_deleted_proxy_logs(db).await {
        tracing::warn!(command = "proxy_log_cleanup", error = %e, "purge deleted proxy_logs failed");
    }
}

/// 按当前 ProxyLogSettings 的保留天数立即清理过期数据，不修改设置。
/// 复用 settings_set 的清理链（run_retention_cleanup）。
#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %aidog_core::logging::new_trace_id()))]
pub async fn proxy_log_cleanup_expired(db: State<'_, Db>) -> Result<(), String> {
    tracing::debug!(command = "proxy_log_cleanup_expired", "command invoked");
    let settings: ProxyLogSettings = gateway::db::get_setting(&db, "proxy", "logging").await
        .ok()
        .flatten()
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or_default();
    run_retention_cleanup(&db, &settings).await;
    Ok(())
}

#[cfg(test)]
#[path = "test_proxy_log.rs"]
mod test_proxy_log;
