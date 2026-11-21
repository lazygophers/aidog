use crate::gateway;
use aidog_db::Db;
use rusqlite::OptionalExtension;

use gateway::models::{
    ProxyLog, ProxyLogFilter, ProxyLogPage, ProxyLogSettings, ProxyLogSummary, RequestLogFilter,
    RequestLogSummary,
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
/// 请求记录页列表（独立表 request_log，不含模型 / token）。
pub async fn request_log_list(
    filter: RequestLogFilter,
    limit: u32,
    offset: u32) -> Result<Vec<RequestLogSummary>, String> {
    let db = aidog_ctx::db();
    tracing::debug!(command = "request_log_list", limit, offset, "command invoked");
    aidog_logs::list_request_logs(db, &filter, limit, offset).await
}
}

crate::tauri_command! {
pub async fn request_log_count(filter: RequestLogFilter) -> Result<u32, String> {
    let db = aidog_ctx::db();
    aidog_logs::count_request_logs(db, &filter).await
}
}

crate::tauri_command! {
pub async fn request_log_get(id: String) -> Result<Option<ProxyLog>, String> {
    let db = aidog_ctx::db();
    aidog_logs::get_request_log(db, &id).await
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
/// 平台使用统计。`window` 缺省 = all（旧调用兼容）；today 与既有今日口径一致。
pub async fn platform_usage_stats(platform_id: u64, window: Option<String>) -> Result<gateway::models::PlatformUsageStats, String> {
    let db = aidog_ctx::db();
    tracing::debug!(command = "platform_usage_stats", platform_id, "command invoked");
    aidog_stats::get_platform_usage_stats_window(db, platform_id, window.as_deref()).await
}
}

crate::tauri_command! {
/// 平台×时间用量序列（平台详情页曲线）。today → 小时桶，其余窗口 → 天桶。
pub async fn platform_usage_series(platform_id: u64, window: Option<String>) -> Result<Vec<gateway::models::PlatformUsageSeriesPoint>, String> {
    let db = aidog_ctx::db();
    aidog_stats::platform_usage_series(db, platform_id, window.as_deref().unwrap_or("all")).await
}
}

crate::tauri_command! {
/// 平台×模型分布（平台详情页），请求数降序。
pub async fn platform_model_distribution(platform_id: u64, window: Option<String>) -> Result<Vec<gateway::models::PlatformModelDistributionRow>, String> {
    let db = aidog_ctx::db();
    aidog_stats::platform_model_distribution(db, platform_id, window.as_deref().unwrap_or("all")).await
}
}

/// 平台健康明细现算（`platform_health_detail` 命令体）。now_ms 可注入（测试定冷却剩余秒）。
/// platform.db 单闭包取齐健康行与平台行；平台行缺失（已删平台）→ 高峰禁用态 false、其余全零。
pub async fn compute_platform_health_detail(
    db: &Db,
    platform_id: u64,
    now_ms: i64,
) -> Result<gateway::models::PlatformHealthDetail, String> {
    let pid = platform_id as i64;
    let (health, platform) = db
        .call_read_platform_traced(None, std::panic::Location::caller(), move |conn| {
            let health = conn
                .query_row(
                    "SELECT breaker_state, breaker_until_ms, quota_cooldown_until_ms, \
                     quota_verdict_until_ms, auth_cooldown_until_ms, balance_cooldown_until_ms, \
                     last_connect_fail_ms, updated_at, breaker_fails \
                     FROM platform_health_state WHERE platform_id = ?1",
                    [pid],
                    |r| {
                        Ok((
                            r.get::<_, String>(0)?,
                            r.get::<_, i64>(1)?,
                            r.get::<_, i64>(2)?,
                            r.get::<_, i64>(3)?,
                            r.get::<_, i64>(4)?,
                            r.get::<_, i64>(5)?,
                            r.get::<_, i64>(6)?,
                            r.get::<_, i64>(7)?,
                            r.get::<_, i64>(8)?,
                        ))
                    },
                )
                .optional()?;
            let platform: Option<(String, String)> = conn
                .query_row(
                    "SELECT extra, platform_type FROM platform WHERE id = ?1 AND deleted_at = 0",
                    [pid],
                    |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)),
                )
                .optional()?;
            Ok((health, platform))
        })
        .await
        .map_err(|e| format!("platform health detail: {e}"))?;

    let remaining = |until_ms: i64| -> i64 { ((until_ms - now_ms).max(0)) / 1000 };
    let (breaker_state, breaker_until_ms, quota_until, verdict_until, auth_until, balance_until, last_connect_fail_ms, updated_at, breaker_fails) =
        health.unwrap_or_default();
    let peak_disabled = match platform {
        Some((extra, ptype)) => {
            let parsed = gateway::models::PlatformExtra::parse(&extra);
            parsed.disable_during_peak
                && {
                    let windows = gateway::peak::peak_for(&extra, &ptype);
                    gateway::peak::is_in_peak_window(&windows, now_ms, "")
                }
        }
        None => false,
    };
    Ok(gateway::models::PlatformHealthDetail {
        platform_id: pid,
        breaker_open: breaker_state == "open",
        breaker_fails,
        breaker_until_ms,
        quota_cooldown_secs: remaining(quota_until),
        quota_verdict_secs: remaining(verdict_until),
        auth_cooldown_secs: remaining(auth_until),
        balance_cooldown_secs: remaining(balance_until),
        last_connect_fail_ms,
        peak_disabled,
        updated_at,
    })
}

crate::tauri_command! {
/// 平台额度窗口水位（详情页板块 F）：extra.quota_windows 配置 + quota_window_usage 现算
/// 已用 / 剩余比例 / 重置时刻（读时惰性重置语义，与执行层窗口边界算法同口径）。
pub async fn platform_quota_window_levels(platform_id: u64) -> Result<Vec<gateway::models::PlatformQuotaWindowLevel>, String> {
    let db = aidog_ctx::db();
    compute_platform_quota_window_levels(db, platform_id, chrono::Utc::now().timestamp_millis()).await
}
}

crate::tauri_command! {
/// 平台限频实时占用（详情页板块 F 限频行）：extra.rate_limits 内存计数快照
/// （`SchedulerState.rate`，并发在途 / 60 秒窗口 RPM / 窗口 TPM）。
/// 读取通道 = `scheduling::global()`（start_proxy 安装；proxy_stop 不卸载，停止后读最后实例快照；
/// 代理从未启动过 → 空列表）。全部为真实内存计数，不虚构、不兜底。
pub async fn platform_rate_occupancy(platform_id: u64) -> Result<Vec<gateway::models::PlatformRateOccupancy>, String> {
    Ok(compute_platform_rate_occupancy(platform_id, chrono::Utc::now().timestamp_millis()))
}
}

/// `platform_rate_occupancy` 命令体，now 可注入（测试）。全局调度器未安装（代理从未启动）→ 空列表。
pub fn compute_platform_rate_occupancy(platform_id: u64, now_ms: i64) -> Vec<gateway::models::PlatformRateOccupancy> {
    let Some(sched) = crate::gateway::scheduling::global() else { return Vec::new() };
    sched
        .rate
        .snapshot_platform(platform_id, now_ms)
        .into_iter()
        .map(|s| gateway::models::PlatformRateOccupancy {
            model: s.model,
            in_flight: s.in_flight,
            rpm_used: s.rpm_used,
            tpm_used: s.tpm_used,
        })
        .collect()
}

/// `platform_quota_window_levels` 命令体，now_ms 可注入（测试定窗口边界）。只读命令：
/// 平台行缺失（已删）→ 空列表。
pub async fn compute_platform_quota_window_levels(
    db: &Db,
    platform_id: u64,
    now_ms: i64,
) -> Result<Vec<gateway::models::PlatformQuotaWindowLevel>, String> {
    let pid = platform_id as i64;
    let extra = db
        .call_read_platform_traced(None, std::panic::Location::caller(), move |conn| {
            conn.query_row(
                "SELECT extra FROM platform WHERE id = ?1 AND deleted_at = 0",
                [pid],
                |r| r.get::<_, String>(0),
            )
            .optional()
            .map(|v| v.unwrap_or_default())
        })
        .await
        .map_err(|e| e.to_string())?;
    let windows = aidog_db::models::parse_quota_windows(&extra);
    let mut usage = gateway::quota_window::load_usage(db, &[platform_id]).await
        .remove(&platform_id).unwrap_or_default();
    usage.sort_by(|a, b| a.window_key.cmp(&b.window_key));
    let levels = gateway::quota_window::water_levels(&windows, &usage, now_ms);
    Ok(levels.into_iter().map(|level| gateway::models::PlatformQuotaWindowLevel {
        key: level.key,
        dimension: level.dimension.into(),
        reset_mode: level.reset_mode.into(),
        used: level.used,
        budget: level.budget,
        remaining_ratio: level.remaining_ratio,
        resets_at_ms: level.resets_at_ms,
    }).collect())
}

crate::tauri_command! {
/// 平台健康明细（平台详情页徽标）：platform_health_state 现算 + 高峰禁带态。
pub async fn platform_health_detail(platform_id: u64) -> Result<gateway::models::PlatformHealthDetail, String> {
    let db = aidog_ctx::db();
    compute_platform_health_detail(db, platform_id, chrono::Utc::now().timestamp_millis()).await
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
pub async fn group_last_requests(group_keys: Vec<String>) -> Result<std::collections::HashMap<String, gateway::models::GroupLastRequest>, String> {
    let db = aidog_ctx::db();
    aidog_stats::group_last_requests(db, &group_keys).await
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
    // 运行中的 proxy 每请求读 settings_cache 的 typed 快照，不回查 DB：
    // 只写 DB 不刷缓存 → 刚打开的日志开关要重启代理才生效（日志页一直空）。
    gateway::proxy::refresh_proxy_settings_cache(db).await;
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
    // mitm_log body 两列对称清空（cc-sub-mitm 票 10：同 upstream 侧口径，SET '' 不删行）。
    if let Err(e) = aidog_logs::cleanup_mitm_log_bodies(
        db,
        settings.upstream_request_retention_days,
        settings.upstream_request_retention_unit,
    )
    .await
    {
        tracing::warn!(command = "proxy_log_cleanup", error = %e, "cleanup mitm_log bodies failed");
    }
    // Delete entire log rows older than overall retention (hard delete → physical row removal)
    if settings.retention_days > 0
        && let Err(e) =
            aidog_logs::cleanup_proxy_logs(db, settings.retention_days, settings.retention_unit)
                .await
    {
        tracing::warn!(command = "proxy_log_cleanup", error = %e, "cleanup proxy_logs failed");
    }
    // quota_snapshot 同策略删整行（chart-engine T4 / #34，spec D4「retention 对齐 90d」：
    // 与 proxy_log retention_days 共用同一清理链与同一设置）。
    // mitm_log 整行同策略删（cc-sub-mitm 票 10 spec §3.2：跟随现有 retention_days，90d）。
    if settings.retention_days > 0
        && let Err(e) = aidog_stats::cleanup_quota_snapshots(
            db,
            settings.retention_days,
            settings.retention_unit,
        )
        .await
    {
        tracing::warn!(command = "proxy_log_cleanup", error = %e, "cleanup quota_snapshots failed");
    }
    if settings.retention_days > 0
        && let Err(e) =
            aidog_logs::cleanup_mitm_logs(db, settings.retention_days, settings.retention_unit)
                .await
    {
        tracing::warn!(command = "proxy_log_cleanup", error = %e, "cleanup mitm_log failed");
    }
    // 管理接口调用记录：固定 7 天（API 票 03），不随 retention_days 设置。
    if let Err(e) = aidog_db::cleanup_api_call_logs(db, aidog_db::API_CALL_LOG_RETENTION_DAYS).await
    {
        tracing::warn!(command = "proxy_log_cleanup", error = %e, "cleanup api_call_log failed");
    }
    // 工具日志：独立保留天数（设置 tool_log/retention_days，缺省 30，0 = 永久）。
    if let Err(e) =
        aidog_db::cleanup_tool_logs(db, super::tool_log::load_retention_days(db).await).await
    {
        tracing::warn!(command = "proxy_log_cleanup", error = %e, "cleanup tool_log failed");
    }
    // 客户端用量库（issue #65 票 01）：独立保留天数（scope=client_usage, key=retention_days，
    // 缺省 90，0 = 永久）。删超期明细 + 空会话壳；进度表是防重复导入真值，不清理。
    if let Err(e) =
        aidog_db::cleanup_client_usage(db, aidog_db::client_usage::load_retention_days(db).await)
            .await
    {
        tracing::warn!(command = "proxy_log_cleanup", error = %e, "cleanup client_usage failed");
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

/// 保留期清理链单轮（桌面壳 `app_setup.rs` 与内核 `spawn_scheduled_jobs` 共用）：
/// 软删硬删 → 通知收件箱 → session_name 缓存 → stats_agg retention → proxy_log 三级
/// → 按库空闲页占比回收（≥25% 全量 VACUUM，否则 incremental）。单步失败 `tracing::warn!`
/// 容错，不阻塞其余步骤。
pub async fn scheduled_cleanup_pass(db: &Db, older_than_secs: i64) {
    match aidog_db::purge_all_soft_deleted(db, older_than_secs).await {
        Ok(map) if !map.is_empty() && map.values().any(|&n| n > 0) => {
            tracing::info!(purged = ?map, "scheduled: purged old soft-deleted rows across all tables");
        }
        Ok(_) => tracing::debug!("scheduled: purge_all_soft_deleted ran, nothing to delete"),
        Err(e) => {
            tracing::warn!(error = %e, "scheduled: purge_all_soft_deleted failed (all tables errored)")
        }
    }
    let retention_days = aidog_db::get_notification_settings(db)
        .await
        .inbox_retention_days;
    if let Err(e) = aidog_db::cleanup_notifications(db, retention_days).await {
        tracing::warn!(error = %e, "scheduled: cleanup notifications failed");
    }
    if let Err(e) = aidog_db::cleanup_session_name_cache(db).await {
        tracing::warn!(error = %e, "scheduled: cleanup session_name_cache failed");
    }
    let stats_settings: gateway::models::StatsSettings =
        aidog_db::get_setting(db, "stats", "settings")
            .await
            .ok()
            .flatten()
            .and_then(|v| serde_json::from_value(v).ok())
            .unwrap_or_default();
    if let Err(e) = aidog_stats::cleanup_stats_agg(db, stats_settings.retention_days).await {
        tracing::warn!(error = %e, "scheduled: cleanup stats_agg failed");
    }
    let log_settings: ProxyLogSettings = aidog_db::get_setting(db, "proxy", "logging")
        .await
        .ok()
        .flatten()
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or_default();
    run_retention_cleanup(db, &log_settings).await;
    match aidog_db::scheduled_vacuum(db).await {
        Ok(actions) => tracing::info!(?actions, "scheduled: vacuum by free-page ratio done"),
        Err(e) => tracing::warn!(error = %e,
            "scheduled: vacuum failed (locked or disk full), will retry next cycle"),
    }
}

#[cfg(test)]
#[path = "test_proxy_log.rs"]
mod test_proxy_log;

crate::tauri_command! {
pub async fn stats_flow_breakdown(
    window: String,
    group_key: Option<String>) -> Result<Vec<gateway::models::FlowRow>, String> {
    let db = aidog_ctx::db();
    aidog_stats::stats_flow_breakdown(db, &window, group_key.as_deref()).await
}
}

crate::tauri_command! {
pub async fn group_daily_usage(
    group_key: String) -> Result<Vec<gateway::models::GroupDailyBreakdown>, String> {
    let db = aidog_ctx::db();
    aidog_stats::group_daily_breakdown(db, &group_key, 7).await
}
}

crate::tauri_command! {
pub async fn group_latest_routing(
    group_key: String) -> Result<Option<gateway::models::LatestRouting>, String> {
    let db = aidog_ctx::db();
    aidog_stats::latest_routing_snapshot(db, &group_key).await
}
}

crate::tauri_command! {
/// 会话归组聚合（magpie 票 08）：近 `days` 天（缺省 7，clamp 1..=90）按
/// proxy_log.session_key 聚合请求数 / token / est_cost；标题 = 会话首条有
/// request_body 的请求前 60 字符；空 session_key 不参与聚合。
pub async fn session_stats(
    days: Option<u32>) -> Result<Vec<gateway::models::SessionStats>, String> {
    let db = aidog_ctx::db();
    aidog_stats::get_session_stats(db, days).await
}
}
