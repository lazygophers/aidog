use crate::gateway;
use aidog_db::Db;
use tauri::State;

use gateway::models::{
    ProxyLog, ProxyLogFilter, ProxyLogPage, ProxyLogSettings, ProxyLogSummary, RequestLogSummary,
};

crate::tauri_command! {
pub async fn proxy_log_list(db: State<'_, Db>, limit: u32, offset: u32) -> Result<Vec<ProxyLogSummary>, String> {
    tracing::debug!(command = "proxy_log_list", limit, offset, "command invoked");
    aidog_logs::list_proxy_logs(&db, limit, offset).await
}
}

crate::tauri_command! {
pub async fn proxy_log_list_filtered(
    db: State<'_, Db>,
    filter: ProxyLogFilter,
    limit: u32,
    offset: u32,
) -> Result<ProxyLogPage, String> {
    tracing::debug!(command = "proxy_log_list_filtered", limit, offset, "command invoked");
    aidog_logs::filtered_list_proxy_logs(&db, &filter, limit, offset).await
}
}

crate::tauri_command! {
/// Logs 页 model 下拉选项（logs-query-ipc-slimming s4）：`SELECT DISTINCT` 直出选项列表，
/// 替代旧的「拉 200 行完整日志再前端去重」。`actual=true` 取 actual_model 列，否则取 model 列
/// （对应前端 filterModelType 状态）；`filter` 通常仅带 exclude_sources，与主列表筛选一致。
pub async fn proxy_log_distinct_models(
    db: State<'_, Db>,
    filter: ProxyLogFilter,
    actual: bool,
    limit: u32,
) -> Result<Vec<String>, String> {
    tracing::debug!(command = "proxy_log_distinct_models", actual, limit, "command invoked");
    aidog_logs::distinct_models_proxy_log(&db, &filter, actual, limit).await
}
}

crate::tauri_command! {
pub async fn proxy_log_count_filtered(
    db: State<'_, Db>,
    filter: ProxyLogFilter,
) -> Result<u32, String> {
    aidog_logs::filtered_count_proxy_logs(&db, &filter).await
}
}

crate::tauri_command! {
/// 请求日志页列表（cli-proxy-request-log s3）。
/// 默认 sources=[test,quota]（db 层兜底）；前端可显式传 filter 覆盖。
/// 返回 RequestLogSummary（含 cli_proxy_provider_name，db 层应用层合并 provider 表，跨库禁 JOIN）。
pub async fn request_log_list(
    db: State<'_, Db>,
    filter: ProxyLogFilter,
    limit: u32,
    offset: u32,
) -> Result<Vec<RequestLogSummary>, String> {
    tracing::debug!(command = "request_log_list", limit, offset, "command invoked");
    aidog_logs::list_request_logs(&db, &filter, limit, offset).await
}
}

crate::tauri_command! {
pub async fn proxy_log_get(id: String, db: State<'_, Db>) -> Result<Option<ProxyLog>, String> {
    tracing::debug!(command = "proxy_log_get", id = %id, "command invoked");
    aidog_logs::get_proxy_log(&db, &id).await
}
}

crate::tauri_command! {
pub async fn proxy_log_clear(db: State<'_, Db>) -> Result<(), String> {
    aidog_logs::clear_proxy_logs(&db).await
}
}

crate::tauri_command! {
pub async fn proxy_log_count(db: State<'_, Db>) -> Result<u32, String> {
    aidog_db::count_proxy_logs(&db).await
}
}

crate::tauri_command! {
pub async fn platform_usage_stats(platform_id: u64, db: State<'_, Db>) -> Result<gateway::models::PlatformUsageStats, String> {
    tracing::debug!(command = "platform_usage_stats", platform_id, "command invoked");
    aidog_stats::get_platform_usage_stats(&db, platform_id).await
}
}

crate::tauri_command! {
pub async fn group_usage_stats(group_key: String, db: State<'_, Db>) -> Result<gateway::models::PlatformUsageStats, String> {
    tracing::debug!(command = "group_usage_stats", group_key = %group_key, "command invoked");
    aidog_stats::get_group_usage_stats(&db, &group_key).await
}
}

crate::tauri_command! {
pub async fn all_group_usage_stats(db: State<'_, Db>) -> Result<std::collections::HashMap<String, gateway::models::PlatformUsageStats>, String> {
    aidog_stats::get_all_group_usage_stats(&db).await
}
}

crate::tauri_command! {
pub async fn all_platform_usage_stats(db: State<'_, Db>) -> Result<std::collections::HashMap<u64, gateway::models::PlatformUsageStats>, String> {
    aidog_stats::platform_usage_stats_all(&db).await
}
}

crate::tauri_command! {
pub async fn get_last_test_result(platform_id: u64, db: State<'_, Db>) -> Result<Option<gateway::models::LastTestResult>, String> {
    tracing::debug!(command = "get_last_test_result", platform_id, "command invoked");
    aidog_stats::get_last_test_result(&db, platform_id).await
}
}

crate::tauri_command! {
pub async fn proxy_log_settings_get(db: State<'_, Db>) -> Result<ProxyLogSettings, String> {
    let val = aidog_db::get_setting(&db, "proxy", "logging").await
        .ok()
        .flatten()
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or_default();
    Ok(val)
}
}

crate::tauri_command! {
pub async fn proxy_log_settings_set(db: State<'_, Db>, settings: ProxyLogSettings) -> Result<(), String> {
    let value = serde_json::to_value(&settings)
        .map_err(|e| format!("serialize log settings: {e}"))?;
    aidog_db::set_setting(&db, gateway::models::SetSettingInput {
        scope: "proxy".into(),
        key: "logging".into(),
        value,
    }).await
        .map_err(|e| { tracing::error!(command = "proxy_log_settings_set", error = %e, "persist log settings failed"); e })?;
    run_retention_cleanup(&db, &settings).await;
    Ok(())
}
}

/// 跑 4 步 retention 清理链（user/upstream fields + retention_days + purge tombstone）。
/// 每步 `tracing::warn!` 容错（单步失败不阻塞其余）。settings_set / cleanup_expired /
/// app_setup 每日调度共用（&Db 入参脱离 State 绑定，便于后台 spawn 调用）。
pub async fn run_retention_cleanup(db: &Db, settings: &ProxyLogSettings) {
    // Run field-level cleanup for user/upstream request data
    if let Err(e) = aidog_db::cleanup_user_request_fields(
        db,
        settings.user_request_retention_days,
        settings.user_request_retention_unit,
    )
    .await
    {
        tracing::warn!(command = "proxy_log_cleanup", error = %e, "cleanup user_request fields failed");
    }
    if let Err(e) = aidog_db::cleanup_upstream_request_fields(
        db,
        settings.upstream_request_retention_days,
        settings.upstream_request_retention_unit,
    )
    .await
    {
        tracing::warn!(command = "proxy_log_cleanup", error = %e, "cleanup upstream_request fields failed");
    }
    // Delete entire log rows older than overall retention (hard delete → physical row removal)
    if settings.retention_days > 0
        && let Err(e) =
            aidog_logs::cleanup_proxy_logs(db, settings.retention_days, settings.retention_unit)
                .await
    {
        tracing::warn!(command = "proxy_log_cleanup", error = %e, "cleanup proxy_logs failed");
    }
    // 清积压 tombstone（本次 cleanup 前历史软删残留）+ incremental_vacuum 回收 free pages。
    // 软删→硬删迁移期一次性清旧 tombstone；日常 retention_days 已硬删则此步为 no-op + 回收。
    if let Err(e) = aidog_logs::purge_deleted_proxy_logs(db).await {
        tracing::warn!(command = "proxy_log_cleanup", error = %e, "purge deleted proxy_logs failed");
    }
}

crate::tauri_command! {
/// 按当前 ProxyLogSettings 的保留天数立即清理过期数据，不修改设置。
/// 复用 settings_set 的清理链（run_retention_cleanup）。
pub async fn proxy_log_cleanup_expired(db: State<'_, Db>) -> Result<(), String> {
    let settings: ProxyLogSettings = aidog_db::get_setting(&db, "proxy", "logging").await
        .ok()
        .flatten()
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or_default();
    run_retention_cleanup(&db, &settings).await;
    Ok(())
}
}

#[cfg(test)]
#[path = "test_proxy_log.rs"]
mod test_proxy_log;
