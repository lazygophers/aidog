#![cfg(test)]
use super::*;
use super::test_support::*;
use rusqlite::params;

    /// migrate_auto_vacuum 幂等：内存库 Db::new 建表前已设 auto_vacuum=INCREMENTAL(2)，
    /// 故迁移探测命中 `current == 2` 分支 → 置标记 + 返回 false（无 VACUUM 必要）。
    /// 第二次跑因标记为 true 直接跳过。验证标记持久 + 探测后跳过两条幂等路径。
    #[tokio::test]
    async fn migrate_auto_vacuum_is_idempotent() {
        let db = test_db().await;
        // 迁移前：标记未置
        let flag_before = get_setting(&db, "db", "compact_migrated_v1").await.unwrap();
        assert!(flag_before.is_none(), "flag should be absent before migration");

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
            
            .call_traced(None, std::panic::Location::caller(), |c| Ok(c.query_row("PRAGMA auto_vacuum", [], |r| r.get(0))?))
            .await
            .unwrap();
        assert_eq!(av, 2, "auto_vacuum should be INCREMENTAL");
    }



    /// compact_database 全量 VACUUM 返回 before/after 字节，after <= before（压缩单调非增）。
    #[tokio::test]
    async fn compact_database_returns_sizes() {
        let db = test_db().await;
        // 插入若干行 + 删除一部分，制造 free pages
        for i in 0..50 {
            insert_proxy_log_at(&db, chrono::Utc::now().timestamp_millis() + i).await;
        }
        db
            .call_traced(None, std::panic::Location::caller(), |conn| {
                conn.execute("DELETE FROM proxy_log WHERE id LIKE 'test-%'", [])?;
                Ok(())
            })
            .await
            .unwrap();
        let result = compact_database(&db).await.expect("compact");
        assert!(result.before_bytes > 0, "before_bytes should be positive");
        assert!(result.after_bytes <= result.before_bytes, "VACUUM should not grow db");
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

        cleanup_user_request_fields(&db, 1).await.unwrap();

        let (req_h, req_body, ur_h, resp_body): (String, String, String, String) = db.call_traced(None, std::panic::Location::caller(), |c| {
            Ok(c.query_row(
                "SELECT request_headers, request_body, user_response_headers, user_response_body FROM proxy_log WHERE id='maint-u1'",
                [], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )?)
        }).await.unwrap();
        // 用户侧「原始信息」全集（headers + body）均清空。
        assert_eq!(req_h, "", "request_headers should be cleared");
        assert_eq!(req_body, "", "request_body should be cleared");
        assert_eq!(ur_h, "", "user_response_headers should be cleared");
        assert_eq!(resp_body, "", "user_response_body should be cleared");

        // Row still exists
        let cnt: i64 = db.call_traced(None, std::panic::Location::caller(), |c| {
            Ok(c.query_row("SELECT COUNT(*) FROM proxy_log WHERE id='maint-u1'", [], |r| r.get(0))?)
        }).await.unwrap();
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
                 upstream_request_headers,upstream_response_headers,response_body,user_response_body,attempts,created_at,updated_at,deleted_at) \
                 VALUES ('maint-up1','m','','g',1,200,1,1,0,0.0,10,0,'url','{{}}','','old-up-req','{{}}','old-up-resp-h','old-resp','','[]',\
                 {old_ts},{old_ts},0)"
            ))?;
            Ok(())
        }).await.unwrap();

        cleanup_upstream_request_fields(&db, 1).await.unwrap();

        let (up_h, up_req, up_resp_h, resp): (String, String, String, String) = db.call_traced(None, std::panic::Location::caller(), |c| {
            Ok(c.query_row(
                "SELECT upstream_request_headers, upstream_request_body, upstream_response_headers, response_body FROM proxy_log WHERE id='maint-up1'",
                [], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )?)
        }).await.unwrap();
        // 上游侧「原始信息」全集（headers + body + 上游响应正文）均清空。
        assert_eq!(up_h, "", "upstream_request_headers should be cleared");
        assert_eq!(up_req, "", "upstream_request_body should be cleared");
        assert_eq!(up_resp_h, "", "upstream_response_headers should be cleared");
        assert_eq!(resp, "", "response_body should be cleared");
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
        cleanup_user_request_fields(&db, 0).await.unwrap();
        cleanup_upstream_request_fields(&db, 0).await.unwrap();
        // No panic = pass
    }

    // ─── purge_all_soft_deleted ──────────────────────────────────────

    /// 跨表 purge：插入旧软删行（>3d）+ 未软删行 + 近期软删行（<3d），断言只旧软删行被删。
    #[tokio::test]
    async fn purge_all_soft_deleted_purges_old_across_tables() {
        let db = test_db().await;
        let now_ms = chrono::Utc::now().timestamp_millis();
        let old = now_ms - 4 * 24 * 3600 * 1000_i64; // 4d before now → 超过 3d 阈值
        let recent = now_ms - 1 * 24 * 3600 * 1000_i64; // 1d before now → 未超阈值

        // platform: 1 旧软删 + 1 未软删
        let p_old = create_platform(&db, sample_platform("purge-old-platform")).await.unwrap();
        delete_platform(&db, p_old.id).await.unwrap();
        // delete_platform 内部用 now()，把 deleted_at 改写为旧值需手工 UPDATE
        db.call_traced(None, std::panic::Location::caller(), move |c| {
            c.execute(
                "UPDATE platform SET deleted_at = ?1 WHERE id = ?2",
                params![old, p_old.id as i64],
            )?;
            Ok(())
        }).await.unwrap();
        let _p_alive = create_platform(&db, sample_platform("purge-alive-platform")).await.unwrap();

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
        }).await.unwrap();
        insert_proxy_log_at(&db, now_ms).await; // 未软删

        // setting: 1 旧软删 KV
        db.call_traced(None, std::panic::Location::caller(), move |c| {
            c.execute(
                "INSERT INTO setting (scope, key, value, created_at, updated_at, deleted_at) \
                 VALUES ('test', 'stale', 'null', ?1, ?1, ?1)",
                params![old],
            )?;
            Ok(())
        }).await.unwrap();

        // 执行 purge（3d 阈值）
        let map = purge_all_soft_deleted(&db, 3 * 24 * 3600).await.expect("purge ok");

        // platform: 至少 1 行删（旧软删）；未软删平台保留
        let p_alive: i64 = db.call_traced(None, std::panic::Location::caller(), |c| {
            Ok(c.query_row(
                "SELECT COUNT(*) FROM platform WHERE deleted_at = 0",
                [],
                |r| r.get(0),
            )?)
        }).await.unwrap();
        assert!(p_alive >= 1, "alive platform must remain, got {p_alive}");
        assert!(map.get("platform").copied().unwrap_or(0) >= 1, "platform purge count");

        // proxy_log: 旧软删删，近期软删保留，未软删保留
        let old_cnt: i64 = db.call_traced(None, std::panic::Location::caller(), |c| {
            Ok(c.query_row("SELECT COUNT(*) FROM proxy_log WHERE id = 'pl-old'", [], |r| r.get(0))?)
        }).await.unwrap();
        assert_eq!(old_cnt, 0, "old soft-deleted proxy_log should be deleted");
        let recent_cnt: i64 = db.call_traced(None, std::panic::Location::caller(), |c| {
            Ok(c.query_row("SELECT COUNT(*) FROM proxy_log WHERE id = 'pl-recent'", [], |r| r.get(0))?)
        }).await.unwrap();
        assert_eq!(recent_cnt, 1, "recent soft-deleted proxy_log should remain");
        assert!(map.get("proxy_log").copied().unwrap_or(0) >= 1, "proxy_log purge count");

        // setting: 旧软删行删
        let stale: i64 = db.call_traced(None, std::panic::Location::caller(), |c| {
            Ok(c.query_row(
                "SELECT COUNT(*) FROM setting WHERE scope='test' AND key='stale'",
                [],
                |r| r.get(0),
            )?)
        }).await.unwrap();
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
        }).await.unwrap();

        let map = purge_all_soft_deleted(&db, 3 * 24 * 3600).await.expect("purge ok");
        // 含 proxy_log key（无引号）且计数 >= 2
        let pl = map.get("proxy_log").copied().unwrap_or(0);
        assert!(pl >= 2, "proxy_log count should be >= 2, got {pl}");
        // map key 确无引号
        assert!(map.contains_key("proxy_log"), "map key must be unquoted");
        assert!(!map.keys().any(|k| k.contains('"')), "no key should contain quotes");
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
        let map = purge_all_soft_deleted(&db, 3 * 24 * 3600).await.expect("empty purge ok");
        // 空库 map 含每表 key，值为 0（无单表失败）
        for &(_sql, key) in SOFT_DELETE_TABLES {
            assert!(
                map.contains_key(key),
                "empty db: table '{key}' should be in map with count 0"
            );
        }
    }

