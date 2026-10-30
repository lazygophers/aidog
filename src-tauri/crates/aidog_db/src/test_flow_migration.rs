//! 请求流向 票 02：migration 20261030-01（proxy_log.client）/ 20261030-02（stats_agg_hourly 6 元组重建）。
#![cfg(test)]
use super::test_support::*;
use rusqlite::Connection;

/// 20261030-02 之前的 stats_agg_hourly（4 元组 UNIQUE + 两个索引）。
const OLD_DDL: &str = "CREATE TABLE stats_agg_hourly (
    id INTEGER PRIMARY KEY AUTOINCREMENT, time_hour TEXT NOT NULL, model TEXT NOT NULL DEFAULT '',
    group_key TEXT NOT NULL DEFAULT '', platform_id INTEGER NOT NULL DEFAULT 0,
    request_count INTEGER NOT NULL DEFAULT 0, success_count INTEGER NOT NULL DEFAULT 0,
    error_count INTEGER NOT NULL DEFAULT 0, sum_input_tokens INTEGER NOT NULL DEFAULT 0,
    sum_output_tokens INTEGER NOT NULL DEFAULT 0, sum_cache_tokens INTEGER NOT NULL DEFAULT 0,
    sum_est_cost REAL NOT NULL DEFAULT 0, sum_duration_ms INTEGER NOT NULL DEFAULT 0,
    created_at INTEGER NOT NULL DEFAULT 0, updated_at INTEGER NOT NULL DEFAULT 0,
    deleted_at INTEGER NOT NULL DEFAULT 0, UNIQUE(time_hour, model, group_key, platform_id));
CREATE INDEX idx_stats_agg_time ON stats_agg_hourly(time_hour);
CREATE INDEX idx_stats_agg_platform ON stats_agg_hourly(platform_id);";

fn count(conn: &Connection, sql: &str) -> i64 {
    conn.query_row(sql, [], |r| r.get(0)).unwrap()
}

fn insert6(conn: &Connection, client: &str, req_model: &str) -> rusqlite::Result<usize> {
    conn.execute(
        "INSERT INTO stats_agg_hourly (time_hour, model, group_key, platform_id, client, req_model) \
         VALUES ('2026-10-30 10:00:00','m','g',1,?1,?2)",
        rusqlite::params![client, req_model],
    )
}

#[test]
fn rebuild_keeps_rows_adds_columns_6_tuple_unique_and_is_idempotent() {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch(OLD_DDL).unwrap();
    for i in 0..3 {
        conn.execute(
            "INSERT INTO stats_agg_hourly (time_hour, model, group_key, platform_id, request_count, sum_est_cost) \
             VALUES ('2026-10-30 10:00:00', ?1, 'g', 1, 5, 1.5)",
            [format!("m{i}")],
        )
        .unwrap();
    }
    crate::schema_late::rebuild_stats_agg_six_tuple(&conn).unwrap();
    assert_eq!(count(&conn, "SELECT COUNT(*) FROM stats_agg_hourly"), 3, "行数不变");
    assert_eq!(
        count(&conn, "SELECT COUNT(*) FROM stats_agg_hourly WHERE client='' AND req_model='' AND request_count=5 AND sum_est_cost=1.5"),
        3,
        "新列 ''，旧值保留"
    );
    assert_eq!(
        count(&conn, "SELECT COUNT(*) FROM sqlite_master WHERE type='index' AND name IN ('idx_stats_agg_time','idx_stats_agg_platform')"),
        2,
        "两索引重建"
    );
    assert_eq!(count(&conn, "SELECT COUNT(*) FROM sqlite_master WHERE name='stats_agg_hourly_old'"), 0);
    // 同 4 元组不同 client / req_model 可插入；同 6 元组冲突。
    insert6(&conn, "claude_code", "req-a").unwrap();
    insert6(&conn, "codex", "req-a").unwrap();
    insert6(&conn, "claude_code", "req-b").unwrap();
    assert!(insert6(&conn, "claude_code", "req-a").is_err(), "6 元组冲突");
    // 幂等：二次运行不报错、不丢数据。
    let before = count(&conn, "SELECT COUNT(*) FROM stats_agg_hourly");
    crate::schema_late::rebuild_stats_agg_six_tuple(&conn).unwrap();
    assert_eq!(count(&conn, "SELECT COUNT(*) FROM stats_agg_hourly"), before);
}

#[tokio::test]
async fn fresh_install_is_new_shape_and_proxy_log_has_client() {
    let (db, _dir) = three_file_db().await;
    let proxy_log_client: i64 = db
        .call_read_proxy_log_traced(None, std::panic::Location::caller(), |c| {
            Ok(c.query_row(
                "SELECT COUNT(*) FROM pragma_table_info('proxy_log') WHERE name='client' AND [notnull]=1",
                [],
                |r| r.get(0),
            )?)
        })
        .await
        .unwrap();
    assert_eq!(proxy_log_client, 1, "proxy_log.client 存在且 NOT NULL");
    let agg_cols: i64 = db
        .call_read_traced(None, std::panic::Location::caller(), |c| {
            Ok(c.query_row(
                "SELECT COUNT(*) FROM pragma_table_info('stats_agg_hourly') WHERE name IN ('client','req_model')",
                [],
                |r| r.get(0),
            )?)
        })
        .await
        .unwrap();
    assert_eq!(agg_cols, 2, "新装直接新形态");
}

/// 跨库搬迁：log.db 带旧形态 stats_agg_hourly，主库已是新形态 → 显式列清单搬迁，不报列数不匹配。
#[tokio::test]
async fn cross_db_move_from_old_shape_log_db_into_new_shape_main() {
    let (db, _dir) = three_file_db().await;
    db.call_proxy_log_traced(None, std::panic::Location::caller(), |c| {
        c.execute_batch(OLD_DDL)?;
        c.execute_batch(
            "INSERT INTO stats_agg_hourly (time_hour, model, group_key, platform_id, request_count) \
             VALUES ('2026-10-30 10:00:00','legacy-m','g',9,7);",
        )?;
        Ok(())
    })
    .await
    .unwrap();
    // 再跑一遍 init：主库 stats 为空 + log.db 有表 → 触发搬迁。
    crate::schema::init_tables_raw(&db, std::sync::Arc::new(|_c, _m| Ok(())))
        .await
        .unwrap();
    let (n, client, req_model): (i64, String, String) = db
        .call_read_traced(None, std::panic::Location::caller(), |c| {
            Ok(c.query_row(
                "SELECT request_count, client, req_model FROM stats_agg_hourly WHERE model='legacy-m'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )?)
        })
        .await
        .unwrap();
    assert_eq!((n, client.as_str(), req_model.as_str()), (7, "", ""));
}
