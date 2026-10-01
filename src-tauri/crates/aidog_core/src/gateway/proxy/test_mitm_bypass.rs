//! 票 10 验收测试：旁路流量进 mitm_log 不进 proxy_log、usage 蹭采样、token 观测无
//! body、失败计数读取接口、master switch gate、retention 清 body / 删整行。
use super::*;
use crate::gateway::models::ProxyLogSettings;
use aidog_db::test_support;
use aidog_logs::MitmLogInsert;
use aidog_middleware::MiddlewareEngine;
use axum::http::{HeaderMap, Method};

/// 测试用 ProxyState（内存 DB），同 test_connect::make_state。
async fn make_state() -> Arc<ProxyState> {
    let db = test_support::test_db().await;
    let (log_tx, log_rx) = tokio::sync::mpsc::channel(1024);
    let state = Arc::new(ProxyState {
        db: Arc::new(db),
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
        log_queue_bytes: std::sync::atomic::AtomicU64::new(0),
    });
    spawn_log_writer(state.clone(), log_rx);
    state
}

/// stub 上游：按路径回固定响应；`x-test-fail: 1` 头 → 400（token 失败计数用）。
/// 同 test_e2e_mitm::spawn_stub_upstream idiom（http stub，聚焦分流与落库断言）。
async fn spawn_stub_upstream() -> String {
    let app = axum::Router::new().fallback(axum::routing::any(
        |req: axum::http::Request<axum::body::Body>| async move {
            if req
                .headers()
                .get("x-test-fail")
                .and_then(|v| v.to_str().ok())
                .is_some_and(|v| v == "1")
            {
                return (StatusCode::BAD_REQUEST, "refresh failed").into_response();
            }
            match req.uri().path() {
                "/api/oauth/usage" => (
                    StatusCode::OK,
                    [("content-type", "application/json")],
                    r#"{"five_hour":{"utilization":61.0},"seven_day":{"utilization":12.4}}"#,
                )
                    .into_response(),
                "/api/oauth/profile" => (
                    StatusCode::OK,
                    [("content-type", "application/json")],
                    r#"{"email":"u@e.com","organization":{"rate_limit_tier":"default_claude_max_20x","subscription_status":"active"}}"#,
                )
                    .into_response(),
                "/v1/oauth/token" => (
                    StatusCode::OK,
                    [("content-type", "application/json")],
                    r#"{"access_token":"at_secret","refresh_token":"rt_secret"}"#,
                )
                    .into_response(),
                _ => (StatusCode::OK, [("content-type", "text/plain")], "ok").into_response(),
            }
        },
    ));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.ok();
    });
    format!("http://{addr}")
}

/// 读 mitm_log 全部行：(host, path, status_code, request_body, response_body, resp_bytes)。
async fn mitm_rows(state: &Arc<ProxyState>) -> Vec<(String, String, i32, String, String, i64)> {
    state
        .db
        .call_read_traced(None, std::panic::Location::caller(), |conn| {
            let mut stmt = conn.prepare(
                "SELECT host, path, status_code, request_body, response_body, resp_bytes FROM mitm_log ORDER BY id",
            )?;
            let rows = stmt
                .query_map([], |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        r.get::<_, String>(1)?,
                        r.get::<_, i32>(2)?,
                        r.get::<_, String>(3)?,
                        r.get::<_, String>(4)?,
                        r.get::<_, i64>(5)?,
                    ))
                })?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            Ok(rows)
        })
        .await
        .expect("read mitm_log")
}

async fn proxy_log_count(state: &Arc<ProxyState>) -> i64 {
    state
        .db
        .call_read_traced(None, std::panic::Location::caller(), |conn| {
            Ok(conn.query_row("SELECT COUNT(*) FROM proxy_log", [], |r| r.get(0))?)
        })
        .await
        .expect("count proxy_log")
}

fn settings_logging() -> ProxyLogSettings {
    ProxyLogSettings {
        enabled: true,
        log_upstream_request: true,
        ..Default::default()
    }
}

/// 分流判定单一真值源覆盖：Core / TokenObserve / UsageSample / Bypass 四类边界。
#[test]
fn classify_mitm_route_covers_boundaries() {
    use axum::http::Method as M;
    // AI API / hello / models 一律 Core（core 有专门 handler，分流会改变既有行为）。
    assert_eq!(
        classify_mitm_route("api.anthropic.com", "/v1/messages", &M::POST),
        MitmRoute::Core
    );
    assert_eq!(
        classify_mitm_route("api.anthropic.com", "/api/hello", &M::HEAD),
        MitmRoute::Core
    );
    assert_eq!(
        classify_mitm_route("api.anthropic.com", "/v1/models", &M::GET),
        MitmRoute::Core
    );
    // token 刷新：host + path 双精确匹配。
    assert_eq!(
        classify_mitm_route("platform.claude.com", "/v1/oauth/token", &M::POST),
        MitmRoute::TokenObserve
    );
    assert_eq!(
        classify_mitm_route("platform.claude.com", "/v1/oauth/other", &M::POST),
        MitmRoute::Bypass
    );
    // usage：GET 才采样（CC 实际形状），其余 method 落旁路。
    assert_eq!(
        classify_mitm_route("api.anthropic.com", "/api/oauth/usage", &M::GET),
        MitmRoute::UsageSample
    );
    assert_eq!(
        classify_mitm_route("api.anthropic.com", "/api/oauth/usage", &M::POST),
        MitmRoute::Bypass
    );
    // 其余 /api/oauth/*（profile / validate 等）+ 任意旁路 host → Bypass。
    assert_eq!(
        classify_mitm_route("api.anthropic.com", "/api/oauth/profile", &M::GET),
        MitmRoute::Bypass
    );
    assert_eq!(
        classify_mitm_route("browser-intake-us5-datadoghq.com", "/telemetry", &M::POST),
        MitmRoute::Bypass
    );
    assert_eq!(
        classify_mitm_route("mcp-proxy.anthropic.com", "/v1/mcp", &M::POST),
        MitmRoute::Bypass
    );
}

/// 票 10 核心验收：旁路 / usage / token 三类全走 mitm_log，proxy_log 零行；
/// token 行 body 恒空（含长期凭证）；usage 蹭出采样行；失败计数接口可读。
#[tokio::test]
async fn mitm_bypass_writes_mitm_log_not_proxy_log() {
    let state = make_state().await;
    let base = spawn_stub_upstream().await;
    let settings = settings_logging();

    // 1. 旁路（Datadog 遥测形状，body 应被记录）。
    let resp = handle_mitm_observed(
        &state,
        MitmRoute::Bypass,
        format!("{base}/telemetry"),
        "browser-intake-us5-datadoghq.com",
        "/telemetry",
        Method::POST,
        HeaderMap::new(),
        Bytes::from(r#"{"e":"metric"}"#),
        "g1",
        &settings,
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);

    // 2. usage 蹭采样。
    let resp = handle_mitm_observed(
        &state,
        MitmRoute::UsageSample,
        format!("{base}/api/oauth/usage"),
        "api.anthropic.com",
        "/api/oauth/usage",
        Method::GET,
        HeaderMap::new(),
        Bytes::new(),
        "g1",
        &settings,
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);

    // 3. token 刷新观测（请求 body 含假凭证，断言永不落库）。
    let resp = handle_mitm_observed(
        &state,
        MitmRoute::TokenObserve,
        format!("{base}/v1/oauth/token"),
        "platform.claude.com",
        "/v1/oauth/token",
        Method::POST,
        HeaderMap::new(),
        Bytes::from(r#"{"grant_type":"refresh_token","refresh_token":"rt_secret"}"#),
        "g1",
        &settings,
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);

    // proxy_log 零行（票 10 目标：旁路不污染统计）。
    assert_eq!(
        proxy_log_count(&state).await,
        0,
        "旁路流量必须不进 proxy_log"
    );

    // mitm_log 三行，逐行断言。
    let rows = mitm_rows(&state).await;
    assert_eq!(rows.len(), 3);

    let (telemetry_host, telemetry_path, st, req_body, resp_body, resp_bytes) = &rows[0];
    assert_eq!(telemetry_host, "browser-intake-us5-datadoghq.com");
    assert_eq!(telemetry_path, "/telemetry");
    assert_eq!(*st, 200);
    assert!(
        req_body.contains("metric"),
        "开关开启时旁路请求 body 应记录"
    );
    assert_eq!(resp_body, "ok");
    assert_eq!(*resp_bytes, 2);

    let (usage_host, usage_path, st, ..) = &rows[1];
    assert_eq!(usage_host, "api.anthropic.com");
    assert_eq!(usage_path, "/api/oauth/usage");
    assert_eq!(*st, 200);

    let (token_host, token_path, st, req_body, resp_body, _) = &rows[2];
    assert_eq!(token_host, "platform.claude.com");
    assert_eq!(token_path, "/v1/oauth/token");
    assert_eq!(*st, 200);
    assert!(
        req_body.is_empty() && resp_body.is_empty(),
        "token 行 body 两列必须恒空（含长期凭证），实际 req={req_body:?} resp={resp_body:?}"
    );
    assert!(
        !resp_body.contains("at_secret") && !resp_body.contains("rt_secret"),
        "凭证不得出现在任何落库列"
    );

    // usage 采样行：pct 为 0-100 百分数原值。
    let sample: (f64, f64, String) = state
        .db
        .call_read_traced(None, std::panic::Location::caller(), |conn| {
            Ok(conn.query_row(
                "SELECT five_hour_pct, seven_day_pct, raw FROM oauth_usage_sample",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )?)
        })
        .await
        .expect("read oauth_usage_sample");
    assert_eq!(sample.0, 61.0);
    assert_eq!(sample.1, 12.4);
    assert!(sample.2.contains("five_hour"));

    // 失败计数（票 12 消费接口）：1 次尝试 0 失败。
    let (attempts, failures) = aidog_logs::oauth_token_refresh_failures(&state.db, 0)
        .await
        .expect("oauth_token_refresh_failures");
    assert_eq!((attempts, failures), (1, 0));
}

/// token 刷新失败（400）→ 观测行 status=400 + 失败计数聚合（open bug #91703 场景）。
#[tokio::test]
async fn mitm_token_failure_counted() {
    let state = make_state().await;
    let base = spawn_stub_upstream().await;
    let settings = settings_logging();

    let mut headers = HeaderMap::new();
    headers.insert("x-test-fail", axum::http::HeaderValue::from_static("1"));
    let resp = handle_mitm_observed(
        &state,
        MitmRoute::TokenObserve,
        format!("{base}/v1/oauth/token"),
        "platform.claude.com",
        "/v1/oauth/token",
        Method::POST,
        headers,
        Bytes::from(r#"{"grant_type":"refresh_token"}"#),
        "g1",
        &settings,
    )
    .await;
    assert_eq!(
        resp.status(),
        StatusCode::BAD_REQUEST,
        "4xx 原样透传给客户端"
    );

    let rows = mitm_rows(&state).await;
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].2, 400, "失败行记上游真实状态码");
    assert!(rows[0].3.is_empty() && rows[0].4.is_empty());

    let (attempts, failures) = aidog_logs::oauth_token_refresh_failures(&state.db, 0)
        .await
        .expect("oauth_token_refresh_failures");
    assert_eq!((attempts, failures), (1, 1));
}

/// master switch 关 → mitm_log 元数据行**照落**（2026-09-28 用户口径：域名命中 MITM
/// 即记录，观测不受 proxy 日志总开关控制），仅 body 两列为空（正文跟随 log_upstream_request）。
#[tokio::test]
async fn mitm_bypass_master_switch_off_still_records_metadata() {
    let state = make_state().await;
    let base = spawn_stub_upstream().await;
    let settings = ProxyLogSettings {
        enabled: false,
        log_upstream_request: true,
        ..Default::default()
    };

    let resp = handle_mitm_observed(
        &state,
        MitmRoute::Bypass,
        format!("{base}/telemetry"),
        "browser-intake-us5-datadoghq.com",
        "/telemetry",
        Method::POST,
        HeaderMap::new(),
        Bytes::from("x"),
        "g1",
        &ProxyLogSettings {
            enabled: false,
            ..settings
        },
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK, "转发不受日志开关影响");

    let rows = mitm_rows(&state).await;
    assert_eq!(rows.len(), 1, "master switch 关时观测行仍必须落库");
    let (host, path, st, req_body, resp_body, _) = &rows[0];
    assert_eq!((host.as_str(), path.as_str(), *st), ("browser-intake-us5-datadoghq.com", "/telemetry", 200));
    // 正文跟随 log_upstream_request（本 fixture 为 true）——mitm 观测不受 proxy 总开关控制。
    assert!(!req_body.is_empty(), "log_upstream_request 开 → 请求正文照记（不受 master switch 影响）");
    assert!(!resp_body.is_empty());
}

/// 流式旁路（SSE）：字节透传 + Drop 兜底落行（resp_bytes 计数）。
#[tokio::test]
async fn mitm_bypass_streaming_counts_bytes_and_logs_on_drop() {
    let state = make_state().await;
    let app = axum::Router::new().fallback(|| async {
        (
            StatusCode::OK,
            [("content-type", "text/event-stream")],
            "data: {\"x\":1}\n\ndata: {\"x\":2}\n\n",
        )
    });
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.ok();
    });

    let resp = handle_mitm_observed(
        &state,
        MitmRoute::Bypass,
        format!("http://{addr}/sse"),
        "claude.ai",
        "/sse",
        Method::GET,
        HeaderMap::new(),
        Bytes::new(),
        "g1",
        &settings_logging(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);

    // 读完整流（触发 Drop 兜底 insert）。
    let body = axum::body::to_bytes(resp.into_body(), 1024 * 1024)
        .await
        .expect("read stream body");
    assert_eq!(body.len(), 30);

    // Drop 内是 tokio::spawn 落库：有界等待（测试专用轮询，上限 2s）。
    let mut logged = Vec::new();
    for _ in 0..100 {
        logged = mitm_rows(&state).await;
        if !logged.is_empty() {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    assert_eq!(logged.len(), 1, "流式旁路必须落 mitm_log 行");
    assert_eq!(logged[0].2, 200);
    assert_eq!(logged[0].5, 30, "resp_bytes 必须等于流字节数");
}

/// retention：body 对称清空（不删行）→ 整行删除（跟随 retention_days）。
#[tokio::test]
async fn mitm_log_retention_clears_bodies_then_rows() {
    let state = make_state().await;
    let old = MitmLogInsert {
        group_name: "g1".into(),
        host: "browser-intake-us5-datadoghq.com".into(),
        path: "/telemetry".into(),
        status_code: 200,
        req_bytes: 1,
        resp_bytes: 1,
        decrypted: true,
        request_body: "old-req".into(),
        response_body: "old-resp".into(),
        created_at: aidog_db::now() - 10 * 24 * 3600 * 1000, // 10 天前
    };
    aidog_logs::insert_mitm_log(&state.db, old)
        .await
        .expect("insert old row");

    // body 清空（upstream 侧口径，1 天）：行保留、body 空。
    aidog_logs::cleanup_mitm_log_bodies(&state.db, 1, aidog_db::models::RetentionUnit::Day)
        .await
        .expect("cleanup bodies");
    let rows = mitm_rows(&state).await;
    assert_eq!(rows.len(), 1);
    assert!(
        rows[0].3.is_empty() && rows[0].4.is_empty(),
        "超期 body 对称清空"
    );

    // 整行删除（retention_days=7 天）：行没了。
    aidog_logs::cleanup_mitm_logs(&state.db, 7, aidog_db::models::RetentionUnit::Day)
        .await
        .expect("cleanup rows");
    assert_eq!(mitm_rows(&state).await.len(), 0);
}

/// 增量（2026-09-28）：① `/api/oauth/*` 观测 body 跟配置——开关开=原文、关=空串，
/// 不再无条件 [REDACTED]；② profile 成功响应蹭采样 upsert cc_oauth_profile（latest-wins）。
#[tokio::test]
async fn mitm_oauth_meta_body_follows_config_and_profile_sampled() {
    let state = make_state().await;
    let base = spawn_stub_upstream().await;

    // 开关全开：body 记原文。
    let resp = handle_mitm_observed(
        &state,
        MitmRoute::Bypass,
        format!("{base}/api/oauth/profile"),
        "api.anthropic.com",
        "/api/oauth/profile",
        Method::GET,
        HeaderMap::new(),
        Bytes::new(),
        "g1",
        &settings_logging(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);

    // master 开、log_upstream_request 关：body 空串（对齐 proxy_log from_log 语义）。
    let resp = handle_mitm_observed(
        &state,
        MitmRoute::Bypass,
        format!("{base}/api/oauth/profile"),
        "api.anthropic.com",
        "/api/oauth/profile",
        Method::GET,
        HeaderMap::new(),
        Bytes::new(),
        "g1",
        &ProxyLogSettings {
            log_upstream_request: false,
            ..settings_logging()
        },
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);

    let rows = mitm_rows(&state).await;
    assert_eq!(rows.len(), 2);
    assert!(
        rows[0].4.contains("rate_limit_tier"),
        "开关开时 /api/oauth/* body 应记原文，实际 {:?}",
        rows[0].4
    );
    assert!(
        rows[1].4.is_empty(),
        "log_upstream_request 关时 body 应为空串，实际 {:?}",
        rows[1].4
    );

    // profile 蹭采样：两次请求只留一行（latest-wins），tier 为 organization.rate_limit_tier。
    let profile: (String, i64) = state
        .db
        .call_read_traced(None, std::panic::Location::caller(), |conn| {
            Ok(conn.query_row(
                "SELECT tier, COUNT(*) FROM cc_oauth_profile WHERE group_name = 'g1' GROUP BY tier",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )?)
        })
        .await
        .expect("read cc_oauth_profile");
    assert_eq!(profile, ("default_claude_max_20x".to_string(), 1));
}

/// Core 双写观测行（serve_plaintext Core 分支落，log_core_mitm_observed）：元数据 + 调用方
/// gate 后的 body；无上游依赖，直调 helper。
#[tokio::test]
async fn core_mitm_observed_row_written() {
    let state = make_state().await;
    log_core_mitm_observed(
        &state,
        "g1",
        "api.anthropic.com",
        "/v1/messages",
        200,
        120,
        340,
        r#"{"model":"claude-3"}"#.to_string(),
        r#"{"id":"msg"}"#.to_string(),
    )
    .await;
    let rows = mitm_rows(&state).await;
    assert_eq!(rows.len(), 1);
    let (host, path, st, req_body, resp_body, resp_bytes) = &rows[0];
    assert_eq!((host.as_str(), path.as_str(), *st), ("api.anthropic.com", "/v1/messages", 200));
    assert!(req_body.contains("claude-3"));
    assert!(resp_body.contains("msg"));
    assert_eq!(*resp_bytes, 340);
}
