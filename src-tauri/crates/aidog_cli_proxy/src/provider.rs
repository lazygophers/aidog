//! CLI 代理 provider CRUD Tauri command（cpa-standalone-module s3）。
//!
//! 薄壳：转 `gateway::db` 的 `*_cli_proxy_provider` 函数。

use aidog_db as db;
// TODO-unknown: self
use aidog_core::gateway::models::{CliProxyProvider, CreateCliProxyProvider};

aidog_core::tauri_command! {
    /// 列出全部 cli_proxy_provider。
    pub async fn cli_proxy_list() -> Result<Vec<CliProxyProvider>, String> {
    let db = aidog_ctx::db();
        db::list_cli_proxy_providers(db).await
    }
}

aidog_core::tauri_command! {
    /// 获取单个 cli_proxy_provider。不存在返回 None。
    pub async fn cli_proxy_get( id: u64) -> Result<Option<CliProxyProvider>, String> {
    let db = aidog_ctx::db();
        tracing::debug!(command = "cli_proxy_get", id, "command invoked");
        db::get_cli_proxy_provider(db, id).await
    }
}

aidog_core::tauri_command! {
    /// 创建 cli_proxy_provider。
    pub async fn cli_proxy_create(
        input: CreateCliProxyProvider) -> Result<CliProxyProvider, String> {
    let db = aidog_ctx::db();
        tracing::debug!(command = "cli_proxy_create", name = %input.name, "command invoked");
        db::create_cli_proxy_provider(db, input).await
    }
}

aidog_core::tauri_command! {
    /// 全量覆写更新 cli_proxy_provider。不存在返回 None。
    pub async fn cli_proxy_update(
        id: u64,
        input: CreateCliProxyProvider) -> Result<Option<CliProxyProvider>, String> {
    let db = aidog_ctx::db();
        tracing::debug!(command = "cli_proxy_update", id, name = %input.name, "command invoked");
        db::update_cli_proxy_provider(db, id, input).await
    }
}

aidog_core::tauri_command! {
    /// 删除 cli_proxy_provider。不存在返回 false。
    pub async fn cli_proxy_delete( id: u64) -> Result<bool, String> {
    let db = aidog_ctx::db();
        tracing::debug!(command = "cli_proxy_delete", id, "command invoked");
        db::delete_cli_proxy_provider(db, id).await
    }
}
