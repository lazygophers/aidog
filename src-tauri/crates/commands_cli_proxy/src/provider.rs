//! CLI 代理 provider CRUD Tauri command（cpa-standalone-module s3）。
//!
//! 薄壳：转 `aidog_core::gateway::db` 的 `*_cli_proxy_provider` 函数。入参/出参类型从
//! aidog_core re-export，保持与 DB 层 struct 字段对齐（跨边界一致性）。

use aidog_core::gateway::db::{self, Db};
use aidog_core::gateway::models::{CliProxyProvider, CreateCliProxyProvider};
use tauri::State;

/// 列出全部 cli_proxy_provider。
#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %aidog_core::logging::new_trace_id()))]
pub async fn cli_proxy_list(db: State<'_, Db>) -> Result<Vec<CliProxyProvider>, String> {
    tracing::debug!(command = "cli_proxy_list", "command invoked");
    db::list_cli_proxy_providers(&db).await
}

/// 获取单个 cli_proxy_provider。不存在返回 None。
#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %aidog_core::logging::new_trace_id()))]
pub async fn cli_proxy_get(db: State<'_, Db>, id: u64) -> Result<Option<CliProxyProvider>, String> {
    tracing::debug!(command = "cli_proxy_get", id, "command invoked");
    db::get_cli_proxy_provider(&db, id).await
}

/// 创建 cli_proxy_provider。
#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %aidog_core::logging::new_trace_id()))]
pub async fn cli_proxy_create(
    db: State<'_, Db>,
    input: CreateCliProxyProvider,
) -> Result<CliProxyProvider, String> {
    tracing::debug!(command = "cli_proxy_create", name = %input.name, "command invoked");
    db::create_cli_proxy_provider(&db, input).await
}

/// 全量覆写更新 cli_proxy_provider。不存在返回 None。
#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %aidog_core::logging::new_trace_id()))]
pub async fn cli_proxy_update(
    db: State<'_, Db>,
    id: u64,
    input: CreateCliProxyProvider,
) -> Result<Option<CliProxyProvider>, String> {
    tracing::debug!(command = "cli_proxy_update", id, name = %input.name, "command invoked");
    db::update_cli_proxy_provider(&db, id, input).await
}

/// 删除 cli_proxy_provider。不存在返回 false。
#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %aidog_core::logging::new_trace_id()))]
pub async fn cli_proxy_delete(db: State<'_, Db>, id: u64) -> Result<bool, String> {
    tracing::debug!(command = "cli_proxy_delete", id, "command invoked");
    db::delete_cli_proxy_provider(&db, id).await
}
