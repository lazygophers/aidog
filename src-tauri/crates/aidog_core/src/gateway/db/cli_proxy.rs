//! CLI 代理 provider CRUD（cpa-standalone-module s1）。
//!
//! 对应 `cli_proxy_provider` 表。list/get 走 read pool，create/update/delete 走 write pool。
//! idiom 对齐 mcp.rs（track_caller + call_platform_traced/call_read_platform_traced）。
//! config-db-split：cli_proxy_provider 表落 platform.db，访问走 platform handle。

use super::*;
use crate::gateway::models::{
    parse_cli_proxy_models, serialize_cli_proxy_models, CliProxyProvider, CreateCliProxyProvider,
};
use rusqlite::{params, OptionalExtension};

/// SELECT 列序
const CLI_PROXY_COLUMNS: &str =
    "id, name, wire_protocol, base_url, api_key, models, extra, quota, status, group_id, created_at, updated_at";

/// 从查询行构造 CliProxyProvider
fn row_to_provider(row: &rusqlite::Row) -> rusqlite::Result<CliProxyProvider> {
    let models_str: String = row.get(5)?;
    Ok(CliProxyProvider {
        id: row.get::<_, i64>(0)? as u64,
        name: row.get(1)?,
        wire_protocol: row.get(2)?,
        base_url: row.get(3)?,
        api_key: row.get(4)?,
        models: parse_cli_proxy_models(&models_str),
        extra: row.get(6)?,
        quota: row.get(7)?,
        status: row.get(8)?,
        group_id: row.get(9)?,
        created_at: row.get(10)?,
        updated_at: row.get(11)?,
    })
}

#[track_caller]
pub fn list_cli_proxy_providers(
    db: &Db,
) -> impl std::future::Future<Output = Result<Vec<CliProxyProvider>, String>> + '_ {
    let __db_caller = std::panic::Location::caller();
    async move {
        db.call_read_platform_traced(None, __db_caller, move |conn| {
            let mut stmt =
                conn.prepare(&format!("SELECT {CLI_PROXY_COLUMNS} FROM cli_proxy_provider ORDER BY id"))?;
            let rows = stmt.query_map([], row_to_provider)?;
            let mut out = Vec::new();
            for r in rows {
                out.push(r?);
            }
            Ok(out)
        })
        .await
        .map_err(|e| format!("list cli_proxy_providers: {e}"))
    }
}

#[track_caller]
pub fn get_cli_proxy_provider(
    db: &Db,
    id: u64,
) -> impl std::future::Future<Output = Result<Option<CliProxyProvider>, String>> + '_ {
    let __db_caller = std::panic::Location::caller();
    async move {
        db.call_read_platform_traced(None, __db_caller, move |conn| {
            Ok(conn
                .query_row(
                    &format!("SELECT {CLI_PROXY_COLUMNS} FROM cli_proxy_provider WHERE id = ?1"),
                    params![id as i64],
                    row_to_provider,
                )
                .optional()?)
        })
        .await
        .map_err(|e| format!("get cli_proxy_provider: {e}"))
    }
}

#[track_caller]
pub fn create_cli_proxy_provider(
    db: &Db,
    input: CreateCliProxyProvider,
) -> impl std::future::Future<Output = Result<CliProxyProvider, String>> + '_ {
    let __db_caller = std::panic::Location::caller();
    async move {
        let ts = now();
        db.call_platform_traced(None, __db_caller, move |conn| {
            let models_str = serialize_cli_proxy_models(&input.models);
            conn.execute(
                "INSERT INTO cli_proxy_provider \
                 (name, wire_protocol, base_url, api_key, models, extra, quota, status, group_id, created_at, updated_at) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?10)",
                params![
                    input.name,
                    input.wire_protocol,
                    input.base_url,
                    input.api_key,
                    models_str,
                    input.extra,
                    input.quota,
                    input.status,
                    input.group_id,
                    ts,
                ],
            )?;
            let id = conn.last_insert_rowid();
            Ok(conn.query_row(
                &format!("SELECT {CLI_PROXY_COLUMNS} FROM cli_proxy_provider WHERE id = ?1"),
                params![id],
                row_to_provider,
            )?)
        })
        .await
        .map_err(|e| format!("create cli_proxy_provider: {e}"))
    }
}

/// 全量覆写更新（无部分更新，对齐 mcp.rs upsert idiom）。不存在返回 None。
#[track_caller]
pub fn update_cli_proxy_provider(
    db: &Db,
    id: u64,
    input: CreateCliProxyProvider,
) -> impl std::future::Future<Output = Result<Option<CliProxyProvider>, String>> + '_ {
    let __db_caller = std::panic::Location::caller();
    async move {
        let ts = now();
        db.call_platform_traced(None, __db_caller, move |conn| {
            let models_str = serialize_cli_proxy_models(&input.models);
            let affected = conn.execute(
                "UPDATE cli_proxy_provider SET \
                   name = ?1, wire_protocol = ?2, base_url = ?3, api_key = ?4, \
                   models = ?5, extra = ?6, quota = ?7, status = ?8, group_id = ?9, updated_at = ?10 \
                 WHERE id = ?11",
                params![
                    input.name,
                    input.wire_protocol,
                    input.base_url,
                    input.api_key,
                    models_str,
                    input.extra,
                    input.quota,
                    input.status,
                    input.group_id,
                    ts,
                    id as i64,
                ],
            )?;
            if affected == 0 {
                return Ok(None);
            }
            Ok(conn
                .query_row(
                    &format!("SELECT {CLI_PROXY_COLUMNS} FROM cli_proxy_provider WHERE id = ?1"),
                    params![id as i64],
                    row_to_provider,
                )
                .optional()?)
        })
        .await
        .map_err(|e| format!("update cli_proxy_provider: {e}"))
    }
}

/// 删除指定 id 的 provider。不存在返回 false。
#[track_caller]
pub fn delete_cli_proxy_provider(
    db: &Db,
    id: u64,
) -> impl std::future::Future<Output = Result<bool, String>> + '_ {
    let __db_caller = std::panic::Location::caller();
    async move {
        db.call_platform_traced(None, __db_caller, move |conn| {
            let affected = conn.execute(
                "DELETE FROM cli_proxy_provider WHERE id = ?1",
                params![id as i64],
            )?;
            Ok(affected > 0)
        })
        .await
        .map_err(|e| format!("delete cli_proxy_provider: {e}"))
    }
}
