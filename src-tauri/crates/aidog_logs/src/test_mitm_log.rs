//! 票 12 读侧单测：旁路行列表 / 采样点升序与尾部截断 / mitm_opaque 计数窗口。
use super::*;
use aidog_db::test_support::test_db;
use aidog_db::Db;

async fn seed() -> Db {
    let db = test_db().await;
    for (i, (host, code)) in [("datadog.example", 200), ("platform.claude.com", 503)].iter().enumerate() {
        insert_mitm_log(
            &db,
            MitmLogInsert {
                group_name: "cc".into(),
                host: (*host).to_string(),
                path: "/v1/track".into(),
                status_code: *code,
                req_bytes: 100,
                resp_bytes: 200,
                decrypted: true,
                request_body: String::new(),
                response_body: String::new(),
                created_at: 1_000 + i as i64,
            },
        )
        .await
        .unwrap();
    }
    db
}

#[tokio::test]
async fn list_mitm_bypass_rows_orders_desc_and_omits_body() {
    let db = seed().await;
    let rows = list_mitm_bypass_rows(&db, 10).await.unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].host, "platform.claude.com"); // created_at 降序
    assert_eq!(rows[0].status_code, 503);
    assert!(rows[0].decrypted);
}

#[tokio::test]
async fn usage_samples_asc_and_tail_limit() {
    let db = test_db().await;
    for pct in [10.0f64, 20.0, 30.0] {
        insert_oauth_usage_sample(&db, "cc", pct, pct * 2.0, "{}").await.unwrap();
        // sampled_at 用 now()（ms），同 ms 内插入顺序不定 → 依赖唯一性不可行，
        // 用 pct 单调 + limit=2 断言尾部两条升序即可（10 被裁掉）。
        tokio::time::sleep(std::time::Duration::from_millis(2)).await;
    }
    let rows = list_oauth_usage_samples(&db, "cc", 2).await.unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].five_hour_pct, 20.0);
    assert_eq!(rows[1].five_hour_pct, 30.0);
    assert!(rows[0].sampled_at < rows[1].sampled_at); // 升序
    // 其它 group 隔离
    assert!(list_oauth_usage_samples(&db, "other", 10).await.unwrap().is_empty());
}

#[tokio::test]
async fn count_mitm_opaque_filters_reason_and_window() {
    let db = test_db().await;
    db.call_traced(None, std::panic::Location::caller(), |conn| {
        conn.execute(
            "INSERT INTO proxy_log (id, platform_id, group_key, model, source_protocol, \
             status_code, est_cost, blocked_by, blocked_reason, created_at, deleted_at) \
             VALUES ('a', 0, '', 'm', 'anthropic', 0, 0, 'router', 'mitm_opaque', 5000, 0), \
                    ('b', 0, '', 'm', 'anthropic', 0, 0, 'router', 'mitm_opaque', 9000, 0), \
                    ('c', 0, '', 'm', 'anthropic', 0, 0, 'router', 'peak', 5000, 0)",
            [],
        )?;
        Ok(())
    })
    .await
    .unwrap();
    assert_eq!(count_mitm_opaque(&db, 0).await.unwrap(), 2);
    assert_eq!(count_mitm_opaque(&db, 6000).await.unwrap(), 1);
}
