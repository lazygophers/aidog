#![cfg(test)]
use crate::*;
use aidog_db::models::*;
use aidog_db::now;
use aidog_db::test_support::*;
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
    final_log.attempts = vec![aidog_db::models::ProxyAttempt {
        platform_id: 1,
        platform_name: "p1".into(),
        status_code: 200,
        error: String::new(),
        duration_ms: 99,
        ts: now_ms,
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
    update_proxy_log_columns(&db, c2.clone(), &c1)
        .await
        .unwrap();

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

/// strip(脱敏) 等价性：log_user_request/log_upstream_request 关时，整侧「原始信息」
/// （headers + body + 上游响应正文）全部清空，只留解析后元数据（group_key/model 等）。
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
    log.response_body = "{\"ok\":true}".into();

    // strip_user=true, strip_upstream=true → 两侧 headers + body + 上游响应正文全清空。
    let cols = ProxyLogColumns::from_log(&log, true, true);
    insert_proxy_log_columns(&db, cols).await.unwrap();
    let row = get_proxy_log(&db, "strip").await.unwrap().unwrap();

    // headers 受开关控制 → 清空。
    assert!(row.request_headers.is_empty());
    assert!(row.user_response_headers.is_empty());
    assert!(row.upstream_request_headers.is_empty());
    assert!(row.upstream_response_headers.is_empty());
    // body + 上游响应正文受开关控制 → 清空。
    assert!(row.request_body.is_empty());
    assert!(row.user_response_body.is_empty());
    assert!(row.upstream_request_body.is_empty());
    assert!(row.response_body.is_empty());
    // 解析后元数据保留。
    assert_eq!(row.group_key, "grp");
    assert_eq!(row.model, "claude-sonnet-4");
}

/// 票 06：占位哨兵已废，strip 对 body 一视同仁清空；终态判定由 done 列承担，
/// 中间态 body 不再承载控制语义。
#[tokio::test]
async fn strip_clears_stream_body_and_keeps_done_flag() {
    let mut log = sample_log("strip-stream", "grp", now());
    log.response_body = "sse bytes...".into();
    log.done = false;
    let cols = ProxyLogColumns::from_log(&log, true, true);
    assert!(
        cols.response_body.is_empty(),
        "strip 后 body 应清空，无占位例外"
    );
    assert_eq!(cols.done, 0, "中间态 done=0 原样入库");
    log.done = true;
    let cols = ProxyLogColumns::from_log(&log, true, true);
    assert_eq!(cols.done, 1, "终态 done=1 不受 strip 影响");
}

/// proxy_log attempts JSON 列往返
#[tokio::test]
async fn proxy_log_attempts_roundtrip() {
    let db = test_db().await;
    let mut log = sample_log("attlog", "g", now());
    log.attempts = vec![
        aidog_db::models::ProxyAttempt {
            platform_id: 1,
            platform_name: "p1".into(),
            status_code: 503,
            error: "boom".into(),
            duration_ms: 12,
            ts: now(),
        },
        aidog_db::models::ProxyAttempt {
            platform_id: 2,
            platform_name: "p2".into(),
            status_code: 200,
            error: String::new(),
            duration_ms: 34,
            ts: now(),
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
    cleanup_proxy_logs(&db, 30, RetentionUnit::Day)
        .await
        .unwrap();
    assert_eq!(
        count_all_proxy_logs(&db).await,
        1,
        "old row should be physically deleted"
    );

    // retention_days=0 → 跳过清理（保持现行为）
    cleanup_proxy_logs(&db, 0, RetentionUnit::Day)
        .await
        .unwrap();
    assert_eq!(
        count_all_proxy_logs(&db).await,
        1,
        "retention_days=0 must skip"
    );
}

/// 建一条带 body 的日志行（四个 body 列各写一段文本），返回该行 body 字节总和。
async fn insert_log_with_bodies(db: &aidog_db::Db, id: &str, created_at: i64) -> i64 {
    let mut log = sample_log(id, "grp", created_at);
    // 含中文 → UTF-8 多字节，验证字节口径（非字符口径）。
    log.request_body = "{\"q\":\"你好\"}".into();
    log.upstream_request_body = "{\"m\":\"x\"}".into();
    log.response_body = "{\"ok\":true}".into();
    log.user_response_body = "{\"ok\":true}".into();
    let bytes = (log.request_body.len()
        + log.upstream_request_body.len()
        + log.response_body.len()
        + log.user_response_body.len()) as i64;
    upsert_proxy_log(db, log).await.unwrap();
    bytes
}

/// 只读红线（票 02 验收①）：estimate_cleanup 调用前后行数与行内容完全不变。
#[tokio::test]
async fn estimate_cleanup_is_read_only() {
    let db = test_db().await;
    let old_created = (chrono::Utc::now() - chrono::Duration::days(100)).timestamp_millis();
    let bytes = insert_log_with_bodies(&db, "old", old_created).await;
    let before = count_all_proxy_logs(&db).await;

    // 连调两次：既验证不改数据，也验证自身幂等（第二次结果与第一次一致）。
    let e1 = estimate_cleanup(&db, 30, RetentionUnit::Day).await.unwrap();
    let e2 = estimate_cleanup(&db, 30, RetentionUnit::Day).await.unwrap();

    assert_eq!(
        count_all_proxy_logs(&db).await,
        before,
        "estimate must not delete rows"
    );
    let row = get_proxy_log(&db, "old").await.unwrap().unwrap();
    assert_eq!(row.request_body, "{\"q\":\"你好\"}", "body must be intact");
    assert_eq!(e1, e2, "estimate must be repeatable");
    assert_eq!(e1.overdue_rows, 1);
    assert_eq!(e1.overdue_body_bytes, bytes);
    assert!(e1.db_size_bytes > 0, "page_count × page_size must be > 0");
}

/// 口径一致（票 02 验收②）：预估的超期行数 = 随后 cleanup 实际删除的行数。
#[tokio::test]
async fn estimate_overdue_rows_matches_actual_deletion() {
    let db = test_db().await;
    let old = (chrono::Utc::now() - chrono::Duration::days(100)).timestamp_millis();
    let older = (chrono::Utc::now() - chrono::Duration::days(60)).timestamp_millis();
    let recent = (chrono::Utc::now() - chrono::Duration::days(1)).timestamp_millis();
    insert_log_with_bodies(&db, "old", old).await;
    insert_log_with_bodies(&db, "older", older).await;
    insert_log_with_bodies(&db, "recent", recent).await;

    let est = estimate_cleanup(&db, 30, RetentionUnit::Day).await.unwrap();
    let before = count_all_proxy_logs(&db).await;
    cleanup_proxy_logs(&db, 30, RetentionUnit::Day)
        .await
        .unwrap();
    let deleted = before - count_all_proxy_logs(&db).await;
    assert_eq!(est.overdue_rows, 2, "两行超 30 天");
    assert_eq!(est.overdue_rows, deleted, "预估行数须等于实际删除行数");
}

/// 永久保留（value=0）：无超期数据，两个超期数为 0，库大小仍返回。
#[tokio::test]
async fn estimate_cleanup_forever_retention_reports_zero_overdue() {
    let db = test_db().await;
    let old = (chrono::Utc::now() - chrono::Duration::days(100)).timestamp_millis();
    insert_log_with_bodies(&db, "old", old).await;

    let est = estimate_cleanup(&db, 0, RetentionUnit::Day).await.unwrap();
    assert_eq!(est.overdue_rows, 0);
    assert_eq!(est.overdue_body_bytes, 0);
    assert!(est.db_size_bytes > 0);
}

/// purge_deleted_proxy_logs 清历史 tombstone：软删残留行（deleted_at != 0）物理删除。
#[tokio::test]
async fn purge_deleted_clears_historical_tombstones() {
    let db = test_db().await;
    // 手动软删一行（deleted_at != 0），模拟迁移前积压 tombstone
    let created = chrono::Utc::now().timestamp_millis();
    insert_proxy_log_at(&db, created).await;
    db.call_traced(None, std::panic::Location::caller(), |conn| {
        conn.execute(
            "UPDATE proxy_log SET deleted_at = ?1 WHERE id LIKE 'test-%'",
            params![now()],
        )?;
        Ok(())
    })
    .await
    .unwrap();
    assert_eq!(
        count_all_proxy_logs(&db).await,
        1,
        "tombstone still occupies row"
    );

    purge_deleted_proxy_logs(&db).await.unwrap();
    assert_eq!(
        count_all_proxy_logs(&db).await,
        0,
        "tombstone should be physically purged"
    );
}

/// list_proxy_logs 基本功能：插入两条日志，分页取回正确。
#[tokio::test]
async fn list_proxy_logs_basic() {
    let db = test_db().await;
    let now = now();
    let l1 = sample_log("l1", "g1", now - 2000);
    let l2 = sample_log("l2", "g1", now - 1000);
    insert_proxy_log_columns(&db, ProxyLogColumns::from_log(&l1, false, false))
        .await
        .unwrap();
    insert_proxy_log_columns(&db, ProxyLogColumns::from_log(&l2, false, false))
        .await
        .unwrap();
    let rows = list_proxy_logs(&db, 10, 0).await.unwrap();
    assert_eq!(rows.len(), 2, "should return both logs");
    // sorted by created_at DESC
    assert_eq!(rows[0].id, "l2");
    assert_eq!(rows[1].id, "l1");
}

/// list_proxy_logs 分页偏移。
#[tokio::test]
async fn list_proxy_logs_offset() {
    let db = test_db().await;
    let now = now();
    insert_proxy_log_columns(
        &db,
        ProxyLogColumns::from_log(&sample_log("p1", "g", now - 3000), false, false),
    )
    .await
    .unwrap();
    insert_proxy_log_columns(
        &db,
        ProxyLogColumns::from_log(&sample_log("p2", "g", now - 2000), false, false),
    )
    .await
    .unwrap();
    insert_proxy_log_columns(
        &db,
        ProxyLogColumns::from_log(&sample_log("p3", "g", now - 1000), false, false),
    )
    .await
    .unwrap();
    let page1 = list_proxy_logs(&db, 2, 0).await.unwrap();
    let page2 = list_proxy_logs(&db, 2, 2).await.unwrap();
    assert_eq!(page1.len(), 2);
    assert_eq!(page2.len(), 1);
    assert_eq!(page2[0].id, "p1"); // oldest
}

/// filtered_list + filtered_count：按 group_key 过滤。
#[tokio::test]
async fn filtered_list_by_group_key() {
    let db = test_db().await;
    let now = now();
    let l_g1 = sample_log("fg1", "group_a", now);
    let l_g2 = sample_log("fg2", "group_b", now);
    insert_proxy_log_columns(&db, ProxyLogColumns::from_log(&l_g1, false, false))
        .await
        .unwrap();
    insert_proxy_log_columns(&db, ProxyLogColumns::from_log(&l_g2, false, false))
        .await
        .unwrap();

    let filter = aidog_db::models::ProxyLogFilter {
        group_key: Some("group_a".to_string()),
        ..Default::default()
    };
    let rows = filtered_list_proxy_logs(&db, &filter, 10, 0)
        .await
        .unwrap()
        .items;
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].id, "fg1");

    let count = filtered_count_proxy_logs(&db, &filter).await.unwrap();
    assert_eq!(count, 1);
}

/// filtered_list：按 status=200(成功) 过滤。
#[tokio::test]
async fn filtered_list_by_status_success() {
    let db = test_db().await;
    let now = now();
    let mut ok = sample_log("ok", "g", now);
    ok.status_code = 200;
    let mut fail = sample_log("fail", "g", now - 1);
    fail.status_code = 503;
    insert_proxy_log_columns(&db, ProxyLogColumns::from_log(&ok, false, false))
        .await
        .unwrap();
    insert_proxy_log_columns(&db, ProxyLogColumns::from_log(&fail, false, false))
        .await
        .unwrap();

    let filter_ok = aidog_db::models::ProxyLogFilter {
        status: Some(200),
        ..Default::default()
    };
    let rows = filtered_list_proxy_logs(&db, &filter_ok, 10, 0)
        .await
        .unwrap()
        .items;
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].id, "ok");

    let filter_fail = aidog_db::models::ProxyLogFilter {
        status: Some(-1),
        ..Default::default()
    };
    let rows2 = filtered_list_proxy_logs(&db, &filter_fail, 10, 0)
        .await
        .unwrap()
        .items;
    assert_eq!(rows2.len(), 1);
    assert_eq!(rows2[0].id, "fail");
}

/// filtered_list：model_type="actual" vs "original"。
#[tokio::test]
async fn filtered_list_by_model_type() {
    let db = test_db().await;
    let now = now();
    let mut l = sample_log("model1", "g", now);
    l.model = "claude-sonnet-4".into();
    l.actual_model = "glm-4-plus".into();
    insert_proxy_log_columns(&db, ProxyLogColumns::from_log(&l, false, false))
        .await
        .unwrap();

    // actual model filter
    let filter_actual = aidog_db::models::ProxyLogFilter {
        model: Some("glm-4-plus".to_string()),
        model_type: Some("actual".to_string()),
        ..Default::default()
    };
    let rows = filtered_list_proxy_logs(&db, &filter_actual, 10, 0)
        .await
        .unwrap()
        .items;
    assert_eq!(rows.len(), 1, "actual model match should work");

    // original model filter
    let filter_orig = aidog_db::models::ProxyLogFilter {
        model: Some("claude-sonnet-4".to_string()),
        model_type: Some("original".to_string()),
        ..Default::default()
    };
    let rows2 = filtered_list_proxy_logs(&db, &filter_orig, 10, 0)
        .await
        .unwrap()
        .items;
    assert_eq!(rows2.len(), 1, "original model match should work");
}

/// filtered_list：has_more 边界（logs-query-ipc-slimming s2，替代精确 COUNT）。
/// 恰好 limit 行 → has_more=false；limit+1 行 → 只截断返回 limit 行 + has_more=true。
#[tokio::test]
async fn filtered_list_has_more_boundary() {
    let db = test_db().await;
    let now = now();
    for i in 0..3 {
        insert_proxy_log_columns(
            &db,
            ProxyLogColumns::from_log(&sample_log(&format!("hm{i}"), "g", now - i), false, false),
        )
        .await
        .unwrap();
    }
    let filter = aidog_db::models::ProxyLogFilter::default();

    // 恰好 3 行、limit=3 → 无下一页
    let exact = filtered_list_proxy_logs(&db, &filter, 3, 0).await.unwrap();
    assert_eq!(exact.items.len(), 3);
    assert!(!exact.has_more, "exact page should not report has_more");

    // limit=2 → 探测到第 3 行，截断为 2 行 + has_more=true
    let short = filtered_list_proxy_logs(&db, &filter, 2, 0).await.unwrap();
    assert_eq!(short.items.len(), 2, "probe row must not leak into items");
    assert!(short.has_more);
}

/// clear_proxy_logs 软删全部日志（设 deleted_at != 0）。
/// list_proxy_logs/filtered_list 的 WHERE deleted_at=0 过滤后结果为空。
#[tokio::test]
async fn clear_proxy_logs_soft_deletes_all() {
    let db = test_db().await;
    let now = now();
    insert_proxy_log_columns(
        &db,
        ProxyLogColumns::from_log(&sample_log("c1", "g", now), false, false),
    )
    .await
    .unwrap();
    insert_proxy_log_columns(
        &db,
        ProxyLogColumns::from_log(&sample_log("c2", "g", now - 1), false, false),
    )
    .await
    .unwrap();
    assert_eq!(count_all_proxy_logs(&db).await, 2);
    clear_proxy_logs(&db).await.unwrap();
    // 软删后 filtered_list（WHERE deleted_at=0）应为空
    let filter = aidog_db::models::ProxyLogFilter::default();
    let rows = filtered_list_proxy_logs(&db, &filter, 100, 0)
        .await
        .unwrap()
        .items;
    assert_eq!(
        rows.len(),
        0,
        "cleared logs should be soft-deleted (hidden from list)"
    );
}

/// filtered_list：时间范围过滤。
#[tokio::test]
async fn filtered_list_by_time_range() {
    let db = test_db().await;
    let now = now();
    let old = sample_log("old", "g", now - 100_000);
    let recent = sample_log("recent", "g", now);
    insert_proxy_log_columns(&db, ProxyLogColumns::from_log(&old, false, false))
        .await
        .unwrap();
    insert_proxy_log_columns(&db, ProxyLogColumns::from_log(&recent, false, false))
        .await
        .unwrap();

    let filter = aidog_db::models::ProxyLogFilter {
        time_start: Some(now - 10_000),
        time_end: Some(now + 10_000),
        ..Default::default()
    };
    let rows = filtered_list_proxy_logs(&db, &filter, 10, 0)
        .await
        .unwrap()
        .items;
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].id, "recent");

    let count = filtered_count_proxy_logs(&db, &filter).await.unwrap();
    assert_eq!(count, 1);
}

/// get_proxy_log 返回完整行 + 不存在时 None。
#[tokio::test]
async fn get_proxy_log_found_and_missing() {
    let db = test_db().await;
    let ts = now();
    let log = sample_log("gpl-1", "g", ts);
    insert_proxy_log_columns(&db, ProxyLogColumns::from_log(&log, false, false))
        .await
        .unwrap();

    let got = get_proxy_log(&db, "gpl-1").await.unwrap();
    assert!(got.is_some(), "should find inserted log");
    assert_eq!(got.unwrap().id, "gpl-1");

    let missing = get_proxy_log(&db, "nonexistent-id").await.unwrap();
    assert!(missing.is_none());
}

/// filtered_list：按 path (request_url LIKE) 过滤。
#[tokio::test]
async fn filtered_list_by_path_search() {
    let db = test_db().await;
    let ts = now();
    let log1 = sample_log("path-a", "g", ts);
    let log2 = sample_log("path-b", "g", ts + 1);
    insert_proxy_log_columns(&db, ProxyLogColumns::from_log(&log1, false, false))
        .await
        .unwrap();
    insert_proxy_log_columns(&db, ProxyLogColumns::from_log(&log2, false, false))
        .await
        .unwrap();
    let filter = aidog_db::models::ProxyLogFilter {
        path: Some("chat".into()),
        ..Default::default()
    };
    let rows = filtered_list_proxy_logs(&db, &filter, 10, 0)
        .await
        .unwrap()
        .items;
    // Both sample logs have same url, both should match "chat" if it's in the url
    // (sample_log sets request_url = "test://api/v1/chat/completions" or similar)
    let count = filtered_count_proxy_logs(&db, &filter).await.unwrap();
    assert_eq!(rows.len() as u32, count);
}

/// filtered_list：status 精确值过滤（非200/非-1分支）。
#[tokio::test]
async fn filtered_list_by_exact_status_code() {
    let db = test_db().await;
    let ts = now();
    let mut log_429 = sample_log("s429", "g", ts);
    log_429.status_code = 429;
    let log_ok = sample_log("s200", "g", ts + 1);
    insert_proxy_log_columns(&db, ProxyLogColumns::from_log(&log_429, false, false))
        .await
        .unwrap();
    insert_proxy_log_columns(&db, ProxyLogColumns::from_log(&log_ok, false, false))
        .await
        .unwrap();

    let filter = aidog_db::models::ProxyLogFilter {
        status: Some(429),
        ..Default::default()
    };
    let rows = filtered_list_proxy_logs(&db, &filter, 10, 0)
        .await
        .unwrap()
        .items;
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].id, "s429");
}

/// filtered_list：model_type="actual" 按 actual_model 过滤。
#[tokio::test]
async fn filtered_list_model_type_actual() {
    let db = test_db().await;
    let ts = now();
    let mut log_a = sample_log("mta-1", "g", ts);
    log_a.actual_model = "claude-3-5-sonnet".into();
    log_a.model = "alias".into();
    let mut log_b = sample_log("mta-2", "g", ts + 1);
    log_b.actual_model = "gpt-4o".into();
    log_b.model = "alias".into();
    insert_proxy_log_columns(&db, ProxyLogColumns::from_log(&log_a, false, false))
        .await
        .unwrap();
    insert_proxy_log_columns(&db, ProxyLogColumns::from_log(&log_b, false, false))
        .await
        .unwrap();

    let filter = aidog_db::models::ProxyLogFilter {
        model: Some("claude-3-5-sonnet".into()),
        model_type: Some("actual".into()),
        ..Default::default()
    };
    let rows = filtered_list_proxy_logs(&db, &filter, 10, 0)
        .await
        .unwrap()
        .items;
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].id, "mta-1");
}

/// list_request_logs (cli-proxy-request-log s3)：默认 sources=[test,quota]，
/// 排除纯代理转发行；LEFT JOIN cli_proxy_provider 带 provider name。
#[tokio::test]
async fn list_request_logs_filters_test_and_quota() {
    let db = test_db().await;
    let now = now();

    // 三行：1 条 test + 1 条 quota + 1 条 anthropic 代理转发
    let mut l_test = sample_log("t1", "g", now);
    l_test.source_protocol = "test".into();
    l_test.cli_proxy_provider_id = Some(7);
    let mut l_quota = sample_log("q1", "g", now - 100);
    l_quota.source_protocol = "quota".into();
    let mut l_fwd = sample_log("f1", "g", now - 50);
    l_fwd.source_protocol = "anthropic".into();
    insert_proxy_log_columns(&db, ProxyLogColumns::from_log(&l_test, false, false))
        .await
        .unwrap();
    insert_proxy_log_columns(&db, ProxyLogColumns::from_log(&l_quota, false, false))
        .await
        .unwrap();
    insert_proxy_log_columns(&db, ProxyLogColumns::from_log(&l_fwd, false, false))
        .await
        .unwrap();

    // 插入关联 cli_proxy_provider id=7（list_request_logs LEFT JOIN 应带出 name）。
    db.call_traced(None, std::panic::Location::caller(), |conn| {
            conn.execute(
                "INSERT INTO cli_proxy_provider (id, name, wire_protocol, base_url, models, extra, status, created_at, updated_at) \
                 VALUES (7, 'prov-A', 'openai', 'https://x', '[]', '{}', 'active', 0, 0)",
                [],
            )?;
            Ok(())
        })
        .await
        .unwrap();

    // 默认 sources 兜底 → 仅返 test + quota 两行（按 created_at DESC：t1, q1）
    let filter_default = aidog_db::models::ProxyLogFilter::default();
    let rows = list_request_logs(&db, &filter_default, 10, 0)
        .await
        .unwrap();
    assert_eq!(
        rows.len(),
        2,
        "default sources should exclude anthropic forward"
    );
    assert_eq!(rows[0].base.id, "t1");
    assert_eq!(rows[1].base.id, "q1");
    // LEFT JOIN：test 行带 provider_id=7 + name="prov-A"；quota 行均 None
    assert_eq!(rows[0].cli_proxy_provider_id, Some(7));
    assert_eq!(rows[0].cli_proxy_provider_name.as_deref(), Some("prov-A"));
    assert!(rows[1].cli_proxy_provider_id.is_none());
    assert!(rows[1].cli_proxy_provider_name.is_none());

    // 显式 sources 覆盖：传 [anthropic] → 仅返代理转发行
    let filter_override = aidog_db::models::ProxyLogFilter {
        sources: Some(vec!["anthropic".into()]),
        ..Default::default()
    };
    let rows2 = list_request_logs(&db, &filter_override, 10, 0)
        .await
        .unwrap();
    assert_eq!(rows2.len(), 1);
    assert_eq!(rows2[0].base.id, "f1");

    // cli_proxy_provider_id 筛选 → 仅返 provider_id=7 的 test 行
    let filter_by_pid = aidog_db::models::ProxyLogFilter {
        cli_proxy_provider_id: Some(7),
        ..Default::default()
    };
    let rows3 = list_request_logs(&db, &filter_by_pid, 10, 0).await.unwrap();
    assert_eq!(rows3.len(), 1);
    assert_eq!(rows3[0].base.id, "t1");

    // provider 已删（DELETE id=7）→ LEFT JOIN 仍返行但 name=None
    db.call_traced(None, std::panic::Location::caller(), |conn| {
        conn.execute("DELETE FROM cli_proxy_provider WHERE id = 7", [])?;
        Ok(())
    })
    .await
    .unwrap();
    let rows4 = list_request_logs(&db, &filter_default, 10, 0)
        .await
        .unwrap();
    let t1 = rows4.iter().find(|r| r.base.id == "t1").unwrap();
    assert_eq!(t1.cli_proxy_provider_id, Some(7), "fk column unchanged");
    assert!(
        t1.cli_proxy_provider_name.is_none(),
        "deleted provider → name None"
    );
}

/// exclude_sources：Logs 主页排除 test/quota → 仅纯代理转发。
/// 契约：list_proxy_logs/exclude_sources 走同一 build_filter_where；这里用 filtered_list 验证。
#[tokio::test]
async fn exclude_sources_hides_test_and_quota() {
    let db = test_db().await;
    let now = now();
    let mut l_test = sample_log("t1", "g", now);
    l_test.source_protocol = "test".into();
    let mut l_quota = sample_log("q1", "g", now - 100);
    l_quota.source_protocol = "quota".into();
    let mut l_fwd = sample_log("f1", "g", now - 50);
    l_fwd.source_protocol = "anthropic".into();
    insert_proxy_log_columns(&db, ProxyLogColumns::from_log(&l_test, false, false))
        .await
        .unwrap();
    insert_proxy_log_columns(&db, ProxyLogColumns::from_log(&l_quota, false, false))
        .await
        .unwrap();
    insert_proxy_log_columns(&db, ProxyLogColumns::from_log(&l_fwd, false, false))
        .await
        .unwrap();

    let filter = aidog_db::models::ProxyLogFilter {
        exclude_sources: Some(vec!["test".into(), "quota".into()]),
        ..Default::default()
    };
    let rows = filtered_list_proxy_logs(&db, &filter, 10, 0)
        .await
        .unwrap()
        .items;
    assert_eq!(rows.len(), 1, "only anthropic forward should remain");
    assert_eq!(rows[0].id, "f1");
}

/// logs-query-ipc-slimming s4：distinct_models_proxy_log 去重 + exclude_sources 生效 +
/// actual 参数选列正确。
#[tokio::test]
async fn distinct_models_dedup_and_excludes_sources() {
    let db = test_db().await;
    let now = now();
    // 两条 model 相同、actual_model 不同的转发行，一条 test 源（应被排除）。
    let mut l1 = sample_log("d1", "g", now);
    l1.model = "claude-sonnet-4".into();
    l1.actual_model = "glm-4-plus".into();
    let mut l2 = sample_log("d2", "g", now - 10);
    l2.model = "claude-sonnet-4".into();
    l2.actual_model = "deepseek-chat".into();
    let mut l3 = sample_log("d3", "g", now - 20);
    l3.source_protocol = "test".into();
    l3.model = "should-not-appear".into();
    insert_proxy_log_columns(&db, ProxyLogColumns::from_log(&l1, false, false))
        .await
        .unwrap();
    insert_proxy_log_columns(&db, ProxyLogColumns::from_log(&l2, false, false))
        .await
        .unwrap();
    insert_proxy_log_columns(&db, ProxyLogColumns::from_log(&l3, false, false))
        .await
        .unwrap();

    let filter = aidog_db::models::ProxyLogFilter {
        exclude_sources: Some(vec!["test".into(), "quota".into()]),
        ..Default::default()
    };
    let models = distinct_models_proxy_log(&db, &filter, false, 200)
        .await
        .unwrap();
    assert_eq!(
        models,
        vec!["claude-sonnet-4".to_string()],
        "dedup on model col, test source excluded"
    );

    let actual_models = distinct_models_proxy_log(&db, &filter, true, 200)
        .await
        .unwrap();
    assert_eq!(
        actual_models,
        vec!["deepseek-chat".to_string(), "glm-4-plus".to_string()]
    );
}

/// 票 10：field_trace 列入库往返 + 受上游日志开关约束。
/// 开关开 → 留痕原样落库；`strip_upstream`（!log_upstream_request）→ 与上游侧原始信息一同清空。
#[tokio::test]
async fn field_trace_roundtrip_and_strip() {
    let db = test_db().await;
    let now_ms = now();

    let mut kept = sample_log("ft-keep", "g", now_ms);
    kept.field_trace = "cap:max_tokens,drop:user".into();
    insert_proxy_log_columns(&db, ProxyLogColumns::from_log(&kept, false, false))
        .await
        .unwrap();
    let row = get_proxy_log(&db, "ft-keep").await.unwrap().unwrap();
    assert_eq!(row.field_trace, "cap:max_tokens,drop:user");

    let mut stripped = sample_log("ft-strip", "g", now_ms);
    stripped.field_trace = "drop:user".into();
    insert_proxy_log_columns(&db, ProxyLogColumns::from_log(&stripped, false, true))
        .await
        .unwrap();
    let row = get_proxy_log(&db, "ft-strip").await.unwrap().unwrap();
    assert_eq!(row.field_trace, "", "关上游日志开关时 field_trace 一并清空");

    // 无丢弃无改写 → 空串，不产生噪音记录。
    let quiet = sample_log("ft-quiet", "g", now_ms);
    insert_proxy_log_columns(&db, ProxyLogColumns::from_log(&quiet, false, false))
        .await
        .unwrap();
    assert_eq!(
        get_proxy_log(&db, "ft-quiet")
            .await
            .unwrap()
            .unwrap()
            .field_trace,
        ""
    );
}

/// 票 10：渐进式写入下 field_trace 变化能被 diff 捕获（首节点空 → 后续节点补留痕）。
#[tokio::test]
async fn field_trace_updated_by_progressive_diff() {
    let db = test_db().await;
    let now_ms = now();
    let n1 = sample_log("ft-prog", "g", now_ms);
    let c1 = ProxyLogColumns::from_log(&n1, false, false);
    insert_proxy_log_columns(&db, c1.clone()).await.unwrap();

    let mut n2 = n1.clone();
    n2.field_trace = "rewrite:middleware".into();
    let c2 = ProxyLogColumns::from_log(&n2, false, false);
    update_proxy_log_columns(&db, c2, &c1).await.unwrap();

    let row = get_proxy_log(&db, "ft-prog").await.unwrap().unwrap();
    assert_eq!(row.field_trace, "rewrite:middleware");
}
