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
    insert_row_full(db, RowSpec { id, session_key, request_body, created_at, input_tokens, est_cost, group_key: "gk_test", client: "", session_id: "" }).await;
}

/// 扩维版：显式指定 group_key / client / session_id（末条请求维度取值断言用）。
struct RowSpec<'a> {
    id: &'a str,
    session_key: &'a str,
    request_body: &'a str,
    created_at: i64,
    input_tokens: i32,
    est_cost: f64,
    group_key: &'a str,
    client: &'a str,
    session_id: &'a str,
}

async fn insert_row_full(db: &aidog_db::Db, r: RowSpec<'_>) {
    let mut log = aidog_db::test_support::sample_log(r.id, r.group_key, r.created_at);
    log.session_key = r.session_key.to_string();
    log.request_body = r.request_body.to_string();
    log.input_tokens = r.input_tokens;
    log.est_cost = r.est_cost;
    log.client = r.client.to_string();
    log.session_id = r.session_id.to_string();
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

/// 会话聚合行扩维（2026-11-21）：group_key / client / session_id 取窗口内末条请求的值。
#[tokio::test]
async fn aggregates_latest_row_group_client_session_id() {
    let (db, _dir) = three_file_db().await;
    let now = aidog_db::now();
    // sess-multi：两条请求跨分组，末条（now-50）为准
    insert_row_full(&db, RowSpec { id: "m1", session_key: "sess-multi", request_body: "", created_at: now - 100, input_tokens: 1, est_cost: 0.0, group_key: "gk_old", client: "claude_code", session_id: "sid-1" }).await;
    insert_row_full(&db, RowSpec { id: "m2", session_key: "sess-multi", request_body: "", created_at: now - 50, input_tokens: 1, est_cost: 0.0, group_key: "gk_new", client: "codex_tui", session_id: "sid-2" }).await;
    // sess-plain：单条，无 client / session_id
    insert_row_full(&db, RowSpec { id: "p1", session_key: "sess-plain", request_body: "", created_at: now - 10, input_tokens: 1, est_cost: 0.0, group_key: "gk_p", client: "", session_id: "" }).await;

    let stats = get_session_stats(&db, None).await.unwrap();
    let m = stats.iter().find(|s| s.session_key == "sess-multi").expect("sess-multi");
    assert_eq!(m.group_key, "gk_new", "同会话跨分组取末条请求的 group_key");
    assert_eq!(m.client, "codex_tui");
    assert_eq!(m.session_id, "sid-2");

    let p = stats.iter().find(|s| s.session_key == "sess-plain").expect("sess-plain");
    assert_eq!(p.group_key, "gk_p");
    assert_eq!(p.client, "", "无 client 列值为空串");
    assert_eq!(p.session_id, "");
}

#[tokio::test]
async fn days_defaults_and_clamps() {
    let (db, _dir) = three_file_db().await;
    // 不落任何行：只验证 None / 0 / 超界不报错（clamp 1..=90）
    assert!(get_session_stats(&db, None).await.unwrap().is_empty());
    assert!(get_session_stats(&db, Some(0)).await.unwrap().is_empty());
    assert!(get_session_stats(&db, Some(10_000)).await.unwrap().is_empty());
}
