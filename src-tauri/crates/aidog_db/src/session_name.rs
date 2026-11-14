//! `session_name_cache` 表（主库 aidog.db）：「会话 → 名字」解析缓存。
//!
//! 主键 `(client, session_id)`，`value` 为 JSON（读取方忽略未知键），`updated_at` Unix 秒。
//! 每次通知都会重新检查来源，缓存只省掉重复全量读（Claude Code transcript 增量偏移等）。
//! 30 天未更新的行由 [`cleanup_session_name_cache`] 清理（定时清理链 + 启动各跑一次）。

use crate::{Db, now};
use rusqlite::{Connection, OptionalExtension, params};

/// 缓存保留秒数（30 天）。
pub const SESSION_NAME_RETENTION_SECS: i64 = 30 * 24 * 3600;

/// 读缓存 value（JSON 文本）。
#[track_caller]
pub fn get_session_name_cache<'a>(
    db: &'a Db,
    client: &'a str,
    session_id: &'a str,
) -> impl std::future::Future<Output = Result<Option<String>, String>> + 'a {
    let __db_caller = std::panic::Location::caller();
    async move {
        let (client, sid) = (client.to_string(), session_id.to_string());
        db.call_read_traced(None, __db_caller, move |conn| {
            conn
                .query_row(
                    "SELECT value FROM session_name_cache WHERE client = ?1 AND session_id = ?2",
                    params![client, sid],
                    |r| r.get::<_, String>(0),
                )
                .optional()
        })
        .await
        .map_err(|e| format!("get session_name_cache: {e}"))
    }
}

/// 写缓存（整行覆盖，刷新 `updated_at`）。
#[track_caller]
pub fn put_session_name_cache<'a>(
    db: &'a Db,
    client: &'a str,
    session_id: &'a str,
    value: &'a str,
) -> impl std::future::Future<Output = Result<(), String>> + 'a {
    let __db_caller = std::panic::Location::caller();
    async move {
        let (client, sid, value) = (client.to_string(), session_id.to_string(), value.to_string());
        db.call_traced(None, __db_caller, move |conn| {
            conn.execute(
                "INSERT INTO session_name_cache (client, session_id, value, updated_at) \
                 VALUES (?1, ?2, ?3, ?4) \
                 ON CONFLICT(client, session_id) DO UPDATE SET \
                 value = excluded.value, updated_at = excluded.updated_at",
                params![client, sid, value, now() / 1000],
            )?;
            Ok(())
        })
        .await
        .map_err(|e| format!("put session_name_cache: {e}"))
    }
}

/// 删 `updated_at` 早于 30 天的行。
pub(crate) fn purge_stale_session_names(conn: &Connection) -> rusqlite::Result<usize> {
    conn.execute(
        "DELETE FROM session_name_cache WHERE updated_at < ?1",
        params![now() / 1000 - SESSION_NAME_RETENTION_SECS],
    )
}

/// 定时清理链入口（桌面 / 内核两处调度共用）。
#[track_caller]
pub fn cleanup_session_name_cache(
    db: &Db,
) -> impl std::future::Future<Output = Result<(), String>> + '_ {
    let __db_caller = std::panic::Location::caller();
    async move {
        db.call_traced(None, __db_caller, |conn| {
            purge_stale_session_names(conn)?;
            Ok(())
        })
        .await
        .map_err(|e| format!("cleanup session_name_cache: {e}"))
    }
}
