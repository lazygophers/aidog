use super::*;
use rusqlite::{params, Connection, Result as SqlResult};

/// 在给定连接上跑 `PRAGMA incremental_vacuum(N)`，回收至多 N 页 free pages。
///
/// auto_vacuum != INCREMENTAL 时为 no-op（SQLite 不报错）；失败仅 warn 不上抛，
/// 因为回收失败不影响数据正确性，下次 retention/手动压缩仍可重试。
pub(crate) fn incremental_vacuum_conn(conn: &Connection, max_pages: i64) {
    // PRAGMA incremental_vacuum 接受一个参数（要回收的最大页数）。rusqlite 用 query
    // 执行（pragma 返回行集），errors_here 仅 warn。
    let sql = format!("PRAGMA incremental_vacuum({max_pages})");
    if let Err(e) = conn.execute_batch(&sql) {
        tracing::warn!(error = %e, "incremental_vacuum failed (auto_vacuum != INCREMENTAL or busy), will retry later");
    }
}

/// 老库 auto_vacuum 迁移：探测当前 auto_vacuum（0=NONE/1=FULL/2=INCREMENTAL），
/// 非 INCREMENTAL(2) 则 `PRAGMA auto_vacuum=INCREMENTAL` + `VACUUM`（VACUUM 重建库切换模式），
/// 成功后置 setting(db/compact_migrated_v1)=true 持久标记，幂等。
///
/// **VACUUM 不在事务内**（rusqlite 独立调用），锁库期间代理请求排队（busy_timeout 兜底）。
/// 失败仅返回 Err，调用方（启动 spawn）warn 不阻塞，不置标记，下次启动重试。
#[track_caller]
pub fn migrate_auto_vacuum(db: &Db) -> impl std::future::Future<Output = Result<bool, String>> + '_ {
    let __db_caller = std::panic::Location::caller();
    async move {
    // 幂等标记：已迁移直接跳过
    if let Ok(Some(v)) = get_setting(db, "db", "compact_migrated_v1").await {
        if v == serde_json::Value::Bool(true) {
            return Ok(false);
        }
    }
    // 探测当前 auto_vacuum 模式
    let current: i64 = db
        
        .call_traced(None, __db_caller, |c| {
            Ok(c.query_row("PRAGMA auto_vacuum", [], |r| r.get::<_, i64>(0))?)
        })
        .await
        .map_err(|e| format!("probe auto_vacuum: {e}"))?;
    if current == 2 {
        // 已是 INCREMENTAL（可能是新装库建表前设过），直接置标记，无需 VACUUM。
        set_setting(
            db,
            SetSettingInput {
                scope: "db".into(),
                key: "compact_migrated_v1".into(),
                value: serde_json::Value::Bool(true),
            },
        )
        .await?;
        return Ok(false);
    }
    // 切换为 INCREMENTAL 并 VACUUM 重建。VACUUM 必须在 autocommit（无活动事务）下执行，
    // 不能包在 transaction 内；此处独立 execute_batch 调用，rusqlite 默认 autocommit。
    db
        .call_traced(None, __db_caller, |c| {
            // 先 checkpoint 把 WAL 内容合并回主库，避免 WAL+VACUUM 模式约束
            let _ = c.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);");
            c.execute_batch("PRAGMA auto_vacuum = INCREMENTAL; VACUUM;")?;
            Ok(())
        })
        .await
        .map_err(|e| format!("migrate auto_vacuum (VACUUM): {e}"))?;
    set_setting(
        db,
        SetSettingInput {
            scope: "db".into(),
            key: "compact_migrated_v1".into(),
            value: serde_json::Value::Bool(true),
        },
    )
    .await?;
    tracing::info!("db auto_vacuum migrated to INCREMENTAL via VACUUM");
    Ok(true)
    }
}

/// 全量 VACUUM 压缩数据库到最小。返回前后字节大小（page_count × page_size）。
///
/// 用于设置页「立即压缩数据库」按钮：比 incremental 更激进，整库重写。
/// VACUUM 不在事务内（独立 conn 调用）；锁库期间请求排队，UI 有警示。
#[track_caller]
pub fn compact_database(db: &Db) -> impl std::future::Future<Output = Result<CompactResult, String>> + '_ {
    let __db_caller = std::panic::Location::caller();
    async move {
    db
        .call_traced(None, __db_caller, |c| {
            let before = db_size_bytes(c)?;
            // WAL checkpoint 再 VACUUM，避免 WAL 内未合并页漏算
            let _ = c.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);");
            c.execute_batch("VACUUM;")?;
            let after = db_size_bytes(c)?;
            Ok(CompactResult {
                before_bytes: before,
                after_bytes: after,
            })
        })
        .await
        .map_err(|e| format!("compact database: {e}"))
    }
}

/// `PRAGMA page_count * PRAGMA page_size` = 当前 DB 文件占用的逻辑字节数。
fn db_size_bytes(conn: &Connection) -> SqlResult<i64> {
    let pages: i64 = conn.query_row("PRAGMA page_count", [], |r| r.get(0))?;
    let page_size: i64 = conn.query_row("PRAGMA page_size", [], |r| r.get(0))?;
    Ok(pages * page_size)
}

/// 全量 VACUUM 结果（手动「压缩数据库」按钮用）。
#[derive(Debug, Clone, Serialize)]
pub struct CompactResult {
    pub before_bytes: i64,
    pub after_bytes: i64,
}

/// Clear user request body fields for logs older than retention_days.
/// `*_headers`（元数据，已脱敏）始终保留至行级 retention 删除；仅清 `*_body`（prompt / 响应正文）。
/// Does NOT delete the log row — keeps token stats and metadata.
#[track_caller]
pub fn cleanup_user_request_fields(db: &Db, retention_days: u32) -> impl std::future::Future<Output = Result<(), String>> + '_ {
    let __db_caller = std::panic::Location::caller();
    async move {
    let Some(cutoff) = retention_cutoff(retention_days) else { return Ok(()); };
    db
        .call_traced(None, __db_caller, move |conn| {
            conn.execute(
                "UPDATE proxy_log SET request_body = '', user_response_body = '' WHERE created_at < ?1 AND (request_body != '' OR user_response_body != '')",
                params![cutoff],
            )?;
            Ok(())
        })
        .await
        .map_err(|e| format!("cleanup user request fields: {e}"))
    }
}

/// Clear upstream request body fields for logs older than retention_days.
/// `*_headers`（元数据，已脱敏）始终保留至行级 retention 删除；仅清 `*_body`（上游请求 / 响应正文）。
/// Does NOT delete the log row — keeps token stats and metadata.
#[track_caller]
pub fn cleanup_upstream_request_fields(db: &Db, retention_days: u32) -> impl std::future::Future<Output = Result<(), String>> + '_ {
    let __db_caller = std::panic::Location::caller();
    async move {
    let Some(cutoff) = retention_cutoff(retention_days) else { return Ok(()); };
    db
        .call_traced(None, __db_caller, move |conn| {
            conn.execute(
                "UPDATE proxy_log SET upstream_request_body = '' WHERE created_at < ?1 AND upstream_request_body != ''",
                params![cutoff],
            )?;
            Ok(())
        })
        .await
        .map_err(|e| format!("cleanup upstream request fields: {e}"))
    }
}

#[track_caller]
pub fn count_proxy_logs(db: &Db) -> impl std::future::Future<Output = Result<u32, String>> + '_ {
    let __db_caller = std::panic::Location::caller();
    async move {
    db
        .call_traced(None, __db_caller, move |conn| {
            Ok(conn.query_row("SELECT COUNT(*) FROM proxy_log WHERE deleted_at = 0", [], |row| row.get(0))?)
        })
        .await
        .map_err(|e| e.to_string())
    }
}

