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

/// 起一个「立即 reset 连接」的上游：accept 后立刻 drop TcpStream，reqwest 收到 connect
/// 错误 → handle_proxy 映射 502 Bad Gateway。替代原 `http://127.0.0.1:1` 死端口 TCP
/// （真发起 connect 占用 FD + 依赖宿主网络栈行为）。
async fn spawn_reset_upstream() -> String {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        // 每个 incoming 连接立即 drop → 对端收到 connection reset。
        while let Ok((stream, _)) = listener.accept().await {
            drop(stream);
        }
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
        listen_addr: std::sync::OnceLock::new(),
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
            join_group_ids: None, default_level_priority: None, expires_at: None,
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

/// Anthropic SDK / claude-cli 只发 x-api-key（无 Authorization）→ 也应解析到 group 并转发，不再 404。
#[tokio::test]
async fn x_api_key_resolves_group_and_forwards() {
    let upstream = spawn_stub_upstream(200, ANTHROPIC_OK).await;
    let state = make_state(test_db().await).await;
    setup_group_with_upstream(&state, "gkxapi", &upstream).await;

    let req = HttpRequest::builder()
        .method("POST")
        .uri("/v1/messages")
        .header("x-api-key", "gkxapi")
        .header("content-type", "application/json")
        .body(Body::from(
            r#"{"model":"claude-3","messages":[{"role":"user","content":"hi"}]}"#.to_string(),
        ))
        .unwrap();
    let resp = handle_proxy(AxumState(state.clone()), req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let logs = crate::gateway::db::list_proxy_logs(&state.db, 100, 0)
        .await
        .unwrap();
    assert!(logs.iter().any(|l| l.status_code == 200 && l.group_key == "gkxapi"));
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

fn get_request(gk: &str, uri: &str) -> Request {
    HttpRequest::builder()
        .method("GET")
        .uri(uri)
        .header("authorization", format!("Bearer {gk}"))
        .body(Body::empty())
        .unwrap()
}

/// GET /v1/models（含 group token）→ handle_models_static：openai 格式静态列表，不 relay 上游。
#[tokio::test]
async fn models_endpoint_returns_static_openai() {
    let state = make_state(test_db().await).await;
    setup_group_with_upstream(&state, "gkm", "http://unused.invalid").await;

    let req = get_request("gkm", "/v1/models");
    let resp = handle_proxy(AxumState(state.clone()), req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body = axum::body::to_bytes(resp.into_body(), 1 << 20)
        .await
        .unwrap();
    let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(v.get("object").and_then(|o| o.as_str()), Some("list"));
    let data = v.get("data").and_then(|d| d.as_array()).unwrap();
    assert!(data.iter().any(|m| m.get("id").and_then(|i| i.as_str()) == Some("claude-opus-4-8")));
}

/// GET /proxy/models 无 Authorization（tokenless）→ 200 + anthropic 格式静态列表（不再 404）。
#[tokio::test]
async fn models_endpoint_tokenless_returns_static_anthropic() {
    let state = make_state(test_db().await).await;
    let req = HttpRequest::builder()
        .method("GET")
        .uri("/proxy/models")
        .body(Body::empty())
        .unwrap();
    let resp = handle_proxy(AxumState(state), req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body = axum::body::to_bytes(resp.into_body(), 1 << 20)
        .await
        .unwrap();
    let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(v.get("has_more").and_then(|h| h.as_bool()), Some(false));
    let data = v.get("data").and_then(|d| d.as_array()).unwrap();
    assert!(data.iter().any(|m| m.get("type").and_then(|t| t.as_str()) == Some("model")));
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
            join_group_ids: None, default_level_priority: None, expires_at: None,
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
            join_group_ids: None, default_level_priority: None, expires_at: None,
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

// ──────────────────────────────────────────────────────────────
// /v1/responses 子端点：handle_responses_subendpoint 全链路
// ──────────────────────────────────────────────────────────────

/// 注册一个 OpenAI 平台 + 显式声明 OpenAIResponses endpoint(base_url=stub) + group 关联。
async fn setup_responses_group(state: &Arc<ProxyState>, gk: &str, base_url: &str) {
    use crate::gateway::models::{ClientType, PlatformEndpoint};
    let plat = crate::gateway::db::create_platform(
        &state.db,
        CreatePlatform {
            name: "respp".into(),
            platform_type: Protocol::OpenAI,
            base_url: base_url.to_string(),
            api_key: "sk-resp".into(),
            extra: String::new(),
            models: None,
            available_models: None,
            endpoints: Some(vec![PlatformEndpoint {
                protocol: Protocol::OpenAIResponses,
                base_url: base_url.to_string(),
                client_type: ClientType::Default,
                coding_plan: false,
            }]),
            manual_budgets: None,
            auto_group: None,
            join_group_ids: None, default_level_priority: None, expires_at: None,
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

fn responses_get(gk: &str, uri: &str) -> Request {
    HttpRequest::builder()
        .method("GET")
        .uri(uri)
        .header("authorization", format!("Bearer {gk}"))
        .body(Body::empty())
        .unwrap()
}

/// GET /v1/responses/{id} → handle_responses_subendpoint 选 OpenAIResponses 平台透传上游。
#[tokio::test]
async fn responses_subendpoint_get_relays_upstream() {
    let upstream = spawn_stub_upstream(200, r#"{"id":"resp_1","object":"response"}"#).await;
    let state = make_state(test_db().await).await;
    setup_responses_group(&state, "gkresp", &upstream).await;

    let req = responses_get("gkresp", "/v1/responses/resp_1");
    let resp = handle_proxy(AxumState(state.clone()), req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body = axum::body::to_bytes(resp.into_body(), 1 << 20).await.unwrap();
    let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(v.get("id").and_then(|x| x.as_str()), Some("resp_1"));

    // 落库：source/target_protocol = openai_responses
    let logs = crate::gateway::db::list_proxy_logs(&state.db, 100, 0).await.unwrap();
    assert!(logs.iter().any(|l| l.group_key == "gkresp"
        && l.source_protocol == "openai_responses"
        && l.status_code == 200));
}

/// POST /v1/responses/{id}/cancel(带 body) → 原样转发 body + method 保留。
#[tokio::test]
async fn responses_subendpoint_post_cancel_forwards_body() {
    let upstream = spawn_stub_upstream(200, r#"{"id":"resp_2","status":"cancelled"}"#).await;
    let state = make_state(test_db().await).await;
    setup_responses_group(&state, "gkrc", &upstream).await;

    let req = HttpRequest::builder()
        .method("POST")
        .uri("/v1/responses/resp_2/cancel")
        .header("authorization", "Bearer gkrc")
        .header("content-type", "application/json")
        .body(Body::from(r#"{"reason":"user"}"#.to_string()))
        .unwrap();
    let resp = handle_proxy(AxumState(state.clone()), req).await;
    assert_eq!(resp.status(), StatusCode::OK);

    let logs = crate::gateway::db::list_proxy_logs(&state.db, 100, 0).await.unwrap();
    let summary = logs.iter().find(|l| l.group_key == "gkrc").unwrap();
    let log = crate::gateway::db::get_proxy_log(&state.db, &summary.id)
        .await
        .unwrap()
        .unwrap();
    // URL 不重复拼 /v1（base_url 已含 /v1）
    assert!(log.upstream_request_url.ends_with("/responses/resp_2/cancel"));
    assert_eq!(log.source_protocol, "openai_responses");
}

/// 子端点上游 5xx → 透传上游状态码（不重试，记录真实 status）。
#[tokio::test]
async fn responses_subendpoint_upstream_error_passthrough() {
    let upstream = spawn_stub_upstream(404, r#"{"error":"not found"}"#).await;
    let state = make_state(test_db().await).await;
    setup_responses_group(&state, "gkr404", &upstream).await;

    let req = responses_get("gkr404", "/v1/responses/resp_missing");
    let resp = handle_proxy(AxumState(state.clone()), req).await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

/// 子端点回退路径：组内平台无 OpenAIResponses endpoint → 取首个 enabled 平台 base_url。
#[tokio::test]
async fn responses_subendpoint_fallback_first_enabled_platform() {
    let upstream = spawn_stub_upstream(200, r#"{"id":"resp_fb","object":"response"}"#).await;
    let state = make_state(test_db().await).await;
    // setup_group_with_upstream 注册的是 Anthropic 平台，无 OpenAIResponses endpoint → 走回退
    setup_group_with_upstream(&state, "gkrfb", &upstream).await;

    let req = responses_get("gkrfb", "/v1/responses/resp_fb");
    let resp = handle_proxy(AxumState(state.clone()), req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let logs = crate::gateway::db::list_proxy_logs(&state.db, 100, 0).await.unwrap();
    assert!(logs.iter().any(|l| l.group_key == "gkrfb" && l.source_protocol == "openai_responses"));
}

/// 子端点：组内无任何 enabled 平台 → 503。
#[tokio::test]
async fn responses_subendpoint_no_platform_503() {
    let state = make_state(test_db().await).await;
    crate::gateway::db::create_group(
        &state.db,
        crate::gateway::db::test_support::sample_group("gkrempty", vec![]),
    )
    .await
    .unwrap();
    let req = responses_get("gkrempty", "/v1/responses/resp_x");
    let resp = handle_proxy(AxumState(state), req).await;
    assert_eq!(resp.status(), StatusCode::SERVICE_UNAVAILABLE);
}

/// 子端点：上游不可达(连接被 reset) → 502 Bad Gateway。
#[tokio::test]
async fn responses_subendpoint_upstream_unreachable_502() {
    let state = make_state(test_db().await).await;
    // 用本地 reset stub 替代死端口 TCP（避免真发起 connect 占 FD + 依赖宿主网络栈）。
    let upstream = spawn_reset_upstream().await;
    setup_responses_group(&state, "gkrdead", &upstream).await;
    let req = responses_get("gkrdead", "/v1/responses/resp_x");
    let resp = handle_proxy(AxumState(state), req).await;
    assert_eq!(resp.status(), StatusCode::BAD_GATEWAY);
}

// ──────────────────────────────────────────────────────────────
// /api/notify：handle_notify 鉴权 + dispatch 全链路
// ──────────────────────────────────────────────────────────────

fn notify_headers(bearer: Option<&str>) -> axum::http::HeaderMap {
    let mut h = axum::http::HeaderMap::new();
    if let Some(b) = bearer {
        h.insert(
            "authorization",
            axum::http::HeaderValue::from_str(&format!("Bearer {b}")).unwrap(),
        );
    }
    h
}

/// notify 无 Authorization → 401。
#[tokio::test]
async fn notify_missing_auth_returns_401() {
    let state = make_state(test_db().await).await;
    let resp = handle_notify(AxumState(state), notify_headers(None), Bytes::from_static(b"{}")).await;
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

/// notify Bearer 空串 → 401。
#[tokio::test]
async fn notify_empty_bearer_returns_401() {
    let state = make_state(test_db().await).await;
    let resp = handle_notify(AxumState(state), notify_headers(Some("")), Bytes::from_static(b"{}")).await;
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

/// notify group_key 不存在 → 401（防任意 token 触发）。
#[tokio::test]
async fn notify_unknown_group_returns_401() {
    let state = make_state(test_db().await).await;
    let resp = handle_notify(
        AxumState(state),
        notify_headers(Some("ghost-key")),
        Bytes::from_static(b"{}"),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

/// notify 鉴权通过但 body 非法 JSON → 400。
#[tokio::test]
async fn notify_bad_body_returns_400() {
    let state = make_state(test_db().await).await;
    crate::gateway::db::create_group(
        &state.db,
        crate::gateway::db::test_support::sample_group("gkn1", vec![]),
    )
    .await
    .unwrap();
    let resp = handle_notify(
        AxumState(state),
        notify_headers(Some("gkn1")),
        Bytes::from_static(b"not json"),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

/// notify 成功路径：鉴权通过 + 合法 body → 200 + DispatchResult JSON（app=None 仅落 inbox/不弹窗）。
#[tokio::test]
async fn notify_success_dispatches_and_returns_result() {
    let state = make_state(test_db().await).await;
    crate::gateway::db::create_group(
        &state.db,
        crate::gateway::db::test_support::sample_group("gkn2", vec![]),
    )
    .await
    .unwrap();
    let body = Bytes::from_static(
        br#"{"type":"TaskComplete","content":"done","vars":{"project":"demo"}}"#,
    );
    let resp = handle_notify(AxumState(state.clone()), notify_headers(Some("gkn2")), body).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(resp.into_body(), 1 << 20).await.unwrap();
    let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    // DispatchResult 字段存在
    assert!(v.get("dispatched").is_some());
    assert!(v.get("inbox").is_some());
}

/// notify 走 event 字段(CC hook 路径) + 注入内置 {group}/{time} 变量 → 200。
#[tokio::test]
async fn notify_event_path_injects_builtin_vars() {
    let state = make_state(test_db().await).await;
    crate::gateway::db::create_group(
        &state.db,
        crate::gateway::db::test_support::sample_group("gkn3", vec![]),
    )
    .await
    .unwrap();
    // 仅 event，无 vars → 内置 group/time 注入分支命中
    let body = Bytes::from_static(br#"{"event":"Stop","content":"hello"}"#);
    let resp = handle_notify(AxumState(state), notify_headers(Some("gkn3")), body).await;
    assert_eq!(resp.status(), StatusCode::OK);
}

// ── fallback 直通：MITM 解密非 API 流量未匹配分组 → 直通原 host + 虚拟桶统计 ──

/// 纯函数：is_api_endpoint 覆盖清单（主路径 + responses 子端点 + count_tokens + models）。
#[test]
fn is_api_endpoint_covers_main_paths() {
    use super::endpoint::is_api_endpoint;
    // API paths
    assert!(is_api_endpoint("/v1/messages"));
    assert!(is_api_endpoint("/v1/messages/count_tokens"));
    assert!(is_api_endpoint("/v1/chat/completions"));
    assert!(is_api_endpoint("/v1/completions"));
    assert!(is_api_endpoint("/v1/responses"));
    assert!(is_api_endpoint("/v1/responses/resp_abc"));
    assert!(is_api_endpoint("/v1/responses/resp_abc/cancel"));
    assert!(is_api_endpoint("/v1/embeddings"));
    assert!(is_api_endpoint("/v1/models"));
    assert!(is_api_endpoint("/proxy/v1/messages"));
    // 非 API paths
    assert!(!is_api_endpoint("/"));
    assert!(!is_api_endpoint("/index.html"));
    assert!(!is_api_endpoint("/some/path"));
    assert!(!is_api_endpoint("/proxy/"));
}

/// 纯函数：should_fallback_passthrough 三分支（MITM 直通 / API 仍 404 / 代理自身 host 不直通）。
#[test]
fn should_fallback_passthrough_decision_matrix() {
    use super::endpoint::should_fallback_passthrough;
    let listen = Some((std::net::IpAddr::V4(std::net::Ipv4Addr::new(127, 0, 0, 1)), 9892u16));

    // MITM 解密（Host = www.baidu.com，非代理自身）+ 非 API → 直通
    assert!(should_fallback_passthrough("www.baidu.com", "/", listen));
    assert!(should_fallback_passthrough("www.baidu.com:443", "/index.html", listen));
    // API path → 不直通（仍 404），即使 Host 是外部
    assert!(!should_fallback_passthrough("www.baidu.com", "/v1/messages", listen));
    assert!(!should_fallback_passthrough("www.baidu.com", "/v1/chat/completions", listen));
    // 代理自身 host 直连 → 不直通（保留 404）
    assert!(!should_fallback_passthrough("127.0.0.1:9892", "/v1/messages", listen));
    assert!(!should_fallback_passthrough("127.0.0.1:9892", "/", listen));
    assert!(!should_fallback_passthrough("localhost:9892", "/", listen));
    // listen_addr = None（测试 / 未启动）→ 保守不直通
    assert!(!should_fallback_passthrough("www.baidu.com", "/", None));
    // 0.0.0.0 bind：客户端通常连 127.0.0.1，视为自身
    let listen_lan = Some((std::net::IpAddr::V4(std::net::Ipv4Addr::new(0, 0, 0, 0)), 9892u16));
    assert!(!should_fallback_passthrough("127.0.0.1:9892", "/", listen_lan));
    // 非 loopback、非 listen ip 的外部 host → MITM 解密灌入 → 直通
    assert!(should_fallback_passthrough("api.example.com", "/foo", listen_lan));
}

/// 无 Authorization + Host = 外部（MITM 解密灌入）+ 非 API path + 上游不可达
/// → fallback 直通原 host，上游失败返 502，proxy_log 落虚拟桶（group_key="未匹配" / cost=0）。
#[tokio::test]
async fn fallback_passthrough_mitm_unmatched_logs_virtual_bucket() {
    let state = make_state(test_db().await).await;
    // 设置 listen_addr（模拟 start_proxy 绑定后的状态）。
    let _ = state.listen_addr.set((std::net::IpAddr::V4(std::net::Ipv4Addr::new(127, 0, 0, 1)), 9892u16));

    // Host = 外部 host（模拟 MITM 解密灌入），path = /（非 API），无 Authorization。
    // 上游 https://nonexistent.invalid 必然 TLS/DNS 失败 → 502。
    let req = HttpRequest::builder()
        .method("GET")
        .uri("/")
        .header("host", "nonexistent.invalid")
        .body(Body::empty())
        .unwrap();
    let resp = handle_proxy(AxumState(state.clone()), req).await;
    // 上游不可达 → 502
    assert_eq!(resp.status(), StatusCode::BAD_GATEWAY);

    // 虚拟桶落库：group_key="未匹配"，platform_id=0，cost=0。
    let logs = crate::gateway::db::list_proxy_logs(&state.db, 100, 0).await.unwrap();
    let bucket = logs.iter().find(|l| l.group_key == "未匹配");
    assert!(bucket.is_some(), "虚拟桶 proxy_log 应落库 (group_key=未匹配), logs: {:?}", logs.iter().map(|l| &l.group_key).collect::<Vec<_>>());
    let b = bucket.unwrap();
    assert_eq!(b.platform_id, 0, "虚拟桶 platform_id=0");
    assert_eq!(b.status_code, 502, "上游不可达 → 502");
    assert_eq!(b.source_protocol, "passthrough_unmatched", "虚拟桶 source_protocol 标记");
}

/// API path + 错 token + Host = 代理自身 → 仍 404（不旁路直通）。
#[tokio::test]
async fn api_path_wrong_token_still_404_no_bypass() {
    let state = make_state(test_db().await).await;
    let _ = state.listen_addr.set((std::net::IpAddr::V4(std::net::Ipv4Addr::new(127, 0, 0, 1)), 9892u16));

    // 错 token + API path + 代理自身 host → 404，不进 fallback。
    let req = HttpRequest::builder()
        .method("POST")
        .uri("/v1/messages")
        .header("host", "127.0.0.1:9892")
        .header("authorization", "Bearer wrong-token")
        .header("content-type", "application/json")
        .body(Body::from(r#"{"model":"m","messages":[]}"#.to_string()))
        .unwrap();
    let resp = handle_proxy(AxumState(state.clone()), req).await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    // 不落虚拟桶
    let logs = crate::gateway::db::list_proxy_logs(&state.db, 100, 0).await.unwrap();
    assert!(!logs.iter().any(|l| l.group_key == "未匹配"), "API path 未匹配不应进虚拟桶");
}
