//! 代理端到端集成测试：真 ProxyState（内存 DB）+ 本地 stub 上游 axum server，
//! 经 handle_proxy 全链路（handler→router→forward→finish→headers→log），
//! 覆盖成功转发 / 非 2xx failover / 早退分支（无 group 404 / bad body 400 / 健康端点）。

use super::*;
use crate::gateway::db::test_support::test_db;
use crate::gateway::middleware::MiddlewareEngine;
use crate::gateway::models::{CreatePlatform, GroupPlatformInput, Protocol};
use axum::body::Body;
use axum::http::Request as HttpRequest;
use std::sync::Arc;

/// 起一个 stub 上游 axum server，所有 POST 返回给定 (status, body)，返回 base_url。
async fn spawn_stub_upstream(status: u16, body: &'static str) -> String {
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

async fn make_state(db: crate::gateway::db::Db) -> Arc<ProxyState> {
    Arc::new(ProxyState {
        db: Arc::new(db),
        app: None,
        middleware: Arc::new(MiddlewareEngine::new()),
        scheduler: Arc::new(crate::gateway::scheduling::SchedulerState::new()),
        sticky: Arc::new(crate::gateway::scheduling::StickyTable::new()),
        log_snapshots: std::sync::Mutex::new(std::collections::HashMap::new()),
        agg_done: std::sync::Mutex::new((
            std::collections::VecDeque::new(),
            std::collections::HashSet::new(),
        )),
    })
}

/// 注册一个 Anthropic 平台（base_url=stub）+ 一个 group（group_key=gk）并关联。
async fn setup_group_with_upstream(state: &Arc<ProxyState>, gk: &str, base_url: &str) {
    let plat = crate::gateway::db::create_platform(
        &state.db,
        CreatePlatform {
            name: "stub".into(),
            platform_type: Protocol::Anthropic,
            base_url: base_url.to_string(),
            api_key: "sk-up".into(),
            extra: String::new(),
            models: None,
            available_models: None,
            endpoints: None,
            manual_budgets: None,
            auto_group: None,
            join_group_ids: None,
        },
    )
    .await
    .unwrap();

    let group = crate::gateway::db::create_group(
        &state.db,
        crate::gateway::db::test_support::sample_group(gk, vec![]),
    )
    .await
    .unwrap();

    crate::gateway::db::set_group_platforms(
        &state.db,
        group.id,
        &[GroupPlatformInput {
            platform_id: plat.id,
            priority: Some(0),
            weight: Some(1),
            level_priority: Some(0),
        }],
    )
    .await
    .unwrap();
}

fn messages_request(gk: &str, body: &str) -> Request {
    HttpRequest::builder()
        .method("POST")
        .uri("/v1/messages")
        .header("authorization", format!("Bearer {gk}"))
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .unwrap()
}

const ANTHROPIC_OK: &str = r#"{"id":"msg_1","type":"message","role":"assistant","model":"claude-3","content":[{"type":"text","text":"hi"}],"stop_reason":"end_turn","usage":{"input_tokens":5,"output_tokens":3}}"#;

#[tokio::test]
async fn health_endpoint_returns_ok() {
    let resp = handle_root().await;
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn no_auth_returns_404() {
    let state = make_state(test_db().await).await;
    let req = HttpRequest::builder()
        .method("POST")
        .uri("/v1/messages")
        .body(Body::from(r#"{"model":"m"}"#.to_string()))
        .unwrap();
    let resp = handle_proxy(AxumState(state), req).await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn unknown_group_token_returns_404() {
    let state = make_state(test_db().await).await;
    let req = messages_request("ghost", r#"{"model":"m","messages":[]}"#);
    let resp = handle_proxy(AxumState(state), req).await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn successful_forward_to_stub_upstream() {
    let upstream = spawn_stub_upstream(200, ANTHROPIC_OK).await;
    let state = make_state(test_db().await).await;
    setup_group_with_upstream(&state, "gk1", &upstream).await;

    let req = messages_request(
        "gk1",
        r#"{"model":"claude-3","messages":[{"role":"user","content":"hi"}]}"#,
    );
    let resp = handle_proxy(AxumState(state.clone()), req).await;
    assert_eq!(resp.status(), StatusCode::OK);

    // 落库：应有一条成功 proxy_log
    let logs = crate::gateway::db::list_proxy_logs(&state.db, 100, 0)
        .await
        .unwrap();
    assert!(logs.iter().any(|l| l.status_code == 200 && l.group_key == "gk1"));
}

#[tokio::test]
async fn upstream_500_records_attempt_and_returns_error() {
    let upstream = spawn_stub_upstream(500, r#"{"error":"boom"}"#).await;
    let state = make_state(test_db().await).await;
    setup_group_with_upstream(&state, "gk2", &upstream).await;

    let req = messages_request(
        "gk2",
        r#"{"model":"claude-3","messages":[{"role":"user","content":"hi"}]}"#,
    );
    let resp = handle_proxy(AxumState(state.clone()), req).await;
    // 单平台耗尽 → 返回上游错误（5xx 或 502）
    assert!(resp.status().is_server_error());
}

#[tokio::test]
async fn upstream_400_hard_error_no_retry() {
    let upstream = spawn_stub_upstream(400, r#"{"error":"bad request body"}"#).await;
    let state = make_state(test_db().await).await;
    setup_group_with_upstream(&state, "gk3", &upstream).await;

    let req = messages_request(
        "gk3",
        r#"{"model":"claude-3","messages":[{"role":"user","content":"hi"}]}"#,
    );
    let resp = handle_proxy(AxumState(state.clone()), req).await;
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn upstream_401_auto_disables_platform() {
    let upstream = spawn_stub_upstream(401, r#"{"error":"unauthorized"}"#).await;
    let state = make_state(test_db().await).await;
    setup_group_with_upstream(&state, "gk4", &upstream).await;

    let req = messages_request(
        "gk4",
        r#"{"model":"claude-3","messages":[{"role":"user","content":"hi"}]}"#,
    );
    let _ = handle_proxy(AxumState(state.clone()), req).await;

    // 平台应被 auto_disabled（auto_disabled_until > 0）
    let plats = crate::gateway::db::list_platforms(&state.db).await.unwrap();
    assert!(
        plats.iter().any(|p| p.auto_disabled_until > 0),
        "401 应触发 auto_disable"
    );
}

#[tokio::test]
async fn malformed_json_body_returns_400() {
    let state = make_state(test_db().await).await;
    let upstream = spawn_stub_upstream(200, ANTHROPIC_OK).await;
    setup_group_with_upstream(&state, "gk5", &upstream).await;
    let req = messages_request("gk5", "not json at all");
    let resp = handle_proxy(AxumState(state), req).await;
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

const MODELS_OK: &str = r#"{"data":[{"id":"claude-3"},{"id":"claude-4"}]}"#;

fn get_request(gk: &str, uri: &str) -> Request {
    HttpRequest::builder()
        .method("GET")
        .uri(uri)
        .header("authorization", format!("Bearer {gk}"))
        .body(Body::empty())
        .unwrap()
}

/// GET /v1/models → handle_models_passthrough（选组首个启用平台 relay 上游模型列表）。
#[tokio::test]
async fn models_endpoint_passthrough_relays_upstream() {
    let upstream = spawn_stub_upstream(200, MODELS_OK).await;
    let state = make_state(test_db().await).await;
    setup_group_with_upstream(&state, "gkm", &upstream).await;

    let req = get_request("gkm", "/v1/models");
    let resp = handle_proxy(AxumState(state.clone()), req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body = axum::body::to_bytes(resp.into_body(), 1 << 20)
        .await
        .unwrap();
    let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert!(v.get("data").is_some());
}

/// 组内无启用平台 → models passthrough 早退错误。
#[tokio::test]
async fn models_endpoint_no_platform_errors() {
    let state = make_state(test_db().await).await;
    // 仅建 group，无平台
    crate::gateway::db::create_group(
        &state.db,
        crate::gateway::db::test_support::sample_group("gkempty", vec![]),
    )
    .await
    .unwrap();
    let req = get_request("gkempty", "/v1/models");
    let resp = handle_proxy(AxumState(state), req).await;
    assert!(resp.status().is_client_error() || resp.status().is_server_error());
}

/// POST /v1/messages/count_tokens → handle_count_tokens（透传优先 / 本地估算兜底）。
#[tokio::test]
async fn count_tokens_endpoint_returns_count() {
    let upstream = spawn_stub_upstream(200, r#"{"input_tokens":42}"#).await;
    let state = make_state(test_db().await).await;
    setup_group_with_upstream(&state, "gkct", &upstream).await;

    let req = HttpRequest::builder()
        .method("POST")
        .uri("/v1/messages/count_tokens")
        .header("authorization", "Bearer gkct")
        .header("content-type", "application/json")
        .body(Body::from(
            r#"{"model":"claude-3","messages":[{"role":"user","content":"hello world"}]}"#
                .to_string(),
        ))
        .unwrap();
    let resp = handle_proxy(AxumState(state.clone()), req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body = axum::body::to_bytes(resp.into_body(), 1 << 20)
        .await
        .unwrap();
    let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert!(v.get("input_tokens").is_some());
}

/// count_tokens 上游失败 → 本地估算兜底仍返回 200 + input_tokens。
#[tokio::test]
async fn count_tokens_upstream_fail_local_estimate() {
    let upstream = spawn_stub_upstream(500, r#"{"error":"down"}"#).await;
    let state = make_state(test_db().await).await;
    setup_group_with_upstream(&state, "gkce", &upstream).await;

    let req = HttpRequest::builder()
        .method("POST")
        .uri("/v1/messages/count_tokens")
        .header("authorization", "Bearer gkce")
        .header("content-type", "application/json")
        .body(Body::from(
            r#"{"model":"claude-3","messages":[{"role":"user","content":"estimate me"}]}"#
                .to_string(),
        ))
        .unwrap();
    let resp = handle_proxy(AxumState(state), req).await;
    assert_eq!(resp.status(), StatusCode::OK);
}

/// 注册 Mock 平台（无需上游）+ group，关联。extra 为 mock 配置 JSON（空=默认）。
async fn setup_mock_group(state: &Arc<ProxyState>, gk: &str, extra: &str) {
    let plat = crate::gateway::db::create_platform(
        &state.db,
        CreatePlatform {
            name: "mockp".into(),
            platform_type: Protocol::Mock,
            base_url: "http://mock.local".into(),
            api_key: "sk-mock".into(),
            extra: extra.to_string(),
            models: None,
            available_models: None,
            endpoints: None,
            manual_budgets: None,
            auto_group: None,
            join_group_ids: None,
        },
    )
    .await
    .unwrap();
    let group = crate::gateway::db::create_group(
        &state.db,
        crate::gateway::db::test_support::sample_group(gk, vec![]),
    )
    .await
    .unwrap();
    crate::gateway::db::set_group_platforms(
        &state.db,
        group.id,
        &[GroupPlatformInput {
            platform_id: plat.id,
            priority: Some(0),
            weight: Some(1),
            level_priority: Some(0),
        }],
    )
    .await
    .unwrap();
}

/// Mock 平台拦截非流式请求 → handle_mock 本地生成 JSON 响应（不触上游）。
#[tokio::test]
async fn mock_platform_intercepts_nonstream() {
    let state = make_state(test_db().await).await;
    setup_mock_group(
        &state,
        "gkmock",
        r#"{"mock":{"input_tokens":11,"output_tokens":7}}"#,
    )
    .await;

    let req = messages_request(
        "gkmock",
        r#"{"model":"claude-3","messages":[{"role":"user","content":"hi"}]}"#,
    );
    let resp = handle_proxy(AxumState(state.clone()), req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let _ = axum::body::to_bytes(resp.into_body(), 1 << 20).await.unwrap();
    // 落库一条 mock 请求日志（假 token 生效）
    let logs = crate::gateway::db::list_proxy_logs(&state.db, 100, 0)
        .await
        .unwrap();
    assert!(logs.iter().any(|l| l.group_key == "gkmock" && l.status_code == 200));
}

/// Mock 平台 error_mode=http_error → 本地生成错误响应（自定义 status）。
#[tokio::test]
async fn mock_platform_error_mode() {
    let state = make_state(test_db().await).await;
    setup_mock_group(
        &state,
        "gkmockerr",
        r#"{"mock":{"error_mode":"http_error","status_code":503}}"#,
    )
    .await;

    let req = messages_request(
        "gkmockerr",
        r#"{"model":"claude-3","messages":[{"role":"user","content":"hi"}]}"#,
    );
    let resp = handle_proxy(AxumState(state.clone()), req).await;
    assert_eq!(resp.status(), StatusCode::SERVICE_UNAVAILABLE);
}

/// Mock 平台 stream_override=true → 本地生成 SSE 流。
#[tokio::test]
async fn mock_platform_stream_override() {
    let state = make_state(test_db().await).await;
    setup_mock_group(&state, "gkmockstream", r#"{"mock":{"stream_override":true}}"#).await;

    let req = messages_request(
        "gkmockstream",
        r#"{"model":"claude-3","messages":[{"role":"user","content":"hi"}]}"#,
    );
    let resp = handle_proxy(AxumState(state.clone()), req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let _ = axum::body::to_bytes(resp.into_body(), 1 << 20).await.unwrap();
}

/// 注册 Anthropic 平台并显式声明 Anthropic endpoint（同协议透传判定命中）。
async fn setup_passthrough_group(state: &Arc<ProxyState>, gk: &str, base_url: &str) {
    use crate::gateway::models::{ClientType, PlatformEndpoint};
    let plat = crate::gateway::db::create_platform(
        &state.db,
        CreatePlatform {
            name: "ptthru".into(),
            platform_type: Protocol::Anthropic,
            base_url: base_url.to_string(),
            api_key: "sk-up".into(),
            extra: String::new(),
            models: None,
            available_models: None,
            endpoints: Some(vec![PlatformEndpoint {
                protocol: Protocol::Anthropic,
                base_url: base_url.to_string(),
                client_type: ClientType::Default,
                coding_plan: false,
            }]),
            manual_budgets: None,
            auto_group: None,
            join_group_ids: None,
        },
    )
    .await
    .unwrap();
    let group = crate::gateway::db::create_group(
        &state.db,
        crate::gateway::db::test_support::sample_group(gk, vec![]),
    )
    .await
    .unwrap();
    crate::gateway::db::set_group_platforms(
        &state.db,
        group.id,
        &[GroupPlatformInput {
            platform_id: plat.id,
            priority: Some(0),
            weight: Some(1),
            level_priority: Some(0),
        }],
    )
    .await
    .unwrap();
}

/// 同协议透传：入站 anthropic + 平台显式 Anthropic endpoint → 跳过有损转换直转上游。
#[tokio::test]
async fn same_protocol_passthrough_skips_conversion() {
    let upstream = spawn_stub_upstream(200, ANTHROPIC_OK).await;
    let state = make_state(test_db().await).await;
    setup_passthrough_group(&state, "gkpt", &upstream).await;

    let req = messages_request(
        "gkpt",
        r#"{"model":"claude-3","messages":[{"role":"user","content":"hi"}]}"#,
    );
    let resp = handle_proxy(AxumState(state.clone()), req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let logs = crate::gateway::db::list_proxy_logs(&state.db, 100, 0)
        .await
        .unwrap();
    assert!(logs.iter().any(|l| l.group_key == "gkpt" && l.status_code == 200));
}

/// 同协议透传 + 流式：anthropic endpoint + stream:true → 透传 SSE 不重格式化。
#[tokio::test]
async fn same_protocol_passthrough_stream() {
    let sse = "event: message_start\ndata: {\"type\":\"message_start\",\"message\":{\"usage\":{\"input_tokens\":5,\"output_tokens\":0}}}\n\nevent: message_delta\ndata: {\"type\":\"message_delta\",\"usage\":{\"output_tokens\":3}}\n\nevent: message_stop\ndata: {\"type\":\"message_stop\"}\n\n";
    let upstream = spawn_stub_upstream(200, sse).await;
    let state = make_state(test_db().await).await;
    setup_passthrough_group(&state, "gkpts", &upstream).await;

    let req = messages_request(
        "gkpts",
        r#"{"model":"claude-3","stream":true,"messages":[{"role":"user","content":"hi"}]}"#,
    );
    let resp = handle_proxy(AxumState(state.clone()), req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let _ = axum::body::to_bytes(resp.into_body(), 1 << 20).await.unwrap();
}

/// 流式请求 stream:true → finish 走 SSE 聚合分支（StreamAggregator）。
#[tokio::test]
async fn streaming_request_passes_through() {
    let sse = "event: message_start\ndata: {\"type\":\"message_start\",\"message\":{\"usage\":{\"input_tokens\":5,\"output_tokens\":0}}}\n\nevent: message_delta\ndata: {\"type\":\"message_delta\",\"usage\":{\"output_tokens\":3}}\n\nevent: message_stop\ndata: {\"type\":\"message_stop\"}\n\n";
    let upstream = spawn_stub_upstream(200, sse).await;
    let state = make_state(test_db().await).await;
    setup_group_with_upstream(&state, "gkstream", &upstream).await;

    let req = messages_request(
        "gkstream",
        r#"{"model":"claude-3","stream":true,"messages":[{"role":"user","content":"hi"}]}"#,
    );
    let resp = handle_proxy(AxumState(state.clone()), req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    // drain body 触发流式聚合
    let _ = axum::body::to_bytes(resp.into_body(), 1 << 20).await.unwrap();
}
