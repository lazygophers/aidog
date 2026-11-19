//! client_usage.db 票 01 验收测试：四文件拆库路由、幂等定位键、保留设置、清理语义。
use crate::client_usage::*;
use crate::test_support::four_file_db;

use crate::Db;
use rusqlite::params;

async fn count_calls(db: &Db) -> i64 {
    db.call_read_client_usage_traced(None, std::panic::Location::caller(), |conn| {
        conn.query_row("SELECT COUNT(*) FROM client_call", [], |r| r.get(0))
    })
    .await
    .expect("count client_call")
}

async fn count_sessions(db: &Db, code: String) -> i64 {
    db.call_read_client_usage_traced(None, std::panic::Location::caller(), move |conn| {
        conn.query_row(
            "SELECT COUNT(*) FROM client_session WHERE client_code = ?1",
            params![code],
            |r| r.get(0),
        )
    })
    .await
    .expect("count client_session")
}

async fn count_progress(db: &Db, code: String) -> i64 {
    db.call_read_client_usage_traced(None, std::panic::Location::caller(), move |conn| {
        conn.query_row(
            "SELECT COUNT(*) FROM client_file_progress WHERE client_code = ?1",
            params![code],
            |r| r.get(0),
        )
    })
    .await
    .expect("count progress")
}

/// 票面验收 ①：四个独立临时文件拆库 + 新表读写走新 handle。
/// 文件库下主库 handle 查 client_call 必 `no such table`（错路由当场炸）；
/// 写后重开 DB 读回。内存库 fallback 三 handle 同连接测不出拆库，故用 four_file_db。
#[tokio::test]
async fn client_usage_four_file_split_and_routing() {
    let (db, dir) = four_file_db().await;
    // 4 个独立 db 文件都在盘上（client_usage.db 由 Db::new 随主库派生）。
    for f in ["aidog.db", "log.db", "platform.db", "client_usage.db"] {
        assert!(
            dir.path().join(f).exists(),
            "{f} 应由 Db::new 建在主库同目录"
        );
    }
    let row = ClientCallRow {
        client_code: "claude_code".into(),
        session_id: "sess-1".into(),
        model: "claude-x".into(),
        input_tokens: 10,
        output_tokens: 20,
        cost: None, // 未定价哨兵
        source_file_id: "f1".into(),
        file_pos: 1,
        ..Default::default()
    };
    assert_eq!(insert_client_calls(&db, &[row.clone()]).await.expect("insert"), 1);
    upsert_client_file_progress(&db, "claude_code".into(), "/tmp/x.jsonl".into(), 100, 1, 100, 0)
        .await
        .expect("upsert progress");
    upsert_client_session(&db, "claude_code".into(), "sess-1".into(), "t".into(), "/w".into(), 1, 2)
        .await
        .expect("upsert session");

    // 错路由判定：主库 / log.db handle 上没有 client_call（拆库实锤）。
    for err_db in [
        db.call_traced(None, std::panic::Location::caller(), |conn| {
            conn.query_row("SELECT COUNT(*) FROM client_call", [], |r| r.get::<_, i64>(0))
        })
        .await
        .err()
        .expect("主库 handle 查 client_call 应报错"),
        db.call_proxy_log_traced(None, std::panic::Location::caller(), |conn| {
            conn.query_row("SELECT COUNT(*) FROM client_call", [], |r| r.get::<_, i64>(0))
        })
        .await
        .err()
        .expect("log.db handle 查 client_call 应报错"),
    ] {
        assert!(
            err_db.to_string().contains("no such table"),
            "错路由应报 no such table，实际: {err_db}"
        );
    }

    // 写后重开 DB 读回。
    drop(db);
    let (db2, _dir) = {
        // TempDir 被 move 进 first db? No—— dir 未 move；重建 Db 于同路径。
        let path = dir.path().join("aidog.db");
        let db2 = Db::new(path.to_str().expect("utf8"))
            .await
            .expect("reopen");
        crate::schema::init_tables_raw(
            &db2,
            std::sync::Arc::new(|_conn, _map| Ok(())),
        )
        .await
        .expect("init tables");
        (db2, dir)
    };
    let cnt = count_calls(&db2).await;
    assert_eq!(cnt, 1, "重开后明细仍在");
    let cost: Option<f64> = db2
        .call_read_client_usage_traced(None, std::panic::Location::caller(), |conn| {
            conn.query_row("SELECT cost FROM client_call LIMIT 1", [], |r| r.get(0))
        })
        .await
        .expect("read cost");
    assert_eq!(cost, None, "未定价 cost 应为 NULL");
}

/// 票面验收 ②：同一定位键重复插入幂等，行数不变。
#[tokio::test]
async fn client_call_insert_idempotent_on_position_key() {
    let (db, _dir) = four_file_db().await;
    let row = ClientCallRow {
        client_code: "codex".into(),
        session_id: "s1".into(),
        source_file_id: "f9".into(),
        file_pos: 7,
        ..Default::default()
    };
    assert_eq!(insert_client_calls(&db, &[row.clone()]).await.expect("1st"), 1);
    assert_eq!(insert_client_calls(&db, &[row]).await.expect("2nd"), 0);
    assert_eq!(count_calls(&db).await, 1);
}

/// 票面验收 ③：保留天数 setting 读写持久化（缺省 90，0 = 永久）。
#[tokio::test]
async fn client_usage_retention_setting_roundtrip() {
    let (db, _dir) = four_file_db().await;
    assert_eq!(load_retention_days(&db).await, 90, "缺省 90");
    save_retention_days(&db, 7).await.expect("save 7");
    assert_eq!(load_retention_days(&db).await, 7);
    save_retention_days(&db, 0).await.expect("save 0");
    assert_eq!(load_retention_days(&db).await, 0, "0 = 永久保留");
}

/// 票面验收 ④：清理删超期明细 + 无明细残留的会话行，保留进度表与未过期数据。
#[tokio::test]
async fn cleanup_client_usage_deletes_expired_and_empty_sessions_keeps_progress() {
    let (db, _dir) = four_file_db().await;
    let now_ms = crate::now();
    let day = 86_400_000i64;
    let old = now_ms - 200 * day;
    // 旧明细（sess-old，200 天前）+ 新明细（sess-live，现在）。
    let mk = |code: &str, sess: &str, pos: i64, t: i64| ClientCallRow {
        client_code: code.into(),
        session_id: sess.into(),
        call_time_ms: t,
        source_file_id: format!("{sess}-f"),
        file_pos: pos,
        ..Default::default()
    };
    insert_client_calls(
        &db,
        &[mk("claude_code", "sess-old", 1, old), mk("claude_code", "sess-live", 1, now_ms)],
    )
    .await
    .expect("insert");
    // 三个会话行：sess-old（明细将被清空 → 会话行随之删）、sess-live（明细保留 → 会话行保留）、
    // sess-empty（本来无明细 → 空壳删除）。
    for s in ["sess-old", "sess-live", "sess-empty"] {
        upsert_client_session(&db, "claude_code".into(), s.into(), String::new(), String::new(), old, old)
            .await
            .expect("upsert session");
    }
    // 无任何明细的会话壳行。
    upsert_client_session(&db, "claude_code".into(), "sess-empty".into(), String::new(), String::new(), old, old)
        .await
        .expect("upsert empty session");
    // 进度行必须保留。
    upsert_client_file_progress(&db, "claude_code".into(), "/p/a.jsonl".into(), 1, 1, 1, 0)
        .await
        .expect("upsert progress");

    cleanup_client_usage(&db, 90).await.expect("cleanup");

    assert_eq!(count_calls(&db).await, 1, "仅剩未过期明细");
    assert_eq!(count_sessions(&db, "claude_code".into()).await, 1, "仅剩有明细的会话");
    assert_eq!(count_progress(&db, "claude_code".into()).await, 1, "进度表不动");
}