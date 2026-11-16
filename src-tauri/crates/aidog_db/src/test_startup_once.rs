//! 启动慢根因回归：proxy_log 的一次性回填 / ANALYZE 不得每次启动重跑（大库上全表重写 + 全表扫描）。
#![cfg(test)]
use super::test_support::*;

async fn reinit(db: &crate::Db) {
    crate::schema::init_tables_raw(db, std::sync::Arc::new(|_c, _m| Ok(())))
        .await
        .unwrap();
}

#[tokio::test]
async fn done_backfill_runs_once_not_every_startup() {
    let (db, _dir) = three_file_db().await;
    db.call_proxy_log_traced(None, std::panic::Location::caller(), |c| {
        c.execute(
            "INSERT INTO proxy_log (id, platform_id, group_key, model, source_protocol, \
             status_code, est_cost, blocked_by, blocked_reason, created_at, deleted_at, done) \
             VALUES ('r', 1, '', 'm', 'anthropic', 200, 0, '', '', 1, 0, 0)",
            [],
        )?;
        Ok(())
    })
    .await
    .unwrap();
    reinit(&db).await; // = 重启
    let done: i64 = db
        .call_read_proxy_log_traced(None, std::panic::Location::caller(), |c| {
            c.query_row("SELECT done FROM proxy_log WHERE id = 'r'", [], |r| r.get(0))
        })
        .await
        .unwrap();
    assert_eq!(done, 0, "一次性回填第二次启动不应再改写存量行");
}
