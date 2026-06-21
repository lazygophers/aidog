#![cfg(test)]
use super::*;
use super::test_support::*;
use rusqlite::params;

    /// 字段完整性红线：渐进式「首节点 INSERT + 后续节点部分列 UPDATE」累积写入后，
    /// proxy_log 整行所有列必须与旧「全列 INSERT OR REPLACE 终态」等价。
    /// 含 strip(脱敏)、token、est_cost、attempts、is_stream、blocked_* 等全字段覆盖。
    #[tokio::test]
    async fn progressive_columns_equals_full_replace() {
        let db = test_db().await;
        let now_ms = now();

        // 构造一条完整请求的「终态」ProxyLog（含全字段非默认值，验证无字段丢失）。
        let mut final_log = sample_log("prog", "grp", now_ms);
        final_log.actual_model = "deepseek-chat".into();
        final_log.request_headers = "{\"x\":\"1\"}".into();
        final_log.request_body = "{\"q\":\"hi\"}".into();
        final_log.upstream_request_headers = "{\"auth\":\"r\"}".into();
        final_log.upstream_request_body = "{\"m\":\"x\"}".into();
        final_log.response_body = "{\"ok\":true}".into();
        final_log.request_url = "http://localhost/v1/messages".into();
        final_log.upstream_request_url = "https://up/chat/completions".into();
        final_log.upstream_response_headers = "{\"ct\":\"json\"}".into();
        final_log.upstream_status_code = 200;
        final_log.user_response_headers = "{\"ct\":\"json\"}".into();
        final_log.user_response_body = "{\"ok\":true}".into();
        final_log.status_code = 200;
        final_log.duration_ms = 321;
        final_log.input_tokens = 111;
        final_log.output_tokens = 222;
        final_log.cache_tokens = 33;
        final_log.est_cost = 0.0042;
        final_log.is_stream = true;
        final_log.attempts = vec![crate::gateway::models::ProxyAttempt {
            platform_id: 1, platform_name: "p1".into(), status_code: 200,
            error: String::new(), duration_ms: 99, ts: now_ms,
        }];
        final_log.retry_count = 0;
        final_log.blocked_by = String::new();
        final_log.blocked_reason = String::new();

        // 旧路径：直接全列 REPLACE 终态。
        let mut old_log = final_log.clone();
        old_log.id = "old".into();
        upsert_proxy_log(&db, old_log).await.unwrap();
        let old_row = get_proxy_log(&db, "old").await.unwrap().unwrap();

        // 新路径：模拟节点序列（每节点带「本阶段新增字段」，其余沿用上次）。
        // 节点1：请求建立（id/group/model/protocols/url，无 token/响应）。
        let mut n1 = sample_log("prog", "grp", now_ms);
        n1.model = final_log.model.clone();
        n1.source_protocol = final_log.source_protocol.clone();
        n1.target_protocol = final_log.target_protocol.clone();
        n1.actual_model = final_log.actual_model.clone();
        n1.request_headers = final_log.request_headers.clone();
        n1.request_body = final_log.request_body.clone();
        n1.request_url = final_log.request_url.clone();
        n1.status_code = 0;
        n1.duration_ms = 0;
        n1.input_tokens = 0;
        n1.output_tokens = 0;
        n1.cache_tokens = 0;
        n1.upstream_status_code = 0;
        n1.response_body = String::new();
        n1.user_response_body = String::new();
        n1.user_response_headers = String::new();
        n1.is_stream = false;
        let c1 = ProxyLogColumns::from_log(&n1, false, false);
        insert_proxy_log_columns(&db, c1.clone()).await.unwrap();

        // 节点2：上游请求/响应头（upstream_* 字段）。
        let mut n2 = n1.clone();
        n2.upstream_request_headers = final_log.upstream_request_headers.clone();
        n2.upstream_request_body = final_log.upstream_request_body.clone();
        n2.upstream_request_url = final_log.upstream_request_url.clone();
        n2.upstream_response_headers = final_log.upstream_response_headers.clone();
        n2.upstream_status_code = final_log.upstream_status_code;
        n2.is_stream = final_log.is_stream;
        let c2 = ProxyLogColumns::from_log(&n2, false, false);
        update_proxy_log_columns(&db, c2.clone(), &c1).await.unwrap();

        // 节点3：终态（token/est_cost/状态/body/attempts）。
        let c3 = ProxyLogColumns::from_log(&final_log, false, false);
        update_proxy_log_columns(&db, c3, &c2).await.unwrap();

        let new_row = get_proxy_log(&db, "prog").await.unwrap().unwrap();

        // 全列等价比对：序列化后比 JSON（覆盖所有字段，id 除外）。
        let mut a = serde_json::to_value(&old_row).unwrap();
        let mut b = serde_json::to_value(&new_row).unwrap();
        a.as_object_mut().unwrap().remove("id");
        b.as_object_mut().unwrap().remove("id");
        assert_eq!(a, b, "渐进式累积写入整行字段须与全列 REPLACE 终态完全等价");
    }



    /// strip(脱敏) 等价性：log_user_request/log_upstream_request 关时，仅 `*_body`
    /// （prompt / 响应正文）被清空；`*_headers`（元数据，auth 已脱敏）始终保留。
    #[tokio::test]
    async fn progressive_columns_strip_equivalence() {
        let db = test_db().await;
        let now_ms = now();
        let mut log = sample_log("strip", "grp", now_ms);
        log.request_headers = "secret-h".into();
        log.request_body = "secret-b".into();
        log.user_response_headers = "ur-h".into();
        log.user_response_body = "ur-b".into();
        log.upstream_request_headers = "up-rh".into();
        log.upstream_request_body = "up-rb".into();
        log.upstream_response_headers = "up-resp-h".into();

        // strip_user=true, strip_upstream=true → 仅 3 个 body 列清空，4 个 headers 列保留。
        let cols = ProxyLogColumns::from_log(&log, true, true);
        insert_proxy_log_columns(&db, cols).await.unwrap();
        let row = get_proxy_log(&db, "strip").await.unwrap().unwrap();

        // headers 始终记录（元数据，auth 已脱敏）。
        assert_eq!(row.request_headers, "secret-h");
        assert_eq!(row.user_response_headers, "ur-h");
        assert_eq!(row.upstream_request_headers, "up-rh");
        assert_eq!(row.upstream_response_headers, "up-resp-h");
        // body 受开关控制 → 清空。
        assert!(row.request_body.is_empty());
        assert!(row.user_response_body.is_empty());
        assert!(row.upstream_request_body.is_empty());
        // 非脱敏字段保留。
        assert_eq!(row.group_key, "grp");
        assert_eq!(row.model, "claude-sonnet-4");
    }



    /// proxy_log attempts JSON 列往返
    #[tokio::test]
    async fn proxy_log_attempts_roundtrip() {
        let db = test_db().await;
        let mut log = sample_log("attlog", "g", now());
        log.attempts = vec![
            crate::gateway::models::ProxyAttempt {
                platform_id: 1, platform_name: "p1".into(), status_code: 503,
                error: "boom".into(), duration_ms: 12, ts: now(),
            },
            crate::gateway::models::ProxyAttempt {
                platform_id: 2, platform_name: "p2".into(), status_code: 200,
                error: String::new(), duration_ms: 34, ts: now(),
            },
        ];
        log.retry_count = 1;
        upsert_proxy_log(&db, log).await.unwrap();
        let fetched = get_proxy_log(&db, "attlog").await.unwrap().unwrap();
        assert_eq!(fetched.attempts.len(), 2);
        assert_eq!(fetched.attempts[0].status_code, 503);
        assert_eq!(fetched.attempts[1].platform_name, "p2");
        assert_eq!(fetched.retry_count, 1);
    }



    /// cleanup_proxy_logs 硬删：retention_days 内旧行物理删除（COUNT=0），不留 tombstone。
    #[tokio::test]
    async fn cleanup_proxy_logs_hard_deletes_old_rows() {
        let db = test_db().await;
        // 两行：1 行 100 天前（应删），1 行 1 天前（应保留）。
        let old_created = (chrono::Utc::now() - chrono::Duration::days(100)).timestamp_millis();
        let recent_created = (chrono::Utc::now() - chrono::Duration::days(1)).timestamp_millis();
        insert_proxy_log_at(&db, old_created).await;
        insert_proxy_log_at(&db, recent_created).await;
        assert_eq!(count_all_proxy_logs(&db).await, 2);

        // retention_days=30 → 删除 100 天前行，保留 1 天前行
        cleanup_proxy_logs(&db, 30).await.unwrap();
        assert_eq!(count_all_proxy_logs(&db).await, 1, "old row should be physically deleted");

        // retention_days=0 → 跳过清理（保持现行为）
        cleanup_proxy_logs(&db, 0).await.unwrap();
        assert_eq!(count_all_proxy_logs(&db).await, 1, "retention_days=0 must skip");
    }



    /// purge_deleted_proxy_logs 清历史 tombstone：软删残留行（deleted_at != 0）物理删除。
    #[tokio::test]
    async fn purge_deleted_clears_historical_tombstones() {
        let db = test_db().await;
        // 手动软删一行（deleted_at != 0），模拟迁移前积压 tombstone
        let created = chrono::Utc::now().timestamp_millis();
        insert_proxy_log_at(&db, created).await;
        db
            .call_traced(None, std::panic::Location::caller(), |conn| {
                conn.execute("UPDATE proxy_log SET deleted_at = ?1 WHERE id LIKE 'test-%'", params![now()])?;
                Ok(())
            })
            .await
            .unwrap();
        assert_eq!(count_all_proxy_logs(&db).await, 1, "tombstone still occupies row");

        purge_deleted_proxy_logs(&db).await.unwrap();
        assert_eq!(count_all_proxy_logs(&db).await, 0, "tombstone should be physically purged");
    }
