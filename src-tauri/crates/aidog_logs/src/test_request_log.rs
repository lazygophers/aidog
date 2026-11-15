#![cfg(test)]
//! request_log 表：写 → 重开 DB 读回（three_file_db，错路由当场 no such table）。
use crate::*;
use aidog_db::Db;
use aidog_db::models::*;
use aidog_db::now;
use aidog_db::test_support::*;

async fn set_raw(db: &Db, on: bool) {
    let settings = ProxyLogSettings {
        log_user_request: on,
        log_upstream_request: on,
        ..Default::default()
    };
    aidog_db::set_setting(
        db,
        SetSettingInput {
            scope: "proxy".into(),
            key: "logging".into(),
            value: serde_json::to_value(settings).unwrap(),
        },
    )
    .await
    .unwrap();
}

fn oob(id: &str, kind: &str, platform_id: u64, status: i32, at: i64) -> ProxyLog {
    ProxyLog {
        id: id.into(),
        platform_id,
        status_code: status,
        duration_ms: 12,
        model: "m-req".into(),
        actual_model: "m-act".into(),
        input_tokens: 7,
        output_tokens: 8,
        user_agent: "ua-x".into(),
        request_body: "{\"q\":1}".into(),
        response_body: "up-resp".into(),
        request_url: "/model-test/1".into(),
        target_protocol: "openai".into(),
        upstream_status_code: status,
        created_at: at,
        updated_at: at,
        ..Default::default()
    }
    .out_of_band(&format!("[{kind}]"), kind)
}

#[tokio::test]
async fn write_then_reopen_read_roundtrip_without_model_or_tokens() {
    let (db, dir) = three_file_db().await;
    set_raw(&db, true).await;
    let t = now();
    upsert_request_log(&db, oob("a1", "test", 5, 200, t)).await.unwrap();
    upsert_request_log(&db, oob("a2", "quota", 6, 500, t - 10)).await.unwrap();
    drop(db);

    let db2 = Db::new(dir.path().join("aidog.db").to_str().unwrap()).await.unwrap();
    let all = list_request_logs(&db2, &RequestLogFilter::default(), 10, 0).await.unwrap();
    assert_eq!(all.len(), 2);
    assert_eq!(all[0].id, "a1");
    assert_eq!(all[0].kind, "test");
    assert_eq!(all[0].platform_id, 5);
    assert_eq!(all[0].user_agent, "ua-x");
    assert_eq!(all[0].group_key, "[test]");

    let d = get_request_log(&db2, "a1").await.unwrap().expect("row");
    assert_eq!(d.request_body, "{\"q\":1}");
    assert_eq!(d.response_body, "up-resp");
    assert_eq!(d.source_protocol, "test");
    assert_eq!(d.target_protocol, "openai");
    assert_eq!(d.model, "", "model 不入 request_log");
    assert_eq!(d.actual_model, "");
    assert_eq!(d.input_tokens, 0);
    assert_eq!(d.output_tokens, 0);
}

#[tokio::test]
async fn filters_and_count() {
    let (db, _dir) = three_file_db().await;
    let t = now();
    upsert_request_log(&db, oob("a1", "test", 5, 200, t)).await.unwrap();
    upsert_request_log(&db, oob("a2", "quota", 6, 500, t - 10)).await.unwrap();
    upsert_request_log(&db, oob("a3", "fetch-models", 5, 200, t - 20)).await.unwrap();
    let ids = |v: Vec<RequestLogSummary>| v.into_iter().map(|r| r.id).collect::<Vec<_>>();
    let f = |kind: Option<&str>, platform_id: Option<u64>, status: Option<i32>| RequestLogFilter {
        kind: kind.map(Into::into),
        platform_id,
        status,
        ..Default::default()
    };
    assert_eq!(ids(list_request_logs(&db, &f(Some("quota"), None, None), 10, 0).await.unwrap()), ["a2"]);
    assert_eq!(ids(list_request_logs(&db, &f(None, Some(5), None), 10, 0).await.unwrap()), ["a1", "a3"]);
    assert_eq!(ids(list_request_logs(&db, &f(None, None, Some(-1)), 10, 0).await.unwrap()), ["a2"]);
    assert_eq!(count_request_logs(&db, &f(None, Some(5), Some(200))).await.unwrap(), 2);
    assert_eq!(count_request_logs(&db, &RequestLogFilter::default()).await.unwrap(), 3);
    assert_eq!(ids(list_request_logs(&db, &RequestLogFilter::default(), 1, 1).await.unwrap()), ["a2"]);
}

#[tokio::test]
async fn raw_info_follows_proxy_log_settings_switch() {
    let (db, _dir) = three_file_db().await;
    set_raw(&db, false).await;
    upsert_request_log(&db, oob("off", "test", 5, 200, now())).await.unwrap();
    let d = get_request_log(&db, "off").await.unwrap().unwrap();
    assert_eq!(d.request_body, "");
    assert_eq!(d.response_body, "");
    assert_eq!(d.request_url, "");
    assert_eq!(d.status_code, 200, "元数据恒记录");
}

#[tokio::test]
async fn request_log_rows_never_land_in_proxy_log() {
    let (db, _dir) = three_file_db().await;
    upsert_request_log(&db, oob("a1", "test", 5, 200, now())).await.unwrap();
    assert_eq!(filtered_count_proxy_logs(&db, &ProxyLogFilter::default()).await.unwrap(), 0);
}

/// 迁移 20261114-01：test/quota/fetch-models 行搬进 request_log 并从 proxy_log 删除；其余行不动；重跑幂等。
#[tokio::test]
async fn migration_moves_rows_and_is_idempotent() {
    let (db, _dir) = three_file_db().await;
    let t = now();
    for (id, src) in [("m1", "test"), ("m2", "quota"), ("m3", "fetch-models"), ("keep", "anthropic")] {
        let mut l = ProxyLog { id: id.into(), source_protocol: src.into(), platform_id: 9, status_code: 200, created_at: t, user_agent: "ua".into(), request_body: "rb".into(), model: "mm".into(), ..Default::default() };
        l.group_key = "g".into();
        insert_proxy_log_columns(&db, ProxyLogColumns::from_log(&l, false, false)).await.unwrap();
    }
    for _ in 0..2 {
        db.call_proxy_log_traced(None, std::panic::Location::caller(), |c| {
            Ok(aidog_db::schema_late::migrate_request_log(c)?)
        })
        .await
        .unwrap();
    }
    let rows = list_request_logs(&db, &RequestLogFilter::default(), 10, 0).await.unwrap();
    let mut ids: Vec<_> = rows.iter().map(|r| r.id.clone()).collect();
    ids.sort();
    assert_eq!(ids, ["m1", "m2", "m3"]);
    let d = get_request_log(&db, "m1").await.unwrap().unwrap();
    assert_eq!((d.source_protocol.as_str(), d.user_agent.as_str(), d.request_body.as_str(), d.platform_id), ("test", "ua", "rb", 9));
    let left = filtered_list_proxy_logs(&db, &ProxyLogFilter::default(), 10, 0).await.unwrap().items;
    assert_eq!(left.iter().map(|r| r.id.as_str()).collect::<Vec<_>>(), ["keep"]);
}

/// 复用 proxy_log retention：整行保留期删行；用户侧 / 上游侧原始信息按各自 retention 清列不删行。
#[tokio::test]
async fn retention_reuses_proxy_log_settings() {
    let (db, _dir) = three_file_db().await;
    set_raw(&db, true).await;
    let day = 86_400_000i64;
    upsert_request_log(&db, oob("old", "test", 5, 200, now() - 3 * day)).await.unwrap();
    upsert_request_log(&db, oob("new", "test", 5, 200, now())).await.unwrap();
    aidog_db::cleanup_user_request_fields(&db, 1, RetentionUnit::Day).await.unwrap();
    aidog_db::cleanup_upstream_request_fields(&db, 1, RetentionUnit::Day).await.unwrap();
    let old = get_request_log(&db, "old").await.unwrap().unwrap();
    assert_eq!((old.request_body.as_str(), old.response_body.as_str()), ("", ""));
    assert_eq!(get_request_log(&db, "new").await.unwrap().unwrap().request_body, "{\"q\":1}");
    cleanup_proxy_logs(&db, 1, RetentionUnit::Day).await.unwrap();
    assert!(get_request_log(&db, "old").await.unwrap().is_none());
    assert!(get_request_log(&db, "new").await.unwrap().is_some());
}

/// 迁移失败不得被吞：request_log 已存在但结构不兼容 → INSERT 报错向上传播，proxy_log 行原样保留（事务回滚）。
#[test]
fn migration_real_failure_propagates_and_keeps_proxy_log_rows() {
    let conn = rusqlite::Connection::open_in_memory().unwrap();
    let cols = "id, source_protocol, group_key, platform_id, target_protocol, status_code, upstream_status_code, duration_ms, user_agent, request_url, request_headers, request_body, user_response_headers, user_response_body, upstream_request_url, upstream_request_headers, upstream_request_body, upstream_response_headers, response_body, created_at";
    conn.execute_batch(&format!(
        "CREATE TABLE proxy_log ({cols}); INSERT INTO proxy_log ({cols}) VALUES ('x','test','g',1,'',200,200,1,'','','','','','','','','','','',1);
         CREATE TABLE request_log (id TEXT PRIMARY KEY);"
    ))
    .unwrap();
    assert!(aidog_db::schema_late::migrate_request_log(&conn).is_err());
    let n: i64 = conn.query_row("SELECT COUNT(*) FROM proxy_log", [], |r| r.get(0)).unwrap();
    assert_eq!(n, 1, "失败回滚后 proxy_log 行必须还在");
}
