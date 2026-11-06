//! 票 08 会话归组聚合：get_session_stats 按 session_key 聚合近 N 天数据。
//! fixture three_file_db：proxy_log 走 log.db 独立路径，读池错路由当场 no such table。

#![cfg(test)]
use crate::*;
use aidog_db::test_support::three_file_db;
use aidog_logs::*;

/// proxy_log 写入走 aidog_logs 正式写路径（insert_proxy_log_columns），
/// 顺带覆盖 PROXY_LOG_COLUMNS / from_log 携带 session_key 的列序一致性。
async fn insert_row(
    db: &aidog_db::Db,
    id: &str,
    session_key: &str,
    request_body: &str,
    created_at: i64,
    input_tokens: i32,
    est_cost: f64,
) {
    let mut log = aidog_db::test_support::sample_log(id, "gk_test", created_at);
    log.session_key = session_key.to_string();
    log.request_body = request_body.to_string();
    log.input_tokens = input_tokens;
    log.est_cost = est_cost;
    insert_proxy_log_columns(db, ProxyLogColumns::from_log(&log, false, false))
        .await
        .unwrap();
}

#[tokio::test]
async fn aggregates_by_session_excluding_empty_key() {
    let (db, _dir) = three_file_db().await;
    let now = aidog_db::now();
    insert_row(&db, "s_a1", "sess-aaa", r#"{"model":"m","messages":[{"role":"user","content":"帮我写一个排序算法，要求时间复杂度 O(n log n)"}]}"#, now - 100, 10, 0.01).await;
    insert_row(&db, "s_a2", "sess-aaa", "", now - 50, 20, 0.02).await;
    insert_row(&db, "s_b1", "sess-bbb", "", now - 30, 5, 0.0).await;
    // 空 session_key：不参与聚合（spec D4）
    insert_row(&db, "s_e1", "", "", now - 10, 999, 9.99).await;

    let stats = get_session_stats(&db, None).await.unwrap();
    assert_eq!(stats.len(), 2, "空 session_key 不产生聚合行，实际：{stats:?}");

    let a = stats.iter().find(|s| s.session_key == "sess-aaa").expect("sess-aaa row");
    assert_eq!(a.requests, 2);
    assert_eq!(a.input_tokens, 30);
    assert!((a.est_cost - 0.03).abs() < 1e-9);
    assert_eq!(a.first_created_at, now - 100);
    assert_eq!(a.last_created_at, now - 50);
    // 标题 = 首条有 request_body 的请求前 60 字符
    let want: String = r#"{"model":"m","messages":[{"role":"user","content":"帮我写一个排"#.chars().take(60).collect();
    assert_eq!(a.title.chars().count(), 60);
    assert!(a.title.starts_with(want.chars().take(20).collect::<String>().as_str()));

    let b = stats.iter().find(|s| s.session_key == "sess-bbb").expect("sess-bbb row");
    assert_eq!(b.requests, 1);
    assert_eq!(b.title, "", "无 request_body 的会话标题为空（log_user_request 关闭口径）");
    // 最近活动优先
    assert_eq!(stats[0].session_key, "sess-bbb");
}

#[tokio::test]
async fn days_window_filters_old_rows() {
    let (db, _dir) = three_file_db().await;
    let now = aidog_db::now();
    let day_ms = 86_400_000i64;
    insert_row(&db, "s_new", "sess-in", "", now - day_ms, 1, 0.0).await;
    insert_row(&db, "s_old", "sess-in", "", now - 8 * day_ms, 1, 0.0).await;

    let stats = get_session_stats(&db, Some(7)).await.unwrap();
    assert_eq!(stats.len(), 1);
    assert_eq!(stats[0].session_key, "sess-in");
    assert_eq!(stats[0].requests, 1, "7 天窗口外的行不计入");

    let all = get_session_stats(&db, Some(30)).await.unwrap();
    assert_eq!(all[0].requests, 2, "30 天窗口两条都计入");
}

#[tokio::test]
async fn days_defaults_and_clamps() {
    let (db, _dir) = three_file_db().await;
    // 不落任何行：只验证 None / 0 / 超界不报错（clamp 1..=90）
    assert!(get_session_stats(&db, None).await.unwrap().is_empty());
    assert!(get_session_stats(&db, Some(0)).await.unwrap().is_empty());
    assert!(get_session_stats(&db, Some(10_000)).await.unwrap().is_empty());
}
