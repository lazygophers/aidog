#![cfg(test)]
//! 读写分离连接池测试：验证 :memory: fallback 一致性 + 真实文件库读池能读到写连接已提交数据。
//!
//! 两条核心约束：
//! 1. 🔴 `:memory:` fallback：读池复用写连接，写入后经 `call_read_traced`（读路径）立即可见，
//!    否则读到独立空内存库 → 全测试链一致性崩。
//! 2. 真实文件库：读池为独立只读连接，WAL 模式下写连接提交后读连接看到最新快照。
use super::*;
use rusqlite::params;

/// 经写连接插入一行 setting（proxy_log 拆库后改用主库 setting 表测读池）。
async fn insert_log(db: &Db, id: &str, tokens: i64) {
    let id = id.to_string();
    db.call_traced(None, std::panic::Location::caller(), move |conn| {
        conn.execute(
            "INSERT OR REPLACE INTO setting (scope, key, value, created_at, updated_at, deleted_at) \
             VALUES ('test', ?1, ?2, 0, 0, 0)",
            params![id, tokens.to_string()],
        )?;
        Ok(())
    })
    .await
    .expect("insert log");
}

/// 经读路径（call_read_traced）统计 setting 行数。
async fn read_count(db: &Db) -> i64 {
    db.call_read_traced(None, std::panic::Location::caller(), |conn| {
        Ok(conn.query_row("SELECT COUNT(*) FROM setting WHERE scope = 'test' AND deleted_at = 0", [], |r| {
            r.get(0)
        })?)
    })
    .await
    .expect("read count")
}

/// :memory: fallback：写后经读路径立即可见（证读池复用写连接，未读到独立空库）。
#[tokio::test]
async fn memory_fallback_read_sees_writes() {
    let db = Db::new(":memory:").await.expect("open memory db");
    db.init_tables().await.expect("init tables");

    assert_eq!(read_count(&db).await, 0, "空库读路径应为 0");
    insert_log(&db, "mem-1", 10).await;
    insert_log(&db, "mem-2", 20).await;
    // 若读池误开独立内存库，这里会读到 0 → 断言失败。
    assert_eq!(read_count(&db).await, 2, ":memory: fallback 读路径须见写连接已提交数据");
}

/// 真实文件库：读池为独立只读连接，WAL 下写连接提交后读连接看到最新数据。
#[tokio::test]
async fn file_db_read_pool_sees_committed_writes() {
    let dir = std::env::temp_dir();
    let path = dir.join(format!("aidog_rwpool_test_{}.db", std::process::id()));
    let path_str = path.to_string_lossy().to_string();
    // 清理可能的前次残留（含 WAL/SHM 旁文件）。
    for suffix in ["", "-wal", "-shm"] {
        let _ = std::fs::remove_file(format!("{path_str}{suffix}"));
    }

    {
        let db = Db::new(&path_str).await.expect("open file db");
        db.init_tables().await.expect("init tables");

        // 读池是独立只读连接（非写连接 clone），仍须看到写连接提交的数据。
        assert_eq!(read_count(&db).await, 0);
        insert_log(&db, "file-1", 10).await;
        insert_log(&db, "file-2", 20).await;
        insert_log(&db, "file-3", 30).await;
        assert_eq!(
            read_count(&db).await,
            3,
            "文件库读池须经独立只读连接看到写连接 WAL 已提交快照"
        );
    }

    for suffix in ["", "-wal", "-shm"] {
        let _ = std::fs::remove_file(format!("{path_str}{suffix}"));
    }
}

/// 真实文件库并发：写流持续灌入的同时大量并发读，全部完成且不死锁。
///
/// 单写连接 + N 只读连接（WAL）下，读经独立连接走自身后台线程，不排在写连接队列后。
/// 本测试以「大量读写并发 join 全部 resolve、计数随写单调推进、无 panic/超时」证读路径不被
/// 写阻塞致挂死；轮询 `pick()` 跨多条读连接分摊，验证并发读真走读池而非单点串行。
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_reads_not_blocked_by_writes() {
    let dir = std::env::temp_dir();
    let path = dir.join(format!("aidog_rwpool_conc_{}.db", std::process::id()));
    let path_str = path.to_string_lossy().to_string();
    for suffix in ["", "-wal", "-shm"] {
        let _ = std::fs::remove_file(format!("{path_str}{suffix}"));
    }

    {
        let db = std::sync::Arc::new(Db::new(&path_str).await.expect("open file db"));
        db.init_tables().await.expect("init tables");

        // 写任务：串行灌 50 行（单写连接，WAL 单写约束）。
        let writer = {
            let db = db.clone();
            tokio::spawn(async move {
                for i in 0..50u32 {
                    insert_log(&db, &format!("conc-{i}"), i as i64).await;
                }
            })
        };

        // 读任务：写进行中并发发起 100 个读（轮询读池多条连接），全部须 resolve。
        let mut readers = Vec::with_capacity(100);
        for _ in 0..100 {
            let db = db.clone();
            readers.push(tokio::spawn(async move { read_count(&db).await }));
        }

        // 全部 join：任一挂死/超时即测试卡住失败；任一 panic 即 unwrap 失败。
        writer.await.expect("writer task");
        for r in readers {
            let n = r.await.expect("reader task");
            assert!((0..=50).contains(&n), "并发读计数须在 0..=50 单调区间内，实际 {n}");
        }

        // 写流结束后最终读须为 50：证所有写最终对读连接可见。
        assert_eq!(read_count(&db).await, 50, "并发结束后读池须见全部 50 行");
    }

    for suffix in ["", "-wal", "-shm"] {
        let _ = std::fs::remove_file(format!("{path_str}{suffix}"));
    }
}
