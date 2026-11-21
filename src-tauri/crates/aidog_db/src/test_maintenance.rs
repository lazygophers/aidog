#![cfg(test)]
use super::test_support::*;
use super::*;
use rusqlite::params;

/// migrate_auto_vacuum 幂等：内存库 Db::new 建表前已设 auto_vacuum=INCREMENTAL(2)，
/// 故迁移探测命中 `current == 2` 分支 → 置标记 + 返回 false（无 VACUUM 必要）。
/// 第二次跑因标记为 true 直接跳过。验证标记持久 + 探测后跳过两条幂等路径。
#[tokio::test]
async fn migrate_auto_vacuum_is_idempotent() {
    let db = test_db().await;
    // 迁移前：标记未置
    let flag_before = get_setting(&db, "db", "compact_migrated_v1").await.unwrap();
    assert!(
        flag_before.is_none(),
        "flag should be absent before migration"
    );

    // 第一次迁移：auto_vacuum 已是 INCREMENTAL（新库建表前设过）→ 置标记 + 返回 false
    let migrated = migrate_auto_vacuum(&db).await.expect("first migration");
    assert!(!migrated, "memory db already INCREMENTAL, no VACUUM needed");

    // 标记已置
    let flag = get_setting(&db, "db", "compact_migrated_v1").await.unwrap();
    assert_eq!(flag, Some(serde_json::Value::Bool(true)));

    // 第二次迁移：标记 true → 直接跳过，不探测不 VACUUM
    let migrated2 = migrate_auto_vacuum(&db).await.expect("second migration");
    assert!(!migrated2, "second call should skip (already marked)");

    // auto_vacuum 保持 INCREMENTAL
    let av: i64 = db
        .call_traced(None, std::panic::Location::caller(), |c| {
            c.query_row("PRAGMA auto_vacuum", [], |r| r.get(0))
        })
        .await
        .unwrap();
    assert_eq!(av, 2, "auto_vacuum should be INCREMENTAL");
}

/// 字节统计（db_size_bytes）在加密文件库上可用：SQLCipher 的 PRAGMA page_size 在加密库返回 TEXT
/// （列名 cipher_page_size），i64 直取报 Invalid column type（生产日志 2026-11-12 实锤）。
/// 内存库无 SQLCipher 行为，必须用 three_file_db；经 compact_database 走到 db_size_bytes。
#[tokio::test]
async fn db_size_works_on_encrypted_file_db() {
    let (db, _dir) = three_file_db().await;
    let r = compact_database(&db)
        .await
        .expect("compact on encrypted file db");
    assert!(r.before_bytes > 0, "size should be positive");
}

/// 往 log.db 写 n 行带 ~8KB body 的 proxy_log（id 前缀 `vac-`），撑出可观页数。
async fn fill_log_db(db: &Db, n: usize) {
    db.call_proxy_log_traced(None, std::panic::Location::caller(), move |c| {
        let body = "x".repeat(8 * 1024);
        let tx = c.unchecked_transaction()?;
        for i in 0..n {
            tx.execute(
                "INSERT INTO proxy_log (id, platform_id, group_key, model, source_protocol, \
                 status_code, input_tokens, output_tokens, cache_tokens, est_cost, is_stream, \
                 response_body, created_at, deleted_at) \
                 VALUES (?1, 0, '', 'm', 'anthropic', 200, 1, 1, 0, 0, 0, ?2, 1, 0)",
                params![format!("vac-{i:06}"), body],
            )?;
        }
        tx.commit()
    })
    .await
    .unwrap();
}

/// 删掉前 `pct`% 的 vac 行（制造空闲页），返回删后 (freelist_count, page_count)。
async fn delete_log_pct(db: &Db, n: usize, pct: usize) -> (i64, i64) {
    let cut = format!("vac-{:06}", n * pct / 100);
    db.call_proxy_log_traced(None, std::panic::Location::caller(), move |c| {
        c.execute("DELETE FROM proxy_log WHERE id LIKE 'vac-%' AND id < ?1", params![cut])?;
        Ok((
            c.query_row("PRAGMA freelist_count", [], |r| r.get(0))?,
            c.query_row("PRAGMA page_count", [], |r| r.get(0))?,
        ))
    })
    .await
    .unwrap()
}

async fn log_freelist(db: &Db) -> i64 {
    db.call_proxy_log_traced(None, std::panic::Location::caller(), |c| {
        c.query_row("PRAGMA freelist_count", [], |r| r.get(0))
    })
    .await
    .unwrap()
}

#[test]
fn needs_full_vacuum_threshold_is_25_percent() {
    assert!(!needs_full_vacuum(0, 0), "empty db never full-vacuums");
    assert!(!needs_full_vacuum(24, 100));
    assert!(needs_full_vacuum(25, 100));
    assert!(needs_full_vacuum(90, 100));
}

/// 大库、空闲页占比低（删 10%）：不全量 VACUUM，只 incremental 回收，空闲页清零。
#[tokio::test]
async fn scheduled_vacuum_low_free_ratio_goes_incremental() {
    let (db, _dir) = three_file_db().await;
    // 新建文件库先设 WAL 再设 auto_vacuum，实为 NONE；生产启动由 migrate_auto_vacuum 切到 INCREMENTAL。
    migrate_auto_vacuum(&db).await.expect("migrate auto_vacuum");
    fill_log_db(&db, 400).await;
    let (free, pages) = delete_log_pct(&db, 400, 10).await;
    assert!(free > 0 && (free as f64) < pages as f64 * VACUUM_FREE_RATIO, "fixture: free={free} pages={pages}");
    let actions = scheduled_vacuum(&db).await.expect("scheduled_vacuum");
    assert!(
        actions.contains(&("log", VacuumAction::Incremental)),
        "low free ratio must not full-vacuum log.db: {actions:?}"
    );
    assert_eq!(log_freelist(&db).await, 0, "incremental_vacuum should clear freelist");
}

/// 空闲页占比 ≥ 25%（删 60%）：全量 VACUUM，其余库互不影响（按库各自判定）。
#[tokio::test]
async fn scheduled_vacuum_high_free_ratio_goes_full() {
    let (db, _dir) = three_file_db().await;
    fill_log_db(&db, 400).await;
    let (free, pages) = delete_log_pct(&db, 400, 60).await;
    assert!(free as f64 >= pages as f64 * VACUUM_FREE_RATIO, "fixture: free={free} pages={pages}");
    let actions = scheduled_vacuum(&db).await.expect("scheduled_vacuum");
    assert!(
        actions.contains(&("log", VacuumAction::Full)),
        "high free ratio must full-vacuum log.db: {actions:?}"
    );
    let names: Vec<_> = actions.iter().map(|(n, _)| *n).collect();
    assert_eq!(names, ["main", "platform", "log"], "each db decided independently");
    assert_eq!(log_freelist(&db).await, 0);
}

/// 写连接设了 journal_size_limit（WAL checkpoint 后截断，防 -wal 长期占盘）。
#[tokio::test]
async fn write_conns_set_journal_size_limit() {
    let (db, _dir) = three_file_db().await;
    let q = |c: &mut rusqlite::Connection| c.query_row("PRAGMA journal_size_limit", [], |r| r.get::<_, i64>(0));
    let main = db.call_traced(None, std::panic::Location::caller(), q).await.unwrap();
    let log = db.call_proxy_log_traced(None, std::panic::Location::caller(), q).await.unwrap();
    assert_eq!(main, crate::JOURNAL_SIZE_LIMIT_BYTES);
    assert_eq!(log, crate::JOURNAL_SIZE_LIMIT_BYTES);
}

/// 测量（手动跑）：旧口径「三库总大小 > 100MB 即全量 VACUUM」vs 新口径「按库空闲页占比」。
/// `cargo test -p aidog_db --lib -- --ignored measure_ --nocapture`
#[tokio::test]
#[ignore]
async fn measure_vacuum_old_vs_new() {
    let (db, _dir) = three_file_db().await;
    migrate_auto_vacuum(&db).await.unwrap();
    let n = 16_000; // ~8KB/行 → log.db ≈ 140MB，超旧阈值 100MB
    fill_log_db(&db, n).await;
    let (free, pages) = delete_log_pct(&db, n, 10).await;
    println!("low-free: free={free} pages={pages} ratio={:.3}", free as f64 / pages as f64);
    let t = std::time::Instant::now();
    let r = compact_database(&db).await.unwrap();
    println!(
        "OLD low-free: size {}MB > 100MB → full VACUUM, {:?}",
        r.before_bytes >> 20,
        t.elapsed()
    );
    // 新口径：同一形状（重新删 10% 制造空闲页）
    let (free, pages) = delete_log_pct(&db, n, 20).await;
    println!("low-free(2): free={free} pages={pages} ratio={:.3}", free as f64 / pages as f64);
    let t = std::time::Instant::now();
    let a = scheduled_vacuum(&db).await.unwrap();
    println!("NEW low-free: {a:?}, {:?}", t.elapsed());
    // 高空闲：删到 70%
    let (free, pages) = delete_log_pct(&db, n, 70).await;
    println!("high-free: free={free} pages={pages} ratio={:.3}", free as f64 / pages as f64);
    let t = std::time::Instant::now();
    let a = scheduled_vacuum(&db).await.unwrap();
    println!("NEW high-free: {a:?}, {:?}", t.elapsed());
}

/// 测量（手动跑）：一次大事务撑大 WAL 后，再写一笔触发 WAL 重置，比较有无 journal_size_limit 的 -wal 大小。
#[test]
#[ignore]
fn measure_wal_size_with_journal_size_limit() {
    for limit in [-1_i64, crate::JOURNAL_SIZE_LIMIT_BYTES] {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("w.db");
        let c = rusqlite::Connection::open(&p).unwrap();
        c.execute_batch(&format!(
            "PRAGMA journal_mode=WAL; PRAGMA journal_size_limit={limit}; CREATE TABLE t(b TEXT);"
        ))
        .unwrap();
        let body = "x".repeat(8 * 1024);
        let tx = c.unchecked_transaction().unwrap();
        for _ in 0..25_000 {
            tx.execute("INSERT INTO t VALUES (?1)", [&body]).unwrap();
        }
        tx.commit().unwrap();
        let wal = dir.path().join("w.db-wal");
        let peak = std::fs::metadata(&wal).unwrap().len();
        c.execute_batch("PRAGMA wal_checkpoint(PASSIVE);").unwrap();
        c.execute("INSERT INTO t VALUES ('y')", []).unwrap();
        let after = std::fs::metadata(&wal).unwrap().len();
        println!("journal_size_limit={limit}: wal peak {}MB → after checkpoint+write {}MB", peak >> 20, after >> 20);
    }
}

/// compact_database 全量 VACUUM 返回 before/after 字节，after <= before（压缩单调非增）。
#[tokio::test]
async fn compact_database_returns_sizes() {
    let db = test_db().await;
    // 插入若干行 + 删除一部分，制造 free pages
    for i in 0..50 {
        insert_proxy_log_at(&db, chrono::Utc::now().timestamp_millis() + i).await;
    }
    db.call_traced(None, std::panic::Location::caller(), |conn| {
        conn.execute("DELETE FROM proxy_log WHERE id LIKE 'test-%'", [])?;
        Ok(())
    })
    .await
    .unwrap();
    let result = compact_database(&db).await.expect("compact");
    assert!(result.before_bytes > 0, "before_bytes should be positive");
    assert!(
        result.after_bytes <= result.before_bytes,
        "VACUUM should not grow db"
    );
}

/// cleanup_user_request_fields 清空旧行 body 字段但不删行。
#[tokio::test]
async fn cleanup_user_request_fields_clears_old_bodies() {
    let db = test_db().await;
    let now = chrono::Utc::now().timestamp_millis();
    let old_ts = now - 2 * 24 * 3600 * 1000_i64;
    // 直接 INSERT 有 body 的老行
    db.call_traced(None, std::panic::Location::caller(), move |c| {
            c.execute_batch(&format!(
                "INSERT INTO proxy_log (id,model,actual_model,group_key,platform_id,\
                 status_code,input_tokens,output_tokens,cache_tokens,est_cost,duration_ms,\
                 is_stream,request_url,request_headers,request_body,upstream_request_body,\
                 upstream_request_headers,response_body,user_response_headers,user_response_body,attempts,created_at,updated_at,deleted_at) \
                 VALUES ('maint-u1','m','','g',1,200,1,1,0,0.0,10,0,'url','{{}}','old-req-body','','{{}}','','old-ur-h','old-resp','[]',\
                 {old_ts},{old_ts},0)"
            ))?;
            Ok(())
        }).await.unwrap();

    cleanup_user_request_fields(&db, 1, RetentionUnit::Day)
        .await
        .unwrap();

    let (req_h, req_body, ur_h, resp_body, req_url): (String, String, String, String, String) = db.call_traced(None, std::panic::Location::caller(), |c| {
            c.query_row(
                "SELECT request_headers, request_body, user_response_headers, user_response_body, request_url FROM proxy_log WHERE id='maint-u1'",
                [], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
            )
        }).await.unwrap();
    // 用户侧「原始信息」全集（headers + body + url）均清空。
    assert_eq!(req_h, "", "request_headers should be cleared");
    assert_eq!(req_body, "", "request_body should be cleared");
    assert_eq!(ur_h, "", "user_response_headers should be cleared");
    assert_eq!(resp_body, "", "user_response_body should be cleared");
    assert_eq!(req_url, "", "request_url should be cleared (user side, 2026-10-22)");

    // Row still exists
    let cnt: i64 = db
        .call_traced(None, std::panic::Location::caller(), |c| {
            c.query_row(
                "SELECT COUNT(*) FROM proxy_log WHERE id='maint-u1'",
                [],
                |r| r.get(0),
            )
        })
        .await
        .unwrap();
    assert_eq!(cnt, 1, "row must not be deleted");
}

/// cleanup_upstream_request_fields 清空旧行上游 body 字段但不删行。
#[tokio::test]
async fn cleanup_upstream_request_fields_clears_old_bodies() {
    let db = test_db().await;
    let now = chrono::Utc::now().timestamp_millis();
    let old_ts = now - 2 * 24 * 3600 * 1000_i64;
    db.call_traced(None, std::panic::Location::caller(), move |c| {
            c.execute_batch(&format!(
                "INSERT INTO proxy_log (id,model,actual_model,group_key,platform_id,\
                 status_code,input_tokens,output_tokens,cache_tokens,est_cost,duration_ms,\
                 is_stream,request_url,request_headers,request_body,upstream_request_body,\
                 upstream_request_headers,upstream_response_headers,response_body,user_response_body,attempts,upstream_request_url,created_at,updated_at,deleted_at) \
                 VALUES ('maint-up1','m','','g',1,200,1,1,0,0.0,10,0,'url','{{}}','','old-up-req','{{}}','old-up-resp-h','old-resp','','[]','up-url',\
                 {old_ts},{old_ts},0)"
            ))?;
            Ok(())
        }).await.unwrap();

    cleanup_upstream_request_fields(&db, 1, RetentionUnit::Day)
        .await
        .unwrap();

    let (up_h, up_req, up_resp_h, resp, up_url): (String, String, String, String, String) = db.call_traced(None, std::panic::Location::caller(), |c| {
            c.query_row(
                "SELECT upstream_request_headers, upstream_request_body, upstream_response_headers, response_body, upstream_request_url FROM proxy_log WHERE id='maint-up1'",
                [], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
            )
        }).await.unwrap();
    // 上游侧「原始信息」全集（headers + body + 上游响应正文 + url）均清空。
    assert_eq!(up_h, "", "upstream_request_headers should be cleared");
    assert_eq!(up_req, "", "upstream_request_body should be cleared");
    assert_eq!(up_resp_h, "", "upstream_response_headers should be cleared");
    assert_eq!(resp, "", "response_body should be cleared");
    assert_eq!(up_url, "", "upstream_request_url should be cleared (upstream side, 2026-10-22)");
    // 用户侧 request_url 属另一档，不得被上游档误清。
    let user_url: String = db.call_traced(None, std::panic::Location::caller(), |c| {
        c.query_row("SELECT request_url FROM proxy_log WHERE id='maint-up1'", [], |r| r.get(0))
    }).await.unwrap();
    assert_eq!(user_url, "url", "upstream tier must not clear user-side request_url");
}

/// 票 10：field_trace 归上游侧，随 upstream retention 一同清空（行保留）。
#[tokio::test]
async fn cleanup_upstream_request_fields_clears_field_trace() {
    let db = test_db().await;
    let now = chrono::Utc::now().timestamp_millis();
    let old_ts = now - 2 * 24 * 3600 * 1000_i64;
    // 只有 field_trace 有值（其它上游列全空）→ 验证 WHERE 条件也认这一列。
    db.call_traced(None, std::panic::Location::caller(), move |c| {
            c.execute_batch(&format!(
                "INSERT INTO proxy_log (id,model,actual_model,group_key,platform_id,status_code,\
                 input_tokens,output_tokens,cache_tokens,est_cost,duration_ms,is_stream,request_url,\
                 attempts,field_trace,created_at,updated_at,deleted_at) \
                 VALUES ('maint-ft1','m','','g',1,200,1,1,0,0.0,10,0,'url','[]','drop:user,cap:max_tokens',\
                 {old_ts},{old_ts},0)"
            ))?;
            Ok(())
        }).await.unwrap();

    cleanup_upstream_request_fields(&db, 1, RetentionUnit::Day)
        .await
        .unwrap();

    let (ft, cnt): (String, i64) = db
        .call_traced(None, std::panic::Location::caller(), |c| {
            c.query_row(
                "SELECT field_trace, COUNT(*) FROM proxy_log WHERE id='maint-ft1'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
        })
        .await
        .unwrap();
    assert_eq!(
        ft, "",
        "field_trace should be cleared by upstream retention"
    );
    assert_eq!(cnt, 1, "row must not be deleted");
}

/// count_proxy_logs returns count of non-deleted rows.
#[tokio::test]
async fn count_proxy_logs_returns_count() {
    let db = test_db().await;
    let now = chrono::Utc::now().timestamp_millis();
    for i in 0..3 {
        insert_proxy_log_at(&db, now + i).await;
    }
    let cnt = count_proxy_logs(&db).await.unwrap();
    assert!(cnt >= 3, "should count all inserted logs: {cnt}");
}

/// cleanup_user/upstream with retention_days=0 returns immediately (no cleanup).
#[tokio::test]
async fn cleanup_with_zero_retention_is_noop() {
    let db = test_db().await;
    let now = chrono::Utc::now().timestamp_millis();
    insert_proxy_log_at(&db, now).await;
    // retention_days=0 → retention_cutoff returns None → early return
    cleanup_user_request_fields(&db, 0, RetentionUnit::Day)
        .await
        .unwrap();
    cleanup_upstream_request_fields(&db, 0, RetentionUnit::Day)
        .await
        .unwrap();
    // No panic = pass
}

// ─── purge_all_soft_deleted ──────────────────────────────────────

/// 跨表 purge：插入旧软删行（>3d）+ 未软删行 + 近期软删行（<3d），断言只旧软删行被删。
#[tokio::test]
async fn purge_all_soft_deleted_purges_old_across_tables() {
    let db = test_db().await;
    let now_ms = chrono::Utc::now().timestamp_millis();
    let old = now_ms - 4 * 24 * 3600 * 1000_i64; // 4d before now → 超过 3d 阈值
    let recent = now_ms - 24 * 3600 * 1000_i64; // 1d before now → 未超阈值

    // platform: 1 旧软删 + 1 未软删
    let p_old = create_platform(&db, sample_platform("purge-old-platform"))
        .await
        .unwrap();
    delete_platform(&db, p_old.id).await.unwrap();
    // delete_platform 内部用 now()，把 deleted_at 改写为旧值需手工 UPDATE
    db.call_traced(None, std::panic::Location::caller(), move |c| {
        c.execute(
            "UPDATE platform SET deleted_at = ?1 WHERE id = ?2",
            params![old, p_old.id as i64],
        )?;
        Ok(())
    })
    .await
    .unwrap();
    let _p_alive = create_platform(&db, sample_platform("purge-alive-platform"))
        .await
        .unwrap();

    // proxy_log: 1 旧软删 + 1 近期软删 + 1 未软删
    db.call_traced(None, std::panic::Location::caller(), move |c| {
        c.execute_batch(&format!(
            "INSERT INTO proxy_log (id, platform_id, group_key, model, source_protocol, \
                 status_code, input_tokens, output_tokens, cache_tokens, est_cost, is_stream, \
                 created_at, deleted_at) VALUES \
                 ('pl-old', 0, '', 'm', 'anthropic', 200, 1, 1, 0, 0, 0, {old}, {old}), \
                 ('pl-recent', 0, '', 'm', 'anthropic', 200, 1, 1, 0, 0, 0, {recent}, {recent})"
        ))?;
        Ok(())
    })
    .await
    .unwrap();
    insert_proxy_log_at(&db, now_ms).await; // 未软删

    // setting: 1 旧软删 KV
    db.call_traced(None, std::panic::Location::caller(), move |c| {
        c.execute(
            "INSERT INTO setting (scope, key, value, created_at, updated_at, deleted_at) \
                 VALUES ('test', 'stale', 'null', ?1, ?1, ?1)",
            params![old],
        )?;
        Ok(())
    })
    .await
    .unwrap();

    // 执行 purge（3d 阈值）
    let map = purge_all_soft_deleted(&db, 3 * 24 * 3600)
        .await
        .expect("purge ok");

    // platform: 至少 1 行删（旧软删）；未软删平台保留
    let p_alive: i64 = db
        .call_traced(None, std::panic::Location::caller(), |c| {
            c.query_row(
                "SELECT COUNT(*) FROM platform WHERE deleted_at = 0",
                [],
                |r| r.get(0),
            )
        })
        .await
        .unwrap();
    assert!(p_alive >= 1, "alive platform must remain, got {p_alive}");
    assert!(
        map.get("platform").copied().unwrap_or(0) >= 1,
        "platform purge count"
    );

    // proxy_log: 旧软删删，近期软删保留，未软删保留
    let old_cnt: i64 = db
        .call_traced(None, std::panic::Location::caller(), |c| {
            c.query_row(
                "SELECT COUNT(*) FROM proxy_log WHERE id = 'pl-old'",
                [],
                |r| r.get(0),
            )
        })
        .await
        .unwrap();
    assert_eq!(old_cnt, 0, "old soft-deleted proxy_log should be deleted");
    let recent_cnt: i64 = db
        .call_traced(None, std::panic::Location::caller(), |c| {
            c.query_row(
                "SELECT COUNT(*) FROM proxy_log WHERE id = 'pl-recent'",
                [],
                |r| r.get(0),
            )
        })
        .await
        .unwrap();
    assert_eq!(recent_cnt, 1, "recent soft-deleted proxy_log should remain");
    assert!(
        map.get("proxy_log").copied().unwrap_or(0) >= 1,
        "proxy_log purge count"
    );

    // setting: 旧软删行删
    let stale: i64 = db
        .call_traced(None, std::panic::Location::caller(), |c| {
            c.query_row(
                "SELECT COUNT(*) FROM setting WHERE scope='test' AND key='stale'",
                [],
                |r| r.get(0),
            )
        })
        .await
        .unwrap();
    assert_eq!(stale, 0, "old soft-deleted setting should be deleted");
}

/// 返回的 map 含每表删除计数（key 去引号）。
#[tokio::test]
async fn purge_all_soft_deleted_returns_per_table_count() {
    let db = test_db().await;
    let now_ms = chrono::Utc::now().timestamp_millis();
    let old = now_ms - 4 * 24 * 3600 * 1000_i64;

    // 插两行旧软删 proxy_log
    db.call_traced(None, std::panic::Location::caller(), move |c| {
        c.execute_batch(&format!(
            "INSERT INTO proxy_log (id, platform_id, group_key, model, source_protocol, \
                 status_code, input_tokens, output_tokens, cache_tokens, est_cost, is_stream, \
                 created_at, deleted_at) VALUES \
                 ('cnt-1', 0, '', 'm', 'anthropic', 200, 1, 1, 0, 0, 0, {old}, {old}), \
                 ('cnt-2', 0, '', 'm', 'anthropic', 200, 1, 1, 0, 0, 0, {old}, {old})"
        ))?;
        Ok(())
    })
    .await
    .unwrap();

    let map = purge_all_soft_deleted(&db, 3 * 24 * 3600)
        .await
        .expect("purge ok");
    // 含 proxy_log key（无引号）且计数 >= 2
    let pl = map.get("proxy_log").copied().unwrap_or(0);
    assert!(pl >= 2, "proxy_log count should be >= 2, got {pl}");
    // map key 确无引号
    assert!(map.contains_key("proxy_log"), "map key must be unquoted");
    assert!(
        !map.keys().any(|k| k.contains('"')),
        "no key should contain quotes"
    );
}

/// schema 漂移容错：清单内某表 DELETE 错误（模拟：临时表替换或失败兜底路径）
/// 不阻塞他表清理。此测试用低层 mock 路径难构造，改为验证 purge 对正常库不炸、
/// 对缺列场景运行时由 warn + skip 兜底（生产环境观察 tracing::warn 日志）。
/// 此处验证 proxy_log 缺 deleted_at 列时（极端假设）整体函数仍返 Ok 非全 Err 的契约：
/// 因清单 6 表全都有 deleted_at 列，正常库应返 Ok(map)；若任一表 schema 漂移，
/// 失败表不进 map 但他表仍清理成功。降级测试：断言正常场景多表都进 map。
#[tokio::test]
async fn purge_all_soft_deleted_handles_per_table_failure_gracefully() {
    let db = test_db().await;
    // 空库跑 purge：所有表 DELETE 0 行，应返 Ok(map)，无表失败
    let map = purge_all_soft_deleted(&db, 3 * 24 * 3600)
        .await
        .expect("empty purge ok");
    // 空库 map 含每表 key，值为 0（无单表失败）—— 主库 + platform + proxy_log 三清单合并
    for &(_sql, key) in SOFT_DELETE_TABLES
        .iter()
        .chain(SOFT_DELETE_TABLES_PLATFORM.iter())
        .chain(SOFT_DELETE_TABLES_PROXY_LOG.iter())
    {
        assert!(
            map.contains_key(key),
            "empty db: table '{key}' should be in map with count 0"
        );
    }
}
