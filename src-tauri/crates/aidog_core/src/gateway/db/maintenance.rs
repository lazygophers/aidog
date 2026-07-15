use super::*;
use rusqlite::{params, Connection, Result as SqlResult};

/// 含 `deleted_at` 列、纳入每日统一软删清理的表清单。
///
/// **真值源**：`schema_early.rs` CREATE TABLE（grep 确认每表含 `deleted_at INTEGER ...`）。
/// 清单内每表由 `purge_all_soft_deleted` 跑 `DELETE WHERE deleted_at > 0 AND deleted_at < cutoff`。
/// 缺 `deleted_at` 列的表（如 `middleware_rule` / `notification` / `mcp_server` —— 三表均无软删列）
/// **禁** 入清单：DELETE 会因 Unknown column 炸运行时；运行时 schema 漂移新增/删除列时
/// `purge_all_soft_deleted` 的 per-table 错误兜底（warn + skip）会吞掉单表失败，不阻塞他表。
///
/// `"group"` 是 SQL 保留字，SQL 标识符引号必须保留；map key 用 `group`（去引号）便于日志可读。
pub(crate) const SOFT_DELETE_TABLES: &[(&str, &str)] = &[
    // (SQL 标识符（含引号）, map key / 日志名（去引号）)
    ("platform", "platform"),
    ("\"group\"", "group"),
    ("group_platform", "group_platform"),
    ("setting", "setting"),
    ("proxy_log", "proxy_log"),
    ("model_price", "model_price"),
];

/// 每日定时清理：跨表永久删除软删行（`deleted_at > 0 AND deleted_at < now - older_than_secs`）。
///
/// - 表驱动：遍历 `SOFT_DELETE_TABLES`，每表独立 `call_traced` DELETE。
/// - 容错：单表失败（如 schema 漂移致缺列、SQL 错误）→ `tracing::warn!(table, error)` + 该表不插 map + 继续；
///   全部失败才返 Err（罕见，仅保留 Result 语义）。
/// - 返回 `HashMap<表名(去引号), 删除行数>`：调用方记 per-table 日志，空 map 或全 0 由调用方降级 debug。
/// - `older_than_secs`：秒为单位的阈值；`deleted_at` 列存毫秒级 Unix 时间戳（与 `now()` 一致），
///   故 cutoff = `now() - older_than_secs * 1000`。
///
/// 与 `platform_lifecycle::purge_old_soft_deleted_platforms`（单表快路径，测试依赖，保留）和
/// `proxy_log::purge_deleted_proxy_logs`（无阈值全删语义不同，保留）独立。
pub fn purge_all_soft_deleted(
    db: &Db,
    older_than_secs: i64,
) -> impl std::future::Future<Output = Result<std::collections::HashMap<String, u64>, String>> + '_ {
    let __db_caller = std::panic::Location::caller();
    async move {
        let cutoff_ms = now() - older_than_secs.saturating_mul(1000);
        let mut map: std::collections::HashMap<String, u64> = std::collections::HashMap::new();
        let mut failures: u32 = 0;
        for &(sql_ident, key) in SOFT_DELETE_TABLES {
            let sql = format!(
                "DELETE FROM {sql_ident} WHERE deleted_at > 0 AND deleted_at < ?1"
            );
            let res = db
                .call_traced(None, __db_caller, move |conn| {
                    Ok(conn.execute(&sql, params![cutoff_ms])? as u64)
                })
                .await;
            match res {
                Ok(n) => {
                    map.insert(key.to_string(), n);
                }
                Err(e) => {
                    failures += 1;
                    tracing::warn!(
                        table = key,
                        error = %e,
                        "purge_all_soft_deleted: skip table (schema drift or SQL error)"
                    );
                }
            }
        }
        if failures as usize == SOFT_DELETE_TABLES.len() {
            return Err(format!(
                "purge_all_soft_deleted: all {failures} tables failed"
            ));
        }
        Ok(map)
    }
}

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
            // VACUUM 清空 sqlite_stat1，重建统计（迁移 034 已建过一次，VACUUM 后须重跑）。
            let _ = c.execute_batch("ANALYZE;");
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
            // VACUUM 重建库会清空 sqlite_stat1，重跑 ANALYZE 重建统计避免规划器退化。
            let _ = c.execute_batch("ANALYZE;");
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

/// 当前 DB 文件占用的逻辑字节数（`page_count * page_size`）。调度器阈值触发全量 VACUUM 用。
/// ponytail: 复用 db_size_bytes，对外只多一层 call_traced 包装。
#[track_caller]
pub fn db_file_size(db: &Db) -> impl std::future::Future<Output = Result<i64, String>> + '_ {
    let __db_caller = std::panic::Location::caller();
    async move {
        db.call_traced(None, __db_caller, |c| Ok(db_size_bytes(c)?))
            .await
            .map_err(|e| format!("db_file_size: {e}"))
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

/// Clear user-side raw fields for logs older than retention_days.
/// 清理列集 = 用户侧「原始信息」全集：request_headers / request_body /
/// user_response_headers / user_response_body（与 from_log 的 strip_user 列集对称）。
/// Does NOT delete the log row — keeps token stats and metadata.
#[track_caller]
pub fn cleanup_user_request_fields(db: &Db, retention_days: u32) -> impl std::future::Future<Output = Result<(), String>> + '_ {
    let __db_caller = std::panic::Location::caller();
    async move {
    let Some(cutoff) = retention_cutoff(retention_days) else { return Ok(()); };
    // proxy_log 在 proxy_log.db（proxy-log-db-split s3），走专用写连接。
    db
        .call_proxy_log_traced(None, __db_caller, move |conn| {
            conn.execute(
                "UPDATE proxy_log SET request_headers = '', request_body = '', user_response_headers = '', user_response_body = '' \
                 WHERE created_at < ?1 AND (request_headers != '' OR request_body != '' OR user_response_headers != '' OR user_response_body != '')",
                params![cutoff],
            )?;
            Ok(())
        })
        .await
        .map_err(|e| format!("cleanup user request fields: {e}"))
    }
}

/// Clear upstream-side raw fields for logs older than retention_days.
/// 清理列集 = 上游侧「原始信息」全集：upstream_request_headers / upstream_request_body /
/// upstream_response_headers / response_body（上游响应正文，与 from_log 的 strip_upstream 列集对称）。
/// response_body 是体积大头（实测真实库 376MB），归本级 retention 回收。回客户端正文
/// user_response_body 归 user_request_retention_days（见 cleanup_user_request_fields）。
/// Does NOT delete the log row — keeps token stats and metadata.
/// 注意：仅改清理逻辑，存量大体积 body 的实际回收发生在用户下次 retention 周期运行
/// 触发本 UPDATE + 后续 incremental_vacuum，迁移本身不强清存量（避免启动期长锁）。
#[track_caller]
pub fn cleanup_upstream_request_fields(db: &Db, retention_days: u32) -> impl std::future::Future<Output = Result<(), String>> + '_ {
    let __db_caller = std::panic::Location::caller();
    async move {
    let Some(cutoff) = retention_cutoff(retention_days) else { return Ok(()); };
    // proxy_log 在 proxy_log.db（proxy-log-db-split s3），走专用写连接。
    db
        .call_proxy_log_traced(None, __db_caller, move |conn| {
            conn.execute(
                "UPDATE proxy_log SET upstream_request_headers = '', upstream_request_body = '', upstream_response_headers = '', response_body = '' \
                 WHERE created_at < ?1 AND (upstream_request_headers != '' OR upstream_request_body != '' OR upstream_response_headers != '' OR response_body != '')",
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
    // proxy_log 在 proxy_log.db（proxy-log-db-split s3），走专用读池。
    db
        .call_read_proxy_log_traced(None, __db_caller, move |conn| {
            Ok(conn.query_row("SELECT COUNT(*) FROM proxy_log WHERE deleted_at = 0", [], |row| row.get(0))?)
        })
        .await
        .map_err(|e| e.to_string())
    }
}

