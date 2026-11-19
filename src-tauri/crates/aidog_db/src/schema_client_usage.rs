//! client_usage.db migrations（issue #65 票 01：独立库，与 main / log / platform 并列）。
//!
//! 编号空间：client_usage.db 独立（同 main / proxy_log / platform 各自独立编号约定），
//! 首批 20261119-01。DDL 只落 client_usage.db（`call_client_usage_traced`），
//! 主库 / log.db / platform.db 不建这三张表。
use rusqlite::{Connection, Result as SqlResult};

/// client_usage.db 建表（幂等，每次启动重放）。
///
/// 不存对话原文：client_call 只落 token / 成本 / 定位键等元数据。
/// `cost` 可空（REAL NULL）= 该模型未定价（resolve_price 兜底价也不适用时的哨兵）。
pub fn run_migrations_client_usage(conn: &Connection) -> SqlResult<()> {
    // Migration 20261119-01: 三表基座。
    conn.execute_batch(
        r#"CREATE TABLE IF NOT EXISTS client_call (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    client_code     TEXT NOT NULL DEFAULT '',
    session_id      TEXT NOT NULL DEFAULT '',
    is_sidechain    INTEGER NOT NULL DEFAULT 0,
    call_time_ms    INTEGER NOT NULL DEFAULT 0,
    model           TEXT NOT NULL DEFAULT '',
    input_tokens    INTEGER NOT NULL DEFAULT 0,
    output_tokens   INTEGER NOT NULL DEFAULT 0,
    cache_read_tokens  INTEGER NOT NULL DEFAULT 0,
    cache_write_tokens INTEGER NOT NULL DEFAULT 0,
    reasoning_tokens   INTEGER NOT NULL DEFAULT 0,
    cost            REAL,
    source_file_id  TEXT NOT NULL DEFAULT '',
    file_pos        INTEGER NOT NULL DEFAULT 0,
    UNIQUE(source_file_id, file_pos)
);
CREATE INDEX IF NOT EXISTS idx_client_call_session
    ON client_call(client_code, session_id, call_time_ms);

CREATE TABLE IF NOT EXISTS client_file_progress (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    client_code TEXT NOT NULL,
    path        TEXT NOT NULL,
    file_size   INTEGER NOT NULL DEFAULT 0,
    mtime_ms    INTEGER NOT NULL DEFAULT 0,
    read_offset INTEGER NOT NULL DEFAULT 0,
    parse_errors INTEGER NOT NULL DEFAULT 0,
    UNIQUE(client_code, path)
);

CREATE TABLE IF NOT EXISTS client_session (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    client_code  TEXT NOT NULL,
    session_id   TEXT NOT NULL,
    title        TEXT NOT NULL DEFAULT '',
    workdir      TEXT NOT NULL DEFAULT '',
    first_call_ms INTEGER NOT NULL DEFAULT 0,
    last_call_ms  INTEGER NOT NULL DEFAULT 0,
    UNIQUE(client_code, session_id)
);
"#,
    )?;
    Ok(())
}
