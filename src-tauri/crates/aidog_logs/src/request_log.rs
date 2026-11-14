//! request_log 表读写（请求记录：测试 / 余额 / 拉模型），落 log.db，独立于 proxy_log。
//! 不存 model / actual_model / token / cost。

use crate::proxy_log::{log_settings, strip_raw_sides};
use aidog_db::Db;
use aidog_db::models::*;
use rusqlite::{OptionalExtension, Result as SqlResult, params};

const COLUMNS: &str = "id, kind, group_key, platform_id, target_protocol, status_code, upstream_status_code, duration_ms, user_agent, request_url, request_headers, request_body, user_response_headers, user_response_body, upstream_request_url, upstream_request_headers, upstream_request_body, upstream_response_headers, response_body, created_at";

/// 写一条请求记录。取 ProxyLog 是为让带外写入方沿用现有构造；model / token 字段不入库。
/// 与 proxy_log 同口径过 `ProxyLogSettings` 两侧「原始信息」开关。
#[track_caller]
pub fn upsert_request_log(
    db: &Db,
    log: ProxyLog,
) -> impl std::future::Future<Output = Result<(), String>> + '_ {
    let __db_caller = std::panic::Location::caller();
    async move {
        let settings = log_settings(db).await;
        let mut log = log;
        strip_raw_sides(&mut log, !settings.log_user_request, !settings.log_upstream_request);
        db.call_proxy_log_traced(None, __db_caller, move |conn| {
            conn.execute(
                &format!("INSERT OR REPLACE INTO request_log ({COLUMNS}) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19,?20)"),
                params![
                    log.id, log.source_protocol, log.group_key, log.platform_id as i64,
                    log.target_protocol, log.status_code, log.upstream_status_code, log.duration_ms,
                    log.user_agent, log.request_url, log.request_headers, log.request_body,
                    log.user_response_headers, log.user_response_body, log.upstream_request_url,
                    log.upstream_request_headers, log.upstream_request_body,
                    log.upstream_response_headers, log.response_body, log.created_at
                ],
            )?;
            Ok(())
        })
        .await
        .map_err(|e| format!("upsert request log: {e}"))
    }
}

fn build_where(f: &RequestLogFilter) -> (String, Vec<Box<dyn rusqlite::types::ToSql>>) {
    let mut parts: Vec<String> = Vec::new();
    let mut p: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
    if let Some(ref k) = f.kind {
        p.push(Box::new(k.clone()));
        parts.push(format!("kind = ?{}", p.len()));
    }
    if let Some(v) = f.platform_id {
        p.push(Box::new(v as i64));
        parts.push(format!("platform_id = ?{}", p.len()));
    }
    match f.status {
        Some(200) => parts.push("status_code >= 200 AND status_code < 300".into()),
        Some(-1) => parts.push("(status_code < 200 OR status_code >= 300)".into()),
        Some(s) => {
            p.push(Box::new(s));
            parts.push(format!("status_code = ?{}", p.len()));
        }
        None => {}
    }
    if let Some(ts) = f.time_start {
        p.push(Box::new(ts));
        parts.push(format!("created_at >= ?{}", p.len()));
    }
    if let Some(ts) = f.time_end {
        p.push(Box::new(ts));
        parts.push(format!("created_at <= ?{}", p.len()));
    }
    let w = if parts.is_empty() { String::new() } else { format!(" WHERE {}", parts.join(" AND ")) };
    (w, p)
}

#[track_caller]
pub fn list_request_logs<'a>(
    db: &'a Db,
    filter: &'a RequestLogFilter,
    limit: u32,
    offset: u32,
) -> impl std::future::Future<Output = Result<Vec<RequestLogSummary>, String>> + 'a {
    let __db_caller = std::panic::Location::caller();
    async move {
        let filter = filter.clone();
        db.call_read_proxy_log_traced(None, __db_caller, move |conn| {
            let (w, mut p) = build_where(&filter);
            p.push(Box::new(limit));
            p.push(Box::new(offset));
            let sql = format!(
                "SELECT id, kind, group_key, platform_id, status_code, duration_ms, user_agent, created_at \
                 FROM request_log{w} ORDER BY created_at DESC LIMIT ? OFFSET ?"
            );
            let mut stmt = conn.prepare(&sql)?;
            let refs: Vec<&dyn rusqlite::types::ToSql> = p.iter().map(|x| x.as_ref()).collect();
            let rows = stmt.query_map(refs.as_slice(), |r| {
                Ok(RequestLogSummary {
                    id: r.get(0)?,
                    kind: r.get(1)?,
                    group_key: r.get(2)?,
                    platform_id: r.get::<_, i64>(3)? as u64,
                    status_code: r.get(4)?,
                    duration_ms: r.get(5)?,
                    user_agent: r.get(6)?,
                    created_at: r.get(7)?,
                })
            })?;
            Ok(rows.collect::<SqlResult<Vec<_>>>()?)
        })
        .await
        .map_err(|e| e.to_string())
    }
}

#[track_caller]
pub fn count_request_logs<'a>(
    db: &'a Db,
    filter: &'a RequestLogFilter,
) -> impl std::future::Future<Output = Result<u32, String>> + 'a {
    let __db_caller = std::panic::Location::caller();
    async move {
        let filter = filter.clone();
        db.call_read_proxy_log_traced(None, __db_caller, move |conn| {
            let (w, p) = build_where(&filter);
            let refs: Vec<&dyn rusqlite::types::ToSql> = p.iter().map(|x| x.as_ref()).collect();
            Ok(conn.query_row(&format!("SELECT COUNT(*) FROM request_log{w}"), refs.as_slice(), |r| r.get(0))?)
        })
        .await
        .map_err(|e| e.to_string())
    }
}

/// 详情：以 ProxyLog 形状返回（缺失的 model / token 为默认值），前端详情页复用。
#[track_caller]
pub fn get_request_log<'a>(
    db: &'a Db,
    id: &'a str,
) -> impl std::future::Future<Output = Result<Option<ProxyLog>, String>> + 'a {
    let __db_caller = std::panic::Location::caller();
    async move {
        let id = id.to_string();
        db.call_read_proxy_log_traced(None, __db_caller, move |conn| {
            Ok(conn
                .query_row(
                    &format!("SELECT {COLUMNS} FROM request_log WHERE id = ?1"),
                    params![id],
                    |r| {
                        Ok(ProxyLog {
                            id: r.get(0)?,
                            source_protocol: r.get(1)?,
                            group_key: r.get(2)?,
                            platform_id: r.get::<_, i64>(3)? as u64,
                            target_protocol: r.get(4)?,
                            status_code: r.get(5)?,
                            upstream_status_code: r.get(6)?,
                            duration_ms: r.get(7)?,
                            user_agent: r.get(8)?,
                            request_url: r.get(9)?,
                            request_headers: r.get(10)?,
                            request_body: r.get(11)?,
                            user_response_headers: r.get(12)?,
                            user_response_body: r.get(13)?,
                            upstream_request_url: r.get(14)?,
                            upstream_request_headers: r.get(15)?,
                            upstream_request_body: r.get(16)?,
                            upstream_response_headers: r.get(17)?,
                            response_body: r.get(18)?,
                            created_at: r.get(19)?,
                            done: true,
                            ..Default::default()
                        })
                    },
                )
                .optional()?)
        })
        .await
        .map_err(|e| e.to_string())
    }
}
