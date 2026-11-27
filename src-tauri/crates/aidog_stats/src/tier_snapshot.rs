//! 每档配额快照（订阅消耗曲线分档改版）：`quota_tier_snapshot` 事件表的写入 / 查询。
//!
//! 真实 quota 查询成功时（estimate::calibrate_from_quota 成功路径，与 quota_snapshot 同点）
//! 每档插一行（migration 20261127-01，主库）。查询按平台返回全部历史（queried_at 升序）。
//! 幂等靠 UNIQUE(platform_id, tier_name, unit, queried_at) + INSERT OR IGNORE。

use aidog_db::models::QuotaTierSnapshot;
use aidog_db::Db;
use rusqlite::params;
use serde::{Deserialize, Serialize};

/// 一次查询各档位的落库行（写入入参；读取走 `QuotaTierSnapshot`）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TierSnapshot {
    pub name: String,
    pub unit: String,
    pub utilization: f64,
    pub limit: Option<f64>,
    pub remaining: Option<f64>,
    pub resets_at: Option<String>,
    pub queried_at: i64,
}

/// 一次查询的各档位落库（每档一行）。失败档（name 空）跳过。
#[track_caller]
pub fn insert_tier_snapshots(
    db: &Db,
    platform_id: u64,
    rows: Vec<TierSnapshot>,
) -> impl std::future::Future<Output = Result<(), String>> + '_ {
    let __db_caller = std::panic::Location::caller();
    async move {
        db.call_traced(None, __db_caller, move |conn| {
            for r in rows {
                if r.name.is_empty() {
                    continue;
                }
                conn.execute(
                    "INSERT OR IGNORE INTO quota_tier_snapshot \
                     (platform_id, tier_name, unit, utilization, limit_value, remaining, resets_at, queried_at) \
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                    params![
                        platform_id as i64,
                        r.name,
                        r.unit,
                        r.utilization,
                        r.limit,
                        r.remaining,
                        r.resets_at.unwrap_or_default(),
                        r.queried_at,
                    ],
                )?;
            }
            Ok(())
        })
        .await
        .map_err(|e| format!("insert tier snapshots: {e}"))
    }
}

/// 该平台全部历史快照（queried_at 升序）。空 → 空数组。
#[track_caller]
pub fn tier_snapshots(
    db: &Db,
    platform_id: u64,
) -> impl std::future::Future<Output = Result<Vec<QuotaTierSnapshot>, String>> + '_ {
    let __db_caller = std::panic::Location::caller();
    async move {
        db.call_read_traced(None, __db_caller, move |conn| {
            let mut stmt = conn
                .prepare(
                    "SELECT tier_name, unit, utilization, limit_value, remaining, resets_at, queried_at \
                     FROM quota_tier_snapshot WHERE platform_id = ?1 ORDER BY queried_at ASC, id ASC",
                )
                ?;
            let rows = stmt
                .query_map(params![platform_id as i64], |r| {
                    Ok(QuotaTierSnapshot {
                        tier_name: r.get(0)?,
                        unit: r.get(1)?,
                        utilization: r.get(2)?,
                        limit_value: r.get(3)?,
                        remaining: r.get(4)?,
                        resets_at: r.get(5)?,
                        queried_at: r.get(6)?,
                    })
                })
                ?
                .collect::<Result<Vec<_>, _>>()
                ?;
            Ok(rows)
        })
        .await
        .map_err(|e| e.to_string())
    }
}

/// 删除超期快照（同 cleanup_quota_snapshots 口径）；`value == 0` 永久保留 → 跳过。
#[track_caller]
pub fn cleanup_tier_snapshots(
    db: &Db,
    value: u32,
    unit: aidog_db::RetentionUnit,
) -> impl std::future::Future<Output = Result<(), String>> + '_ {
    let __db_caller = std::panic::Location::caller();
    async move {
        // retention_cutoff_secs 返回毫秒（与表内 queried_at 同口径）。
        let Some(cutoff) = aidog_db::retention_cutoff_secs(unit.secs(value)) else {
            return Ok(());
        };
        db.call_traced(None, __db_caller, move |conn| {
            conn.execute(
                "DELETE FROM quota_tier_snapshot WHERE queried_at < ?1",
                params![cutoff],
            )?;
            Ok(())
        })
        .await
        .map_err(|e| format!("cleanup tier snapshots: {e}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn snapshots_are_idempotent_and_ordered_per_platform() {
        let (db, _dir) = aidog_db::test_support::three_file_db().await;
        let row = TierSnapshot { name: "weekly_limit".into(), unit: "tokens".into(), utilization: 20.0, limit: Some(100.0), remaining: Some(80.0), resets_at: None, queried_at: 20 };
        let mut earlier = row.clone();
        earlier.queried_at = 10;
        insert_tier_snapshots(&db, 1, vec![row.clone(), earlier, row.clone()]).await.unwrap();
        insert_tier_snapshots(&db, 2, vec![row]).await.unwrap();
        let rows = tier_snapshots(&db, 1).await.unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows.iter().map(|r| r.queried_at).collect::<Vec<_>>(), vec![10, 20]);
        assert_eq!(rows[0].remaining, Some(80.0));
        assert_eq!(rows[0].tier_name, "weekly_limit");
    }

    #[tokio::test]
    async fn empty_name_rows_are_skipped_and_read_is_empty_safe() {
        let (db, _dir) = aidog_db::test_support::three_file_db().await;
        let bad = TierSnapshot { name: "".into(), unit: "tokens".into(), utilization: 0.0, limit: None, remaining: None, resets_at: None, queried_at: 1 };
        insert_tier_snapshots(&db, 3, vec![bad]).await.unwrap();
        assert!(tier_snapshots(&db, 3).await.unwrap().is_empty());
        assert!(tier_snapshots(&db, 999).await.unwrap().is_empty());
    }
}
