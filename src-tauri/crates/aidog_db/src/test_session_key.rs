//! 票 08 会话归组：proxy_log.session_key 列迁移（migration 20261107-01）+ 写读回。
//! three_file_db：proxy_log 走 log.db 独立文件路径，错路由当场 `no such table`
//! （db-pool-routing.md：内存库三 handle 同连接，路由错误测不出）。

#![cfg(test)]

use crate::test_support::three_file_db;

#[tokio::test]
async fn session_key_column_exists_after_migration() {
    let (db, _dir) = three_file_db().await;
    db.call_read_proxy_log_traced(None, std::panic::Location::caller(), move |conn| {
        let has = conn
            .prepare("PRAGMA table_info(proxy_log)")?
            .query_map([], |r| r.get::<_, String>(1))?
            .filter_map(Result::ok)
            .any(|c| c == "session_key");
        assert!(has, "proxy_log 缺 session_key 列（migration 20261107-01 未生效）");
        Ok(())
    })
    .await
    .expect("pragma proxy_log");
}

#[tokio::test]
async fn session_key_write_read_back() {
    let (db, _dir) = three_file_db().await;
    let id = "sk_roundtrip_0001".to_string();
    db.call_proxy_log_traced(None, std::panic::Location::caller(), move |conn| {
        conn.execute(
            "INSERT INTO proxy_log (id, group_key, session_key, created_at, status_code, done) \
             VALUES (?1, 'gk_test', '3f2b8c9e-1111-2222-3333-444455556666', 1000, 200, 1)",
            rusqlite::params![id],
        )?;
        Ok(())
    })
    .await
    .expect("insert with session_key");
    db.call_read_proxy_log_traced(None, std::panic::Location::caller(), move |conn| {
        let got: String = conn.query_row(
            "SELECT session_key FROM proxy_log WHERE id = 'sk_roundtrip_0001'",
            [],
            |r| r.get(0),
        )?;
        assert_eq!(got, "3f2b8c9e-1111-2222-3333-444455556666");
        Ok(())
    })
    .await
    .expect("read back session_key");
}

#[tokio::test]
async fn session_key_defaults_empty_for_existing_rows() {
    let (db, _dir) = three_file_db().await;
    let id = "sk_default_0001".to_string();
    db.call_proxy_log_traced(None, std::panic::Location::caller(), move |conn| {
        // 不带 session_key 的插入（模拟旧写入方 / 存量行）→ DEFAULT ''
        conn.execute(
            "INSERT INTO proxy_log (id, group_key, created_at) VALUES (?1, 'gk_test', 1000)",
            rusqlite::params![id],
        )?;
        Ok(())
    })
    .await
    .expect("insert without session_key");
    db.call_read_proxy_log_traced(None, std::panic::Location::caller(), move |conn| {
        let got: String = conn.query_row(
            "SELECT session_key FROM proxy_log WHERE id = 'sk_default_0001'",
            [],
            |r| r.get(0),
        )?;
        assert_eq!(got, "", "存量行 DEFAULT '' = 未归组（spec D4a）");
        Ok(())
    })
    .await
    .expect("read default session_key");
}
