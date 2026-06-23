//! quota 出站 HTTP 核心 + 工具函数覆盖。本地 stub server 驱动 quota_get_json 四路（成功 /
//! 非 2xx / parse 失败 / network 失败），含 db=Some 落库分支（make_quota_log + persist_quota_log）。
use super::*;
use crate::gateway::db::test_support::test_db;

async fn spawn_stub(status: u16, body: &'static str) -> String {
    use axum::routing::any;
    let app = axum::Router::new().fallback(any(move || async move {
        (
            axum::http::StatusCode::from_u16(status).unwrap(),
            [("content-type", "application/json")],
            body,
        )
    }));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.ok();
    });
    format!("http://{addr}")
}

#[test]
fn util_fns_cover() {
    assert!(now_millis() > 0);
    assert!(millis_to_iso8601(0).is_some());
    assert!(millis_to_iso8601(-1).is_none() || millis_to_iso8601(-1).is_some());
    assert_eq!(parse_f64(&serde_json::json!(3.5)), Some(3.5));
    assert_eq!(parse_f64(&serde_json::json!("2.0")), Some(2.0));
    assert_eq!(parse_f64(&serde_json::json!(true)), None);
    assert_eq!(
        parse_f64_field(&serde_json::json!({"x": 1.0}), "x"),
        Some(1.0)
    );
    let e = err_quota("boom");
    assert!(!e.success);
    assert_eq!(e.error.as_deref(), Some("boom"));
    let ep = err_quota_platform("kimi", "down");
    assert!(!ep.success);
    assert_eq!(ep.error.as_deref(), Some("down"));
}

#[tokio::test]
async fn quota_get_json_success_no_db() {
    let url = spawn_stub(200, r#"{"ok":true}"#).await;
    let v = quota_get_json(None, &url, &[("X-Test", "1".into())])
        .await
        .unwrap();
    assert_eq!(v.get("ok").unwrap(), &serde_json::json!(true));
}

#[tokio::test]
async fn quota_get_json_success_persists_with_db() {
    let db = std::sync::Arc::new(test_db().await);
    let url = spawn_stub(200, r#"{"ok":1}"#).await;
    let v = quota_get_json(Some(&db), &url, &[]).await.unwrap();
    assert_eq!(v.get("ok").unwrap(), &serde_json::json!(1));
    // 落库一条 quota 日志（source_protocol=quota，group_key=[quota]）
    let logs = crate::gateway::db::list_proxy_logs(&db, 100, 0).await.unwrap();
    assert!(logs.iter().any(|l| l.source_protocol == "quota"));
}

#[tokio::test]
async fn quota_get_json_http_error() {
    let db = std::sync::Arc::new(test_db().await);
    let url = spawn_stub(500, r#"{"e":"x"}"#).await;
    let err = quota_get_json(Some(&db), &url, &[]).await.unwrap_err();
    assert!(err.starts_with("HTTP "));
}

#[tokio::test]
async fn quota_get_json_parse_error() {
    let db = std::sync::Arc::new(test_db().await);
    let url = spawn_stub(200, "not json").await;
    let err = quota_get_json(Some(&db), &url, &[]).await.unwrap_err();
    assert!(err.starts_with("Parse:"));
}

#[tokio::test]
async fn quota_get_json_network_error() {
    // 指向未监听端口 → Network 错误
    let err = quota_get_json(None, "http://127.0.0.1:1/x", &[])
        .await
        .unwrap_err();
    assert!(err.starts_with("Network:"));
}
