#![cfg(test)]
use super::test_support::*;
use super::*;
use rusqlite::params;

/// 三文件库：写后重开 DB 读回；字段正确、success 派生、倒序。
#[tokio::test]
async fn api_call_log_roundtrip_across_reopen() {
    let (db, dir) = three_file_db().await;
    insert_api_call_log(&db, "platform_list", 200, 12, "curl/8", API_CALL_SOURCE_THIRD_PARTY).await.unwrap();
    insert_api_call_log(&db, "platform_delete", 403, 3, "", API_CALL_SOURCE_SETTINGS_TEST).await.unwrap();
    drop(db);
    let db = Db::new(dir.path().join("aidog.db").to_str().unwrap()).await.unwrap();
    crate::schema::init_tables_raw(&db, std::sync::Arc::new(|_c, _m| Ok(()))).await.unwrap();
    let rows = list_api_call_logs(&db, 10).await.unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].command, "platform_delete");
    assert!(!rows[0].success);
    assert_eq!(rows[0].status_code, 403);
    assert_eq!(rows[0].source, "settings_test");
    assert_eq!(rows[1].command, "platform_list");
    assert!(rows[1].success);
    assert_eq!(rows[1].duration_ms, 12);
    assert_eq!(rows[1].user_agent, "curl/8");
    assert_eq!(rows[1].source, "third_party");
}

/// 迁移幂等：同一 log.db 重复跑 early migration 不报错、不丢行。
#[tokio::test]
async fn api_call_log_migration_idempotent() {
    let (db, _dir) = three_file_db().await;
    insert_api_call_log(&db, "x", 200, 1, "", API_CALL_SOURCE_THIRD_PARTY).await.unwrap();
    for _ in 0..2 {
        db.call_proxy_log_traced(None, std::panic::Location::caller(), |c| {
            crate::schema_early::run_migrations_proxy_log_early(c)?;
            Ok(())
        })
        .await
        .unwrap();
    }
    assert_eq!(list_api_call_logs(&db, 10).await.unwrap().len(), 1);
}

/// 超过 7 天的行被清理，7 天内的保留。
#[tokio::test]
async fn api_call_log_cleanup_drops_rows_older_than_retention() {
    let (db, _dir) = three_file_db().await;
    insert_api_call_log(&db, "fresh", 200, 1, "", API_CALL_SOURCE_THIRD_PARTY).await.unwrap();
    let old = now() - 8 * 86_400_000;
    let edge = now() - 6 * 86_400_000;
    db.call_proxy_log_traced(None, std::panic::Location::caller(), move |c| {
        for (cmd, ts) in [("old", old), ("edge", edge)] {
            c.execute(
                "INSERT INTO api_call_log (created_at, command) VALUES (?1, ?2)",
                params![ts, cmd],
            )?;
        }
        Ok(())
    })
    .await
    .unwrap();
    cleanup_api_call_logs(&db, API_CALL_LOG_RETENTION_DAYS).await.unwrap();
    let mut names: Vec<_> = list_api_call_logs(&db, 10).await.unwrap().into_iter().map(|r| r.command).collect();
    names.sort();
    assert_eq!(names, ["edge", "fresh"]);
}
