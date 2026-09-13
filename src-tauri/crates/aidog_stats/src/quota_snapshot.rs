//! 配额快照（chart-engine D4 / #34）：`quota_snapshot` 事件式历史表的写入 / 查询 / retention。
//!
//! 无定时器：真实余额查询成功时（estimate::calibrate_from_quota 成功路径）调
//! [`insert_quota_snapshot`] 顺手插一行。retention 对齐 proxy_log `retention_days`
//! （`run_retention_cleanup` 链删整行）。表落主库（migration 20260913-01）。

use aidog_db::models::*;
use aidog_db::{Db, incremental_vacuum_conn, now, retention_cutoff_secs};
use rusqlite::params;

/// 真实余额查询成功后插入一条快照（主库写槽，短事务）。
#[track_caller]
pub fn insert_quota_snapshot(
    db: &Db,
    platform_id: u64,
    est_balance_remaining: f64,
) -> impl std::future::Future<Output = Result<(), String>> + '_ {
    let __db_caller = std::panic::Location::caller();
    async move {
        db.call_traced(None, __db_caller, move |conn| {
            conn.execute(
                "INSERT INTO quota_snapshot (platform_id, est_balance_remaining, created_at)
                 VALUES (?1, ?2, ?3)",
                params![platform_id as i64, est_balance_remaining, now()],
            )?;
            Ok(())
        })
        .await
        .map_err(|e| format!("insert quota snapshot: {e}"))
    }
}

/// 查询时间窗内快照序列（created_at 升序）。缺省窗口：过去 30 天到当前
/// （快照仅在真实查询时产生，频率低，窗口取宽于散点的 7d 缺省）。
/// 空平台 / 空窗 → 空数组，不报错。
#[track_caller]
pub fn quota_snapshots<'a>(
    db: &'a Db,
    query: &'a QuotaSnapshotsQuery,
) -> impl std::future::Future<Output = Result<Vec<QuotaSnapshot>, String>> + 'a {
    let __db_caller = std::panic::Location::caller();
    async move {
        let end = query.end.unwrap_or_else(now);
        let start = query
            .start
            .unwrap_or_else(|| end - 30 * 86_400_000);
        let platform_id = query.platform_id.map(|p| p as i64);
        db.call_read_traced(None, __db_caller, move |conn| {
            let mut stmt = conn
                .prepare(
                    "SELECT platform_id, est_balance_remaining, created_at FROM quota_snapshot \
                     WHERE created_at >= ?1 AND created_at <= ?2",
                )
                .map_err(|e| tokio_rusqlite::Error::Other(e.into()))?;
            let rows = stmt
                .query_map(params![start, end], |r| {
                    Ok(QuotaSnapshot {
                        platform_id: r.get(0)?,
                        est_balance_remaining: r.get(1)?,
                        created_at: r.get(2)?,
                    })
                })
                .map_err(|e| tokio_rusqlite::Error::Other(e.into()))?
                .collect::<Result<Vec<_>, _>>()
                .map_err(|e| tokio_rusqlite::Error::Other(e.into()))?;
            let mut rows = rows;
            if let Some(pid) = platform_id {
                rows.retain(|s| s.platform_id == pid);
            }
            rows.sort_by_key(|s| s.created_at);
            Ok(rows)
        })
        .await
        .map_err(|e| e.to_string())
    }
}

/// 删除超期快照（整行硬删，同 `cleanup_stats_agg` / `cleanup_proxy_logs` 口径）。
/// `value == 0`（永久保留）→ 跳过。挂进 proxy_log 的 `run_retention_cleanup` 链。
#[track_caller]
pub fn cleanup_quota_snapshots(
    db: &Db,
    value: u32,
    unit: RetentionUnit,
) -> impl std::future::Future<Output = Result<(), String>> + '_ {
    let __db_caller = std::panic::Location::caller();
    async move {
        let Some(cutoff) = retention_cutoff_secs(unit.secs(value)) else {
            return Ok(());
        };
        db.call_traced(None, __db_caller, move |conn| {
            conn.execute(
                "DELETE FROM quota_snapshot WHERE created_at < ?1",
                params![cutoff],
            )?;
            incremental_vacuum_conn(conn, 100);
            Ok(())
        })
        .await
        .map_err(|e| format!("cleanup quota snapshots: {e}"))
    }
}
