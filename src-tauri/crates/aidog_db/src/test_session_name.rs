//! session_name_cache：三文件库（主库路由）写后重开读回 + 30 天清理。
use super::session_name::*;
use super::test_support::*;
use crate::{Db, now};
use rusqlite::params;

async fn set_updated_at(db: &Db, client: &'static str, sid: &'static str, secs: i64) {
    db.call_traced(None, std::panic::Location::caller(), move |conn| {
        conn.execute(
            "UPDATE session_name_cache SET updated_at = ?3 WHERE client = ?1 AND session_id = ?2",
            params![client, sid, secs],
        )?;
        Ok(())
    })
    .await
    .unwrap();
}

#[tokio::test]
async fn put_then_reopen_reads_back_and_upserts() {
    let (db, dir) = three_file_db().await;
    assert_eq!(get_session_name_cache(&db, "claude_code", "s1").await.unwrap(), None);
    put_session_name_cache(&db, "claude_code", "s1", r#"{"name":"a"}"#).await.unwrap();
    put_session_name_cache(&db, "claude_code", "s1", r#"{"name":"b"}"#).await.unwrap();
    put_session_name_cache(&db, "codex", "s1", r#"{"name":"c"}"#).await.unwrap();
    drop(db);

    // 重开同一组文件：数据真落盘；主键含 client，同 session_id 不串。
    let db2 = Db::new(dir.path().join("aidog.db").to_str().unwrap()).await.unwrap();
    assert_eq!(
        get_session_name_cache(&db2, "claude_code", "s1").await.unwrap().as_deref(),
        Some(r#"{"name":"b"}"#)
    );
    assert_eq!(
        get_session_name_cache(&db2, "codex", "s1").await.unwrap().as_deref(),
        Some(r#"{"name":"c"}"#)
    );
}

#[tokio::test]
async fn cleanup_drops_rows_older_than_30_days() {
    let (db, dir) = three_file_db().await;
    put_session_name_cache(&db, "claude_code", "old", "{}").await.unwrap();
    put_session_name_cache(&db, "claude_code", "fresh", "{}").await.unwrap();
    let day = 24 * 3600;
    set_updated_at(&db, "claude_code", "old", now() / 1000 - 31 * day).await;
    set_updated_at(&db, "claude_code", "fresh", now() / 1000 - 29 * day).await;

    cleanup_session_name_cache(&db).await.unwrap();
    drop(db);

    let db2 = Db::new(dir.path().join("aidog.db").to_str().unwrap()).await.unwrap();
    assert_eq!(get_session_name_cache(&db2, "claude_code", "old").await.unwrap(), None);
    assert!(get_session_name_cache(&db2, "claude_code", "fresh").await.unwrap().is_some());
}

#[tokio::test]
async fn startup_init_purges_stale_rows() {
    let (db, _dir) = three_file_db().await;
    put_session_name_cache(&db, "codex", "old", "{}").await.unwrap();
    set_updated_at(&db, "codex", "old", now() / 1000 - 40 * 24 * 3600).await;
    // 再跑一遍 init（= 下次启动）：迁移幂等且顺带清过期行。
    crate::schema::init_tables_raw(&db, std::sync::Arc::new(|_c, _m| Ok(())))
        .await
        .unwrap();
    assert_eq!(get_session_name_cache(&db, "codex", "old").await.unwrap(), None);
}
