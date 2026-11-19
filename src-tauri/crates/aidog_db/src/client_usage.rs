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
pub fn upsert_client_file_progress(
    db: &Db,
    client_code: String,
    path: String,
    file_size: i64,
    mtime_ms: i64,
    read_offset: i64,
    parse_errors: i64,
) -> impl std::future::Future<Output = Result<(), String>> + '_ {
    let __db_caller = std::panic::Location::caller();
    async move {
        db.call_client_usage_traced(None, __db_caller, move |conn| {
            conn.execute(
                "INSERT INTO client_file_progress (client_code, path, file_size, mtime_ms, read_offset, parse_errors) \
                 VALUES (?1,?2,?3,?4,?5,?6) \
                 ON CONFLICT(client_code, path) DO UPDATE SET \
                   file_size=excluded.file_size, mtime_ms=excluded.mtime_ms, \
                   read_offset=excluded.read_offset, parse_errors=excluded.parse_errors",
                params![client_code, path, file_size, mtime_ms, read_offset, parse_errors],
            )?;
            Ok(())
        })
        .await
        .map_err(|e| format!("upsert client_file_progress: {e}"))
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
                   title=excluded.title, workdir=excluded.workdir, \
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
