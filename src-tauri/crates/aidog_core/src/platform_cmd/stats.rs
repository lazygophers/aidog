use crate::gateway;
use aidog_db::Db;
use gateway::models::*;
use tauri::State;


crate::tauri_command! {
pub async fn stats_query(
    db: State<'_, Db>,
    query: StatsQuery,
) -> Result<StatsResult, String> {
    aidog_stats::query_stats(&db, &query).await
}
}

crate::tauri_command! {
/// 批量统计查询：浮窗 N 卡一次 IPC 拉全部卡数据，替代每卡独立 `stats_query` fan-out。
/// 返回顺序与 `queries` 一一对应；单卡值与逐卡 `stats_query` 完全一致。
pub async fn stats_query_batch(
    db: State<'_, Db>,
    queries: Vec<StatsQuery>,
) -> Result<Vec<StatsResult>, String> {
    tracing::debug!(command = "stats_query_batch", count = queries.len(), "command invoked");
    aidog_stats::query_stats_batch(&db, queries).await
}
}

use gateway::models::StatsSettings;

crate::tauri_command! {
pub async fn stats_settings_get(db: State<'_, Db>) -> Result<StatsSettings, String> {
    Ok(aidog_db::get_setting(&db, "stats", "settings").await
        .ok()
        .flatten()
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or_default())
}
}

crate::tauri_command! {
pub async fn stats_settings_set(db: State<'_, Db>, settings: StatsSettings) -> Result<(), String> {
    let value = serde_json::to_value(&settings)
        .map_err(|e| format!("serialize stats settings: {e}"))?;
    aidog_db::set_setting(&db, gateway::models::SetSettingInput {
        scope: "stats".into(),
        key: "settings".into(),
        value,
    }).await
        .map_err(|e| { tracing::error!(command = "stats_settings_set", error = %e, "persist stats settings failed"); e })?;
    // 落盘后按新 retention 清理聚合表（0=永久跳过）。
    if let Err(e) = aidog_stats::cleanup_stats_agg(&db, settings.retention_days).await {
        tracing::warn!(command = "stats_settings_set", error = %e, "cleanup stats_agg failed");
    }
    Ok(())
}
}

crate::tauri_command! {
/// 清空 stats_agg_hourly 后从 proxy_log 全量重建（用户启用日志后修复历史聚合用）。
pub async fn stats_rebuild_from_logs(db: State<'_, Db>) -> Result<(), String> {
    aidog_stats::rebuild_stats_agg_from_logs(&db).await
}
}

#[cfg(test)]
#[path = "test_stats.rs"]
mod test_stats;
