use super::*;
use aidog_db::test_support::three_file_db;

#[tokio::test]
async fn retention_defaults_to_30_and_roundtrips() {
    let (db, _dir) = three_file_db().await;
    assert_eq!(load_retention_days(&db).await, aidog_db::TOOL_LOG_DEFAULT_RETENTION_DAYS);
    save_retention_days(&db, 7).await.unwrap();
    assert_eq!(load_retention_days(&db).await, 7);
    save_retention_days(&db, 0).await.unwrap();
    assert_eq!(load_retention_days(&db).await, 0, "0 = 永久保留，不被默认值吞掉");
}

/// proxy_log_cleanup_expired 的清理链按工具日志自己的保留天数清，不随 proxy_log retention。
#[tokio::test]
async fn retention_cleanup_chain_uses_tool_log_setting() {
    let (db, _dir) = three_file_db().await;
    let old = aidog_db::now() - 40 * 86_400_000;
    db.call_proxy_log_traced(None, std::panic::Location::caller(), move |c| {
        c.execute("INSERT INTO tool_log (created_at, tool, query) VALUES (?1,'web_search','old')", rusqlite::params![old])?;
        Ok(())
    })
    .await
    .unwrap();
    save_retention_days(&db, 90).await.unwrap();
    crate::proxy_cmd::proxy_log::run_retention_cleanup(&db, &Default::default()).await;
    let n = aidog_db::list_tool_logs(&db, &Default::default(), 10, 0).await.unwrap().total;
    assert_eq!(n, 1, "90 天保留：40 天前的行还在");
    save_retention_days(&db, 30).await.unwrap();
    crate::proxy_cmd::proxy_log::run_retention_cleanup(&db, &Default::default()).await;
    let n = aidog_db::list_tool_logs(&db, &Default::default(), 10, 0).await.unwrap().total;
    assert_eq!(n, 0, "30 天保留：40 天前的行被清");
}
