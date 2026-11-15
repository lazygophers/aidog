//! 管理接口调用记录读写（log.db，API 票 03）。只存元数据，不存请求与响应正文。

use crate::models::ApiCallLog;
use crate::{Db, now, retention_cutoff};
use rusqlite::params;

/// 保留天数（固定 7 天，随 `run_retention_cleanup` 清理）。
pub const API_CALL_LOG_RETENTION_DAYS: u32 = 7;

/// 来源：第三方调用。
pub const API_CALL_SOURCE_THIRD_PARTY: &str = "third_party";
/// 来源：设置页测试面板（请求带 `X-Aidog-Source: settings-test`）。
pub const API_CALL_SOURCE_SETTINGS_TEST: &str = "settings_test";

/// 落一行调用记录。`success` 由 `status < 400` 派生。
#[track_caller]
pub fn insert_api_call_log<'a>(
    db: &'a Db,
    command: &'a str,
    status_code: i32,
    duration_ms: i64,
    user_agent: &'a str,
    source: &'a str,
) -> impl std::future::Future<Output = Result<(), String>> + 'a {
    let __db_caller = std::panic::Location::caller();
    async move {
        let (command, user_agent, source) =
            (command.to_string(), user_agent.to_string(), source.to_string());
        let ts = now();
        db.call_proxy_log_traced(None, __db_caller, move |conn| {
            conn.execute(
                "INSERT INTO api_call_log (created_at, command, status_code, success, duration_ms, user_agent, source) VALUES (?1,?2,?3,?4,?5,?6,?7)",
                params![ts, command, status_code, (status_code < 400) as i64, duration_ms, user_agent, source],
            )?;
            Ok(())
        })
        .await
        .map_err(|e| format!("insert api_call_log: {e}"))
    }
}

/// 最近调用（created_at 倒序）。
#[track_caller]
pub fn list_api_call_logs(
    db: &Db,
    limit: i64,
) -> impl std::future::Future<Output = Result<Vec<ApiCallLog>, String>> + '_ {
    let __db_caller = std::panic::Location::caller();
    async move {
        db.call_read_proxy_log_traced(None, __db_caller, move |conn| {
            let mut stmt = conn.prepare(
                "SELECT id, created_at, command, status_code, success, duration_ms, user_agent, source FROM api_call_log ORDER BY created_at DESC, id DESC LIMIT ?1",
            )?;
            let rows = stmt.query_map(params![limit], |r| {
                Ok(ApiCallLog {
                    id: r.get(0)?,
                    created_at: r.get(1)?,
                    command: r.get(2)?,
                    status_code: r.get(3)?,
                    success: r.get::<_, i64>(4)? != 0,
                    duration_ms: r.get(5)?,
                    user_agent: r.get(6)?,
                    source: r.get(7)?,
                })
            })?;
            rows.collect::<rusqlite::Result<Vec<_>>>()
        })
        .await
        .map_err(|e| e.to_string())
    }
}

/// 删超过 `days` 天的行（`days == 0` 跳过）。
#[track_caller]
pub fn cleanup_api_call_logs(
    db: &Db,
    days: u32,
) -> impl std::future::Future<Output = Result<(), String>> + '_ {
    let __db_caller = std::panic::Location::caller();
    async move {
        let Some(cutoff) = retention_cutoff(days) else {
            return Ok(());
        };
        db.call_proxy_log_traced(None, __db_caller, move |conn| {
            conn.execute("DELETE FROM api_call_log WHERE created_at < ?1", params![cutoff])?;
            Ok(())
        })
        .await
        .map_err(|e| format!("cleanup api_call_log: {e}"))
    }
}
