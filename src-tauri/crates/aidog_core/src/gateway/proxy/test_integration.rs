//! 代理端到端集成测试：真 ProxyState（内存 DB）+ 本地 stub 上游 axum server，
//! 经 handle_proxy 全链路（handler→router→forward→finish→headers→log），
//! 覆盖成功转发 / 非 2xx failover / 早退分支（无 group 404 / bad body 400 / 健康端点）。

use super::*;
use aidog_db::test_support::test_db;
use aidog_middleware::MiddlewareEngine;
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

async fn make_state(db: aidog_db::Db) -> Arc<ProxyState> {
    let (log_tx, log_rx) = tokio::sync::mpsc::channel(1024);
    let state = Arc::new(ProxyState {
        db: Arc::new(db),
        app: None,
        middleware: Arc::new(MiddlewareEngine::new()),
        scheduler: Arc::new(crate::gateway::scheduling::SchedulerState::new()),
        sticky: Arc::new(crate::gateway::scheduling::StickyTable::new()),
        log_snapshots: dashmap::DashMap::new(),
        agg_done: std::sync::Mutex::new((
            std::collections::VecDeque::new(),
            std::collections::HashSet::new(),
        )),
        listen_addr: std::sync::OnceLock::new(),
        settings_cache: Arc::new(tokio::sync::RwLock::new(Default::default())),
        log_tx,
    });
    spawn_log_writer(state.clone(), log_rx);
    state
}

/// 注册一个 Anthropic 平台（base_url=stub）+ 一个 group（group_key=gk）并关联。
async fn setup_group_with_upstream(state: &Arc<ProxyState>, gk: &str, base_url: &str) {
    let plat = aidog_db::create_platform(
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

    let group = aidog_db::create_group(
        &state.db,
        aidog_db::test_support::sample_group(gk, vec![]),
    )
    .await
    .unwrap();

    aidog_db::set_group_platforms(
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

#[test]
fn hello_endpoint_matches_prefixed_paths() {
    assert!(is_hello_endpoint("/api/hello"));
    assert!(is_hello_endpoint("/proxy/api/hello"));
    assert!(is_hello_endpoint("/proxy/api/hello/")); // 容尾斜杠
    assert!(!is_hello_endpoint("/api/hello/world"));
    assert!(!is_hello_endpoint("/v1/messages"));
    assert!(!is_hello_endpoint("/api/group-info"));
}

/// Claude Code 启动预热探测 `HEAD /proxy/api/hello`（无 Authorization）：
/// 应 200 + 不落 proxy_log（修复前：resolve_group None → 404 且每次探测落一行）。
#[tokio::test]
async fn hello_probe_returns_ok_and_no_log() {
    let state = make_state(test_db().await).await;
    for method in ["HEAD", "GET"] {
        let req = HttpRequest::builder()
            .method(method)
            .uri("/proxy/api/hello")
            .body(Body::empty())
            .unwrap();
        let resp = handle_proxy(AxumState(state.clone()), req).await;
        assert_eq!(resp.status(), StatusCode::OK, "method={method}");
    }
    flush_log_queue(&state).await;
    let logs = aidog_logs::list_proxy_logs(&state.db, 100, 0).await.unwrap();
    assert!(logs.is_empty(), "预热探测不应落 proxy_log, got: {logs:?}");
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
    flush_log_queue(&state).await;
    let logs = aidog_logs::list_proxy_logs(&state.db, 100, 0)
        .await
        .unwrap();
    assert!(logs.iter().any(|l| l.status_code == 200 && l.group_key == "gkxapi"));
}

/// 复现 request 3ed5a698：客户端发 `disable_thinking: true`，上游（MiniMax-M2 这类内置思考
/// 模型）不认显式禁用，仍回 thinking 块。同协议透传路径此前原样下发 → 用户看到「禁用没生效」。
/// 修复后响应侧剥离 thinking 块（content 被剔空则补空 text 块，Anthropic 拒收空数组）。
#[tokio::test]
async fn disable_thinking_strips_thinking_from_passthrough_response() {
    const THINKING_ONLY: &str = r#"{"id":"msg_t","type":"message","role":"assistant","model":"MiniMax-M2","content":[{"type":"thinking","thinking":"让我分析…","signature":"sig"}],"stop_reason":"max_tokens","usage":{"input_tokens":726,"output_tokens":300}}"#;
    let upstream = spawn_stub_upstream(200, THINKING_ONLY).await;
    let state = make_state(test_db().await).await;
    setup_group_with_upstream(&state, "gkdt", &upstream).await;

    let req = messages_request(
        "gkdt",
        r#"{"model":"haiku","max_tokens":300,"disable_thinking":true,"messages":[{"role":"user","content":"hi"}]}"#,
    );
    let resp = handle_proxy(AxumState(state.clone()), req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body = axum::body::to_bytes(resp.into_body(), 1 << 20).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let content = json["content"].as_array().unwrap();
    assert!(
        content.iter().all(|b| b["type"] != "thinking"),
        "disable_thinking 时 thinking 块必须剥掉: {json}"
    );
    assert_eq!(content.len(), 1, "剔空后补空 text 块: {json}");
    assert_eq!(content[0]["type"], "text");
}

/// 未禁用思考时不动上游 thinking 块（避免上一条修复误伤正常思考链路）。
#[tokio::test]
async fn thinking_blocks_preserved_without_disable_flag() {
    const WITH_THINKING: &str = r#"{"id":"msg_t2","type":"message","role":"assistant","model":"m","content":[{"type":"thinking","thinking":"想","signature":"sig"},{"type":"text","text":"答案"}],"stop_reason":"end_turn","usage":{"input_tokens":5,"output_tokens":3}}"#;
    let upstream = spawn_stub_upstream(200, WITH_THINKING).await;
    let state = make_state(test_db().await).await;
    setup_group_with_upstream(&state, "gkkeep", &upstream).await;

    let req = messages_request("gkkeep", r#"{"model":"m","messages":[{"role":"user","content":"hi"}]}"#);
    let resp = handle_proxy(AxumState(state.clone()), req).await;
    let body = axum::body::to_bytes(resp.into_body(), 1 << 20).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["content"][0]["type"], "thinking", "{json}");
    assert_eq!(json["content"][1]["text"], "答案");
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
    flush_log_queue(&state).await;
    let logs = aidog_logs::list_proxy_logs(&state.db, 100, 0)
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
    flush_log_queue(&state).await;
    let plats = aidog_db::list_platforms(&state.db).await.unwrap();
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
    let plat = aidog_db::create_platform(
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
    let group = aidog_db::create_group(
        &state.db,
        aidog_db::test_support::sample_group(gk, vec![]),
    )
    .await
    .unwrap();
    aidog_db::set_group_platforms(
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
    flush_log_queue(&state).await;
    let logs = aidog_logs::list_proxy_logs(&state.db, 100, 0)
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

/// Mock 平台 error_rate=0.05 + error_mode=rate_limit_429 → 200 次真实请求中 429 比例落 5%±3%。
#[tokio::test]
async fn mock_platform_error_rate_ratio() {
    let state = make_state(test_db().await).await;
    setup_mock_group(
        &state,
        "gkmockrate",
        r#"{"mock":{"error_mode":"rate_limit_429","error_rate":0.05}}"#,
    )
    .await;

    let mut hits_429 = 0;
    for _ in 0..200 {
        let req = messages_request(
            "gkmockrate",
            r#"{"model":"claude-3","messages":[{"role":"user","content":"hi"}]}"#,
        );
        let resp = handle_proxy(AxumState(state.clone()), req).await;
        if resp.status() == StatusCode::TOO_MANY_REQUESTS {
            hits_429 += 1;
        } else {
            assert_eq!(resp.status(), StatusCode::OK);
        }
    }
    let ratio = hits_429 as f64 / 200.0;
    assert!((0.02..=0.08).contains(&ratio), "429 ratio {ratio} ({hits_429}/200) out of [0.02, 0.08]");
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

/// Mock 平台 ttft_ms/inter_chunk_ms 独立生效：首包时延 ≈ ttft_ms，chunk 间隔 ≈ inter_chunk_ms
/// （而非旧的 delay_ms 一值两用）。±50% 容差防 CI 抖动误报。
#[tokio::test]
async fn mock_platform_ttft_and_inter_chunk_split() {
    let state = make_state(test_db().await).await;
    setup_mock_group(
        &state,
        "gkttft",
        r#"{"mock":{"stream_override":true,"ttft_ms":80,"inter_chunk_ms":20,"chunk_count":3}}"#,
    )
    .await;

    let req = messages_request(
        "gkttft",
        r#"{"model":"claude-3","messages":[{"role":"user","content":"hi"}]}"#,
    );
    let t0 = std::time::Instant::now();
    let resp = handle_proxy(AxumState(state.clone()), req).await;
    assert_eq!(resp.status(), StatusCode::OK);

    let mut stream = resp.into_body().into_data_stream();
    let mut prev = t0;
    let mut ttft = None;
    let mut inter_chunk_gaps = vec![];
    while let Some(chunk) = stream.next().await {
        chunk.unwrap();
        let now = std::time::Instant::now();
        if ttft.is_none() {
            ttft = Some(now.duration_since(t0));
        } else {
            inter_chunk_gaps.push(now.duration_since(prev));
        }
        prev = now;
    }

    let ttft = ttft.unwrap();
    assert!(
        ttft.as_millis() >= 40 && ttft.as_millis() <= 160,
        "ttft={ttft:?} 应 ≈80ms±50%"
    );
    assert!(!inter_chunk_gaps.is_empty(), "应有 >1 个 chunk 才能测间隔");
    for gap in &inter_chunk_gaps {
        assert!(
            gap.as_millis() <= 60,
            "inter_chunk gap={gap:?} 应 ≈20ms（容差上限 60ms），不应回退到旧 delay_ms=0"
        );
    }
}

/// 只设 delay_ms（不设 ttft_ms/inter_chunk_ms）时行为与改动前一致：
/// 两者各自回落 delay_ms，首包与逐 chunk 间隔均 ≈ delay_ms。
#[tokio::test]
async fn mock_platform_delay_ms_only_backward_compat() {
    let state = make_state(test_db().await).await;
    setup_mock_group(
        &state,
        "gkdelay",
        r#"{"mock":{"stream_override":true,"delay_ms":30,"chunk_count":2}}"#,
    )
    .await;

    let req = messages_request(
        "gkdelay",
        r#"{"model":"claude-3","messages":[{"role":"user","content":"hi"}]}"#,
    );
    let t0 = std::time::Instant::now();
    let resp = handle_proxy(AxumState(state.clone()), req).await;
    assert_eq!(resp.status(), StatusCode::OK);

    let mut stream = resp.into_body().into_data_stream();
    let mut prev = t0;
    let mut ttft = None;
    let mut gaps = vec![];
    while let Some(chunk) = stream.next().await {
        chunk.unwrap();
        let now = std::time::Instant::now();
        if ttft.is_none() {
            ttft = Some(now.duration_since(t0));
        } else {
            gaps.push(now.duration_since(prev));
        }
        prev = now;
    }
    // 首包 = delay_ms（顶层 sleep）+ delay_ms（stream 首 chunk 前 sleep）≈ 60ms
    assert!(ttft.unwrap().as_millis() >= 40, "delay_ms-only 首包应含双重 30ms sleep");
    for gap in &gaps {
        assert!(gap.as_millis() >= 15 && gap.as_millis() <= 90, "gap={gap:?} 应 ≈30ms±");
    }
}

/// 注册 Anthropic 平台并显式声明 Anthropic endpoint（同协议透传判定命中）。
async fn setup_passthrough_group(state: &Arc<ProxyState>, gk: &str, base_url: &str) {
    use crate::gateway::models::PlatformEndpoint;
    let plat = aidog_db::create_platform(
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
                client_type: "default".to_string(),
                coding_plan: false,
            }]),
            manual_budgets: None,
            auto_group: None,
            join_group_ids: None, default_level_priority: None, expires_at: None,
        },
    )
    .await
    .unwrap();
    let group = aidog_db::create_group(
        &state.db,
        aidog_db::test_support::sample_group(gk, vec![]),
    )
    .await
    .unwrap();
    aidog_db::set_group_platforms(
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
    flush_log_queue(&state).await;
    let logs = aidog_logs::list_proxy_logs(&state.db, 100, 0)
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
    use crate::gateway::models::PlatformEndpoint;
    let plat = aidog_db::create_platform(
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
                client_type: "default".to_string(),
                coding_plan: false,
            }]),
            manual_budgets: None,
            auto_group: None,
            join_group_ids: None, default_level_priority: None, expires_at: None,
        },
    )
    .await
    .unwrap();
    let group = aidog_db::create_group(
        &state.db,
        aidog_db::test_support::sample_group(gk, vec![]),
    )
    .await
    .unwrap();
    aidog_db::set_group_platforms(
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
    flush_log_queue(&state).await;
    let logs = aidog_logs::list_proxy_logs(&state.db, 100, 0).await.unwrap();
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

    flush_log_queue(&state).await;
    let logs = aidog_logs::list_proxy_logs(&state.db, 100, 0).await.unwrap();
    let summary = logs.iter().find(|l| l.group_key == "gkrc").unwrap();
    let log = aidog_logs::get_proxy_log(&state.db, &summary.id)
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
    flush_log_queue(&state).await;
    let logs = aidog_logs::list_proxy_logs(&state.db, 100, 0).await.unwrap();
    assert!(logs.iter().any(|l| l.group_key == "gkrfb" && l.source_protocol == "openai_responses"));
}

/// 子端点：组内无任何 enabled 平台 → 503。
#[tokio::test]
async fn responses_subendpoint_no_platform_503() {
    let state = make_state(test_db().await).await;
    aidog_db::create_group(
        &state.db,
        aidog_db::test_support::sample_group("gkrempty", vec![]),
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
    aidog_db::create_group(
        &state.db,
        aidog_db::test_support::sample_group("gkn1", vec![]),
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
    aidog_db::create_group(
        &state.db,
        aidog_db::test_support::sample_group("gkn2", vec![]),
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
    aidog_db::create_group(
        &state.db,
        aidog_db::test_support::sample_group("gkn3", vec![]),
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

/// 纯函数：should_fallback_passthrough 三分支（MITM 直通 / 代理自身 host 不直通 / listen=None 保守）。
///
/// Bug B 修法后语义：host 判定前置，**不看 path**。host 非自身（MITM 灌入 / forward proxy）→ 恒 true；
/// host 自身（含错 token 探测代理）→ 恒 false（保留 404）。原「API path 在外部 host 也不直通」
/// 用例已删除（那是 Bug B 本身，违反 MITM 透明转发语义）；API path 在 MITM host 直通的场景
/// 由下方 `mitm_decrypted_api_path_falls_through_to_orig_host` 端到端覆盖。
#[test]
fn should_fallback_passthrough_decision_matrix() {
    use super::endpoint::should_fallback_passthrough;
    let listen = Some((std::net::IpAddr::V4(std::net::Ipv4Addr::new(127, 0, 0, 1)), 9892u16));

    // MITM 解密（Host = 外部域名，非代理自身）→ 直通，**不看 path**（Bug B 修法核心）
    assert!(should_fallback_passthrough("www.baidu.com", listen));
    assert!(should_fallback_passthrough("www.baidu.com:443", listen));
    // GLM 真实场景 host=open.bigmodel.cn → 直通（path 维度由 e2e 测试覆盖）
    assert!(should_fallback_passthrough("open.bigmodel.cn", listen));
    // 代理自身 host 直连 → 不直通（保留 404）
    assert!(!should_fallback_passthrough("127.0.0.1:9892", listen));
    assert!(!should_fallback_passthrough("localhost:9892", listen));
    // listen_addr = None（测试 / 未启动）→ 保守不直通
    assert!(!should_fallback_passthrough("www.baidu.com", None));
    // 0.0.0.0 bind：客户端通常连 127.0.0.1，视为自身
    let listen_lan = Some((std::net::IpAddr::V4(std::net::Ipv4Addr::new(0, 0, 0, 0)), 9892u16));
    assert!(!should_fallback_passthrough("127.0.0.1:9892", listen_lan));
    // 非 loopback、非 listen ip 的外部 host → MITM 解密灌入 → 直通
    assert!(should_fallback_passthrough("api.example.com", listen_lan));
    // loopback 名 + 端口不匹配 = 本机其他服务 → 允许直通（非 API 不破坏鉴权）
    assert!(should_fallback_passthrough("localhost:8080", listen));
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
    flush_log_queue(&state).await;
    let logs = aidog_logs::list_proxy_logs(&state.db, 100, 0).await.unwrap();
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
    flush_log_queue(&state).await;
    let logs = aidog_logs::list_proxy_logs(&state.db, 100, 0).await.unwrap();
    assert!(!logs.iter().any(|l| l.group_key == "未匹配"), "API path 未匹配不应进虚拟桶");
}

/// Bug B 核心修复回归：MITM 解密灌入 + API path（含 /v1/messages）+ 上游真实 key
/// → fallback 直通原 host（不再 404），proxy_log.request_url 含完整 url（host + path + query）。
///
/// 用户场景：智谱 anthropic 兼容端点 host=open.bigmodel.cn + path=/api/anthropic/v1/messages
/// + Authorization 上游真实 key（代理无对应 group）→ 修复前 is_api_endpoint 拦死 → 404；
/// 修复后 host 判定前置 → 透明转发原 host（stub 上游接收原始 Authorization + path）。
///
/// 用 spawn_stub_http_echo 起本地 stub，通过 raw TCP 发 absolute-form HTTP 请求（与
/// absolute_form_http_forward_returns_orig_body_not_health_endpoint 同款手法），target=stub。
/// absolute-form 保留 http scheme（MITM origin-form 无 scheme 默认 https 会失败连 HTTP stub）。
#[tokio::test]
async fn mitm_decrypted_api_path_falls_through_to_orig_host() {
    let state = make_state(test_db().await).await;
    // 代理监听端口与 stub 不同（stub 端口由 spawn 异步绑定），Host=stub 端口被判为「非自身」→ 直通。
    let _ = state.listen_addr.set((std::net::IpAddr::V4(std::net::Ipv4Addr::new(127, 0, 0, 1)), 65535u16));

    let stub_url = spawn_stub_http_echo("glm-anthropic-ok-sentinel").await;
    let proxy_url = spawn_proxy_router(state.clone()).await;

    // raw TCP 发 absolute-form HTTP POST：target=stub 的 /api/anthropic/v1/messages?beta=true。
    // reqwest/hyper 客户端发 HTTPS forward 会自动转 CONNECT；HTTP forward 走 absolute-form，
    // 但 reqwest 对 POST + .proxy() 可能转 CONNECT 不可控，故直接手写 h1 字节最贴近 curl -x。
    let req_bytes = format!(
        "POST {stub_url}/api/anthropic/v1/messages?beta=true HTTP/1.1\r\n\
         Host: {stub_authority}\r\n\
         Authorization: Bearer glm-real-upstream-key\r\n\
         Content-Type: application/json\r\n\
         Content-Length: {clen}\r\n\
         Connection: close\r\n\
         \r\n\
         {body}",
        stub_authority = stub_url.strip_prefix("http://").unwrap_or(&stub_url),
        clen = r#"{"model":"glm-4.6","messages":[]}"#.len(),
        body = r#"{"model":"glm-4.6","messages":[]}"#,
    );
    let proxy_addr = proxy_url.strip_prefix("http://").unwrap_or(&proxy_url);
    let mut stream = tokio::net::TcpStream::connect(proxy_addr).await.unwrap();
    tokio::io::AsyncWriteExt::write_all(&mut stream, req_bytes.as_bytes()).await.unwrap();

    // 读完整响应（Connection: close）。
    let mut resp_buf = Vec::with_capacity(4096);
    tokio::io::AsyncReadExt::read_to_end(&mut stream, &mut resp_buf).await.ok();
    let resp_str = String::from_utf8_lossy(&resp_buf);
    // Bug B 修复前：is_api_endpoint 拦死 → 404 no matching group；
    // Bug B 修复后：host 判定前置 → 透明转发 stub → 200 + stub 原文。
    assert!(
        resp_str.contains("200 OK") || resp_str.contains("HTTP/1.1 200"),
        "Bug B 修复：MITM + API path 须透明转发 stub 返 200（修复前 404），实际: {resp_str}"
    );
    assert!(
        resp_str.contains("glm-anthropic-ok-sentinel"),
        "必须返回 stub 原文（透明转发），实际: {resp_str}"
    );

    // proxy_log 落虚拟桶 + 完整 url（host + path + query）。
    flush_log_queue(&state).await;
    let logs = aidog_logs::list_proxy_logs(&state.db, 50, 0).await.unwrap();
    let bucket = logs.iter().find(|l| l.group_key == "未匹配")
        .expect("虚拟桶 proxy_log 应落库");
    assert_eq!(bucket.platform_id, 0);
    assert_eq!(bucket.status_code, 200);

    // Bug A：request_url 含 scheme://host/path?query 完整 url（不再是 origin-form path-only）。
    let full = aidog_logs::get_proxy_log(&state.db, &bucket.id).await.unwrap().unwrap();
    let stub_authority = stub_url.strip_prefix("http://").unwrap_or(&stub_url);
    assert!(
        full.request_url.contains(stub_authority),
        "request_url 含 host（Bug A 修复），实际: {}",
        full.request_url
    );
    assert!(
        full.request_url.contains("/api/anthropic/v1/messages?beta=true"),
        "request_url 含 path+query，实际: {}",
        full.request_url
    );
    assert!(
        full.request_url.starts_with("http://"),
        "scheme 自适应：absolute-form http → upstream 用 http://，实际: {}",
        full.request_url
    );
    assert!(
        full.upstream_request_url == full.request_url,
        "fallback 直通 upstream_request_url 应等同 request_url，实际: upstream={} request={}",
        full.upstream_request_url, full.request_url
    );
}

// ── forward proxy absolute-form 测试 ──
//
// absolute-form URI（`GET http://host/path`）必须绕过 `.route("/")` 健康端点进 handle_proxy
// 走 fallback 直通（与 MITM 解密灌入同语义）。reverse proxy path-only URI 不受影响。
//
// ponytail: 用 `build_router` 起 axum::serve 真 listener，reqwest 客户端配 .proxy() 发
// absolute-form 请求（reqwest 明文 HTTP 走 absolute-form，非 CONNECT），最贴近 curl `-x` 行为。

/// 起一个 stub HTTP 上游，所有方法/path 返回固定 200 + body（含 sentinel 便于断言）。
async fn spawn_stub_http_echo(body: &'static str) -> String {
    let app = axum::Router::new().fallback(axum::routing::any(move || async move {
        (axum::http::StatusCode::OK, [("content-type", "text/plain")], body)
    }));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.ok() });
    format!("http://127.0.0.1:{port}", port = addr.port())
}

/// 起一个代理 Router（build_router + axum::serve），返回 base_url + 共享 state。
async fn spawn_proxy_router(state: Arc<ProxyState>) -> String {
    let app = super::build_router(state.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.ok() });
    format!("http://127.0.0.1:{port}", port = addr.port())
}

/// 用 reqwest Client 配 .proxy() 发 absolute-form HTTP forward 请求。
/// reqwest 明文 HTTP + .proxy() → 客户端发 `GET http://host/path HTTP/1.1`（非 CONNECT）。
async fn forward_proxy_get(proxy_url: &str, target_url: &str) -> reqwest::Response {
    let client = reqwest::Client::builder()
        .proxy(reqwest::Proxy::http(proxy_url).unwrap())
        .build()
        .unwrap();
    client.get(target_url).send().await.unwrap()
}

/// absolute-form HTTP forward：`curl -x http://proxy http://stub/` → 返回 stub 原始正文，
/// **不**命中 `.route("/")` 健康端点 JSON。proxy_log 落虚拟桶。
#[tokio::test]
async fn absolute_form_http_forward_returns_orig_body_not_health_endpoint() {
    let state = make_state(test_db().await).await;
    // listen_addr 设为代理自身 loopback（任意端口均可，识别代理自身用）；具体端口由
    // spawn_proxy_router 异步绑定，should_fallback_passthrough 用 loopback host 名 + 端口
    // 比对识别。这里设一个**非** stub 端口的 loopback 地址，让外部 host（stub）被判为「非自身」。
    let _ = state.listen_addr.set((std::net::IpAddr::V4(std::net::Ipv4Addr::new(127, 0, 0, 1)), 65535u16));

    let stub_url = spawn_stub_http_echo("baidu-orig-html-sentinel").await;
    let proxy_url = spawn_proxy_router(state.clone()).await;

    let resp = forward_proxy_get(&proxy_url, &stub_url).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body = resp.text().await.unwrap();
    assert!(
        body.contains("baidu-orig-html-sentinel"),
        "absolute-form forward 必须返回 stub 原始正文（非健康端点 JSON），实际: {body}"
    );
    assert!(
        !body.contains("\"service\":\"aidog\""),
        "absolute-form forward 不应命中健康端点 JSON，实际: {body}"
    );

    // proxy_log 落虚拟桶（group_key=未匹配 / cost=0 / source_protocol=passthrough_unmatched）。
    flush_log_queue(&state).await;
    let logs = aidog_logs::list_proxy_logs(&state.db, 50, 0).await.unwrap();
    let bucket = logs.iter().find(|l| l.group_key == "未匹配");
    assert!(bucket.is_some(), "absolute-form forward 必须落虚拟桶 proxy_log");
    let b = bucket.unwrap();
    assert_eq!(b.platform_id, 0, "虚拟桶 platform_id=0");
    assert_eq!(b.status_code, 200, "上游 stub 返 200 → 终态 200");
    assert_eq!(b.source_protocol, "passthrough_unmatched");
    // 取完整行查 upstream_request_url（summary 不含此字段）。
    let full = aidog_logs::get_proxy_log(&state.db, &b.id).await.unwrap().unwrap();
    assert_eq!(full.est_cost, 0.0, "虚拟桶不计费");
    assert!(
        full.upstream_request_url.contains("127.0.0.1"),
        "上游 URL 指向 stub，实际: {}",
        full.upstream_request_url
    );
    assert!(
        full.upstream_request_url.starts_with("http://"),
        "scheme 自适应：明文 HTTP absolute-form → http:// upstream，实际: {}",
        full.upstream_request_url
    );
}

/// reverse proxy 健康端点不回归：path-only URI（`GET /`）仍命中 `.route("/")` 健康端点返 JSON，
/// middleware 不误判（path-only 无 scheme/host → next.run 进正常路由）。
#[tokio::test]
async fn path_only_uri_still_hits_health_endpoint_no_regression() {
    let state = make_state(test_db().await).await;
    let _ = state.listen_addr.set((std::net::IpAddr::V4(std::net::Ipv4Addr::new(127, 0, 0, 1)), 65535u16));

    let proxy_url = spawn_proxy_router(state.clone()).await;

    // 直接 GET proxy_url 的 `/`（path-only URI），不配 forward proxy。
    let resp = reqwest::Client::new().get(&proxy_url).send().await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = resp.text().await.unwrap();
    assert!(
        body.contains("\"service\":\"aidog\""),
        "path-only GET / 必须命中健康端点返 aidog JSON（不回归），实际: {body}"
    );

    // 健康端点不落 proxy_log（跳过日志）。
    flush_log_queue(&state).await;
    let logs = aidog_logs::list_proxy_logs(&state.db, 50, 0).await.unwrap();
    assert!(logs.is_empty(), "健康端点不落 proxy_log，实际: {logs:?}");
}

/// absolute-form HTTPS URI（`GET https://host/path`，非 CONNECT）→ middleware 识别后
/// handle_proxy fallback 直通；scheme 自适应 → upstream_request_url 用 https://。
///
/// 现实客户端走 HTTPS forward 普遍发 CONNECT 隧道（CONNECT handler 独立路径），但 absolute-form
/// HTTPS URI 的协议解析逻辑与 HTTP 同构（middleware 仅识别 scheme+host 即转），此用例锁 URI 构造语义。
/// 用 raw TCP 手发 absolute-form HTTPS URI（避免 reqwest 自动转 CONNECT），stub 上游用 HTTP
/// 模拟（fallback 直通构造 https:// URL，TLS 握手失败 → 502，但 proxy_log upstream_request_url
/// 已落库，可断言 scheme 自适应）。
#[tokio::test]
async fn absolute_form_https_uri_scheme_adaptive() {
    let state = make_state(test_db().await).await;
    let _ = state.listen_addr.set((std::net::IpAddr::V4(std::net::Ipv4Addr::new(127, 0, 0, 1)), 65535u16));

    let app = super::build_router(state.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let proxy_addr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.ok() });

    // raw TCP 发 absolute-form HTTPS URI（`GET https://example.invalid/ HTTP/1.1`）。
    // 手写 h1 请求：reqwest/hyper 客户端发 HTTPS forward 会自动转 CONNECT，禁用之需 raw 字节。
    let req_bytes = b"GET https://example.invalid/path HTTP/1.1\r\n\
Host: example.invalid\r\n\
Connection: close\r\n\
\r\n";
    let mut stream = tokio::net::TcpStream::connect(proxy_addr).await.unwrap();
    tokio::io::AsyncWriteExt::write_all(&mut stream, req_bytes).await.unwrap();

    // 读完整响应（Connection: close 上游会关连接）。
    let mut resp_buf = Vec::with_capacity(4096);
    tokio::io::AsyncReadExt::read_to_end(&mut stream, &mut resp_buf).await.ok();
    let resp_str = String::from_utf8_lossy(&resp_buf);
    // example.invalid 不可达 → 502（fallback 直通构造 https://example.invalid/path 失败）。
    assert!(
        resp_str.contains("502") || resp_str.contains("Bad Gateway"),
        "absolute-form HTTPS → 不可达 host 返 502，实际: {resp_str}"
    );

    // proxy_log upstream_request_url 必须以 https:// 开头（scheme 自适应生效）。
    flush_log_queue(&state).await;
    let logs = aidog_logs::list_proxy_logs(&state.db, 50, 0).await.unwrap();
    let bucket = logs.iter().find(|l| l.group_key == "未匹配");
    let b = bucket.expect("absolute-form HTTPS 必须落虚拟桶 proxy_log");
    let full = aidog_logs::get_proxy_log(&state.db, &b.id).await.unwrap().unwrap();
    assert!(
        full.upstream_request_url.starts_with("https://"),
        "HTTPS absolute-form scheme 自适应 → upstream URL 用 https://，实际: {}",
        full.upstream_request_url
    );
    assert!(
        full.upstream_request_url.contains("example.invalid"),
        "upstream URL 含目标 host，实际: {}",
        full.upstream_request_url
    );
}

// ──────────────────────────────────────────────────────────────
// transport 瞬断：同平台重试 + 流式截断终态
// ──────────────────────────────────────────────────────────────

/// 起一个「前 `fail_first` 个连接直接掐、之后正常返 200」的上游。
/// 模拟中转站（如 cometapi）把已建立的连接中途掐断这一主要故障形态。
async fn spawn_flaky_upstream(fail_first: usize) -> String {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let seen = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    tokio::spawn(async move {
        while let Ok((mut stream, _)) = listener.accept().await {
            let n = seen.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            if n < fail_first {
                drop(stream);
                continue;
            }
            tokio::spawn(async move {
                let mut buf = [0u8; 8192];
                let _ = stream.read(&mut buf).await;
                let resp = format!(
                    "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{ANTHROPIC_OK}",
                    ANTHROPIC_OK.len()
                );
                let _ = stream.write_all(resp.as_bytes()).await;
                let _ = stream.flush().await;
            });
        }
    });
    format!("http://{addr}")
}

/// 起一个「答半截 SSE 就断连」的上游：手写 chunked 响应，发一个 message_start 事件后
/// 不发结束 chunk 直接关连接 → reqwest 在 bytes_stream 中途返 Err（上游截断）。
async fn spawn_truncating_sse_upstream() -> String {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        while let Ok((mut stream, _)) = listener.accept().await {
            tokio::spawn(async move {
                let mut buf = [0u8; 8192];
                let _ = stream.read(&mut buf).await;
                let head = "HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\ntransfer-encoding: chunked\r\n\r\n";
                let _ = stream.write_all(head.as_bytes()).await;
                let ev = "event: message_start\ndata: {\"type\":\"message_start\",\"message\":{\"id\":\"m1\",\"model\":\"claude-3\",\"usage\":{\"input_tokens\":7,\"output_tokens\":0}}}\n\n";
                let _ = stream
                    .write_all(format!("{:x}\r\n{ev}\r\n", ev.len()).as_bytes())
                    .await;
                let _ = stream.flush().await;
                // 不发 "0\r\n\r\n" 结束 chunk，直接断 → 对端 body 读到一半报错。
                drop(stream);
            });
        }
    });
    format!("http://{addr}")
}

/// 单平台组（无 failover 候选）遇上游掐连接：同平台原地重试后成功，客户端拿 200 而非 502。
/// 修复前 is_last_candidate=true 直接落 502（DB 实证 2763/2771 条 502 的 retry_count 全为 0）。
#[tokio::test]
async fn transport_error_retries_same_platform_then_succeeds() {
    let upstream = spawn_flaky_upstream(1).await;
    let state = make_state(test_db().await).await;
    setup_group_with_upstream(&state, "gkflaky", &upstream).await;

    let req = messages_request(
        "gkflaky",
        r#"{"model":"claude-3","messages":[{"role":"user","content":"hi"}]}"#,
    );
    let resp = handle_proxy(AxumState(state.clone()), req).await;
    assert_eq!(
        resp.status(),
        StatusCode::OK,
        "首连被掐 → 同平台重试应救回 200（单平台组无 failover 候选）"
    );

    flush_log_queue(&state).await;
    let logs = aidog_logs::list_proxy_logs(&state.db, 100, 0).await.unwrap();
    let row = logs
        .iter()
        .find(|l| l.group_key == "gkflaky")
        .expect("必须落一条 proxy_log");
    assert_eq!(row.status_code, 200);
    assert!(
        row.retry_count >= 1,
        "重试次数必须记账（供 UI 看出上游抖动），实际 retry_count={}",
        row.retry_count
    );
}

/// 重试次数用尽仍被掐 → 502，且错误文本带 source 链（不再只有裸
/// "error sending request for url (...)"，那句在 2771 条 502 里完全一致、零诊断价值）。
#[tokio::test]
async fn transport_error_exhausted_logs_error_chain() {
    let upstream = spawn_flaky_upstream(usize::MAX).await;
    let state = make_state(test_db().await).await;
    setup_group_with_upstream(&state, "gkdead", &upstream).await;

    let req = messages_request(
        "gkdead",
        r#"{"model":"claude-3","messages":[{"role":"user","content":"hi"}]}"#,
    );
    let resp = handle_proxy(AxumState(state.clone()), req).await;
    assert_eq!(resp.status(), StatusCode::BAD_GATEWAY);

    flush_log_queue(&state).await;
    let logs = aidog_logs::list_proxy_logs(&state.db, 100, 0).await.unwrap();
    let summary = logs
        .iter()
        .find(|l| l.group_key == "gkdead")
        .expect("必须落一条 proxy_log");
    let full = aidog_logs::get_proxy_log(&state.db, &summary.id)
        .await
        .unwrap()
        .unwrap();
    // 断言落在 attempts.error：response_body / user_response_body 受 log_upstream_request /
    // log_user_request 开关控制（测试默认全关即清空），attempts 是不受开关影响的解析后元数据。
    // err_chain 展开后至少两层（顶层 "error sending request for url (...)" + hyper/io 层原因）。
    let last = full.attempts.last().expect("必须记录 attempt");
    assert!(
        last.error.matches(": ").count() >= 3,
        "502 错误必须展开 source 链（\"upstream error: 顶层: 次层\"），实际: {}",
        last.error
    );
}

/// 上游流中途被掐 → 终态 502，不再谎报 200。
/// 修复前 flush 硬编码 status_code=200（stream.rs），DB 实证 platform 4 的 3370 条流式 200 里
/// 2231 条 output_tokens=0 且 body 停在半截 JSON —— 全是被掐断却记成成功的流。
#[tokio::test]
async fn truncated_stream_logs_502_not_200() {
    let upstream = spawn_truncating_sse_upstream().await;
    let state = make_state(test_db().await).await;
    setup_group_with_upstream(&state, "gktrunc", &upstream).await;

    let req = messages_request(
        "gktrunc",
        r#"{"model":"claude-3","stream":true,"messages":[{"role":"user","content":"hi"}]}"#,
    );
    let resp = handle_proxy(AxumState(state.clone()), req).await;
    // 已发出的 SSE 头/内容保留（不向客户端硬报错），终态判定落在日志侧。
    assert_eq!(resp.status(), StatusCode::OK);
    let _ = axum::body::to_bytes(resp.into_body(), 1 << 20).await.unwrap();

    // guard.flush 走 tokio::spawn 落库，给写队列一个调度窗口。
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    flush_log_queue(&state).await;
    let logs = aidog_logs::list_proxy_logs(&state.db, 100, 0).await.unwrap();
    let row = logs
        .iter()
        .find(|l| l.group_key == "gktrunc")
        .expect("必须落一条 proxy_log");
    assert_eq!(
        row.status_code, 502,
        "上游截断的流必须记 502（修复前恒 200）"
    );
    assert_eq!(
        row.input_tokens, 7,
        "截断前已聚合的 token 必须保留（记录不因失败而丢）"
    );
}

// ═══════════════ 票 09：Gemini 同协议透传的模型口径 ═══════════════

/// 记录每次上游请求 (uri, body) 的 stub 上游，返回 (base_url, 记录表)。
/// `spawn_stub_upstream` 丢弃请求体，无法断言「实际打到上游的是什么」。
async fn spawn_recording_upstream(
    body: &'static str,
) -> (String, Arc<std::sync::Mutex<Vec<(String, String)>>>) {
    use axum::routing::any;
    let rec: Arc<std::sync::Mutex<Vec<(String, String)>>> = Arc::new(std::sync::Mutex::new(Vec::new()));
    let rec_srv = rec.clone();
    let app = axum::Router::new().fallback(any(move |req: HttpRequest<Body>| {
        let rec = rec_srv.clone();
        async move {
            let uri = req.uri().to_string();
            let bytes = axum::body::to_bytes(req.into_body(), 1 << 20).await.unwrap_or_default();
            rec.lock().unwrap().push((uri, String::from_utf8_lossy(&bytes).to_string()));
            (
                axum::http::StatusCode::OK,
                [("content-type", "application/json")],
                body,
            )
        }
    }));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.ok();
    });
    (format!("http://{addr}"), rec)
}

const GEMINI_OK: &str = r#"{"candidates":[{"content":{"parts":[{"text":"hi"}],"role":"model"},"finishReason":"STOP","index":0}],"usageMetadata":{"promptTokenCount":5,"candidatesTokenCount":3,"totalTokenCount":8}}"#;

/// 注册 Gemini 平台 + 显式 Gemini endpoint（同协议透传命中），group 带
/// `gemini-src → gemini-target` 模型改写。
async fn setup_gemini_passthrough_group(state: &Arc<ProxyState>, gk: &str, base_url: &str) {
    use crate::gateway::models::{ModelMapping, PlatformEndpoint};
    let plat = aidog_db::create_platform(
        &state.db,
        CreatePlatform {
            name: "gem".into(),
            platform_type: Protocol::Gemini,
            base_url: base_url.to_string(),
            api_key: "sk-up".into(),
            extra: String::new(),
            models: None,
            available_models: None,
            endpoints: Some(vec![PlatformEndpoint {
                protocol: Protocol::Gemini,
                base_url: base_url.to_string(),
                client_type: "default".to_string(),
                coding_plan: false,
            }]),
            manual_budgets: None,
            auto_group: None,
            join_group_ids: None, default_level_priority: None, expires_at: None,
        },
    )
    .await
    .unwrap();
    let group = aidog_db::create_group(
        &state.db,
        aidog_db::test_support::sample_group(
            gk,
            vec![ModelMapping {
                source_model: "gemini-src".to_string(),
                target_platform_id: plat.id,
                target_model: "gemini-target".to_string(),
                request_timeout_secs: 0,
                connect_timeout_secs: 0,
            }],
        ),
    )
    .await
    .unwrap();
    aidog_db::set_group_platforms(
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

/// 票 09：gemini 同协议透传 + 路由改写模型 → 上游 URL path 的模型段是改写后的模型名，
/// 且 body 不含顶层 `model` 键（Gemini generateContent 规范无此字段，多送会被判未知字段）。
#[tokio::test]
async fn gemini_passthrough_uses_remapped_model_in_path_not_body() {
    let (upstream, rec) = spawn_recording_upstream(GEMINI_OK).await;
    let state = make_state(test_db().await).await;
    setup_gemini_passthrough_group(&state, "gkgem", &upstream).await;

    // 两种客户端形态都要成立：① 真实 gemini 形态（model 只在 URL path）；
    // ② body 里额外带了 model 键（该键须被剔除，不得原样带着旧模型名上送）。
    for inbound in [
        r#"{"contents":[{"role":"user","parts":[{"text":"hi"}]}]}"#,
        r#"{"model":"gemini-src","contents":[{"role":"user","parts":[{"text":"hi"}]}]}"#,
    ] {
        let req = HttpRequest::builder()
            .method("POST")
            .uri("/v1beta/models/gemini-src:generateContent")
            .header("authorization", "Bearer gkgem")
            .header("content-type", "application/json")
            .body(Body::from(inbound))
            .unwrap();
        let resp = handle_proxy(AxumState(state.clone()), req).await;
        assert_eq!(resp.status(), StatusCode::OK, "inbound={inbound}");
    }

    let captured = rec.lock().unwrap().clone();
    assert_eq!(captured.len(), 2, "上游应恰收到两次请求: {captured:?}");
    for (uri, body) in &captured {
        assert!(
            uri.contains("gemini-target") && !uri.contains("gemini-src"),
            "URL path 的模型段应是路由改写后的模型名: {uri}"
        );
        let sent: serde_json::Value = serde_json::from_str(body).expect("上游 body 应是合法 JSON");
        assert!(
            sent.get("model").is_none(),
            "gemini 出站 body 不应含顶层 model 键（客户端原体带的也须剔除）: {sent}"
        );
        assert_eq!(sent["contents"][0]["parts"][0]["text"], "hi", "透传体其余内容不动: {sent}");
    }
}
