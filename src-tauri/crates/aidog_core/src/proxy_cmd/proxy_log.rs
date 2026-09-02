use crate::gateway;
use aidog_db::Db;

use gateway::models::{
    ProxyLog, ProxyLogFilter, ProxyLogPage, ProxyLogSettings, ProxyLogSummary, RequestLogSummary,
};

crate::tauri_command! {
pub async fn proxy_log_list( limit: u32, offset: u32) -> Result<Vec<ProxyLogSummary>, String> {
    let db = aidog_ctx::db();
    tracing::debug!(command = "proxy_log_list", limit, offset, "command invoked");
    aidog_logs::list_proxy_logs(db, limit, offset).await
}
}

crate::tauri_command! {
pub async fn proxy_log_list_filtered(
    filter: ProxyLogFilter,
    limit: u32,
    offset: u32) -> Result<ProxyLogPage, String> {
    let db = aidog_ctx::db();
    tracing::debug!(command = "proxy_log_list_filtered", limit, offset, "command invoked");
    aidog_logs::filtered_list_proxy_logs(db, &filter, limit, offset).await
}
}

crate::tauri_command! {
/// Logs 页 model 下拉选项（logs-query-ipc-slimming s4）：`SELECT DISTINCT` 直出选项列表，
/// 替代旧的「拉 200 行完整日志再前端去重」。`actual=true` 取 actual_model 列，否则取 model 列
/// （对应前端 filterModelType 状态）；`filter` 通常仅带 exclude_sources，与主列表筛选一致。
pub async fn proxy_log_distinct_models(
    filter: ProxyLogFilter,
    actual: bool,
    limit: u32) -> Result<Vec<String>, String> {
    let db = aidog_ctx::db();
    tracing::debug!(command = "proxy_log_distinct_models", actual, limit, "command invoked");
    aidog_logs::distinct_models_proxy_log(db, &filter, actual, limit).await
}
}

crate::tauri_command! {
pub async fn proxy_log_count_filtered(
    filter: ProxyLogFilter) -> Result<u32, String> {
    let db = aidog_ctx::db();
    aidog_logs::filtered_count_proxy_logs(db, &filter).await
}
}

crate::tauri_command! {
/// 请求日志页列表（cli-proxy-request-log s3）。
/// 默认 sources=[test,quota]（db 层兜底）；前端可显式传 filter 覆盖。
/// 返回 RequestLogSummary（含 cli_proxy_provider_name，db 层应用层合并 provider 表，跨库禁 JOIN）。
pub async fn request_log_list(
    filter: ProxyLogFilter,
    limit: u32,
    offset: u32) -> Result<Vec<RequestLogSummary>, String> {
    let db = aidog_ctx::db();
    tracing::debug!(command = "request_log_list", limit, offset, "command invoked");
    aidog_logs::list_request_logs(db, &filter, limit, offset).await
}
}

crate::tauri_command! {
pub async fn proxy_log_get(id: String) -> Result<Option<ProxyLog>, String> {
    let db = aidog_ctx::db();
    tracing::debug!(command = "proxy_log_get", id = %id, "command invoked");
    aidog_logs::get_proxy_log(db, &id).await
}
}

crate::tauri_command! {
pub async fn proxy_log_clear() -> Result<(), String> {
    let db = aidog_ctx::db();
    aidog_logs::clear_proxy_logs(db).await
}
}

crate::tauri_command! {
pub async fn proxy_log_count() -> Result<u32, String> {
    let db = aidog_ctx::db();
    aidog_db::count_proxy_logs(db).await
}
}

crate::tauri_command! {
pub async fn platform_usage_stats(platform_id: u64) -> Result<gateway::models::PlatformUsageStats, String> {
    let db = aidog_ctx::db();
    tracing::debug!(command = "platform_usage_stats", platform_id, "command invoked");
    aidog_stats::get_platform_usage_stats(db, platform_id).await
}
}

crate::tauri_command! {
pub async fn group_usage_stats(group_key: String) -> Result<gateway::models::PlatformUsageStats, String> {
    let db = aidog_ctx::db();
    tracing::debug!(command = "group_usage_stats", group_key = %group_key, "command invoked");
    aidog_stats::get_group_usage_stats(db, &group_key).await
}
}

crate::tauri_command! {
pub async fn all_group_usage_stats() -> Result<std::collections::HashMap<String, gateway::models::PlatformUsageStats>, String> {
    let db = aidog_ctx::db();
    aidog_stats::get_all_group_usage_stats(db).await
}
}

crate::tauri_command! {
pub async fn all_platform_usage_stats() -> Result<std::collections::HashMap<u64, gateway::models::PlatformUsageStats>, String> {
    let db = aidog_ctx::db();
    aidog_stats::platform_usage_stats_all(db).await
}
}

crate::tauri_command! {
pub async fn get_last_test_result(platform_id: u64) -> Result<Option<gateway::models::LastTestResult>, String> {
    let db = aidog_ctx::db();
    tracing::debug!(command = "get_last_test_result", platform_id, "command invoked");
    aidog_stats::get_last_test_result(db, platform_id).await
}
}

crate::tauri_command! {
pub async fn proxy_log_settings_get() -> Result<ProxyLogSettings, String> {
    let db = aidog_ctx::db();
    let val = aidog_db::get_setting(db, "proxy", "logging").await
        .ok()
        .flatten()
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or_default();
    Ok(val)
}
}

crate::tauri_command! {
pub async fn proxy_log_settings_set( settings: ProxyLogSettings) -> Result<(), String> {
    let db = aidog_ctx::db();
    let value = serde_json::to_value(&settings)
        .map_err(|e| format!("serialize log settings: {e}"))?;
    aidog_db::set_setting(db, gateway::models::SetSettingInput {
        scope: "proxy".into(),
        key: "logging".into(),
        value,
    }).await
        .map_err(|e| { tracing::error!(command = "proxy_log_settings_set", error = %e, "persist log settings failed"); e })?;
    run_retention_cleanup(db, &settings).await;
    // 保留期变了 → 调度周期必须跟着变：唤醒清理循环重读设置重算周期，不必重启应用。
    wake_retention_scheduler();
    Ok(())
}
}

/// 清理调度器唤醒信号（单消费者：app_setup 的清理循环）。
///
/// `notify_one` 语义：设置保存时若循环正在跑清理（无 waiter），permit 会被存住，
/// 循环下一次 `notified()` 立即返回 —— 唤醒不会丢。
fn retention_wake() -> &'static tokio::sync::Notify {
    static WAKE: std::sync::OnceLock<tokio::sync::Notify> = std::sync::OnceLock::new();
    WAKE.get_or_init(tokio::sync::Notify::new)
}

/// 唤醒清理调度器：让它立刻重读 `ProxyLogSettings` 并重算周期。
pub fn wake_retention_scheduler() {
    retention_wake().notify_one();
}

/// 读当前 `ProxyLogSettings` 派生本轮调度周期（派生规则见 `cleanup_interval_secs`）。
/// 设置读不到时按 `Default`（新装 6h → 90 分钟）走，不返回写死的 24 小时。
pub async fn retention_cleanup_interval(db: &Db) -> std::time::Duration {
    let settings: ProxyLogSettings = aidog_db::get_setting(db, "proxy", "logging")
        .await
        .ok()
        .flatten()
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or_default();
    std::time::Duration::from_secs(settings.cleanup_interval_secs())
}

/// 等下一个清理周期：`sleep(interval)` 与「设置变更唤醒」谁先到就返回谁。
/// 返回 `true` = 被设置变更唤醒（调用方应重算周期，本轮不清理：`settings_set` 已自跑清理链）；
/// `false` = 周期到点（调用方执行清理）。
pub async fn wait_next_retention_cycle(interval: std::time::Duration) -> bool {
    tokio::select! {
        _ = tokio::time::sleep(interval) => false,
        _ = retention_wake().notified() => true,
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
pub async fn proxy_log_cleanup_expired() -> Result<(), String> {
    let db = aidog_ctx::db();
    let settings: ProxyLogSettings = aidog_db::get_setting(db, "proxy", "logging").await
        .ok()
        .flatten()
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or_default();
    run_retention_cleanup(db, &settings).await;
    Ok(())
}
}

crate::tauri_command! {
/// 手动清理前的只读预估：超期行数 + 这些行 body 字节总和 + log.db 当前大小。
/// 口径按当前 ProxyLogSettings 的整行保留期，与 proxy_log_cleanup_expired 的删除谓词同源。
pub async fn proxy_log_cleanup_estimate() -> Result<gateway::models::CleanupEstimate, String> {
    let db = aidog_ctx::db();
    let settings: ProxyLogSettings = aidog_db::get_setting(db, "proxy", "logging").await
        .ok()
        .flatten()
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or_default();
    aidog_logs::estimate_cleanup(db, settings.retention_days, settings.retention_unit).await
}
}

#[cfg(test)]
#[path = "test_proxy_log.rs"]
mod test_proxy_log;
