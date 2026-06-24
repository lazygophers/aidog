use super::*;
use rusqlite::{params, OptionalExtension, Result as SqlResult};

/// proxy_log 全列序（INSERT / 单行 SELECT 共用，与表定义列序一致）
const PROXY_LOG_COLUMNS: &str =
    "id, group_key, model, actual_model, source_protocol, target_protocol, platform_id, request_headers, request_body, upstream_request_headers, upstream_request_body, response_body, request_url, upstream_request_url, upstream_response_headers, upstream_status_code, user_response_headers, user_response_body, status_code, duration_ms, input_tokens, output_tokens, cache_tokens, est_cost, is_stream, attempts, retry_count, blocked_by, blocked_reason, created_at, updated_at, deleted_at";

/// 从查询行构造 ProxyLog（列序须与 PROXY_LOG_COLUMNS 一致）
fn row_to_proxy_log(row: &rusqlite::Row) -> SqlResult<crate::gateway::models::ProxyLog> {
    Ok(crate::gateway::models::ProxyLog {
        id: row.get(0)?,
        group_key: row.get(1)?,
        model: row.get(2)?,
        actual_model: row.get(3)?,
        source_protocol: row.get(4)?,
        target_protocol: row.get(5)?,
        platform_id: row.get::<_, i64>(6)? as u64,
        request_headers: row.get(7)?,
        request_body: row.get(8)?,
        upstream_request_headers: row.get(9)?,
        upstream_request_body: row.get(10)?,
        response_body: row.get(11)?,
        request_url: row.get(12)?,
        upstream_request_url: row.get(13)?,
        upstream_response_headers: row.get(14)?,
        upstream_status_code: row.get(15)?,
        user_response_headers: row.get(16)?,
        user_response_body: row.get(17)?,
        status_code: row.get(18)?,
        duration_ms: row.get(19)?,
        input_tokens: row.get(20)?,
        output_tokens: row.get(21)?,
        cache_tokens: row.get(22)?,
        est_cost: row.get(23)?,
        is_stream: row.get::<_, i64>(24)? == 1,
        attempts: crate::gateway::models::parse_attempts(&row.get::<_, String>(25)?),
        retry_count: row.get(26)?,
        blocked_by: row.get(27)?,
        blocked_reason: row.get(28)?,
        created_at: row.get(29)?,
        updated_at: row.get(30)?,
        deleted_at: row.get(31)?,
    })
}

/// Upsert (INSERT OR REPLACE) a proxy log entry — used for incremental logging.
/// 取 owned `ProxyLog`：调用方（upsert_log）已为脱敏 clone 一份，此处接管所有权
/// 直接 move 进后台线程闭包，消除原先「调用方 clone + 本函数再 clone」的双重全量复制。
#[track_caller]
pub fn upsert_proxy_log(db: &Db, log: crate::gateway::models::ProxyLog) -> impl std::future::Future<Output = Result<(), String>> + '_ {
    let __db_caller = std::panic::Location::caller();
    async move {
    db
        .call_traced(None, __db_caller, move |conn| {
            let attempts_str = crate::gateway::models::serialize_attempts(&log.attempts);
            // 固定 SQL（列序常量）→ prepare_cached 命中 rusqlite statement cache，省每次写的 prepare 开销
            let mut stmt = conn.prepare_cached(
                &format!("INSERT OR REPLACE INTO proxy_log ({PROXY_LOG_COLUMNS})
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19,?20,?21,?22,?23,?24,?25,?26,?27,?28,?29,?30,?31,?32)"),
            )?;
            stmt.execute(
                params![log.id, log.group_key, log.model, log.actual_model, log.source_protocol, log.target_protocol, log.platform_id as i64, log.request_headers, log.request_body, log.upstream_request_headers, log.upstream_request_body, log.response_body, log.request_url, log.upstream_request_url, log.upstream_response_headers, log.upstream_status_code, log.user_response_headers, log.user_response_body, log.status_code, log.duration_ms, log.input_tokens, log.output_tokens, log.cache_tokens, log.est_cost, log.is_stream as i64, attempts_str, log.retry_count, log.blocked_by, log.blocked_reason, log.created_at, log.updated_at, log.deleted_at],
            )?;
            Ok(())
        })
        .await
        .map_err(|e| format!("upsert proxy log: {e}"))
    }
}

/// 渐进式日志的「DB 就绪列快照」：32 列已转成入库类型（脱敏已在构造时就地应用）。
///
/// 用途：替代每节点全列 INSERT OR REPLACE 重写。构造一次 → 首节点 INSERT 建行，
/// 后续节点与上一快照逐列 diff，仅 UPDATE 变化列。配合 upsert_log 的按需脱敏，
/// 彻底消除 proxy.rs 每次写都 `log.clone()` 整结构的开销。
///
/// 字段顺序与值语义须与 `PROXY_LOG_COLUMNS` / `upsert_proxy_log` 完全一致（字段完整性红线）。
#[derive(Clone, PartialEq)]
pub struct ProxyLogColumns {
    pub id: String,
    pub group_key: String,
    pub model: String,
    pub actual_model: String,
    pub source_protocol: String,
    pub target_protocol: String,
    pub platform_id: i64,
    pub request_headers: String,
    pub request_body: String,
    pub upstream_request_headers: String,
    pub upstream_request_body: String,
    pub response_body: String,
    pub request_url: String,
    pub upstream_request_url: String,
    pub upstream_response_headers: String,
    pub upstream_status_code: i32,
    pub user_response_headers: String,
    pub user_response_body: String,
    pub status_code: i32,
    pub duration_ms: i32,
    pub input_tokens: i32,
    pub output_tokens: i32,
    pub cache_tokens: i32,
    pub est_cost: f64,
    pub is_stream: i64,
    pub attempts: String,
    pub retry_count: i32,
    pub blocked_by: String,
    pub blocked_reason: String,
    pub created_at: i64,
    pub updated_at: i64,
    pub deleted_at: i64,
}

impl ProxyLogColumns {
    /// 由 `ProxyLog` 构造入库列快照。
    /// `*_headers` 字段（元数据，Authorization 已在上游脱敏为 `[REDACTED]`）始终入库，
    /// 不受 `log_user_request` / `log_upstream_request` 开关控制；仅 `*_body`（prompt / 响应正文，
    /// 含敏感内容）受 `strip_user` / `strip_upstream` 控制就地清空。
    /// attempts 在此序列化一次。仅克隆 String 字段（入库本就需 owned 值），不克隆整 ProxyLog 结构。
    pub fn from_log(log: &crate::gateway::models::ProxyLog, strip_user: bool, strip_upstream: bool) -> Self {
        let empty = String::new;
        ProxyLogColumns {
            id: log.id.clone(),
            group_key: log.group_key.clone(),
            model: log.model.clone(),
            actual_model: log.actual_model.clone(),
            source_protocol: log.source_protocol.clone(),
            target_protocol: log.target_protocol.clone(),
            platform_id: log.platform_id as i64,
            request_headers: log.request_headers.clone(),
            request_body: if strip_user { empty() } else { log.request_body.clone() },
            upstream_request_headers: log.upstream_request_headers.clone(),
            upstream_request_body: if strip_upstream { empty() } else { log.upstream_request_body.clone() },
            response_body: log.response_body.clone(),
            request_url: log.request_url.clone(),
            upstream_request_url: log.upstream_request_url.clone(),
            upstream_response_headers: log.upstream_response_headers.clone(),
            upstream_status_code: log.upstream_status_code,
            user_response_headers: log.user_response_headers.clone(),
            user_response_body: if strip_user { empty() } else { log.user_response_body.clone() },
            status_code: log.status_code,
            duration_ms: log.duration_ms,
            input_tokens: log.input_tokens,
            output_tokens: log.output_tokens,
            cache_tokens: log.cache_tokens,
            est_cost: log.est_cost,
            is_stream: log.is_stream as i64,
            attempts: crate::gateway::models::serialize_attempts(&log.attempts),
            retry_count: log.retry_count,
            blocked_by: log.blocked_by.clone(),
            blocked_reason: log.blocked_reason.clone(),
            created_at: log.created_at,
            updated_at: log.updated_at,
            deleted_at: log.deleted_at,
        }
    }

    /// 与上一快照 `old` 逐列对比，返回 (列名, 绑定值) 的变化集。id 主键不在内（用于 WHERE）。
    fn changed_since(&self, old: &ProxyLogColumns) -> Vec<(&'static str, Box<dyn rusqlite::types::ToSql + Send>)> {
        let mut out: Vec<(&'static str, Box<dyn rusqlite::types::ToSql + Send>)> = Vec::new();
        macro_rules! diff {
            ($col:literal, $field:ident) => {
                if self.$field != old.$field {
                    out.push(($col, Box::new(self.$field.clone())));
                }
            };
        }
        diff!("group_key", group_key);
        diff!("model", model);
        diff!("actual_model", actual_model);
        diff!("source_protocol", source_protocol);
        diff!("target_protocol", target_protocol);
        diff!("platform_id", platform_id);
        diff!("request_headers", request_headers);
        diff!("request_body", request_body);
        diff!("upstream_request_headers", upstream_request_headers);
        diff!("upstream_request_body", upstream_request_body);
        diff!("response_body", response_body);
        diff!("request_url", request_url);
        diff!("upstream_request_url", upstream_request_url);
        diff!("upstream_response_headers", upstream_response_headers);
        diff!("upstream_status_code", upstream_status_code);
        diff!("user_response_headers", user_response_headers);
        diff!("user_response_body", user_response_body);
        diff!("status_code", status_code);
        diff!("duration_ms", duration_ms);
        diff!("input_tokens", input_tokens);
        diff!("output_tokens", output_tokens);
        diff!("cache_tokens", cache_tokens);
        diff!("est_cost", est_cost);
        diff!("is_stream", is_stream);
        diff!("attempts", attempts);
        diff!("retry_count", retry_count);
        diff!("blocked_by", blocked_by);
        diff!("blocked_reason", blocked_reason);
        diff!("created_at", created_at);
        diff!("updated_at", updated_at);
        diff!("deleted_at", deleted_at);
        out
    }
}

/// 渐进式日志首节点：INSERT 建行（非 REPLACE，行不应已存在）。失败上抛。
#[track_caller]
pub fn insert_proxy_log_columns(db: &Db, cols: ProxyLogColumns) -> impl std::future::Future<Output = Result<(), String>> + '_ {
    let __db_caller = std::panic::Location::caller();
    async move {
    // cols.id == proxy_log.id == 请求 span 的 request_id（32-hex），用作 SQL 日志归属键。
    let req_id = cols.id.clone();
    db
        .call_traced(Some(&req_id), __db_caller, move |conn| {
            // 固定 SQL（列序常量）→ prepare_cached 命中 statement cache（渐进式日志首节点高频）
            let mut stmt = conn.prepare_cached(
                &format!("INSERT INTO proxy_log ({PROXY_LOG_COLUMNS})
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19,?20,?21,?22,?23,?24,?25,?26,?27,?28,?29,?30,?31,?32)"),
            )?;
            stmt.execute(
                params![cols.id, cols.group_key, cols.model, cols.actual_model, cols.source_protocol, cols.target_protocol, cols.platform_id, cols.request_headers, cols.request_body, cols.upstream_request_headers, cols.upstream_request_body, cols.response_body, cols.request_url, cols.upstream_request_url, cols.upstream_response_headers, cols.upstream_status_code, cols.user_response_headers, cols.user_response_body, cols.status_code, cols.duration_ms, cols.input_tokens, cols.output_tokens, cols.cache_tokens, cols.est_cost, cols.is_stream, cols.attempts, cols.retry_count, cols.blocked_by, cols.blocked_reason, cols.created_at, cols.updated_at, cols.deleted_at],
            )?;
            Ok(())
        })
        .await
        .map_err(|e| format!("insert proxy log: {e}"))
    }
}

/// 渐进式日志后续节点：仅 UPDATE 相对 `prev` 变化的列。无变化则 no-op（不发 SQL）。
/// 若目标行不存在（理论不应，节点1 必先 INSERT），UPDATE 影响 0 行，静默（与旧 REPLACE
/// 的「不存在则建行」语义偏离已由 upsert_log 的快照存在性保证：有快照 ⇒ 已 INSERT 过）。
#[track_caller]
pub fn update_proxy_log_columns<'a>(db: &'a Db, new: ProxyLogColumns, prev: &'a ProxyLogColumns) -> impl std::future::Future<Output = Result<(), String>> + 'a {
    let __db_caller = std::panic::Location::caller();
    async move {
    let changed = new.changed_since(prev);
    if changed.is_empty() {
        return Ok(());
    }
    let id = new.id.clone();
    // id == proxy_log.id == request_id，用作 SQL 日志归属键。
    let req_id = id.clone();
    db
        .call_traced(Some(&req_id), __db_caller, move |conn| {
            let set_sql: String = changed
                .iter()
                .enumerate()
                .map(|(i, (col, _))| format!("{col} = ?{}", i + 1))
                .collect::<Vec<_>>()
                .join(", ");
            let id_idx = changed.len() + 1;
            let sql = format!("UPDATE proxy_log SET {set_sql} WHERE id = ?{id_idx}");
            let mut binds: Vec<&dyn rusqlite::types::ToSql> = changed.iter().map(|(_, v)| v.as_ref() as &dyn rusqlite::types::ToSql).collect();
            binds.push(&id);
            conn.execute(&sql, binds.as_slice())?;
            Ok(())
        })
        .await
        .map_err(|e| format!("update proxy log: {e}"))
    }
}

#[track_caller]
pub fn list_proxy_logs(db: &Db, limit: u32, offset: u32) -> impl std::future::Future<Output = Result<Vec<crate::gateway::models::ProxyLogSummary>, String>> + '_ {
    let __db_caller = std::panic::Location::caller();
    async move {
    db
        .call_read_traced(None, __db_caller, move |conn| {
            let mut stmt = conn.prepare_cached(
                "SELECT id, group_key, model, actual_model, source_protocol, target_protocol, platform_id, status_code, duration_ms, input_tokens, output_tokens, cache_tokens, is_stream, retry_count, created_at
                 FROM proxy_log WHERE deleted_at = 0 ORDER BY created_at DESC LIMIT ?1 OFFSET ?2",
            )?;
            let rows = stmt.query_map(params![limit, offset], row_to_proxy_log_summary)?;
            Ok(rows.collect::<SqlResult<Vec<_>>>()?)
        })
        .await
        .map_err(|e| e.to_string())
    }
}

/// Summary row mapper (column order must match SELECT)
fn row_to_proxy_log_summary(row: &rusqlite::Row) -> SqlResult<crate::gateway::models::ProxyLogSummary> {
    Ok(crate::gateway::models::ProxyLogSummary {
        id: row.get(0)?,
        group_key: row.get(1)?,
        model: row.get(2)?,
        actual_model: row.get(3)?,
        source_protocol: row.get(4)?,
        target_protocol: row.get(5)?,
        platform_id: row.get::<_, i64>(6)? as u64,
        status_code: row.get(7)?,
        duration_ms: row.get(8)?,
        input_tokens: row.get(9)?,
        output_tokens: row.get(10)?,
        cache_tokens: row.get(11)?,
        is_stream: row.get::<_, i64>(12)? == 1,
        retry_count: row.get(13)?,
        created_at: row.get(14)?,
    })
}

#[track_caller]
pub fn filtered_list_proxy_logs<'a>(
    db: &'a Db,
    filter: &'a crate::gateway::models::ProxyLogFilter,
    limit: u32,
    offset: u32,
) -> impl std::future::Future<Output = Result<Vec<crate::gateway::models::ProxyLogSummary>, String>> + 'a {
    let __db_caller = std::panic::Location::caller();
    async move {
    let filter = filter.clone();
    db
        .call_read_traced(None, __db_caller, move |conn| {
            let (where_sql, mut p) = build_filter_where(&filter);
            p.push(Box::new(limit));
            p.push(Box::new(offset));
            let sql = format!(
                "SELECT id, group_key, model, actual_model, source_protocol, target_protocol, platform_id, status_code, duration_ms, input_tokens, output_tokens, cache_tokens, is_stream, retry_count, created_at \
                 FROM proxy_log WHERE deleted_at = 0{where_sql} ORDER BY created_at DESC LIMIT ? OFFSET ?"
            );
            let mut stmt = conn.prepare(&sql)?;
            let refs: Vec<&dyn rusqlite::types::ToSql> = p.iter().map(|x| x.as_ref()).collect();
            let rows = stmt.query_map(refs.as_slice(), row_to_proxy_log_summary)?;
            Ok(rows.collect::<SqlResult<Vec<_>>>()?)
        })
        .await
        .map_err(|e| e.to_string())
    }
}

#[track_caller]
pub fn filtered_count_proxy_logs<'a>(
    db: &'a Db,
    filter: &'a crate::gateway::models::ProxyLogFilter,
) -> impl std::future::Future<Output = Result<u32, String>> + 'a {
    let __db_caller = std::panic::Location::caller();
    async move {
    let filter = filter.clone();
    db
        .call_read_traced(None, __db_caller, move |conn| {
            let (where_sql, p) = build_filter_where(&filter);
            let sql = format!("SELECT COUNT(*) FROM proxy_log WHERE deleted_at = 0{where_sql}");
            let refs: Vec<&dyn rusqlite::types::ToSql> = p.iter().map(|x| x.as_ref()).collect();
            Ok(conn.query_row(&sql, refs.as_slice(), |row| row.get(0))?)
        })
        .await
        .map_err(|e| e.to_string())
    }
}

/// Build WHERE clause extensions + params from filter.
/// Returns (" AND ...", params). Empty filter → ("", []).
fn build_filter_where(filter: &crate::gateway::models::ProxyLogFilter) -> (String, Vec<Box<dyn rusqlite::types::ToSql>>) {
    let mut parts: Vec<String> = Vec::new();
    let mut p: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
    let mut idx = 1u32;

    if let Some(ref v) = filter.platform_id {
        parts.push(format!("AND platform_id = ?{idx}"));
        p.push(Box::new(*v as i64));
        idx += 1;
    }
    if let Some(ref v) = filter.group_key {
        parts.push(format!("AND group_key = ?{idx}"));
        p.push(Box::new(v.clone()));
        idx += 1;
    }
    if let Some(s) = filter.status {
        if s == 200 {
            parts.push("AND status_code >= 200 AND status_code < 300".to_string());
        } else if s == -1 {
            parts.push("AND (status_code < 200 OR status_code >= 300)".to_string());
        } else {
            parts.push(format!("AND status_code = ?{idx}"));
            p.push(Box::new(s));
            idx += 1;
        }
    }
    if let Some(ts) = filter.time_start {
        parts.push(format!("AND created_at >= ?{idx}"));
        p.push(Box::new(ts));
        idx += 1;
    }
    if let Some(ts) = filter.time_end {
        parts.push(format!("AND created_at <= ?{idx}"));
        p.push(Box::new(ts));
        idx += 1;
    }
    if let Some(ref v) = filter.model {
        let col = match filter.model_type.as_deref() {
            Some("actual") => "actual_model",
            _ => "model",
        };
        parts.push(format!("AND {col} = ?{idx}"));
        p.push(Box::new(v.clone()));
        idx += 1;
    }
    if let Some(ref v) = filter.path {
        let trimmed = v.trim();
        if !trimmed.is_empty() {
            parts.push(format!("AND request_url LIKE ?{idx}"));
            p.push(Box::new(format!("%{}%", trimmed)));
        }
    }

    let where_sql = if parts.is_empty() { String::new() } else { format!(" {}", parts.join(" ")) };
    (where_sql, p)
}

#[track_caller]
pub fn get_proxy_log<'a>(db: &'a Db, id: &'a str) -> impl std::future::Future<Output = Result<Option<crate::gateway::models::ProxyLog>, String>> + 'a {
    let __db_caller = std::panic::Location::caller();
    async move {
    let id = id.to_string();
    db
        .call_read_traced(None, __db_caller, move |conn| {
            let mut stmt = conn.prepare_cached(&format!(
                "SELECT {PROXY_LOG_COLUMNS} FROM proxy_log WHERE id = ?1 AND deleted_at = 0"
            ))?;
            Ok(stmt.query_row(params![id], row_to_proxy_log).optional()?)
        })
        .await
        .map_err(|e| e.to_string())
    }
}

#[track_caller]
pub fn clear_proxy_logs(db: &Db) -> impl std::future::Future<Output = Result<(), String>> + '_ {
    let __db_caller = std::panic::Location::caller();
    async move {
    db
        .call_traced(None, __db_caller, move |conn| {
            conn.execute("UPDATE proxy_log SET deleted_at = ?1 WHERE deleted_at = 0", params![now()])?;
            Ok(())
        })
        .await
        .map_err(|e| format!("clear proxy logs: {e}"))
    }
}

/// Delete logs older than N days. Pass 0 to skip.
///
/// 硬删（`DELETE FROM`），非软删：retention_days 语义 = 过期清除，所有 proxy_log 查询
/// 均 `WHERE deleted_at = 0`，软删 tombstone 无消费方（无 un-delete UI / 聚合）。
/// 硬删后调 `incremental_vacuum(100)` 回收 free pages（需 auto_vacuum=INCREMENTAL，老库
/// 未迁移时为 no-op 不报错）。每次至多回收 100 页避免长锁，busy_timeout=5000 兜底排队。
#[track_caller]
pub fn cleanup_proxy_logs(db: &Db, retention_days: u32) -> impl std::future::Future<Output = Result<(), String>> + '_ {
    let __db_caller = std::panic::Location::caller();
    async move {
    let Some(cutoff) = retention_cutoff(retention_days) else { return Ok(()); };
    db
        .call_traced(None, __db_caller, move |conn| {
            conn.execute(
                "DELETE FROM proxy_log WHERE created_at < ?1 AND deleted_at = 0",
                params![cutoff],
            )?;
            incremental_vacuum_conn(conn, 100);
            // 行删 + free page 回收后选择度变化，重建 sqlite_stat1 给规划器真实统计
            // （ANALYZE proxy_log 仅扫该表索引，开销随表大小但远低于全库 VACUUM）。
            let _ = conn.execute("ANALYZE proxy_log", []);
            Ok(())
        })
        .await
        .map_err(|e| format!("cleanup proxy logs: {e}"))
    }
}

/// 物理删除所有历史软删 tombstone（`deleted_at != 0`），回收 free pages。
///
/// 迁移期（cleanup_proxy_logs 由软删改硬删）清积压 tombstone；日常可被
/// proxy_log_settings_set 调用链在 retention 硬删后追加触发。
#[track_caller]
pub fn purge_deleted_proxy_logs(db: &Db) -> impl std::future::Future<Output = Result<(), String>> + '_ {
    let __db_caller = std::panic::Location::caller();
    async move {
    db
        .call_traced(None, __db_caller, move |conn| {
            conn.execute("DELETE FROM proxy_log WHERE deleted_at != 0", [])?;
            incremental_vacuum_conn(conn, 100);
            Ok(())
        })
        .await
        .map_err(|e| format!("purge deleted proxy logs: {e}"))
    }
}

// ─── Stats aggregation (stats_agg_hourly) ──────────────────────

