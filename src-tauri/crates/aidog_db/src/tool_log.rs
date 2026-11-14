//! 工具日志读写（log.db，与 api_call_log 同库同 handle）。

use crate::models::{ToolLog, ToolLogFilter, ToolLogPage};
use crate::{Db, now, retention_cutoff};
use rusqlite::{params, params_from_iter, types::Value};

/// 默认保留天数。
pub const TOOL_LOG_DEFAULT_RETENTION_DAYS: u32 = 30;

const COLS: &str = "id, created_at, tool, trigger, group_name, platform, query, result_count, engines, duration_ms, success, error";

/// 落一行；`created_at` 由此处盖章。返回新行 id。
#[track_caller]
pub fn insert_tool_log<'a>(
    db: &'a Db,
    row: &'a ToolLog,
) -> impl std::future::Future<Output = Result<i64, String>> + 'a {
    let __db_caller = std::panic::Location::caller();
    async move {
        let r = row.clone();
        let ts = now();
        db.call_proxy_log_traced(None, __db_caller, move |conn| {
            conn.execute(
                "INSERT INTO tool_log (created_at, tool, trigger, group_name, platform, query, result_count, engines, duration_ms, success, error, result) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12)",
                params![ts, r.tool, r.trigger, r.group_name, r.platform, r.query, r.result_count, r.engines, r.duration_ms, r.success as i64, r.error, r.result],
            )?;
            Ok(conn.last_insert_rowid())
        })
        .await
        .map_err(|e| format!("insert tool_log: {e}"))
    }
}

fn map_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<ToolLog> {
    Ok(ToolLog {
        id: r.get(0)?,
        created_at: r.get(1)?,
        tool: r.get(2)?,
        trigger: r.get(3)?,
        group_name: r.get(4)?,
        platform: r.get(5)?,
        query: r.get(6)?,
        result_count: r.get(7)?,
        engines: r.get(8)?,
        duration_ms: r.get(9)?,
        success: r.get::<_, i64>(10)? != 0,
        error: r.get(11)?,
        result: String::new(),
    })
}

/// 分页列表（倒序，不带 result）。
#[track_caller]
pub fn list_tool_logs<'a>(
    db: &'a Db,
    filter: &'a ToolLogFilter,
    limit: i64,
    offset: i64,
) -> impl std::future::Future<Output = Result<ToolLogPage, String>> + 'a {
    let __db_caller = std::panic::Location::caller();
    async move {
        let mut conds = Vec::new();
        let mut args: Vec<Value> = Vec::new();
        for (col, v) in [
            ("tool", &filter.tool),
            ("trigger", &filter.trigger),
            ("group_name", &filter.group_name),
        ] {
            if let Some(v) = v.as_ref().filter(|s| !s.is_empty()) {
                conds.push(format!("{col} = ?"));
                args.push(Value::Text(v.clone()));
            }
        }
        let wh = if conds.is_empty() { String::new() } else { format!(" WHERE {}", conds.join(" AND ")) };
        db.call_read_proxy_log_traced(None, __db_caller, move |conn| {
            let total: i64 = conn.query_row(
                &format!("SELECT COUNT(*) FROM tool_log{wh}"),
                params_from_iter(args.iter()),
                |r| r.get(0),
            )?;
            let mut a = args.clone();
            a.push(Value::Integer(limit));
            a.push(Value::Integer(offset));
            let mut stmt = conn.prepare(&format!(
                "SELECT {COLS} FROM tool_log{wh} ORDER BY created_at DESC, id DESC LIMIT ? OFFSET ?"
            ))?;
            let rows = stmt
                .query_map(params_from_iter(a.iter()), map_row)?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            Ok(ToolLogPage { total, rows })
        })
        .await
        .map_err(|e| e.to_string())
    }
}

/// 详情（带 result）。
#[track_caller]
pub fn get_tool_log(
    db: &Db,
    id: i64,
) -> impl std::future::Future<Output = Result<Option<ToolLog>, String>> + '_ {
    let __db_caller = std::panic::Location::caller();
    async move {
        db.call_read_proxy_log_traced(None, __db_caller, move |conn| {
            let mut stmt =
                conn.prepare(&format!("SELECT {COLS}, result FROM tool_log WHERE id = ?1"))?;
            let mut it = stmt.query_map(params![id], |r| {
                let mut l = map_row(r)?;
                l.result = r.get(12)?;
                Ok(l)
            })?;
            it.next().transpose()
        })
        .await
        .map_err(|e| e.to_string())
    }
}

/// 删超过 `days` 天的行（`days == 0` 跳过）。
#[track_caller]
pub fn cleanup_tool_logs(
    db: &Db,
    days: u32,
) -> impl std::future::Future<Output = Result<(), String>> + '_ {
    let __db_caller = std::panic::Location::caller();
    async move {
        let Some(cutoff) = retention_cutoff(days) else {
            return Ok(());
        };
        db.call_proxy_log_traced(None, __db_caller, move |conn| {
            conn.execute("DELETE FROM tool_log WHERE created_at < ?1", params![cutoff])?;
            Ok(())
        })
        .await
        .map_err(|e| format!("cleanup tool_log: {e}"))
    }
}
