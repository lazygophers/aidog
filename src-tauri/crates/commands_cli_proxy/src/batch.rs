//! CLI 代理 provider 批量操作（cli-proxy-batch-delete s1）。
//!
//! 3 个独立 batch command，各单条 SQL（IN 占位符）原子执行：
//! - batch_delete_cli_proxy_providers: 物理删
//! - batch_override_cli_proxy_models:   覆盖 models JSON
//! - batch_set_cli_proxy_quota:         覆盖 quota JSON

use aidog_core::gateway::{
    db::{now, Db},
    models::{serialize_cli_proxy_models, BatchReport},
};
use rusqlite::{params_from_iter, ToSql};
use tauri::State;

/// 批量删除 cli_proxy_provider（物理删，无级联）
#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %aidog_core::logging::new_trace_id()))]
pub async fn batch_delete_cli_proxy_providers(
    db: State<'_, Db>,
    ids: Vec<u64>,
) -> Result<BatchReport, String> {
    tracing::debug!(command = "batch_delete_cli_proxy_providers", count = ids.len(), "command invoked");
    if ids.is_empty() {
        return Ok(BatchReport { applied: 0, skipped: vec![] });
    }

    let placeholders = ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
    let sql = format!("DELETE FROM cli_proxy_provider WHERE id IN ({placeholders})");
    let bind: Vec<i64> = ids.iter().map(|id| *id as i64).collect();

    let changes = db
        .call_traced(None, std::panic::Location::caller(), move |conn| {
            let affected = conn.execute(&sql, params_from_iter(bind.iter()))?;
            Ok(affected as u64)
        })
        .await
        .map_err(|e| format!("batch_delete_cli_proxy_providers: {e}"))?;

    Ok(BatchReport { applied: changes, skipped: vec![] })
}

/// 批量覆盖 cli_proxy_provider models（完全覆盖，非追加）
#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %aidog_core::logging::new_trace_id()))]
pub async fn batch_override_cli_proxy_models(
    db: State<'_, Db>,
    ids: Vec<u64>,
    models: Vec<String>,
) -> Result<BatchReport, String> {
    tracing::debug!(command = "batch_override_cli_proxy_models", count = ids.len(), "command invoked");
    if ids.is_empty() {
        return Ok(BatchReport { applied: 0, skipped: vec![] });
    }

    let models_str = serialize_cli_proxy_models(&models);
    let ts = now();
    let placeholders = ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
    let sql = format!(
        "UPDATE cli_proxy_provider SET models = ?, updated_at = ? WHERE id IN ({placeholders})"
    );
    let ids_i64: Vec<i64> = ids.iter().map(|id| *id as i64).collect();

    let changes = db
        .call_traced(None, std::panic::Location::caller(), move |conn| {
            // ponytail: Vec<&dyn ToSql> 统一异类型（String + i64 + i64…）供 params_from_iter
            let mut bind: Vec<&dyn ToSql> = Vec::with_capacity(2 + ids_i64.len());
            bind.push(&models_str);
            bind.push(&ts);
            bind.extend(ids_i64.iter().map(|id| id as &dyn ToSql));
            let affected = conn.execute(&sql, params_from_iter(bind))?;
            Ok(affected as u64)
        })
        .await
        .map_err(|e| format!("batch_override_cli_proxy_models: {e}"))?;

    Ok(BatchReport { applied: changes, skipped: vec![] })
}

/// 批量设置 cli_proxy_provider quota（覆盖整 quota JSON）
#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %aidog_core::logging::new_trace_id()))]
pub async fn batch_set_cli_proxy_quota(
    db: State<'_, Db>,
    ids: Vec<u64>,
    quota: String,
) -> Result<BatchReport, String> {
    tracing::debug!(command = "batch_set_cli_proxy_quota", count = ids.len(), "command invoked");
    if ids.is_empty() {
        return Ok(BatchReport { applied: 0, skipped: vec![] });
    }

    let ts = now();
    let placeholders = ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
    let sql = format!(
        "UPDATE cli_proxy_provider SET quota = ?, updated_at = ? WHERE id IN ({placeholders})"
    );
    let ids_i64: Vec<i64> = ids.iter().map(|id| *id as i64).collect();

    let changes = db
        .call_traced(None, std::panic::Location::caller(), move |conn| {
            let mut bind: Vec<&dyn ToSql> = Vec::with_capacity(2 + ids_i64.len());
            bind.push(&quota);
            bind.push(&ts);
            bind.extend(ids_i64.iter().map(|id| id as &dyn ToSql));
            let affected = conn.execute(&sql, params_from_iter(bind))?;
            Ok(affected as u64)
        })
        .await
        .map_err(|e| format!("batch_set_cli_proxy_quota: {e}"))?;

    Ok(BatchReport { applied: changes, skipped: vec![] })
}
