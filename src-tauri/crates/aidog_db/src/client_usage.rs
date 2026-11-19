//! 客户端用量库读写（client_usage.db，issue #65 票 01）。
//!
//! 与主库 / log.db / platform.db 并列的第 4 个库文件；三表读写全部走
//! `call_client_traced`-family handles。不存对话原文，只落 token / 成本 / 定位键等元数据。
use crate::{Db, now, retention_cutoff};
use rusqlite::params;

/// 保留天数设置（scope=client_usage, key=retention_days）缺省值。
pub const CLIENT_USAGE_DEFAULT_RETENTION_DAYS: u32 = 90;

const SCOPE: &str = "client_usage";
const KEY: &str = "retention_days";

/// 读保留天数（0 = 永久保留）。
pub async fn load_retention_days(db: &Db) -> u32 {
    crate::get_setting(db, SCOPE, KEY)
        .await
        .ok()
        .flatten()
        .and_then(|v| v.as_u64())
        .map_or(CLIENT_USAGE_DEFAULT_RETENTION_DAYS, |d| d as u32)
}

/// 写保留天数（0 = 每次清理调度跳过）。
pub async fn save_retention_days(db: &Db, days: u32) -> Result<(), String> {
    crate::set_setting(
        db,
        crate::models::SetSettingInput {
            scope: SCOPE.into(),
            key: KEY.into(),
            value: serde_json::json!(days),
        },
    )
    .await
}

/// 一条调用明细的入库载荷（不存对话原文）。
#[derive(Debug, Clone, Default, serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub struct ClientCallRow {
    pub client_code: String,
    pub session_id: String,
    pub is_sidechain: bool,
    pub call_time_ms: i64,
    pub model: String,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub cache_read_tokens: i64,
    pub cache_write_tokens: i64,
    pub reasoning_tokens: i64,
    /// None = 该模型未定价（哨兵，非 0 成本）。
    pub cost: Option<f64>,
    pub source_file_id: String,
    pub file_pos: i64,
}

/// 批量落调用明细；同一 (source_file_id, file_pos) 幂等跳过（ON CONFLICT DO NOTHING）。
/// 返回实际新插入行数。`call_time_ms` 缺省时此处盖章为当前时间。
#[track_caller]
pub fn insert_client_calls<'a>(
    db: &'a Db,
    rows: &'a [ClientCallRow],
) -> impl std::future::Future<Output = Result<usize, String>> + 'a {
    let __db_caller = std::panic::Location::caller();
    async move {
        let ts = now();
        let rows: Vec<ClientCallRow> = rows
            .iter()
            .map(|r| {
                let mut r = r.clone();
                if r.call_time_ms == 0 {
                    r.call_time_ms = ts;
                }
                r
            })
            .collect();
        db.call_client_usage_traced(None, __db_caller, move |conn| {
            let mut n = 0usize;
            for r in &rows {
                let inserted = conn.execute(
                    "INSERT INTO client_call (client_code, session_id, is_sidechain, call_time_ms, model, input_tokens, output_tokens, cache_read_tokens, cache_write_tokens, reasoning_tokens, cost, source_file_id, file_pos) \
                     VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13) \
                     ON CONFLICT(source_file_id, file_pos) DO NOTHING",
                    params![
                        r.client_code, r.session_id, r.is_sidechain as i64, r.call_time_ms,
                        r.model, r.input_tokens, r.output_tokens, r.cache_read_tokens,
                        r.cache_write_tokens, r.reasoning_tokens, r.cost,
                        r.source_file_id, r.file_pos,
                    ],
                )?;
                n += inserted;
            }
            Ok(n)
        })
        .await
        .map_err(|e| format!("insert client_call: {e}"))
    }
}

/// 更新读取进度（UNIQUE(client_code, path) 幂等 upsert，进度表是防重复导入的真值）。
#[track_caller]
pub fn upsert_client_file_progress<'a>(
    db: &'a Db,
    prog: ClientFileProgress,
) -> impl std::future::Future<Output = Result<(), String>> + 'a {
    let __db_caller = std::panic::Location::caller();
    async move {
        db.call_client_usage_traced(None, __db_caller, move |conn| {
            conn.execute(
                "INSERT INTO client_file_progress (client_code, path, file_size, mtime_ms, read_offset, parse_errors, last_msg_id) \
                 VALUES (?1,?2,?3,?4,?5,?6,?7) \
                 ON CONFLICT(client_code, path) DO UPDATE SET \
                   file_size=excluded.file_size, mtime_ms=excluded.mtime_ms, \
                   read_offset=excluded.read_offset, parse_errors=excluded.parse_errors, \
                   last_msg_id=excluded.last_msg_id",
                params![
                    prog.client_code, prog.path, prog.file_size, prog.mtime_ms,
                    prog.read_offset, prog.parse_errors, prog.last_msg_id,
                ],
            )?;
            Ok(())
        })
        .await
        .map_err(|e| format!("upsert client_file_progress: {e}"))
    }
}

/// 进度行（增量扫描起点；`last_msg_id` 供跨扫描去重同 id 多行的 message）。
#[derive(Debug, Clone, serde::Serialize)]
pub struct ClientFileProgress {
    pub client_code: String,
    pub path: String,
    pub file_size: i64,
    pub mtime_ms: i64,
    pub read_offset: i64,
    pub parse_errors: i64,
    pub last_msg_id: String,
}

/// 列一个客户端的全部进度行（增量导入的输入）。
#[track_caller]
pub fn list_client_file_progress<'a>(
    db: &'a Db,
    client_code: &'a str,
) -> impl std::future::Future<Output = Result<Vec<ClientFileProgress>, String>> + 'a {
    let __db_caller = std::panic::Location::caller();
    let client_code = client_code.to_string();
    async move {
        db.call_read_client_usage_traced(None, __db_caller, move |conn| {
            let mut stmt = conn.prepare(
                "SELECT client_code, path, file_size, mtime_ms, read_offset, parse_errors, last_msg_id \
                 FROM client_file_progress WHERE client_code = ?1",
            )?;
            let rows = stmt
                .query_map(params![client_code], |r| {
                    Ok(ClientFileProgress {
                        client_code: r.get(0)?,
                        path: r.get(1)?,
                        file_size: r.get(2)?,
                        mtime_ms: r.get(3)?,
                        read_offset: r.get(4)?,
                        parse_errors: r.get(5)?,
                        last_msg_id: r.get(6)?,
                    })
                })?
                .collect::<Result<Vec<_>, _>>()?;
            Ok(rows)
        })
        .await
        .map_err(|e| format!("list client_file_progress: {e}"))
    }
}

/// 截断 / 替换后的明细重建前置：删该文件的明细与进度行（重读从 0 开始）。
/// 文件删除场景（票 03 监听 / 定时对齐）同样走此函数：末尾顺带清扫本客户端
/// 无明细残留的空会话壳（同 `cleanup_client_usage` 的空壳语义）。
#[track_caller]
pub fn delete_client_file_state<'a>(
    db: &'a Db,
    client_code: &'a str,
    path: &str,
) -> impl std::future::Future<Output = Result<(), String>> + 'a {
    let __db_caller = std::panic::Location::caller();
    let (client_code, path) = (client_code.to_string(), path.to_string());
    async move {
        db.call_client_usage_traced(None, __db_caller, move |conn| {
            conn.execute("DELETE FROM client_call WHERE source_file_id = ?1", params![path])?;
            conn.execute(
                "DELETE FROM client_file_progress WHERE client_code = ?1 AND path = ?2",
                params![client_code, path],
            )?;
            conn.execute(
                "DELETE FROM client_session WHERE client_code = ?1 AND NOT EXISTS \
                 (SELECT 1 FROM client_call c WHERE c.client_code = client_session.client_code \
                  AND c.session_id = client_session.session_id)",
                params![client_code],
            )?;
            Ok(())
        })
        .await
        .map_err(|e| format!("delete client file state: {e}"))
    }
}

/// 更新会话聚合行（UNIQUE(client_code, session_id) 幂等 upsert）。
#[track_caller]
pub fn upsert_client_session(
    db: &Db,
    client_code: String,
    session_id: String,
    title: String,
    workdir: String,
    first_call_ms: i64,
    last_call_ms: i64,
) -> impl std::future::Future<Output = Result<(), String>> + '_ {
    let __db_caller = std::panic::Location::caller();
    async move {
        db.call_client_usage_traced(None, __db_caller, move |conn| {
            conn.execute(
                "INSERT INTO client_session (client_code, session_id, title, workdir, first_call_ms, last_call_ms) \
                 VALUES (?1,?2,?3,?4,?5,?6) \
                 ON CONFLICT(client_code, session_id) DO UPDATE SET \
                   title=CASE WHEN excluded.title = '' THEN client_session.title ELSE excluded.title END, \
                   workdir=excluded.workdir, \
                   first_call_ms=MIN(client_session.first_call_ms, excluded.first_call_ms), \
                   last_call_ms=MAX(client_session.last_call_ms, excluded.last_call_ms)",
                params![client_code, session_id, title, workdir, first_call_ms, last_call_ms],
            )?;
            Ok(())
        })
        .await
        .map_err(|e| format!("upsert client_session: {e}"))
    }
}

/// 清理：删超期 client_call 明细 + 无明细残留的 client_session 行；进度表不动。
/// `days == 0` 跳过。挂 run_retention_cleanup 链（aidog_core proxy_cmd/proxy_log.rs）。
#[track_caller]
pub fn cleanup_client_usage(
    db: &Db,
    days: u32,
) -> impl std::future::Future<Output = Result<(), String>> + '_ {
    let __db_caller = std::panic::Location::caller();
    async move {
        let Some(cutoff) = retention_cutoff(days) else {
            return Ok(());
        };
        db.call_client_usage_traced(None, __db_caller, move |conn| {
            conn.execute("DELETE FROM client_call WHERE call_time_ms < ?1", params![cutoff])?;
            // 明细删净后会话行才删（无明细残留的空会话壳）。
            conn.execute(
                "DELETE FROM client_session WHERE NOT EXISTS \
                 (SELECT 1 FROM client_call c WHERE c.client_code = client_session.client_code \
                  AND c.session_id = client_session.session_id)",
                [],
            )?;
            Ok(())
        })
        .await
        .map_err(|e| format!("cleanup client_usage: {e}"))
    }
}

/// 汇总查询载荷（cost 求和只含已定价行；`unpriced_calls` = cost IS NULL 行数）。
#[derive(Debug, Default, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientUsageSummary {
    pub calls: i64,
    pub unpriced_calls: i64,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub cache_read_tokens: i64,
    pub cache_write_tokens: i64,
    pub reasoning_tokens: i64,
    pub cost: f64,
}

/// 汇总：区间内（可选客户端过滤）调用数 / token 合计 / 成本合计。
#[track_caller]
pub fn client_usage_summary<'a>(
    db: &'a Db,
    from_ms: i64,
    to_ms: i64,
    client_code: Option<&str>,
) -> impl std::future::Future<Output = Result<ClientUsageSummary, String>> + 'a {
    let __db_caller = std::panic::Location::caller();
    let client = client_code.map(str::to_string);
    async move {
        db.call_read_client_usage_traced(None, __db_caller, move |conn| {
            let mut sql = "SELECT COUNT(*), \
                COALESCE(SUM(input_tokens),0), COALESCE(SUM(output_tokens),0), \
                COALESCE(SUM(cache_read_tokens),0), COALESCE(SUM(cache_write_tokens),0), \
                COALESCE(SUM(reasoning_tokens),0), COALESCE(SUM(cost),0), \
                COUNT(CASE WHEN cost IS NULL THEN 1 END) \
                FROM client_call WHERE call_time_ms BETWEEN ?1 AND ?2".to_string();
            let mut args: Vec<Box<dyn rusqlite::types::ToSql>> =
                vec![Box::new(from_ms), Box::new(to_ms)];
            if let Some(c) = &client {
                sql.push_str(" AND client_code = ?3");
                args.push(Box::new(c.clone()));
            }
            conn.query_row(&sql, rusqlite::params_from_iter(args), |r| {
                Ok(ClientUsageSummary {
                    calls: r.get(0)?,
                    input_tokens: r.get(1)?,
                    output_tokens: r.get(2)?,
                    cache_read_tokens: r.get(3)?,
                    cache_write_tokens: r.get(4)?,
                    reasoning_tokens: r.get(5)?,
                    cost: r.get(6)?,
                    unpriced_calls: r.get(7)?,
                })
            })
        })
        .await
        .map_err(|e| format!("client_usage_summary: {e}"))
    }
}

/// 单日趋势行（按 call_time_ms 折 UTC 自然日聚合）。
#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientUsageTrendRow {
    /// UTC 日界（毫秒）。
    pub day_ms: i64,
    pub calls: i64,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub cost: f64,
    pub unpriced_calls: i64,
}

/// 趋势：按 UTC 日聚合的调用 / token / 成本。
#[track_caller]
pub fn client_usage_trend<'a>(
    db: &'a Db,
    from_ms: i64,
    to_ms: i64,
    client_code: Option<&str>,
) -> impl std::future::Future<Output = Result<Vec<ClientUsageTrendRow>, String>> + 'a {
    let __db_caller = std::panic::Location::caller();
    let client = client_code.map(str::to_string);
    async move {
        db.call_read_client_usage_traced(None, __db_caller, move |conn| {
            let mut sql = "SELECT (call_time_ms / 86400000) * 86400000, COUNT(*), \
                COALESCE(SUM(input_tokens),0), COALESCE(SUM(output_tokens),0), \
                COALESCE(SUM(cost),0), COUNT(CASE WHEN cost IS NULL THEN 1 END) \
                FROM client_call WHERE call_time_ms BETWEEN ?1 AND ?2".to_string();
            let mut args: Vec<Box<dyn rusqlite::types::ToSql>> = vec![Box::new(from_ms), Box::new(to_ms)];
            if let Some(c) = &client {
                sql.push_str(" AND client_code = ?3");
                args.push(Box::new(c.clone()));
            }
            sql.push_str(" GROUP BY 1 ORDER BY 1");
            let rows = conn
                .prepare(&sql)?
                .query_map(rusqlite::params_from_iter(args), |r| {
                    Ok(ClientUsageTrendRow {
                        day_ms: r.get(0)?,
                        calls: r.get(1)?, input_tokens: r.get(2)?, output_tokens: r.get(3)?,
                        cost: r.get(4)?, unpriced_calls: r.get(5)?,
                    })
                })?
                .collect::<Result<Vec<_>, _>>()?;
            Ok(rows)
        })
        .await
        .map_err(|e| format!("client_usage_trend: {e}"))
    }
}

/// 按模型聚合行。
#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientUsageModelRow {
    pub model: String,
    pub calls: i64,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub cost: f64,
    pub unpriced_calls: i64,
}

/// 按模型聚合（成本合计、未定价计数），按成本降序。
#[track_caller]
pub fn client_usage_by_model<'a>(
    db: &'a Db,
    from_ms: i64,
    to_ms: i64,
    client_code: Option<&str>,
) -> impl std::future::Future<Output = Result<Vec<ClientUsageModelRow>, String>> + 'a {
    let __db_caller = std::panic::Location::caller();
    let client = client_code.map(str::to_string);
    async move {
        db.call_read_client_usage_traced(None, __db_caller, move |conn| {
            let mut sql = "SELECT model, COUNT(*), COALESCE(SUM(input_tokens),0), \
                COALESCE(SUM(output_tokens),0), COALESCE(SUM(cost),0), \
                COUNT(CASE WHEN cost IS NULL THEN 1 END) \
                FROM client_call WHERE call_time_ms BETWEEN ?1 AND ?2".to_string();
            let mut args: Vec<Box<dyn rusqlite::types::ToSql>> = vec![Box::new(from_ms), Box::new(to_ms)];
            if let Some(c) = &client {
                sql.push_str(" AND client_code = ?3");
                args.push(Box::new(c.clone()));
            }
            sql.push_str(" GROUP BY model ORDER BY 5 DESC, 2 DESC");
            let rows = conn
                .prepare(&sql)?
                .query_map(rusqlite::params_from_iter(args), |r| {
                    Ok(ClientUsageModelRow {
                        model: r.get(0)?,
                        calls: r.get(1)?,
                        input_tokens: r.get(2)?,
                        output_tokens: r.get(3)?, cost: r.get(4)?,
                        unpriced_calls: r.get(5)?,
                    })
                })?
                .collect::<Result<Vec<_>, _>>()?;
            Ok(rows)
        })
        .await
        .map_err(|e| format!("client_usage_by_model: {e}"))
    }
}

/// 会话行（查询载荷）。
#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientUsageSessionRow {
    pub client_code: String,
    pub session_id: String,
    pub title: String,
    pub workdir: String,
    pub first_call_ms: i64,
    pub last_call_ms: i64,
    pub calls: i64,
    pub cost: f64,
    pub input_tokens: i64,
    pub output_tokens: i64,
}

/// 会话列表：按最后调用时间倒序，含聚合调用数 / 成本 / token。
#[track_caller]
pub fn client_usage_sessions<'a>(
    db: &'a Db,
    from_ms: i64,
    to_ms: i64,
    client_code: Option<&str>,
    limit: i64,
) -> impl std::future::Future<Output = Result<Vec<ClientUsageSessionRow>, String>> + 'a {
    let __db_caller = std::panic::Location::caller();
    let client = client_code.map(str::to_string);
    async move {
        db.call_read_client_usage_traced(None, __db_caller, move |conn| {
            let mut sql = "SELECT s.client_code, s.session_id, s.title, s.workdir, s.first_call_ms, s.last_call_ms, \
                COALESCE(cnt.calls,0), COALESCE(cnt.cost,0), COALESCE(cnt.input,0), COALESCE(cnt.output,0) \
                FROM client_session s LEFT JOIN \
                (SELECT client_code, session_id, COUNT(*) AS calls, COALESCE(SUM(cost),0) AS cost, \
                        COALESCE(SUM(input_tokens),0) AS input, COALESCE(SUM(output_tokens),0) AS output \
                 FROM client_call WHERE call_time_ms BETWEEN ?1 AND ?2 GROUP BY 1,2) cnt \
                ON cnt.client_code = s.client_code AND cnt.session_id = s.session_id \
                WHERE 1=1".to_string();
            let mut args: Vec<Box<dyn rusqlite::types::ToSql>> = vec![Box::new(from_ms), Box::new(to_ms)];
            if let Some(c) = &client {
                sql.push_str(" AND s.client_code = ?3");
                args.push(Box::new(c.clone()));
            }
            sql.push_str(" ORDER BY s.last_call_ms DESC LIMIT ");
            sql.push_str(&limit.to_string());
            let rows = conn
                .prepare(&sql)?
                .query_map(rusqlite::params_from_iter(args), |r| {
                    Ok(ClientUsageSessionRow {
                        client_code: r.get(0)?,
                        session_id: r.get(1)?,
                        title: r.get(2)?,
                        workdir: r.get(3)?,
                        first_call_ms: r.get(4)?,
                        last_call_ms: r.get(5)?,
                        calls: r.get(6)?,
                        cost: r.get(7)?,
                        input_tokens: r.get(8)?,
                        output_tokens: r.get(9)?,
                    })
                })?
                .collect::<Result<Vec<_>, _>>()?;
            Ok(rows)
        })
        .await
        .map_err(|e| format!("client_usage_sessions: {e}"))
    }
}
