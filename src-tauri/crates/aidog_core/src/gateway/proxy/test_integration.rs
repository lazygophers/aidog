//! 代理端到端集成测试：真 ProxyState（内存 DB）+ 本地 stub 上游 axum server，
//! 经 handle_proxy 全链路（handler→router→forward→finish→headers→log），
//! 覆盖成功转发 / 非 2xx failover / 早退分支（无 group 404 / bad body 400 / 健康端点）。

use super::*;
use crate::gateway::models::{CreatePlatform, GroupPlatformInput, Protocol};
use aidog_db::test_support::test_db;
use aidog_middleware::MiddlewareEngine;
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

/// 注册一个 Anthropic 平台（base_url=stub）+ 一个 group（group_key=gk）并关联。
async fn setup_group_with_upstream(state: &Arc<ProxyState>, gk: &str, base_url: &str) {
    setup_group_with_upstream_key(state, gk, base_url, "sk-up").await;
}

async fn setup_group_with_upstream_key(
    state: &Arc<ProxyState>,
    gk: &str,
    base_url: &str,
    api_key: &str,
) {
    let plat = aidog_db::create_platform(
        &state.db,
        CreatePlatform {
            name: "stub".into(),
            platform_type: Protocol::Anthropic,
            base_url: base_url.to_string(),
            api_key: api_key.into(),
            extra: String::new(),
            models: None,
            available_models: None,
            endpoints: None,
            manual_budgets: None,
            auto_group: None,
            join_group_ids: None,
            expires_at: None,
            quota_source: None,
        },
    )
    .await
    .unwrap();

    let group = aidog_db::create_group(&state.db, aidog_db::test_support::sample_group(gk, vec![]))
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
/// 应 200 + 落 request log（source_protocol=test），方便用户从请求日志页看到探测请求。
#[tokio::test]
async fn hello_probe_returns_ok_and_records_request_log() {
    let state = make_state(test_db().await).await;
    set_log_settings(
        &state,
        ProxyLogSettings {
            log_user_request: true,
            log_upstream_request: true,
            ..Default::default()
        },
    ).await;
    let mut ids = Vec::new();
    for method in ["HEAD", "GET"] {
        let req = HttpRequest::builder()
            .method(method)
            .uri("/proxy/api/hello")
            .body(Body::empty())
            .unwrap();
        let resp = handle_proxy(AxumState(state.clone()), req).await;
        assert_eq!(resp.status(), StatusCode::OK, "method={method}");
        flush_log_queue(&state).await;
        let logs = aidog_logs::list_proxy_logs(&state.db, 100, 0)
            .await
            .unwrap();
        let id = logs.last().expect("hello log missing").id.clone();
        ids.push(id);
    }
    assert_eq!(ids.len(), 2);
    for id in ids {
        let row = aidog_logs::get_proxy_log(&state.db, &id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(row.source_protocol, "test");
        assert_eq!(row.status_code, 200);
        assert!(row.done);
        assert!(row.request_url.ends_with("/proxy/api/hello"));
    }
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
    assert!(
        logs.iter()
            .any(|l| l.status_code == 200 && l.group_key == "gkxapi")
    );
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
    let body = axum::body::to_bytes(resp.into_body(), 1 << 20)
        .await
        .unwrap();
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

    let req = messages_request(
        "gkkeep",
        r#"{"model":"m","messages":[{"role":"user","content":"hi"}]}"#,
    );
    let resp = handle_proxy(AxumState(state.clone()), req).await;
    let body = axum::body::to_bytes(resp.into_body(), 1 << 20)
        .await
        .unwrap();
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
    assert!(
        logs.iter()
            .any(|l| l.status_code == 200 && l.group_key == "gk1")
    );
}

// ── 实际模型口径（2026-09-21）：非流式 actual_model 取上游自报模型名，缺失回退请求模型 ──
// 旧缺陷：actual_model 写的是路由 target_model（与请求一致时无感，映射/别名场景统计按
// 路由名聚合）。用户口径：统计按上游响应自报模型；上游没报才回退请求模型。
const NO_MODEL_OK: &str = r#"{"id":"msg_2","type":"message","role":"assistant","content":[{"type":"text","text":"hi"}],"stop_reason":"end_turn","usage":{"input_tokens":5,"output_tokens":3}}"#;

#[tokio::test]
async fn nonstream_actual_model_uses_upstream_served_model() {
    let upstream = spawn_stub_upstream(200, ANTHROPIC_OK).await; // 响应 model = "claude-3"
    let state = make_state(test_db().await).await;
    setup_group_with_upstream(&state, "gk_am1", &upstream).await;

    // 请求名与上游自报名不同：actual_model 必须取上游的，不能是请求名/路由名。
    let req = messages_request(
        "gk_am1",
        r#"{"model":"request-model","messages":[{"role":"user","content":"hi"}]}"#,
    );
    let resp = handle_proxy(AxumState(state.clone()), req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let log = last_log_for(&state, "gk_am1").await;
    assert_eq!(
        log.actual_model, "claude-3",
        "actual_model 应为上游自报模型"
    );
    assert_eq!(log.model, "request-model");
}

#[tokio::test]
async fn nonstream_actual_model_falls_back_to_requested_when_upstream_silent() {
    let upstream = spawn_stub_upstream(200, NO_MODEL_OK).await; // 响应无 model 字段
    let state = make_state(test_db().await).await;
    setup_group_with_upstream(&state, "gk_am2", &upstream).await;

    let req = messages_request(
        "gk_am2",
        r#"{"model":"request-model","messages":[{"role":"user","content":"hi"}]}"#,
    );
    let resp = handle_proxy(AxumState(state.clone()), req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let log = last_log_for(&state, "gk_am2").await;
    assert_eq!(
        log.actual_model, "request-model",
        "上游未自报模型时应回退请求模型"
    );
}

/// 写日志设置进 DB，并按生产同款路径重建 ProxyState 设置缓存（mod.rs start_proxy 同款 load_from）。
/// 只写 DB 不刷缓存 = 旧值仍生效（这正是要防的陈旧路径）。
async fn set_log_settings(state: &Arc<ProxyState>, settings: ProxyLogSettings) {
    aidog_db::set_setting(
        &state.db,
        crate::gateway::models::SetSettingInput {
            scope: "proxy".into(),
            key: "logging".into(),
            value: serde_json::to_value(&settings).unwrap(),
        },
    )
    .await
    .unwrap();
    *state.settings_cache.write().await =
        super::settings_cache::ProxySettingsCache::load_from(&state.db).await;
}

/// 发一条非流式请求后取该 group 最新终态日志行。
async fn last_log_for(state: &Arc<ProxyState>, gk: &str) -> aidog_db::models::ProxyLog {
    flush_log_queue(state).await;
    let logs = aidog_logs::list_proxy_logs(&state.db, 100, 0)
        .await
        .unwrap();
    let id = logs
        .iter()
        .find(|l| l.group_key == gk)
        .expect("log row missing")
        .id
        .clone();
    aidog_logs::get_proxy_log(&state.db, &id)
        .await
        .unwrap()
        .unwrap()
}

/// 开关矩阵（用户报告「上游实际请求没记录」的精确验收）：
/// log_upstream_request=true / log_user_request=false 时——
/// 上游侧（upstream_request_headers/body + response_body）非空；用户侧（request_body /
/// user_response_body）为空；auth 头脱敏。
#[tokio::test]
async fn log_switches_record_upstream_side_only() {
    let upstream = spawn_stub_upstream(200, ANTHROPIC_OK).await;
    let state = make_state(test_db().await).await;
    setup_group_with_upstream(&state, "gklogup", &upstream).await;
    set_log_settings(
        &state,
        ProxyLogSettings {
            log_user_request: false,
            log_upstream_request: true,
            ..Default::default()
        },
    )
    .await;

    let req = messages_request(
        "gklogup",
        r#"{"model":"claude-3","messages":[{"role":"user","content":"hi"}]}"#,
    );
    let resp = handle_proxy(AxumState(state.clone()), req).await;
    assert_eq!(resp.status(), StatusCode::OK);

    let row = last_log_for(&state, "gklogup").await;
    assert!(
        !row.upstream_request_headers.is_empty(),
        "上游请求头必须记录"
    );
    assert!(
        !row.upstream_request_headers.contains("sk-up"),
        "上游 auth 头必须脱敏"
    );
    assert!(
        !row.upstream_request_body.is_empty(),
        "上游请求体必须记录（用户报告缺失的字段）"
    );
    assert!(row.upstream_request_body.contains("\"model\""));
    assert!(
        !row.response_body.is_empty(),
        "上游响应正文属上游侧，必须记录"
    );
    assert!(
        row.request_body.is_empty(),
        "用户侧关闭时 request_body 必须为空"
    );
    assert!(
        row.user_response_body.is_empty(),
        "用户侧关闭时 user_response_body 必须为空"
    );
}

/// 对称矩阵：log_user_request=true / log_upstream_request=false——用户侧记录、上游侧清空。
#[tokio::test]
async fn log_switches_record_user_side_only() {
    let upstream = spawn_stub_upstream(200, ANTHROPIC_OK).await;
    let state = make_state(test_db().await).await;
    setup_group_with_upstream(&state, "gkloguser", &upstream).await;
    set_log_settings(
        &state,
        ProxyLogSettings {
            log_user_request: true,
            log_upstream_request: false,
            ..Default::default()
        },
    )
    .await;

    let req = messages_request(
        "gkloguser",
        r#"{"model":"claude-3","messages":[{"role":"user","content":"hi"}]}"#,
    );
    let resp = handle_proxy(AxumState(state.clone()), req).await;
    assert_eq!(resp.status(), StatusCode::OK);

    let row = last_log_for(&state, "gkloguser").await;
    assert!(
        !row.request_body.is_empty(),
        "用户侧开启时 request_body 必须记录"
    );
    assert!(
        !row.user_response_body.is_empty(),
        "用户侧开启时 user_response_body 必须记录"
    );
    assert!(row.upstream_request_body.is_empty(), "上游侧关闭时必须清空");
    assert!(
        row.response_body.is_empty(),
        "上游侧关闭时上游响应正文必须清空"
    );
    assert!(
        row.upstream_request_headers.is_empty(),
        "上游侧关闭时上游请求头必须清空"
    );
}

/// 用户侧 request_headers 脱敏（db-log-redact-hardening 票 01）：六头名单（含
/// cookie/set-cookie）脱敏（带 scheme 的凭证保留 scheme），名单外头原样落库。浏览器型客户端发 cookie 也不明文进 log.db。
#[tokio::test]
async fn user_side_request_headers_redact_cookie_and_auth() {
    let upstream = spawn_stub_upstream(200, ANTHROPIC_OK).await;
    let state = make_state(test_db().await).await;
    setup_group_with_upstream(&state, "gkredact", &upstream).await;
    set_log_settings(
        &state,
        ProxyLogSettings {
            log_user_request: true,
            log_upstream_request: true,
            ..Default::default()
        },
    )
    .await;

    let req = HttpRequest::builder()
        .method("POST")
        .uri("/v1/messages")
        .header("authorization", "Bearer gkredact")
        .header("content-type", "application/json")
        .header("cookie", "session=secret-cookie-value")
        .header("SeT-CoOkIe", "a=secret")
        .header("x-api-key", "gkredact")
        .header("user-agent", "claude-cli/1.0")
        .body(Body::from(
            r#"{"model":"claude-3","messages":[{"role":"user","content":"hi"}]}"#.to_string(),
        ))
        .unwrap();
    let resp = handle_proxy(AxumState(state.clone()), req).await;
    assert_eq!(resp.status(), StatusCode::OK);

    let row = last_log_for(&state, "gkredact").await;
    let h: serde_json::Value = serde_json::from_str(&row.request_headers).unwrap();
    assert_eq!(h["cookie"], "[REDACTED]", "cookie 必须脱敏");
    assert_eq!(h["set-cookie"], "[REDACTED]", "set-cookie 必须脱敏");
    assert_eq!(h["authorization"], "Bearer [REDACTED]", "保留 scheme，只脱 key 部分");
    assert_eq!(h["x-api-key"], "[REDACTED]");
    assert_eq!(h["user-agent"], "claude-cli/1.0", "名单外头原样");
    assert_eq!(h["content-type"], "application/json", "名单外头原样");
}

/// 上游请求头 key 全脱敏（db-log-redact-hardening 票 02）：长 key 落库不得出现
/// 「首 4 + **** + 尾 4」残段——首尾 8 字符跨行一致，可当关联指纹。
#[tokio::test]
async fn upstream_request_headers_no_partial_key_fragment() {
    let upstream = spawn_stub_upstream(200, ANTHROPIC_OK).await;
    let state = make_state(test_db().await).await;
    let key = "sk-upstream-long-key-1234567890";
    setup_group_with_upstream_key(&state, "gkfullred", &upstream, key).await;
    set_log_settings(
        &state,
        ProxyLogSettings {
            log_user_request: false,
            log_upstream_request: true,
            ..Default::default()
        },
    )
    .await;

    let req = messages_request(
        "gkfullred",
        r#"{"model":"claude-3","messages":[{"role":"user","content":"hi"}]}"#,
    );
    let resp = handle_proxy(AxumState(state.clone()), req).await;
    assert_eq!(resp.status(), StatusCode::OK);

    let row = last_log_for(&state, "gkfullred").await;
    assert!(
        !row.upstream_request_headers.contains("****"),
        "上游 key 必须整值脱敏，无残段: {}",
        row.upstream_request_headers
    );
    let h: serde_json::Value = serde_json::from_str(&row.upstream_request_headers).unwrap();
    assert_eq!(h["x-api-key"], "[REDACTED]", "平台定位靠 platform_name 字段，key 残段不需要");
}

/// 透传路径（ADR 0008）上游请求头全脱敏回归：authorization/cookie 脱敏，
/// 名单外头原样；无 **** 残段。
#[tokio::test]
async fn passthrough_upstream_headers_full_redaction() {
    let upstream = spawn_stub_upstream(200, ANTHROPIC_OK).await;
    let state = make_state(test_db().await).await;
    setup_passthrough_group(&state, "gkptred", &upstream).await;
    set_log_settings(
        &state,
        ProxyLogSettings {
            log_user_request: true,
            log_upstream_request: true,
            ..Default::default()
        },
    )
    .await;

    let req = HttpRequest::builder()
        .method("POST")
        .uri("/v1/messages")
        .header("authorization", "Bearer gkptred")
        .header("content-type", "application/json")
        .header("cookie", "session=secret-cookie")
        .header("anthropic-version", "2023-06-01")
        .body(Body::from(
            r#"{"model":"claude-3","messages":[{"role":"user","content":"hi"}]}"#.to_string(),
        ))
        .unwrap();
    let resp = handle_proxy(AxumState(state.clone()), req).await;
    assert_eq!(resp.status(), StatusCode::OK);

    let row = last_log_for(&state, "gkptred").await;
    let h: serde_json::Value = serde_json::from_str(&row.upstream_request_headers).unwrap();
    assert!(
        h.get("authorization")
            .is_none_or(|v| v == "[REDACTED]" || v.as_str().is_some_and(|s| s.ends_with(" [REDACTED]"))),
        "透传侧鉴权头要么整头剥除要么脱敏，不得明文"
    );
    assert_eq!(h["x-api-key"], "[REDACTED]", "透传侧注入的平台鉴权头全脱敏");
    assert_eq!(h["cookie"], "[REDACTED]", "透传侧 cookie 全脱敏不回退");
    assert!(
        !row.upstream_request_headers.contains("****"),
        "透传侧不得出现部分脱敏残段"
    );
    assert_eq!(h["anthropic-version"], "2023-06-01", "名单外头原样");
}

/// 两侧原始信息全关（缺省）：元数据行照落——2026-10-22 移除总开关后元数据恒记录，
/// 原始列（headers/body/url）按侧清空。
#[tokio::test]
async fn log_raw_off_writes_metadata_row() {
    let upstream = spawn_stub_upstream(200, ANTHROPIC_OK).await;
    let state = make_state(test_db().await).await;
    setup_group_with_upstream(&state, "gklogoff", &upstream).await;
    set_log_settings(
        &state,
        ProxyLogSettings {
            ..Default::default()
        },
    )
    .await;

    let req = messages_request(
        "gklogoff",
        r#"{"model":"claude-3","messages":[{"role":"user","content":"hi"}]}"#,
    );
    let resp = handle_proxy(AxumState(state.clone()), req).await;
    assert_eq!(resp.status(), StatusCode::OK);

    flush_log_queue(&state).await;
    let logs = aidog_logs::list_proxy_logs(&state.db, 100, 0)
        .await
        .unwrap();
    let row = logs
        .iter()
        .find(|l| l.group_key == "gklogoff")
        .expect("原始信息全关时元数据行必须照落");
    assert_eq!(row.status_code, 200);
    assert!(row.input_tokens + row.output_tokens > 0, "token 元数据保留");
    // 原始列断言走整行读取（Summary 不含 headers/body/url 列）
    let full = aidog_logs::get_proxy_log(&state.db, &row.id)
        .await
        .unwrap()
        .expect("整行必须存在");
    assert_eq!(full.request_headers, "");
    assert_eq!(full.request_body, "");
    assert_eq!(full.upstream_request_headers, "");
    assert_eq!(full.request_url, "", "用户侧关 → request_url 清空");
    assert_eq!(full.upstream_request_url, "", "上游侧关 → upstream_request_url 清空");
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
async fn censorship_blocked_fails_over_and_auto_disables_platform() {
    let blocked = spawn_stub_upstream(
        400,
        r#"{"error":{"message":"The content you provided or machine outputted is blocked.","type":"censorship_blocked"}}"#,
    )
    .await;
    let fallback = spawn_stub_upstream(200, ANTHROPIC_OK).await;
    let state = make_state(test_db().await).await;

    let first = aidog_db::create_platform(
        &state.db,
        CreatePlatform {
            name: "blocked".into(),
            platform_type: Protocol::Anthropic,
            base_url: blocked,
            api_key: "sk-blocked".into(),
            extra: String::new(),
            models: None,
            available_models: None,
            endpoints: None,
            manual_budgets: None,
            auto_group: None,
            join_group_ids: None,
            expires_at: None,
            quota_source: None,
        },
    )
    .await
    .unwrap();
    let second = aidog_db::create_platform(
        &state.db,
        CreatePlatform {
            name: "fallback".into(),
            platform_type: Protocol::Anthropic,
            base_url: fallback,
            api_key: "sk-fallback".into(),
            extra: String::new(),
            models: None,
            available_models: None,
            endpoints: None,
            manual_budgets: None,
            auto_group: None,
            join_group_ids: None,
            expires_at: None,
            quota_source: None,
        },
    )
    .await
    .unwrap();
    let group = aidog_db::create_group(
        &state.db,
        aidog_db::test_support::sample_group("gkcensor", vec![]),
    )
    .await
    .unwrap();
    aidog_db::set_group_platforms(
        &state.db,
        group.id,
        &[
            GroupPlatformInput {
                platform_id: first.id,
                priority: Some(0),
                weight: Some(1),
                level_priority: Some(1),
            },
            GroupPlatformInput {
                platform_id: second.id,
                priority: Some(1),
                weight: Some(1),
                level_priority: Some(1),
            },
        ],
    )
    .await
    .unwrap();

    let response = handle_proxy(
        AxumState(state.clone()),
        messages_request(
            "gkcensor",
            r#"{"model":"claude-3","messages":[{"role":"user","content":"hi"}]}"#,
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let disabled = aidog_db::get_platform(&state.db, first.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        disabled.status,
        crate::gateway::models::PlatformStatus::AutoDisabled
    );
    assert!(!disabled.enabled);
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
async fn upstream_401_cools_platform_in_memory() {
    // 2026-09-15：401 不再写 DB auto_disabled（旧路径 1 小时起指数退避），
    // 改调度器内存 auth 冷却（固定 5 分钟），到点自动回调度。
    let upstream = spawn_stub_upstream(401, r#"{"error":"unauthorized"}"#).await;
    let state = make_state(test_db().await).await;
    setup_group_with_upstream(&state, "gk4", &upstream).await;

    let req = messages_request(
        "gk4",
        r#"{"model":"claude-3","messages":[{"role":"user","content":"hi"}]}"#,
    );
    let _ = handle_proxy(AxumState(state.clone()), req).await;

    flush_log_queue(&state).await;
    let plats = aidog_db::list_platforms(&state.db).await.unwrap();
    assert!(
        plats.iter().all(|p| p.auto_disabled_until == 0),
        "401 不应写 DB auto_disabled（改内存冷却，UI 状态不变）"
    );
    let pid = plats[0].id;
    assert!(
        state.scheduler.auth_cooled(pid, aidog_db::now() + 1000),
        "401 应设 5 分钟内存 auth 冷却"
    );
    assert!(
        !state.scheduler.auth_cooled(
            pid,
            aidog_db::now() + crate::gateway::scheduling::AUTH_COOLDOWN_MS + 1000
        ),
        "auth 冷却到点必须失效"
    );
}

#[tokio::test]
async fn upstream_403_does_not_auto_disable_platform() {
    // 403 一律不自动禁用（区域封锁常返 403，误禁用代价高）
    let upstream = spawn_stub_upstream(403, r#"{"error":"forbidden"}"#).await;
    let state = make_state(test_db().await).await;
    setup_group_with_upstream(&state, "gk403", &upstream).await;

    let req = messages_request(
        "gk403",
        r#"{"model":"claude-3","messages":[{"role":"user","content":"hi"}]}"#,
    );
    let _ = handle_proxy(AxumState(state.clone()), req).await;

    flush_log_queue(&state).await;
    let plats = aidog_db::list_platforms(&state.db).await.unwrap();
    assert!(
        plats.iter().all(|p| p.auto_disabled_until == 0),
        "403 不应触发 auto_disable"
    );
}

#[tokio::test]
async fn upstream_401_region_blocked_does_not_auto_disable_platform() {
    // 区域封锁的 401 不自动禁用（换代理即恢复，禁用无意义）
    let upstream = spawn_stub_upstream(
        401,
        r#"{"error":{"message":"Country, region, or territory not supported"}}"#,
    )
    .await;
    let state = make_state(test_db().await).await;
    setup_group_with_upstream(&state, "gk401r", &upstream).await;

    let req = messages_request(
        "gk401r",
        r#"{"model":"claude-3","messages":[{"role":"user","content":"hi"}]}"#,
    );
    let _ = handle_proxy(AxumState(state.clone()), req).await;

    flush_log_queue(&state).await;
    let plats = aidog_db::list_platforms(&state.db).await.unwrap();
    assert!(
        plats.iter().all(|p| p.auto_disabled_until == 0),
        "区域封锁 401 不应触发 auto_disable"
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

/// GET /v1/models（含 group token）→ handle_models_list：openai 格式列表，不 relay 上游。
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
    assert!(
        data.iter()
            .any(|m| m.get("id").and_then(|i| i.as_str()) == Some("claude-opus-4-8"))
    );
    // typesafe 官方决策模型同样在列表里（default_model_ids 含 typesafe 平台）
    assert!(
        data.iter()
            .any(|m| m.get("id").and_then(|i| i.as_str()) == Some("jev-latest")),
        "/v1/models must list typesafe decision model"
    );
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
    assert!(
        data.iter()
            .any(|m| m.get("type").and_then(|t| t.as_str()) == Some("model"))
    );
}

// ── 网关模型发现：分组模型分类槽位链 ∪ 直通模型（Claude Code GET /v1/models）──

/// 建分组 `gk`：一个 Anthropic 平台，sonnet 槽链 + extra.direct_models。
async fn setup_group_models(
    state: &Arc<ProxyState>,
    gk: &str,
    sonnet: &[&str],
    direct: &[&str],
) {
    let plat = aidog_db::create_platform(
        &state.db,
        CreatePlatform {
            name: format!("p-{gk}"),
            platform_type: Protocol::Anthropic,
            base_url: "http://unused.invalid".into(),
            api_key: "sk-up".into(),
            extra: serde_json::json!({ "direct_models": direct }).to_string(),
            models: Some(PlatformModels {
                sonnet: sonnet.iter().map(|s| s.to_string()).collect(),
                ..Default::default()
            }),
            available_models: None,
            endpoints: None,
            manual_budgets: None,
            auto_group: None,
            join_group_ids: None,
            expires_at: None,
            quota_source: None,
        },
    )
    .await
    .unwrap();
    let group = aidog_db::create_group(&state.db, aidog_db::test_support::sample_group(gk, vec![]))
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

async fn models_ids(state: &Arc<ProxyState>, headers: &[(&str, &str)]) -> Vec<String> {
    let mut b = HttpRequest::builder().method("GET").uri("/v1/models");
    for (k, v) in headers {
        b = b.header(*k, *v);
    }
    let resp = handle_proxy(AxumState(state.clone()), b.body(Body::empty()).unwrap()).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body = axum::body::to_bytes(resp.into_body(), 1 << 20).await.unwrap();
    let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
    v["data"]
        .as_array()
        .unwrap()
        .iter()
        .map(|m| m["id"].as_str().unwrap().to_string())
        .collect()
}

/// group token → 槽位链全部 ∪ 直通模型，去重；不含默认清单。
#[tokio::test]
async fn models_discovery_group_token_returns_slot_chain_union_direct() {
    let (db, _dir) = aidog_db::test_support::three_file_db().await;
    let state = make_state(db).await;
    setup_group_models(&state, "gdisc", &["claude-slot-a", "claude-slot-b"], &["claude-direct-x", "claude-slot-a"]).await;
    let ids = models_ids(&state, &[("authorization", "Bearer gdisc")]).await;
    assert_eq!(ids, vec!["claude-slot-a", "claude-slot-b", "claude-direct-x"]);
}

/// 无 token → 默认清单（不 404）。
#[tokio::test]
async fn models_discovery_tokenless_returns_default_list() {
    let (db, _dir) = aidog_db::test_support::three_file_db().await;
    let state = make_state(db).await;
    setup_group_models(&state, "gdisc", &["claude-slot-a"], &["claude-direct-x"]).await;
    let ids = models_ids(&state, &[]).await;
    assert!(ids.iter().any(|i| i == "claude-opus-4-8"));
    assert!(!ids.iter().any(|i| i == "claude-direct-x"));
}

/// 未知分组 token → 仍是默认清单（探测不 404）。
#[tokio::test]
async fn models_discovery_unknown_group_returns_default_list() {
    let (db, _dir) = aidog_db::test_support::three_file_db().await;
    let state = make_state(db).await;
    setup_group_models(&state, "gdisc", &["claude-slot-a"], &["claude-direct-x"]).await;
    let ids = models_ids(&state, &[("authorization", "Bearer nope")]).await;
    assert!(ids.iter().any(|i| i == "claude-opus-4-8"));
    assert!(!ids.iter().any(|i| i == "claude-direct-x"));
}

/// X-Aidog-Group 头胜过 token → 该组的并集。
#[tokio::test]
async fn models_discovery_group_header_selects_union() {
    let (db, _dir) = aidog_db::test_support::three_file_db().await;
    let state = make_state(db).await;
    setup_group_models(&state, "gda", &["claude-a1"], &["claude-da"]).await;
    setup_group_models(&state, "gdb", &["claude-b1"], &["claude-db"]).await;
    let ids = models_ids(&state, &[("authorization", "Bearer gda"), ("x-aidog-group", "gdb")]).await;
    assert_eq!(ids, vec!["claude-b1", "claude-db"]);
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
            join_group_ids: None,
            expires_at: None,
            quota_source: None,
        },
    )
    .await
    .unwrap();
    let group = aidog_db::create_group(&state.db, aidog_db::test_support::sample_group(gk, vec![]))
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
    let _ = axum::body::to_bytes(resp.into_body(), 1 << 20)
        .await
        .unwrap();
    // 落库一条 mock 请求日志（假 token 生效）
    flush_log_queue(&state).await;
    let logs = aidog_logs::list_proxy_logs(&state.db, 100, 0)
        .await
        .unwrap();
    assert!(
        logs.iter()
            .any(|l| l.group_key == "gkmock" && l.status_code == 200)
    );
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
    assert!(
        (0.02..=0.08).contains(&ratio),
        "429 ratio {ratio} ({hits_429}/200) out of [0.02, 0.08]"
    );
}

/// Mock 平台 stream_override=true → 本地生成 SSE 流。
#[tokio::test]
async fn mock_platform_stream_override() {
    let state = make_state(test_db().await).await;
    setup_mock_group(
        &state,
        "gkmockstream",
        r#"{"mock":{"stream_override":true}}"#,
    )
    .await;

    let req = messages_request(
        "gkmockstream",
        r#"{"model":"claude-3","messages":[{"role":"user","content":"hi"}]}"#,
    );
    let resp = handle_proxy(AxumState(state.clone()), req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let _ = axum::body::to_bytes(resp.into_body(), 1 << 20)
        .await
        .unwrap();
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

    // 预热：进程内首个请求含一次性冷启动开销（单跑实测多 ~150-220ms，基线同），不计入被测 ttft。
    let warm = handle_proxy(
        AxumState(state.clone()),
        messages_request(
            "gkttft",
            r#"{"model":"claude-3","messages":[{"role":"user","content":"hi"}]}"#,
        ),
    )
    .await;
    let _ = axum::body::to_bytes(warm.into_body(), 1 << 20)
        .await
        .unwrap();

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
    assert!(
        ttft.unwrap().as_millis() >= 40,
        "delay_ms-only 首包应含双重 30ms sleep"
    );
    for gap in &gaps {
        assert!(
            gap.as_millis() >= 15 && gap.as_millis() <= 90,
            "gap={gap:?} 应 ≈30ms±"
        );
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
            join_group_ids: None,
            expires_at: None,
            quota_source: None,
        },
    )
    .await
    .unwrap();
    let group = aidog_db::create_group(&state.db, aidog_db::test_support::sample_group(gk, vec![]))
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
    assert!(
        logs.iter()
            .any(|l| l.group_key == "gkpt" && l.status_code == 200)
    );
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
    let _ = axum::body::to_bytes(resp.into_body(), 1 << 20)
        .await
        .unwrap();
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
    let _ = axum::body::to_bytes(resp.into_body(), 1 << 20)
        .await
        .unwrap();
}

/// 路由耗时票 01：真实转发路径落库后 routing.eval_ms 与胜出 attempt 的 ttfb_ms 均为 Some，
/// 且 ttfb_ms ≤ duration_ms；非流式 + 流式两条路径各验一次。
#[tokio::test]
async fn routing_eval_ms_and_attempt_ttfb_recorded_nonstream_and_stream() {
    let sse = "event: message_start\ndata: {\"type\":\"message_start\",\"message\":{\"usage\":{\"input_tokens\":5,\"output_tokens\":0}}}\n\nevent: message_delta\ndata: {\"type\":\"message_delta\",\"usage\":{\"output_tokens\":3}}\n\nevent: message_stop\ndata: {\"type\":\"message_stop\"}\n\n";
    for (gk, body, stream) in [
        ("gktt1", ANTHROPIC_OK, false),
        ("gktt2", sse, true),
    ] {
        let upstream = spawn_stub_upstream(200, body).await;
        let state = make_state(test_db().await).await;
        setup_group_with_upstream(&state, gk, &upstream).await;
        let req_body = format!(
            r#"{{"model":"claude-3","stream":{stream},"messages":[{{"role":"user","content":"hi"}}]}}"#
        );
        let resp = handle_proxy(AxumState(state.clone()), messages_request(gk, &req_body)).await;
        assert_eq!(resp.status(), StatusCode::OK);
        let _ = axum::body::to_bytes(resp.into_body(), 1 << 20).await.unwrap();
        let log = last_log_for(&state, gk).await;
        let eval = log.routing.as_ref().and_then(|r| r.eval_ms);
        assert!(eval.is_some(), "{gk}: routing.eval_ms 应有值");
        let a = log.attempts.last().expect("有胜出 attempt");
        let ttfb = a.ttfb_ms.unwrap_or_else(|| panic!("{gk}: 胜出 attempt 应有 ttfb_ms"));
        assert!(ttfb <= a.duration_ms, "{gk}: ttfb {ttfb} ≤ duration {}", a.duration_ms);
    }
}

/// 连接失败的尝试没有响应头 → ttfb_ms 为 None（eval_ms 照常有）。
#[tokio::test]
async fn connect_failure_attempt_has_no_ttfb() {
    let upstream = spawn_reset_upstream().await;
    let state = make_state(test_db().await).await;
    setup_group_with_upstream(&state, "gktt3", &upstream).await;
    let resp = handle_proxy(
        AxumState(state.clone()),
        messages_request("gktt3", r#"{"model":"claude-3","messages":[{"role":"user","content":"hi"}]}"#),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::BAD_GATEWAY);
    let log = last_log_for(&state, "gktt3").await;
    assert!(!log.attempts.is_empty());
    assert!(log.attempts.iter().all(|a| a.ttfb_ms.is_none()), "{:?}", log.attempts);
    assert!(log.routing.and_then(|r| r.eval_ms).is_some());
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
            join_group_ids: None,
            expires_at: None,
            quota_source: None,
        },
    )
    .await
    .unwrap();
    let group = aidog_db::create_group(&state.db, aidog_db::test_support::sample_group(gk, vec![]))
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
    let body = axum::body::to_bytes(resp.into_body(), 1 << 20)
        .await
        .unwrap();
    let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(v.get("id").and_then(|x| x.as_str()), Some("resp_1"));

    // 落库：source/target_protocol = openai_responses
    flush_log_queue(&state).await;
    let logs = aidog_logs::list_proxy_logs(&state.db, 100, 0)
        .await
        .unwrap();
    assert!(logs.iter().any(|l| l.group_key == "gkresp"
        && l.source_protocol == "openai_responses"
        && l.status_code == 200));
}

/// POST /v1/responses/{id}/cancel(带 body) → 原样转发 body + method 保留。
#[tokio::test]
async fn responses_subendpoint_post_cancel_forwards_body() {
    let upstream = spawn_stub_upstream(200, r#"{"id":"resp_2","status":"cancelled"}"#).await;
    let state = make_state(test_db().await).await;
    set_log_settings(
        &state,
        ProxyLogSettings {
            log_user_request: true,
            log_upstream_request: true,
            ..Default::default()
        },
    ).await;
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
    let logs = aidog_logs::list_proxy_logs(&state.db, 100, 0)
        .await
        .unwrap();
    let summary = logs.iter().find(|l| l.group_key == "gkrc").unwrap();
    let log = aidog_logs::get_proxy_log(&state.db, &summary.id)
        .await
        .unwrap()
        .unwrap();
    // URL 不重复拼 /v1（base_url 已含 /v1）
    assert!(
        log.upstream_request_url
            .ends_with("/responses/resp_2/cancel")
    );
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
    let logs = aidog_logs::list_proxy_logs(&state.db, 100, 0)
        .await
        .unwrap();
    assert!(
        logs.iter()
            .any(|l| l.group_key == "gkrfb" && l.source_protocol == "openai_responses")
    );
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

// ── 票 06：客户端白名单 × responses 子端点 / WebSocket 握手（本地假上游，无真实外呼）──

fn responses_req_ua(gk: &str, method: &str, uri: &str, ua: Option<&str>) -> Request {
    let mut b = HttpRequest::builder()
        .method(method)
        .uri(uri)
        .header("authorization", format!("Bearer {gk}"));
    if let Some(ua) = ua {
        b = b.header("user-agent", ua);
    }
    let body = if method == "POST" { Body::from("{}") } else { Body::empty() };
    b.body(body).unwrap()
}

/// 禁用组内某平台（status + enabled 同时关，等效退化形态用）。
async fn disable_platform_named(state: &Arc<ProxyState>, name: &str) {
    let id = aidog_db::list_platforms(&state.db).await.unwrap().into_iter().find(|p| p.name == name).unwrap().id;
    aidog_db::update_platform(&state.db, aidog_db::models::UpdatePlatform {
        id, name: None, platform_type: None, base_url: None, api_key: None, extra: None,
        models: None, available_models: None, endpoints: None,
        enabled: Some(false), status: Some(aidog_db::models::PlatformStatus::Disabled), manual_budgets: None,
        join_group_ids: None, expires_at: None, quota_source: None,
    })
    .await
    .unwrap();
    state.db.invalidate_group_details_cache();
}

/// responses 子端点五种方法各一例：白名单拦下 → 417 + 审计行；放行 UA → 200；
/// 未配白名单的组零变化。
#[tokio::test]
async fn whitelist_responses_subendpoints_all_methods() {
    let state = make_state(test_db().await).await;
    let up = spawn_stub_upstream(200, r#"{"id":"resp_1","object":"response"}"#).await;
    setup_multi_platform_group(&state, "gkrwl", &[("RA", &up, WL_CC_EXTRA), ("RB", &up, WL_CC_EXTRA)]).await;
    setup_multi_platform_group(&state, "gkrplain", &[("RC", &up, ""), ("RD", &up, "")]).await;

    let cases = [
        ("GET", "/v1/responses/resp_1"),
        ("POST", "/v1/responses/resp_1/cancel"),
        ("DELETE", "/v1/responses/resp_1"),
        ("POST", "/v1/responses/compact"),
        ("GET", "/v1/responses/resp_1/input_items"),
    ];
    for (method, uri) in cases {
        let resp = handle_proxy(AxumState(state.clone()), responses_req_ua("gkrwl", method, uri, Some("curl/8.4.0"))).await;
        assert_eq!(resp.status(), StatusCode::EXPECTATION_FAILED, "{method} {uri}");
        let log = last_log_for(&state, "gkrwl").await;
        assert_eq!(log.status_code, 417, "{method} {uri}");
        assert_eq!(log.blocked_by, "router");
        assert_eq!(log.blocked_reason, "client_not_allowed");
        assert_eq!(log.est_cost, 0.0);
        assert_eq!(log.user_agent, "curl/8.4.0");

        let resp = handle_proxy(AxumState(state.clone()), responses_req_ua("gkrwl", method, uri, Some("claude-cli/2.0.1 (external, cli)"))).await;
        assert_eq!(resp.status(), StatusCode::OK, "allowed {method} {uri}");
        let resp = handle_proxy(AxumState(state.clone()), responses_req_ua("gkrplain", method, uri, Some("curl/8.4.0"))).await;
        assert_eq!(resp.status(), StatusCode::OK, "no whitelist {method} {uri}");
    }

    // 无 UA → 认不出 → 417
    let resp = handle_proxy(AxumState(state.clone()), responses_req_ua("gkrwl", "GET", "/v1/responses/resp_1", None)).await;
    assert_eq!(resp.status(), StatusCode::EXPECTATION_FAILED);
}

/// 票 06 缺口收口：组内有透传平台时 responses 子端点先被透传路径认领，同样受白名单约束。
#[tokio::test]
async fn whitelist_responses_subendpoint_on_passthrough_group_417() {
    let (upstream, seen) = spawn_recording_upstream(r#"{"id":"resp_1"}"#).await;
    let state = make_state(test_db().await).await;
    setup_passthrough_platform_group_extra(&state, "gkprs", &upstream, WL_CC_EXTRA).await;
    let resp = handle_proxy(AxumState(state.clone()), responses_req_ua("gkprs", "GET", "/v1/responses/resp_1", Some("curl/8.4.0"))).await;
    assert_eq!(resp.status(), StatusCode::EXPECTATION_FAILED);
    assert_client_417_log(&state, "gkprs", "curl/8.4.0").await;
    assert_eq!(seen.lock().unwrap().len(), 0, "blocked request must not reach upstream");
    let resp = handle_proxy(AxumState(state.clone()), responses_req_ua("gkprs", "GET", "/v1/responses/resp_1", Some("claude-cli/2.0.1 (external, cli)"))).await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(seen.lock().unwrap().len(), 1);
}

/// 被拦平台跳过、落到未配白名单的平台；等效退化（只剩被拦平台 + 禁用平台）→ 417（同聊天路径单平台口径）；
/// 字面单平台组被拦 → 417。blocked_reason 恒为 client_not_allowed（responses 只认 enabled，无「真混合」形态，混合 503 见聊天路径）。
#[tokio::test]
async fn whitelist_responses_fallthrough_degenerate_and_mixed() {
    let state = make_state(test_db().await).await;
    let up = spawn_stub_upstream(200, r#"{"id":"resp_1"}"#).await;
    setup_multi_platform_group(&state, "gkrfall", &[("FA", &up, WL_CC_EXTRA), ("FB", &up, "")]).await;
    let b_id = aidog_db::list_platforms(&state.db).await.unwrap().into_iter().find(|p| p.name == "FB").unwrap().id;
    let resp = handle_proxy(AxumState(state.clone()), responses_req_ua("gkrfall", "GET", "/v1/responses/x", Some("curl/8.4.0"))).await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(last_log_for(&state, "gkrfall").await.platform_id, b_id);

    disable_platform_named(&state, "FB").await;
    let resp = handle_proxy(AxumState(state.clone()), responses_req_ua("gkrfall", "GET", "/v1/responses/x", Some("curl/8.4.0"))).await;
    assert_eq!(resp.status(), StatusCode::EXPECTATION_FAILED);
    let log = last_log_for(&state, "gkrfall").await;
    assert_eq!((log.blocked_by.as_str(), log.blocked_reason.as_str()), ("router", "client_not_allowed"));
    assert_eq!(log.status_code, 417);

    setup_multi_platform_group(&state, "gkrsole", &[("SA", &up, WL_CC_EXTRA)]).await;
    let resp = handle_proxy(AxumState(state.clone()), responses_req_ua("gkrsole", "GET", "/v1/responses/x", Some("curl/8.4.0"))).await;
    assert_eq!(resp.status(), StatusCode::EXPECTATION_FAILED);
}

fn ws_handshake_req(gk: &str, ua: Option<&str>) -> Request {
    let mut b = HttpRequest::builder()
        .method("GET")
        .uri("/v1/realtime")
        .header("authorization", format!("Bearer {gk}"))
        .header("connection", "Upgrade")
        .header("upgrade", "websocket")
        .header("sec-websocket-version", "13")
        .header("sec-websocket-key", "dGhlIHNhbXBsZSBub25jZQ==");
    if let Some(ua) = ua {
        b = b.header("user-agent", ua);
    }
    b.body(Body::empty()).unwrap()
}

/// WS 握手：白名单拦下 → 握手阶段直接 HTTP 417（无 101、不连上游）+ 审计行；无 UA 同拦；
/// 放行 UA / 未配白名单 → 越过闸门走到上游连接（reset 上游 → 502，非 417，无 blocked_reason）。
/// 单透传平台即「等效退化形态」：select_candidates 单平台短路绕过过滤，由 ws.rs 对选中平台再判。
#[tokio::test]
async fn whitelist_ws_handshake_rejects_417_and_allows() {
    let state = make_state(test_db().await).await;
    let dead = spawn_reset_upstream().await;
    setup_passthrough_platform_group_extra(&state, "gkws", &dead, WL_CC_EXTRA).await;

    for ua in [Some("curl/8.4.0"), None] {
        let resp = handle_proxy(AxumState(state.clone()), ws_handshake_req("gkws", ua)).await;
        assert_eq!(resp.status(), StatusCode::EXPECTATION_FAILED, "ua={ua:?}");
        assert!(resp.headers().get("upgrade").is_none(), "拒绝时不升级");
        let log = last_log_for(&state, "gkws").await;
        assert_eq!(log.status_code, 417);
        assert_eq!(log.blocked_by, "router");
        assert_eq!(log.blocked_reason, "client_not_allowed");
        assert_eq!(log.upstream_status_code, 0, "未连上游");
        assert_eq!(log.user_agent, ua.unwrap_or(""));
    }

    let resp = handle_proxy(AxumState(state.clone()), ws_handshake_req("gkws", Some("claude-cli/2.0.1 (external, cli)"))).await;
    assert_eq!(resp.status(), StatusCode::BAD_GATEWAY, "放行 → 走到上游连接（本地 reset 上游）");
    assert_eq!(last_log_for(&state, "gkws").await.blocked_reason, "");

    setup_passthrough_platform_group(&state, "gkwsplain", &dead).await;
    let resp = handle_proxy(AxumState(state.clone()), ws_handshake_req("gkwsplain", Some("curl/8.4.0"))).await;
    assert_eq!(resp.status(), StatusCode::BAD_GATEWAY);
}

// ── 票 02：平台白名单 UA 规则（本地假上游，无真实外呼）──

/// 规则：UA 含 openviking；另含一条 contains curl，用来证明「内置认出 → 规则不参与」。
const UA_RULE_EXTRA: &str = r#"{"client_ua_rules":[{"kind":"contains","pattern":"openviking"},{"kind":"contains","pattern":"curl"}]}"#;
const UA_RULE_PLUS_CC_EXTRA: &str = r#"{"allowed_clients":["claude_code"],"client_ua_rules":[{"kind":"prefix","pattern":"openviking/"}]}"#;
const OV_UA: &str = "OpenViking/0.5.2 (darwin)";

/// 多平台组（普通路由路径）：只写规则没勾内置 → 认不出的 UA 规则命中放行；其余未知 / 无 UA → 417；
/// 内置认出（curl）即使规则含 curl 也拒（内置优先）；claude-cli 被内置认出且未勾 → 拒。
#[tokio::test]
async fn ua_rules_multi_platform_group() {
    let state = make_state(test_db().await).await;
    let up = spawn_stub_upstream(200, ANTHROPIC_OK).await;
    setup_multi_platform_group(&state, "gkur", &[("A", &up, UA_RULE_EXTRA), ("B", &up, UA_RULE_EXTRA)]).await;
    let resp = handle_proxy(AxumState(state.clone()), messages_request_ua("gkur", Some(OV_UA))).await;
    assert_eq!(resp.status(), StatusCode::OK, "规则命中的未知客户端放行");
    assert_eq!(last_log_for(&state, "gkur").await.client, "ua:OpenViking");
    for ua in [Some("Other/1.0"), None, Some("curl/8.4.0"), Some("claude-cli/2.0.1 (external, cli)")] {
        let resp = handle_proxy(AxumState(state.clone()), messages_request_ua("gkur", ua)).await;
        assert_eq!(resp.status(), StatusCode::EXPECTATION_FAILED, "{ua:?}");
        assert_client_417_log(&state, "gkur", ua.unwrap_or("")).await;
    }
}

/// 多平台组：A 规则命中 → 两平台都在；A 规则不命中被拦 → 落到未配白名单的 B。
#[tokio::test]
async fn ua_rules_multi_platform_fallthrough() {
    let state = make_state(test_db().await).await;
    let up = spawn_stub_upstream(200, ANTHROPIC_OK).await;
    setup_multi_platform_group(&state, "gkurf", &[("A", &up, UA_RULE_EXTRA), ("B", &up, "")]).await;
    let b_id = aidog_db::list_platforms(&state.db).await.unwrap().into_iter().find(|p| p.name == "B").unwrap().id;
    let resp = handle_proxy(AxumState(state.clone()), messages_request_ua("gkurf", Some("Other/1.0"))).await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(last_log_for(&state, "gkurf").await.platform_id, b_id, "A 被拦，落到 B");
}

/// 勾选 + 规则并存：勾的内置放行，规则放行未知；其余拒。
#[tokio::test]
async fn ua_rules_combined_with_ticked_clients() {
    let state = make_state(test_db().await).await;
    let up = spawn_stub_upstream(200, ANTHROPIC_OK).await;
    setup_multi_platform_group(&state, "gkurc", &[("A", &up, UA_RULE_PLUS_CC_EXTRA)]).await;
    for (ua, want) in [
        ("claude-cli/2.0.1 (external, cli)", StatusCode::OK),
        (OV_UA, StatusCode::OK),
        ("curl/8.4.0", StatusCode::EXPECTATION_FAILED),
        ("Other/1.0", StatusCode::EXPECTATION_FAILED),
    ] {
        let resp = handle_proxy(AxumState(state.clone()), messages_request_ua("gkurc", Some(ua))).await;
        assert_eq!(resp.status(), want, "{ua}");
    }
}

/// 字面单平台组（len==1，单平台短路路径）与等效退化形态（多平台组只剩 1 个可用平台）分开验证。
#[tokio::test]
async fn ua_rules_literal_single_and_degenerate_single() {
    let state = make_state(test_db().await).await;
    let up = spawn_stub_upstream(200, ANTHROPIC_OK).await;
    setup_multi_platform_group(&state, "gkurl", &[("A", &up, UA_RULE_EXTRA)]).await;
    let resp = handle_proxy(AxumState(state.clone()), messages_request_ua("gkurl", Some(OV_UA))).await;
    assert_eq!(resp.status(), StatusCode::OK, "字面单平台：规则命中放行");
    let resp = handle_proxy(AxumState(state.clone()), messages_request_ua("gkurl", Some("Other/1.0"))).await;
    assert_eq!(resp.status(), StatusCode::EXPECTATION_FAILED, "字面单平台：未命中 417");
    assert_client_417_log(&state, "gkurl", "Other/1.0").await;
    let resp = handle_proxy(AxumState(state.clone()), messages_request_ua("gkurl", Some("curl/8.4.0"))).await;
    assert_eq!(resp.status(), StatusCode::EXPECTATION_FAILED, "字面单平台：内置认出且未勾 → 拒（规则含 curl 也不救）");

    setup_multi_platform_group(&state, "gkurd", &[("DA", &up, UA_RULE_EXTRA), ("DB", &up, "")]).await;
    set_platform_status(&state, "DB", aidog_db::models::PlatformStatus::Disabled).await;
    let resp = handle_proxy(AxumState(state.clone()), messages_request_ua("gkurd", Some(OV_UA))).await;
    assert_eq!(resp.status(), StatusCode::OK, "等效单平台：规则命中放行");
    let resp = handle_proxy(AxumState(state.clone()), messages_request_ua("gkurd", Some("Other/1.0"))).await;
    assert_eq!(resp.status(), StatusCode::EXPECTATION_FAILED, "等效单平台：不得静默打到唯一平台");
    assert_client_417_log(&state, "gkurd", "Other/1.0").await;
}

/// 单平台 + status bypass：白名单规则优先级高于 bypass。
#[tokio::test]
async fn ua_rules_beat_status_bypass_single_platform() {
    let state = make_state(test_db().await).await;
    let up = spawn_stub_upstream(200, ANTHROPIC_OK).await;
    setup_multi_platform_group(&state, "gkurb", &[("A", &up, UA_RULE_EXTRA)]).await;
    set_platform_status(&state, "A", aidog_db::models::PlatformStatus::AutoDisabled).await;
    let resp = handle_proxy(AxumState(state.clone()), messages_request_ua("gkurb", Some("Other/1.0"))).await;
    assert_eq!(resp.status(), StatusCode::EXPECTATION_FAILED);
    let resp = handle_proxy(AxumState(state.clone()), messages_request_ua("gkurb", Some(OV_UA))).await;
    assert_eq!(resp.status(), StatusCode::OK, "规则命中的客户端仍享受 bypass");
}

/// responses 子端点路径认规则。
#[tokio::test]
async fn ua_rules_responses_subendpoints() {
    let state = make_state(test_db().await).await;
    let up = spawn_stub_upstream(200, r#"{"id":"resp_1","object":"response"}"#).await;
    setup_multi_platform_group(&state, "gkurr", &[("RA", &up, UA_RULE_EXTRA), ("RB", &up, UA_RULE_EXTRA)]).await;
    for (method, uri) in [("GET", "/v1/responses/resp_1"), ("POST", "/v1/responses/compact")] {
        let resp = handle_proxy(AxumState(state.clone()), responses_req_ua("gkurr", method, uri, Some(OV_UA))).await;
        assert_eq!(resp.status(), StatusCode::OK, "{method} {uri}");
        let resp = handle_proxy(AxumState(state.clone()), responses_req_ua("gkurr", method, uri, Some("Other/1.0"))).await;
        assert_eq!(resp.status(), StatusCode::EXPECTATION_FAILED, "{method} {uri}");
        assert_client_417_log(&state, "gkurr", "Other/1.0").await;
    }
}

/// 透传平台（聊天路径）与 WebSocket 握手路径认规则。
#[tokio::test]
async fn ua_rules_passthrough_chat_and_ws() {
    let (upstream, seen) = spawn_recording_upstream(r#"{"id":"x"}"#).await;
    let state = make_state(test_db().await).await;
    setup_passthrough_platform_group_extra(&state, "gkurp", &upstream, UA_RULE_EXTRA).await;
    let chat = r#"{"model":"claude-3","max_tokens":10,"messages":[{"role":"user","content":"hi"}]}"#;
    let post = |ua: &'static str| {
        HttpRequest::builder()
            .method("POST")
            .uri("/v1/messages")
            .header("authorization", "Bearer gkurp")
            .header("content-type", "application/json")
            .header("user-agent", ua)
            .body(Body::from(chat))
            .unwrap()
    };
    let resp = handle_proxy(AxumState(state.clone()), post("Other/1.0")).await;
    assert_eq!(resp.status(), StatusCode::EXPECTATION_FAILED);
    assert_eq!(seen.lock().unwrap().len(), 0, "blocked request must not reach upstream");
    let resp = handle_proxy(AxumState(state.clone()), post(OV_UA)).await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(seen.lock().unwrap().len(), 1);

    let dead = spawn_reset_upstream().await;
    setup_passthrough_platform_group_extra(&state, "gkurws", &dead, UA_RULE_EXTRA).await;
    let resp = handle_proxy(AxumState(state.clone()), ws_handshake_req("gkurws", Some("Other/1.0"))).await;
    assert_eq!(resp.status(), StatusCode::EXPECTATION_FAILED);
    assert_client_417_log(&state, "gkurws", "Other/1.0").await;
    let resp = handle_proxy(AxumState(state.clone()), ws_handshake_req("gkurws", Some(OV_UA))).await;
    assert_eq!(resp.status(), StatusCode::BAD_GATEWAY, "规则命中 → 越过闸门走到上游连接");
}


// ──────────────────────────────────────────────────────────────
// /api/notify：handle_notify 鉴权 + dispatch 全链路
// ──────────────────────────────────────────────────────────────

fn notify_ci(ip: &str) -> axum::extract::ConnectInfo<std::net::SocketAddr> {
    axum::extract::ConnectInfo(ip.parse().unwrap())
}

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
    let resp = handle_notify(
        AxumState(state),
        notify_ci("127.0.0.1:50000"),
        notify_headers(None),
        Bytes::from_static(b"{}"),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

/// notify Bearer 空串 → 401。
#[tokio::test]
async fn notify_empty_bearer_returns_401() {
    let state = make_state(test_db().await).await;
    let resp = handle_notify(
        AxumState(state),
        notify_ci("127.0.0.1:50000"),
        notify_headers(Some("")),
        Bytes::from_static(b"{}"),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

/// notify group_key 不存在 → 401（防任意 token 触发）。
#[tokio::test]
async fn notify_unknown_group_returns_401() {
    let state = make_state(test_db().await).await;
    let resp = handle_notify(
        AxumState(state),
        notify_ci("127.0.0.1:50000"),
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
        notify_ci("127.0.0.1:50000"),
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
    let resp =
        handle_notify(AxumState(state.clone()), notify_ci("127.0.0.1:50000"), notify_headers(Some("gkn2")), body)
            .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(resp.into_body(), 1 << 20)
        .await
        .unwrap();
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
    let resp =
        handle_notify(AxumState(state), notify_ci("127.0.0.1:50000"), notify_headers(Some("gkn3")), body).await;
    assert_eq!(resp.status(), StatusCode::OK);
}

/// notify loopback 闸：非回环来源（bind_lan 下同网段设备）直接 403，不碰鉴权。
#[tokio::test]
async fn notify_non_loopback_rejected_403() {
    let state = make_state(test_db().await).await;
    let resp = handle_notify(
        AxumState(state),
        notify_ci("192.168.1.5:50000"),
        notify_headers(Some("any")),
        Bytes::from_static(b"{}"),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
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
    assert!(is_api_endpoint("/v1/rerank") && is_api_endpoint("/proxy/v2/rerank"));
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
    let listen = Some((
        std::net::IpAddr::V4(std::net::Ipv4Addr::new(127, 0, 0, 1)),
        9892u16,
    ));

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
    let listen_lan = Some((
        std::net::IpAddr::V4(std::net::Ipv4Addr::new(0, 0, 0, 0)),
        9892u16,
    ));
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
    let _ = state.listen_addr.set((
        std::net::IpAddr::V4(std::net::Ipv4Addr::new(127, 0, 0, 1)),
        9892u16,
    ));

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
    let logs = aidog_logs::list_proxy_logs(&state.db, 100, 0)
        .await
        .unwrap();
    let bucket = logs.iter().find(|l| l.group_key == "未匹配");
    assert!(
        bucket.is_some(),
        "虚拟桶 proxy_log 应落库 (group_key=未匹配), logs: {:?}",
        logs.iter().map(|l| &l.group_key).collect::<Vec<_>>()
    );
    let b = bucket.unwrap();
    assert_eq!(b.platform_id, 0, "虚拟桶 platform_id=0");
    assert_eq!(b.status_code, 502, "上游不可达 → 502");
    assert_eq!(
        b.source_protocol, "passthrough_unmatched",
        "虚拟桶 source_protocol 标记"
    );
}

/// API path + 错 token + Host = 代理自身 → 仍 404（不旁路直通）。
#[tokio::test]
async fn api_path_wrong_token_still_404_no_bypass() {
    let state = make_state(test_db().await).await;
    let _ = state.listen_addr.set((
        std::net::IpAddr::V4(std::net::Ipv4Addr::new(127, 0, 0, 1)),
        9892u16,
    ));

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
    let logs = aidog_logs::list_proxy_logs(&state.db, 100, 0)
        .await
        .unwrap();
    assert!(
        !logs.iter().any(|l| l.group_key == "未匹配"),
        "API path 未匹配不应进虚拟桶"
    );
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
    set_log_settings(
        &state,
        ProxyLogSettings {
            log_user_request: true,
            log_upstream_request: true,
            ..Default::default()
        },
    ).await;
    // 代理监听端口与 stub 不同（stub 端口由 spawn 异步绑定），Host=stub 端口被判为「非自身」→ 直通。
    let _ = state.listen_addr.set((
        std::net::IpAddr::V4(std::net::Ipv4Addr::new(127, 0, 0, 1)),
        65535u16,
    ));

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
    tokio::io::AsyncWriteExt::write_all(&mut stream, req_bytes.as_bytes())
        .await
        .unwrap();

    // 读完整响应（Connection: close）。
    let mut resp_buf = Vec::with_capacity(4096);
    tokio::io::AsyncReadExt::read_to_end(&mut stream, &mut resp_buf)
        .await
        .ok();
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
    let bucket = logs
        .iter()
        .find(|l| l.group_key == "未匹配")
        .expect("虚拟桶 proxy_log 应落库");
    assert_eq!(bucket.platform_id, 0);
    assert_eq!(bucket.status_code, 200);

    // Bug A：request_url 含 scheme://host/path?query 完整 url（不再是 origin-form path-only）。
    let full = aidog_logs::get_proxy_log(&state.db, &bucket.id)
        .await
        .unwrap()
        .unwrap();
    let stub_authority = stub_url.strip_prefix("http://").unwrap_or(&stub_url);
    assert!(
        full.request_url.contains(stub_authority),
        "request_url 含 host（Bug A 修复），实际: {}",
        full.request_url
    );
    assert!(
        full.request_url
            .contains("/api/anthropic/v1/messages?beta=true"),
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
        full.upstream_request_url,
        full.request_url
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
        (
            axum::http::StatusCode::OK,
            [("content-type", "text/plain")],
            body,
        )
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
    tokio::spawn(async move {
        axum::serve(
            listener,
            app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
        )
        .await
        .ok();
    });
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
    set_log_settings(
        &state,
        ProxyLogSettings {
            log_user_request: true,
            log_upstream_request: true,
            ..Default::default()
        },
    ).await;
    // listen_addr 设为代理自身 loopback（任意端口均可，识别代理自身用）；具体端口由
    // spawn_proxy_router 异步绑定，should_fallback_passthrough 用 loopback host 名 + 端口
    // 比对识别。这里设一个**非** stub 端口的 loopback 地址，让外部 host（stub）被判为「非自身」。
    let _ = state.listen_addr.set((
        std::net::IpAddr::V4(std::net::Ipv4Addr::new(127, 0, 0, 1)),
        65535u16,
    ));

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
    assert!(
        bucket.is_some(),
        "absolute-form forward 必须落虚拟桶 proxy_log"
    );
    let b = bucket.unwrap();
    assert_eq!(b.platform_id, 0, "虚拟桶 platform_id=0");
    assert_eq!(b.status_code, 200, "上游 stub 返 200 → 终态 200");
    assert_eq!(b.source_protocol, "passthrough_unmatched");
    // 取完整行查 upstream_request_url（summary 不含此字段）。
    let full = aidog_logs::get_proxy_log(&state.db, &b.id)
        .await
        .unwrap()
        .unwrap();
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
    let _ = state.listen_addr.set((
        std::net::IpAddr::V4(std::net::Ipv4Addr::new(127, 0, 0, 1)),
        65535u16,
    ));

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
    set_log_settings(
        &state,
        ProxyLogSettings {
            log_user_request: true,
            log_upstream_request: true,
            ..Default::default()
        },
    ).await;
    let _ = state.listen_addr.set((
        std::net::IpAddr::V4(std::net::Ipv4Addr::new(127, 0, 0, 1)),
        65535u16,
    ));

    let app = super::build_router(state.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let proxy_addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(
            listener,
            app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
        )
        .await
        .ok();
    });

    // raw TCP 发 absolute-form HTTPS URI（`GET https://example.invalid/ HTTP/1.1`）。
    // 手写 h1 请求：reqwest/hyper 客户端发 HTTPS forward 会自动转 CONNECT，禁用之需 raw 字节。
    let req_bytes = b"GET https://example.invalid/path HTTP/1.1\r\n\
Host: example.invalid\r\n\
Connection: close\r\n\
\r\n";
    let mut stream = tokio::net::TcpStream::connect(proxy_addr).await.unwrap();
    tokio::io::AsyncWriteExt::write_all(&mut stream, req_bytes)
        .await
        .unwrap();

    // 读完整响应（Connection: close 上游会关连接）。
    let mut resp_buf = Vec::with_capacity(4096);
    tokio::io::AsyncReadExt::read_to_end(&mut stream, &mut resp_buf)
        .await
        .ok();
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
    let full = aidog_logs::get_proxy_log(&state.db, &b.id)
        .await
        .unwrap()
        .unwrap();
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
    let logs = aidog_logs::list_proxy_logs(&state.db, 100, 0)
        .await
        .unwrap();
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
    let logs = aidog_logs::list_proxy_logs(&state.db, 100, 0)
        .await
        .unwrap();
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
    let _ = axum::body::to_bytes(resp.into_body(), 1 << 20)
        .await
        .unwrap();

    // guard.flush 走 tokio::spawn 落库，给写队列一个调度窗口。
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    flush_log_queue(&state).await;
    let logs = aidog_logs::list_proxy_logs(&state.db, 100, 0)
        .await
        .unwrap();
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
    let rec: Arc<std::sync::Mutex<Vec<(String, String)>>> =
        Arc::new(std::sync::Mutex::new(Vec::new()));
    let rec_srv = rec.clone();
    let app = axum::Router::new().fallback(any(move |req: HttpRequest<Body>| {
        let rec = rec_srv.clone();
        async move {
            let uri = req.uri().to_string();
            let bytes = axum::body::to_bytes(req.into_body(), 1 << 20)
                .await
                .unwrap_or_default();
            rec.lock()
                .unwrap()
                .push((uri, String::from_utf8_lossy(&bytes).to_string()));
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
            join_group_ids: None,
            expires_at: None,
            quota_source: None,
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
        assert_eq!(
            sent["contents"][0]["parts"][0]["text"], "hi",
            "透传体其余内容不动: {sent}"
        );
    }
}

#[tokio::test]
async fn stage_prepare_send_preserves_remapped_body_and_upstream_log() {
    let (upstream, rec) = spawn_recording_upstream(GEMINI_OK).await;
    let state = make_state(test_db().await).await;
    setup_gemini_passthrough_group(&state, "stage-seam", &upstream).await;
    set_log_settings(
        &state,
        ProxyLogSettings {
            log_upstream_request: true,
            ..Default::default()
        },
    )
    .await;
    let request = HttpRequest::builder()
        .method("POST")
        .uri("/v1beta/models/gemini-src:generateContent")
        .header("authorization", "Bearer stage-seam")
        .header("content-type", "application/json")
        .body(Body::from(
            r#"{"model":"gemini-src","contents":[{"role":"user","parts":[{"text":"hi"}]}]}"#,
        ))
        .unwrap();
    let response = handle_proxy(AxumState(state.clone()), request).await;
    assert_eq!(response.status(), StatusCode::OK);
    let row = only_log_of(&state, "stage-seam").await;
    let captured = rec.lock().unwrap().clone();
    assert_eq!(captured.len(), 1);
    assert!(captured[0].0.contains("gemini-target"));
    assert!(!captured[0].0.contains("gemini-src"));
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&captured[0].1).unwrap(),
        serde_json::json!({
            "contents": [{"role": "user", "parts": [{"text": "hi"}]}]
        })
    );
    assert_eq!(row.upstream_request_body, captured[0].1);
    assert_eq!(row.target_protocol, "gemini");
    assert_eq!(row.model, "gemini-src");
    assert!(row.done);
}

/// 04-endpoint-security 验收 1：无鉴权调试端点 `/api/debug/bench-query` 已从产品中删除，
/// 路由表不再有它 —— 请求落 fallback（handle_proxy 无匹配 group）返 404，且响应体
/// 绝不含旧 handler 的量测字段。
#[tokio::test]
async fn debug_bench_query_endpoint_is_gone() {
    let state = make_state(test_db().await).await;
    let base = spawn_proxy_router(state).await;

    let resp = reqwest::Client::new()
        .post(format!("{base}/api/debug/bench-query"))
        .header("content-type", "application/json")
        .body(r#"{"which":"logs","n":1}"#)
        .send()
        .await
        .unwrap();

    assert_eq!(
        resp.status(),
        reqwest::StatusCode::NOT_FOUND,
        "调试端点必须已删除（落 fallback → 404）"
    );
    let body = resp.text().await.unwrap();
    assert!(
        !body.contains("durations_ms"),
        "响应体不得含旧 bench handler 字段，实际: {body}"
    );
}

// ═══════════════ proxy-log-done-flag：终态落库必须置 done ═══════════════
//
// 缺陷（.scratch/proxy-log-done-flag/01-missing-done-flag.md）：22 个终态 upsert_log 调用点
// 漏写 `log.done = true`，而 `process_upsert` 的 emit 门与 stats_agg 门同为
// `status_code != 0 && done`（log.rs），于是上游 4xx/5xx、mock、count_tokens、responses
// 子端点、devin 的请求既不推 `proxy-log-updated`，也不进 `stats_agg_hourly`。
// 下列测试逐类钉住「终态行 done=1」，`emit_gate_*`（test_log.rs）钉住「中间态不放行」。

/// 任意 Protocol + extra 的平台 + group（`setup_group_with_upstream` 只能建 Anthropic 平台）。
async fn setup_group_with_platform(
    state: &Arc<ProxyState>,
    gk: &str,
    platform_type: Protocol,
    base_url: &str,
    extra: &str,
) {
    let plat = aidog_db::create_platform(
        &state.db,
        CreatePlatform {
            name: format!("p_{gk}"),
            platform_type,
            base_url: base_url.to_string(),
            api_key: "sk-up".into(),
            extra: extra.to_string(),
            models: None,
            available_models: None,
            endpoints: None,
            manual_budgets: None,
            auto_group: None,
            join_group_ids: None,
            expires_at: None,
            quota_source: None,
        },
    )
    .await
    .unwrap();
    let group = aidog_db::create_group(&state.db, aidog_db::test_support::sample_group(gk, vec![]))
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

/// 取某 group 的唯一 proxy_log 整行（断言恰好一条，避免误取别的测试的行）。
/// 走 `get_proxy_log` 而非 list：`done` 只在整行上（summary 不带该列）。
async fn only_log_of(state: &Arc<ProxyState>, gk: &str) -> aidog_db::models::ProxyLog {
    flush_log_queue(state).await;
    let logs = aidog_logs::list_proxy_logs(&state.db, 200, 0)
        .await
        .unwrap();
    let mut mine: Vec<_> = logs.into_iter().filter(|l| l.group_key == gk).collect();
    assert_eq!(mine.len(), 1, "group {gk} 应恰好落一条 proxy_log");
    let id = mine.pop().unwrap().id;
    aidog_logs::get_proxy_log(&state.db, &id)
        .await
        .unwrap()
        .expect("proxy_log 整行必须可读")
}

/// stats_agg_hourly 里该 group 的 (request_count, error_count)。
async fn agg_counts(db: &aidog_db::Db, gk: &str) -> (i64, i64) {
    let g = gk.to_string();
    db.write_conn()
        .call(move |c| {
            Ok(c.query_row(
                "SELECT COALESCE(SUM(request_count),0), COALESCE(SUM(error_count),0) \
                 FROM stats_agg_hourly WHERE group_key = ?1",
                rusqlite::params![g],
                |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?)),
            )?)
        })
        .await
        .unwrap()
}

/// P0：上游 4xx/5xx 的终态行必须 done=1 并进 stats_agg_hourly（错误率不再系统性偏低）。
/// 修复前实测：3×500 + 3×400 → 0 条事件、done=0、stats_agg_hourly 零行。
#[tokio::test]
async fn upstream_non_success_marks_done_and_enters_stats_agg() {
    for (upstream_status, gk) in [(500u16, "gkdone500"), (400u16, "gkdone400")] {
        let upstream = spawn_stub_upstream(upstream_status, r#"{"error":"boom"}"#).await;
        let state = make_state(test_db().await).await;
        setup_group_with_upstream(&state, gk, &upstream).await;

        let req = messages_request(
            gk,
            r#"{"model":"claude-3","messages":[{"role":"user","content":"hi"}]}"#,
        );
        let resp = handle_proxy(AxumState(state.clone()), req).await;
        assert_eq!(resp.status().as_u16(), upstream_status);

        let row = only_log_of(&state, gk).await;
        assert_eq!(row.status_code, upstream_status as i32);
        assert!(
            row.done,
            "上游 {upstream_status} 的终态行必须 done=1，否则不 emit、不进统计"
        );
        let (requests, errors) = agg_counts(&state.db, gk).await;
        assert_eq!(
            requests, 1,
            "上游 {upstream_status} 必须进 stats_agg_hourly"
        );
        assert_eq!(errors, 1, "上游 {upstream_status} 必须计入 error_count");
    }
}

/// mock 平台 5 条落库路径（http_error / 429 / 非流式 200 / 流式 200）全部 done=1。
/// timeout 分支 sleep 600s 不可测，与 http_error 共用同一形状，不单测。
#[tokio::test]
async fn mock_platform_terminal_logs_are_done() {
    let cases = [
        (
            "gkmkerr",
            r#"{"mock":{"error_mode":"http_error","status_code":503}}"#,
            r#"{"model":"claude-3","messages":[{"role":"user","content":"hi"}]}"#,
            503,
        ),
        (
            "gkmk429",
            r#"{"mock":{"error_mode":"rate_limit_429"}}"#,
            r#"{"model":"claude-3","messages":[{"role":"user","content":"hi"}]}"#,
            429,
        ),
        (
            "gkmkok",
            r#"{"mock":{"input_tokens":11,"output_tokens":7}}"#,
            r#"{"model":"claude-3","messages":[{"role":"user","content":"hi"}]}"#,
            200,
        ),
        (
            "gkmkstream",
            r#"{"mock":{"stream_override":true}}"#,
            r#"{"model":"claude-3","messages":[{"role":"user","content":"hi"}]}"#,
            200,
        ),
    ];
    for (gk, extra, body, want_status) in cases {
        let state = make_state(test_db().await).await;
        setup_mock_group(&state, gk, extra).await;
        let resp = handle_proxy(AxumState(state.clone()), messages_request(gk, body)).await;
        assert_eq!(resp.status().as_u16(), want_status, "gk={gk}");
        let _ = axum::body::to_bytes(resp.into_body(), 1 << 20)
            .await
            .unwrap();

        let row = only_log_of(&state, gk).await;
        assert!(row.done, "mock 终态行必须 done=1（gk={gk}）");
        let (requests, _) = agg_counts(&state.db, gk).await;
        assert_eq!(requests, 1, "mock 请求必须进 stats_agg_hourly（gk={gk}）");
    }
}

fn count_tokens_request(gk: &str) -> Request {
    HttpRequest::builder()
        .method("POST")
        .uri("/v1/messages/count_tokens")
        .header("authorization", format!("Bearer {gk}"))
        .header("content-type", "application/json")
        .body(Body::from(
            r#"{"model":"claude-3","messages":[{"role":"user","content":"hi"}]}"#.to_string(),
        ))
        .unwrap()
}

/// count_tokens 两条终态路径（上游透传成功 / 上游不支持走本地估算兜底）都 done=1。
/// 统计口径不变：count_tokens 被 log.rs 的 `!is_count_tokens` 门排除在 stats_agg 之外，
/// 本修复只让日志页对它实时刷新。
#[tokio::test]
async fn count_tokens_terminal_logs_are_done() {
    for (upstream_status, upstream_body, gk) in [
        (200u16, r#"{"input_tokens":42}"#, "gkct200"),
        (500u16, r#"{"error":"unsupported"}"#, "gkctfb"),
    ] {
        let upstream = spawn_stub_upstream(upstream_status, upstream_body).await;
        let state = make_state(test_db().await).await;
        setup_group_with_upstream(&state, gk, &upstream).await;

        let resp = handle_proxy(AxumState(state.clone()), count_tokens_request(gk)).await;
        assert_eq!(resp.status(), StatusCode::OK, "gk={gk}");

        let row = only_log_of(&state, gk).await;
        assert_eq!(row.status_code, 200);
        assert!(row.done, "count_tokens 终态行必须 done=1（gk={gk}）");
        let (requests, _) = agg_counts(&state.db, gk).await;
        assert_eq!(requests, 0, "count_tokens 照旧不进 stats_agg（gk={gk}）");
    }
}

/// responses 子端点：上游正常响应 / 上游连不上（502）两条终态路径都 done=1。
#[tokio::test]
async fn responses_subendpoint_terminal_logs_are_done() {
    for (gk, upstream, want) in [
        (
            "gkrsok",
            spawn_stub_upstream(200, r#"{"id":"resp_1","object":"response"}"#).await,
            200u16,
        ),
        ("gkrs502", spawn_reset_upstream().await, 502u16),
    ] {
        let state = make_state(test_db().await).await;
        setup_group_with_platform(&state, gk, Protocol::OpenAIResponses, &upstream, "").await;

        let resp = handle_proxy(
            AxumState(state.clone()),
            get_request(gk, "/v1/responses/resp_1"),
        )
        .await;
        assert_eq!(resp.status().as_u16(), want, "gk={gk}");

        let row = only_log_of(&state, gk).await;
        assert_eq!(row.status_code, want as i32);
        assert!(row.done, "responses 子端点终态行必须 done=1（gk={gk}）");
        let (requests, _) = agg_counts(&state.db, gk).await;
        assert_eq!(
            requests, 1,
            "responses 子端点请求必须进 stats_agg（gk={gk}）"
        );
    }
}

/// 假 Devin API：POST /sessions 建会话，GET /sessions/{id} 直接给终态，
/// GET /sessions/{id}/messages 回一条 devin message。`status` 由参数决定（exit / error）。
async fn spawn_stub_devin(session_status: &'static str) -> String {
    use axum::routing::{get, post};
    let app = axum::Router::new()
        .route(
            "/sessions",
            post(|| async { axum::Json(serde_json::json!({"session_id":"dev_1"})) }),
        )
        .route(
            "/sessions/{id}",
            get(move || async move {
                axum::Json(serde_json::json!({"status": session_status, "acus_consumed": 1.5}))
            }),
        )
        .route(
            "/sessions/{id}/messages",
            get(|| async { axum::Json(serde_json::json!([{"source":"devin","message":"done"}])) }),
        );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.ok();
    });
    format!("http://{addr}")
}

/// devin 的四条终态路径（配置错误统一出口 / 会话 error / 非流式 exit / 伪流式）都 done=1。
#[tokio::test]
async fn devin_terminal_logs_are_done() {
    const ORG: &str = r#"{"devin":{"org_id":"org_1"}}"#;
    let ok_upstream = spawn_stub_devin("exit").await;
    let err_upstream = spawn_stub_devin("error").await;

    // (gk, base_url, extra, body, 期望状态码)
    let cases = [
        // extra 缺 org_id → devin_error 统一出口（400）
        (
            "gkdvcfg",
            ok_upstream.clone(),
            "",
            r#"{"model":"devin","messages":[{"role":"user","content":"hi"}]}"#,
            400u16,
        ),
        // 会话终态 error → 502
        (
            "gkdverr",
            err_upstream.clone(),
            ORG,
            r#"{"model":"devin","messages":[{"role":"user","content":"hi"}]}"#,
            502,
        ),
        // 非流式 exit → 200
        (
            "gkdvok",
            ok_upstream.clone(),
            ORG,
            r#"{"model":"devin","messages":[{"role":"user","content":"hi"}]}"#,
            200,
        ),
        // 伪流式 exit → 200（SSE chunk 已就地构造完毕，落库即终态）
        (
            "gkdvstream",
            ok_upstream.clone(),
            ORG,
            r#"{"model":"devin","stream":true,"messages":[{"role":"user","content":"hi"}]}"#,
            200,
        ),
    ];
    for (gk, base_url, extra, body, want) in cases {
        let state = make_state(test_db().await).await;
        setup_group_with_platform(&state, gk, Protocol::Devin, &base_url, extra).await;
        let resp = handle_proxy(AxumState(state.clone()), messages_request(gk, body)).await;
        assert_eq!(resp.status().as_u16(), want, "gk={gk}");
        let _ = axum::body::to_bytes(resp.into_body(), 1 << 20)
            .await
            .unwrap();

        let row = only_log_of(&state, gk).await;
        assert_eq!(row.status_code, want as i32, "gk={gk}");
        assert!(row.done, "devin 终态行必须 done=1（gk={gk}）");
        let (requests, _) = agg_counts(&state.db, gk).await;
        assert_eq!(requests, 1, "devin 请求必须进 stats_agg（gk={gk}）");
    }
}

// ══ jev-decision-proxy：/v1/systemone 决策请求端到端 ══

use crate::gateway::models::PlatformModels;
use aidog_db::models::ModelEntry;
use std::sync::Mutex as StdMutex;

/// 决策 stub 上游：记录每条请求的 (path, body)，按队列顺序返回 (status, body)。
async fn spawn_decision_upstream(
    responses: Vec<(u16, &'static str)>,
) -> (String, std::sync::Arc<StdMutex<Vec<(String, String)>>>) {
    let hits: std::sync::Arc<StdMutex<Vec<(String, String)>>> = std::sync::Arc::new(StdMutex::new(Vec::new()));
    let queue = std::sync::Arc::new(StdMutex::new(std::collections::VecDeque::from(
        responses,
    )));
    let h2 = hits.clone();
    let app = axum::Router::new().fallback(axum::routing::any(move |req: Request| {
        let hits = h2.clone();
        let queue = queue.clone();
        async move {
            let path = req.uri().path().to_string();
            let body = axum::body::to_bytes(req.into_body(), 1024 * 1024)
                .await
                .map(|b| String::from_utf8_lossy(&b).to_string())
                .unwrap_or_default();
            hits.lock().unwrap().push((path, body));
            let (status, resp) = queue.lock().unwrap().pop_front().unwrap_or((200, "{}"));
            (
                axum::http::StatusCode::from_u16(status).unwrap(),
                [("content-type", "application/json")],
                resp,
            )
        }
    }));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.ok(); });
    (format!("http://{addr}"), hits)
}

/// 注册带 jev 槽位 + typesafe endpoint 的平台与 group。
async fn setup_decision_group(
    state: &Arc<ProxyState>,
    gk: &str,
    base_url: &str,
    models: PlatformModels,
    available: Option<Vec<&str>>,
    protocol: Protocol,
) {
    let plat = aidog_db::create_platform(
        &state.db,
        CreatePlatform {
            name: format!("stub-{gk}"),
            platform_type: Protocol::OpenAI,
            base_url: base_url.to_string(),
            api_key: "sk-up".into(),
            extra: String::new(),
            models: Some(models),
            available_models: available.map(|v| v.into_iter().map(String::from).collect()),
            endpoints: Some(vec![crate::gateway::models::PlatformEndpoint {
                protocol,
                base_url: format!("{base_url}/v1"),
                client_type: "default".into(),
                coding_plan: false,
            }]),
            manual_budgets: None,
            auto_group: None,
            join_group_ids: None,
            expires_at: None,
            quota_source: None,
        },
    )
    .await
    .unwrap();
    let group = aidog_db::create_group(&state.db, aidog_db::test_support::sample_group(gk, vec![]))
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

fn decision_request(gk: &str, model: &str) -> Request {
    HttpRequest::builder()
        .method("POST")
        .uri("/v1/systemone")
        .header("authorization", format!("Bearer {gk}"))
        .header("content-type", "application/json")
        .body(Body::from(
            serde_json::json!({"model": model, "usage": {}, "answers": []}).to_string(),
        ))
        .unwrap()
}

const OR_RESPONSE: &str = r#"{"id":"gen_1","provider":"openrouter","model":"jev-1.13","answers":[{"text":"yes"}],"usage":{"input_tokens":1000,"output_tokens":50,"cost":0.000125}}"#;

/// R10 URL + R8 model 改写 + R12 剥离 + R14 usage.cost 采用，全链路。
#[tokio::test]
async fn decision_request_end_to_end() {
    let (upstream, hits) = spawn_decision_upstream(vec![(200, OR_RESPONSE)]).await;
    let state = make_state(test_db().await).await;
    setup_decision_group(
        &state,
        "gkdec",
        &upstream,
        PlatformModels {
            jev: vec!["jev-latest".into()],
            ..Default::default()
        },
        None,
        Protocol::TypeSafe,
    )
    .await;

    let resp = handle_proxy(AxumState(state.clone()), decision_request("gkdec", "whatever"))
        .await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(
        resp.headers().get("content-type").unwrap(),
        "application/json",
        "decision response must be JSON, not octet-stream"
    );
    let body = axum::body::to_bytes(resp.into_body(), 1024 * 1024).await.unwrap();
    let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(v.get("id"), None, "id stripped (R12)");
    assert_eq!(v.get("provider"), None, "provider stripped (R12)");
    assert_eq!(v.get("usage").unwrap().get("cost"), None);
    assert_eq!(v.get("model").unwrap(), "jev-1.13");

    // 先拷出来再 drop guard：guard 跨 await（last_log_for）触发 clippy await_holding_lock。
    let (path, req_body) = {
        let hits = hits.lock().unwrap();
        let (p, b) = hits[0].clone();
        (p, b)
    };
    assert_eq!(path, "/v1/systemone", "URL = base_url + /systemone (R10)");
    // preserve_order：上游请求体保持客户端字段顺序（model 原位改写，非字母序）
    assert_eq!(req_body, r#"{"model":"jev-latest","usage":{},"answers":[]}"#);
    let sent: serde_json::Value = serde_json::from_str(&req_body).unwrap();
    assert_eq!(sent.get("model").unwrap(), "jev-latest", "model rewritten to slot (R8)");

    let log = last_log_for(&state, "gkdec").await;
    assert_eq!(log.status_code, 200);
    assert_eq!(log.source_protocol, "typesafe");
    assert_eq!(log.actual_model, "jev-latest");
    assert_eq!(log.input_tokens, 1000);
    assert_eq!(log.output_tokens, 50);
    assert!((log.est_cost - 0.000125).abs() < 1e-9, "upstream usage.cost adopted (R14), got {}", log.est_cost);
}

/// 票 16 B4（R14 零改动复核）：上游 `usage.cost` 在场时全盘采纳——即使 usage 同时带
/// `server_tool_use` 按次计数（本票已提取入库），本地也不重算、不叠加按次项。
#[tokio::test]
async fn decision_upstream_cost_wins_over_tool_counts() {
    let resp_body = r#"{"id":"g1","provider":"p","model":"jev-1.13","answers":[],"usage":{"input_tokens":1000,"output_tokens":50,"cost":0.000125,"server_tool_use":{"web_search_requests":100}}}"#;
    let (upstream, _hits) = spawn_decision_upstream(vec![(200, resp_body)]).await;
    let state = make_state(test_db().await).await;
    setup_decision_group(
        &state,
        "gkdec-tools",
        &upstream,
        PlatformModels {
            jev: vec!["jev-latest".into()],
            ..Default::default()
        },
        None,
        Protocol::TypeSafe,
    )
    .await;

    let resp = handle_proxy(AxumState(state.clone()), decision_request("gkdec-tools", "whatever"))
        .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let log = last_log_for(&state, "gkdec-tools").await;
    // 计数只在内存 ProxyLog 里参与计价（B1：不落列），DB 读回恒空——此处只断言 est 口径。
    assert!(
        log.tool_call_counts.is_empty(),
        "按次计数不落列（PROXY_LOG_COLUMNS 零变化）"
    );
    assert!(
        (log.est_cost - 0.000125).abs() < 1e-9,
        "上游 cost 在场必须全盘采纳（不叠加按次项）, got {}",
        log.est_cost
    );
}

/// 不信任上游 `usage.cost: 0`（用户 2026-10-01 确认）：registry 价 > 0 → 按 registry 价；
/// registry 标 `free: true` → 0；全 0 价未标 free 视同未定价 → PriceSyncSettings 默认价（3 $/M）。
#[tokio::test]
async fn decision_upstream_zero_cost_uses_registry_price() {
    let resp_body = r#"{"model":"jev-1.13","answers":[],"usage":{"input_tokens":1000,"output_tokens":0,"cost":0}}"#;
    for (gk, price, expected) in [
        ("gkz1", r#"{"input":4.2e-8,"output":0}"#, 1000.0 * 4.2e-8),
        ("gkzfree", r#"{"free":true,"input":0,"output":0}"#, 0.0),
        ("gkz0", r#"{"input":0,"output":0}"#, 1000.0 * 3.0 / 1_000_000.0),
    ] {
        let (upstream, _hits) = spawn_decision_upstream(vec![(200, resp_body)]).await;
        let state = make_state(test_db().await).await;
        aidog_db::upsert_model_entries(
            &state.db,
            vec![ModelEntry {
                platform_code: "openai".into(),
                model_id: "jev-1.13".into(),
                display_name: "jev-1.13".into(),
                canonical_model: "jev-1.13".into(),
                family: String::new(),
                version: String::new(),
                predecessor: String::new(),
                capabilities: vec!["decision".into()],
                builtin_tools_excluded: vec![],
                max_input_tokens: None,
                max_output_tokens: None,
                context_window: Some(64000),
                official: true,
                price_data: format!(r#"{{"price":{price}}}"#),
                updated_at: aidog_db::now(),
            }],
        )
        .await
        .unwrap();
        setup_decision_group(&state, gk, &upstream, PlatformModels::default(), Some(vec!["jev-1.13"]), Protocol::TypeSafe).await;

        let resp = handle_proxy(AxumState(state.clone()), decision_request(gk, "jev-1.13")).await;
        assert_eq!(resp.status(), StatusCode::OK);
        let log = last_log_for(&state, gk).await;
        assert!(
            (log.est_cost - expected).abs() < 1e-12,
            "{gk}: expected {expected}, got {}",
            log.est_cost
        );
    }
}

/// R14 回落：无 usage.cost → registry 价 × input_tokens（output 价 0）。
#[tokio::test]
async fn decision_cost_falls_back_to_registry_price() {
    let resp_body = r#"{"id":"x","provider":"y","model":"jev-1.13","answers":[],"usage":{"input_tokens":1000,"output_tokens":0}}"#;
    let (upstream, _hits) = spawn_decision_upstream(vec![(200, resp_body)]).await;
    let state = make_state(test_db().await).await;
    // registry 条目：openai 平台 + jev-1.13，decision 能力，input 4.2e-8 / output 0
    aidog_db::upsert_model_entries(
        &state.db,
        vec![ModelEntry {
            platform_code: "openai".into(),
            model_id: "jev-1.13".into(),
            display_name: "jev-1.13".into(),
            canonical_model: "jev-1.13".into(),
            family: String::new(),
            version: String::new(),
            predecessor: String::new(),
            capabilities: vec!["decision".into()],
            builtin_tools_excluded: vec![],
            max_input_tokens: None,
            max_output_tokens: None,
            context_window: Some(64000),
            official: true,
            price_data: r#"{"price":{"input":4.2e-8,"output":0}}"#.into(),
            updated_at: aidog_db::now(),
        }],
    )
    .await
    .unwrap();
    setup_decision_group(
        &state,
        "gkfb",
        &upstream,
        PlatformModels::default(),
        Some(vec!["jev-1.13"]),
        Protocol::TypeSafe,
    )
    .await;

    let resp = handle_proxy(AxumState(state.clone()), decision_request("gkfb", "jev-1.13")).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let log = last_log_for(&state, "gkfb").await;
    assert_eq!(log.actual_model, "jev-1.13", "R8: requested model in available_models kept");
    let expected = 1000.0 * 4.2e-8;
    assert!(
        (log.est_cost - expected).abs() < 1e-12,
        "registry price fallback (R14): expected {expected}, got {}",
        log.est_cost
    );
}

/// R13：上游 529 → 换下个候选平台成功。
#[tokio::test]
async fn decision_529_fails_over_to_next_platform() {
    let (up1, hits1) = spawn_decision_upstream(vec![(529, r#"{"error":"overloaded"}"#)]).await;
    let (up2, hits2) = spawn_decision_upstream(vec![(200, OR_RESPONSE)]).await;
    let state = make_state(test_db().await).await;
    let models = PlatformModels {
        jev: vec!["jev-latest".into()],
        ..Default::default()
    };
    setup_decision_group(&state, "gkfo1", &up1, models.clone(), None, Protocol::TypeSafe).await;
    // 第二平台挂同一 group：直接造 group 复用 setup 不便，手写第二个平台进同组
    let g = aidog_db::list_groups(&state.db).await.unwrap()
        .into_iter().find(|g| g.group_key == "gkfo1").unwrap();
    let plat2 = aidog_db::create_platform(
        &state.db,
        CreatePlatform {
            name: "stub-fo2".into(),
            platform_type: Protocol::OpenAI,
            base_url: up2.clone(),
            api_key: "sk-up".into(),
            extra: String::new(),
            models: Some(models),
            available_models: None,
            endpoints: Some(vec![crate::gateway::models::PlatformEndpoint {
                protocol: Protocol::OpenAI,
                base_url: format!("{up2}/v1"),
                client_type: "default".into(),
                coding_plan: false,
            }]),
            manual_budgets: None,
            auto_group: None,
            join_group_ids: None,
            expires_at: None,
            quota_source: None,
        },
    )
    .await
    .unwrap();
    aidog_db::set_group_platforms(
        &state.db,
        g.id,
        &[
            GroupPlatformInput {
                platform_id: aidog_db::list_platforms(&state.db).await.unwrap()
                    .into_iter().find(|p| p.name == "stub-gkfo1").unwrap().id,
                priority: Some(0),
                weight: Some(1),
                level_priority: Some(0),
            },
            GroupPlatformInput {
                platform_id: plat2.id,
                priority: Some(1),
                weight: Some(1),
                level_priority: Some(0),
            },
        ],
    )
    .await
    .unwrap();

    let resp = handle_proxy(AxumState(state.clone()), decision_request("gkfo1", "m")).await;
    assert_eq!(resp.status(), StatusCode::OK, "529 must fail over (R13)");
    assert_eq!(hits1.lock().unwrap().len(), 1);
    assert_eq!(hits2.lock().unwrap().len(), 1, "second platform received the retry");
    let log = last_log_for(&state, "gkfo1").await;
    assert_eq!(log.retry_count, 1);
    assert_eq!(log.attempts.len(), 2);
}

/// R7：分组无决策平台 → 400 + proxy_log blocked_by/blocked_reason。
#[tokio::test]
async fn decision_no_platform_400_and_audit_log() {
    let state = make_state(test_db().await).await;
    setup_group_with_upstream(&state, "gknone", "https://example.invalid").await;

    let resp = handle_proxy(AxumState(state.clone()), decision_request("gknone", "m")).await;
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let log = last_log_for(&state, "gknone").await;
    assert_eq!(log.status_code, 400);
    assert_eq!(log.blocked_by, "router");
    assert_eq!(log.blocked_reason, "no_decision_platform");
    assert_eq!(log.est_cost, 0.0);
}

/// R3 端到端：聊天请求打到只有纯决策平台的分组 → 400 + blocked_reason=no_chat_platform。
#[tokio::test]
async fn chat_no_chat_platform_400_and_audit_log() {
    let state = make_state(test_db().await).await;
    setup_decision_group(
        &state,
        "gknochat",
        "https://example.invalid",
        PlatformModels {
            jev: vec!["jev-latest".into()],
            ..Default::default()
        },
        None,
        Protocol::TypeSafe,
    )
    .await;
    let resp = handle_proxy(
        AxumState(state.clone()),
        messages_request("gknochat", r#"{"model":"claude-3","max_tokens":10,"messages":[{"role":"user","content":"hi"}]}"#),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let log = last_log_for(&state, "gknochat").await;
    assert_eq!(log.status_code, 400);
    assert_eq!(log.blocked_by, "router");
    assert_eq!(log.blocked_reason, "no_chat_platform");
    assert_eq!(log.est_cost, 0.0);
}

/// 两个 jev 槽决策平台挂同一 group（priority 0 → up1，1 → up2）。
async fn setup_two_decision_platforms(state: &Arc<ProxyState>, gk: &str, up1: &str, up2: &str) {
    let models = PlatformModels {
        jev: vec!["jev-latest".into()],
        ..Default::default()
    };
    setup_decision_group(state, gk, up1, models.clone(), None, Protocol::TypeSafe).await;
    let first = aidog_db::list_platforms(&state.db).await.unwrap()
        .into_iter().find(|p| p.name == format!("stub-{gk}")).unwrap();
    let second = aidog_db::create_platform(
        &state.db,
        CreatePlatform {
            name: format!("stub-{gk}-2"),
            platform_type: Protocol::OpenAI,
            base_url: up2.to_string(),
            api_key: "sk-up".into(),
            extra: String::new(),
            models: Some(models),
            available_models: None,
            endpoints: Some(vec![crate::gateway::models::PlatformEndpoint {
                protocol: Protocol::TypeSafe,
                base_url: format!("{up2}/v1"),
                client_type: "default".into(),
                coding_plan: false,
            }]),
            manual_budgets: None,
            auto_group: None,
            join_group_ids: None,
            expires_at: None,
            quota_source: None,
        },
    )
    .await
    .unwrap();
    let g = aidog_db::list_groups(&state.db).await.unwrap()
        .into_iter().find(|g| g.group_key == gk).unwrap();
    let gp = |platform_id, priority| GroupPlatformInput {
        platform_id,
        priority: Some(priority),
        weight: Some(1),
        level_priority: Some(0),
    };
    aidog_db::set_group_platforms(&state.db, g.id, &[gp(first.id, 0), gp(second.id, 1)])
        .await
        .unwrap();
}

/// R13：决策路径上游 429 / 401 换下个候选；400 / 422 是请求硬错，不换平台直接回给客户端。
#[tokio::test]
async fn decision_upstream_4xx_handling() {
    for (code, fails_over) in [(429u16, true), (401, true), (400, false), (422, false)] {
        let gk = format!("gk4xx{code}");
        let (up1, hits1) = spawn_decision_upstream(vec![(code, r#"{"error":{"message":"nope"}}"#)]).await;
        let (up2, hits2) = spawn_decision_upstream(vec![(200, OR_RESPONSE)]).await;
        let state = make_state(test_db().await).await;
        setup_two_decision_platforms(&state, &gk, &up1, &up2).await;

        let resp = handle_proxy(AxumState(state.clone()), decision_request(&gk, "m")).await;
        assert_eq!(hits1.lock().unwrap().len(), 1, "{code}: first platform hit");
        let log = last_log_for(&state, &gk).await;
        if fails_over {
            assert_eq!(resp.status(), StatusCode::OK, "{code} must fail over");
            assert_eq!(hits2.lock().unwrap().len(), 1, "{code}: second platform received retry");
            assert_eq!(log.status_code, 200);
            assert_eq!(log.attempts.len(), 2);
        } else {
            assert_eq!(resp.status().as_u16(), code, "{code} returned to client as-is");
            assert_eq!(hits2.lock().unwrap().len(), 0, "{code} must not retry next platform");
            assert_eq!(log.status_code, code as i32);
        }
    }
}

/// 日志开关全关：决策请求的用户侧 / 上游侧 body 都不入库，只留元数据。
#[tokio::test]
async fn decision_log_switches_off_drop_bodies() {
    let (upstream, _hits) = spawn_decision_upstream(vec![(200, OR_RESPONSE)]).await;
    let state = make_state(test_db().await).await;
    setup_decision_group(
        &state,
        "gkdlog",
        &upstream,
        PlatformModels {
            jev: vec!["jev-latest".into()],
            ..Default::default()
        },
        None,
        Protocol::TypeSafe,
    )
    .await;
    set_log_settings(
        &state,
        ProxyLogSettings {
            log_user_request: false,
            log_upstream_request: false,
            ..Default::default()
        },
    )
    .await;

    let resp = handle_proxy(AxumState(state.clone()), decision_request("gkdlog", "m")).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let row = last_log_for(&state, "gkdlog").await;
    assert!(row.request_body.is_empty(), "request_body");
    assert!(row.user_response_body.is_empty(), "user_response_body");
    assert!(row.upstream_request_body.is_empty(), "upstream_request_body");
    assert!(row.response_body.is_empty(), "response_body");
    assert_eq!(row.status_code, 200);
    assert_eq!(row.input_tokens, 1000, "metadata kept");
}

/// 把 group 内全部平台设为「高峰禁用 + 全天高峰窗口」。
async fn set_all_platforms_peak_disabled(state: &Arc<ProxyState>) {
    for p in aidog_db::list_platforms(&state.db).await.unwrap() {
        aidog_db::update_platform(&state.db, aidog_db::models::UpdatePlatform {
            id: p.id, name: None, platform_type: None, base_url: None, api_key: None,
            extra: Some(r#"{"disable_during_peak":true,"peak":[{"start_hour":0,"end_hour":24,"multiplier":1.5}]}"#.to_string()),
            models: None, available_models: None, endpoints: None,
            enabled: None, status: None, manual_budgets: None,
            join_group_ids: None, expires_at: None, quota_source: None,
        })
        .await
        .unwrap();
    }
}

/// 决策路径高峰禁用对齐聊天路径：503 + blocked_reason='peak'（不是 400）。
#[tokio::test]
async fn decision_peak_disabled_503_and_audit_log() {
    let state = make_state(test_db().await).await;
    setup_decision_group(
        &state,
        "gkpeak",
        "https://example.invalid",
        PlatformModels {
            jev: vec!["jev-latest".into()],
            ..Default::default()
        },
        None,
        Protocol::TypeSafe,
    )
    .await;
    set_all_platforms_peak_disabled(&state).await;

    let resp = handle_proxy(AxumState(state.clone()), decision_request("gkpeak", "m")).await;
    assert_eq!(resp.status(), StatusCode::SERVICE_UNAVAILABLE);
    let log = last_log_for(&state, "gkpeak").await;
    assert_eq!(log.status_code, 503);
    assert_eq!(log.blocked_by, "router");
    assert_eq!(log.blocked_reason, "peak");
    assert_eq!(log.est_cost, 0.0);
}

/// 聊天路径高峰禁用（共用 route_fail_response 后行为不变）：503 + blocked_reason='peak'。
#[tokio::test]
async fn chat_peak_disabled_503_and_audit_log() {
    let state = make_state(test_db().await).await;
    setup_group_with_upstream(&state, "gkchatpeak", "https://example.invalid").await;
    set_all_platforms_peak_disabled(&state).await;

    let resp = handle_proxy(
        AxumState(state.clone()),
        messages_request("gkchatpeak", r#"{"model":"claude-3","max_tokens":10,"messages":[{"role":"user","content":"hi"}]}"#),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::SERVICE_UNAVAILABLE);
    let log = last_log_for(&state, "gkchatpeak").await;
    assert_eq!(log.status_code, 503);
    assert_eq!(log.blocked_by, "router");
    assert_eq!(log.blocked_reason, "peak");
}

/// 高峰禁用开关再关掉（extra 移除 disable_during_peak 键，前端 false=删键语义）后恢复路由。
#[tokio::test]
async fn chat_peak_disabled_toggle_off_restores_routing() {
    let state = make_state(test_db().await).await;
    let upstream = spawn_stub_upstream(200, ANTHROPIC_OK).await;
    setup_group_with_upstream(&state, "gkpeakoff", &upstream).await;
    set_all_platforms_peak_disabled(&state).await;

    let resp = handle_proxy(
        AxumState(state.clone()),
        messages_request("gkpeakoff", r#"{"model":"claude-3","max_tokens":10,"messages":[{"role":"user","content":"hi"}]}"#),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::SERVICE_UNAVAILABLE, "开启时应 503 peak");

    // 关闭：键从 extra 删除（峰值窗口保留），update_platform 持久化后应恢复转发
    for p in aidog_db::list_platforms(&state.db).await.unwrap() {
        aidog_db::update_platform(&state.db, aidog_db::models::UpdatePlatform {
            id: p.id, name: None, platform_type: None, base_url: None, api_key: None,
            extra: Some(r#"{"peak":[{"start_hour":0,"end_hour":24,"multiplier":1.5}]}"#.to_string()),
            models: None, available_models: None, endpoints: None,
            enabled: None, status: None, manual_budgets: None,
            join_group_ids: None, expires_at: None, quota_source: None,
        })
        .await
        .unwrap();
    }

    let resp2 = handle_proxy(
        AxumState(state.clone()),
        messages_request("gkpeakoff", r#"{"model":"claude-3","max_tokens":10,"messages":[{"role":"user","content":"hi"}]}"#),
    )
    .await;
    assert_eq!(resp2.status(), StatusCode::OK, "关闭后应恢复 200 转发");
}

/// 票 06（用户裁决 2026-10-16）：分组平台 platform_type 为枚举外值（协议未知）→
/// 503 + proxy_log blocked_by='router', blocked_reason='unknown_protocol', est_cost=0。
/// DB 直写脏值模拟降级读新数据（UpdatePlatform API 走 Option<Protocol>，写不进枚举外值）。
#[tokio::test]
async fn chat_unknown_protocol_503_and_audit_log() {
    let state = make_state(test_db().await).await;
    setup_group_with_upstream(&state, "gkunknown", "https://example.invalid").await;
    for p in aidog_db::list_platforms(&state.db).await.unwrap() {
        let pid = p.id as i64;
        state
            .db
            .call_platform_traced(None, std::panic::Location::caller(), move |conn| {
                conn.execute(
                    "UPDATE platform SET platform_type='no_such_protocol' WHERE id=?1",
                    rusqlite::params![pid],
                )?;
                Ok(())
            })
            .await
            .unwrap();
    }
    state.db.invalidate_group_details_cache();

    let resp = handle_proxy(
        AxumState(state.clone()),
        messages_request("gkunknown", r#"{"model":"claude-3","max_tokens":10,"messages":[{"role":"user","content":"hi"}]}"#),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::SERVICE_UNAVAILABLE);
    let log = last_log_for(&state, "gkunknown").await;
    assert_eq!(log.status_code, 503);
    assert_eq!(log.blocked_by, "router");
    assert_eq!(log.blocked_reason, "unknown_protocol");
    assert_eq!(log.est_cost, 0.0, "unknown protocol must not be priced");
}

/// 注册一个透传平台（Protocol::Passthrough）+ group 并关联。
async fn setup_passthrough_platform_group(state: &Arc<ProxyState>, gk: &str, base_url: &str) {
    setup_passthrough_platform_group_extra(state, gk, base_url, "").await;
}

/// 同上，平台 extra 可指定（认证头配置 auth_header / auth_template）。
async fn setup_passthrough_platform_group_extra(
    state: &Arc<ProxyState>,
    gk: &str,
    base_url: &str,
    extra: &str,
) {
    let plat = aidog_db::create_platform(
        &state.db,
        CreatePlatform {
            name: "pt".into(),
            platform_type: Protocol::Passthrough,
            base_url: base_url.to_string(),
            api_key: "sk-up".into(),
            extra: extra.to_string(),
            models: None,
            available_models: None,
            endpoints: None,
            manual_budgets: None,
            auto_group: None,
            join_group_ids: None,
            expires_at: None,
            quota_source: None,
        },
    )
    .await
    .unwrap();
    let group = aidog_db::create_group(&state.db, aidog_db::test_support::sample_group(gk, vec![]))
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

/// ADR 0008：透传平台不识别协议，body 解析不出（非 JSON / 不符合 path 推断的协议 /
/// 未知 path / count_tokens 等子端点）都必须原样转发到上游，不得 400。
/// 能解析出 usage 时照常记 tokens。
#[tokio::test]
async fn passthrough_platform_relays_unparseable_requests_verbatim() {
    const OPENAI_USAGE: &str = r#"{"id":"x","usage":{"prompt_tokens":7,"completion_tokens":2}}"#;
    let (upstream, seen) = spawn_recording_upstream(OPENAI_USAGE).await;
    let state = make_state(test_db().await).await;
    setup_passthrough_platform_group(&state, "gkraw", &upstream).await;

    let cases: [(&str, &str, &str); 9] = [
        ("POST", "/v1/messages", "not json at all"),
        ("POST", "/v1/messages", r#"{"foo":1}"#),
        ("POST", "/v1/custom/thing", r#"{"input":"x"}"#),
        ("POST", "/v1/messages/count_tokens", r#"{"model":"m","messages":[]}"#),
        ("GET", "/v1/files", ""),
        ("GET", "/v1/responses/resp_1", ""),
        ("POST", "/v1/systemone", r#"{"answers":[]}"#),
        ("POST", "/v1/embeddings", r#"{"model":"m","input":"x"}"#),
        ("GET", "/v1/models", ""),
    ];
    for (method, path, body) in cases {
        let req = HttpRequest::builder()
            .method(method)
            .uri(path)
            .header("authorization", "Bearer gkraw")
            .header("content-type", "application/json")
            .body(Body::from(body.to_string()))
            .unwrap();
        let resp = handle_proxy(AxumState(state.clone()), req).await;
        assert_eq!(resp.status(), StatusCode::OK, "{method} {path} body={body}");
        let got = seen.lock().unwrap().last().cloned().expect("upstream not hit");
        assert_eq!(got, (path.to_string(), body.to_string()), "{method} {path}");
    }
    assert_eq!(seen.lock().unwrap().len(), cases.len());

    let log = last_log_for(&state, "gkraw").await;
    assert_eq!(log.status_code, 200);
    assert_eq!((log.input_tokens, log.output_tokens), (7, 2));
}

// ═══════════════ ADR 0009：入站密钥通道 + 未匹配分组日志 ═══════════════

/// 记录每次上游请求 (uri, 小写头名→值) 的 stub 上游，返回固定 body。
type SeenHeaders = Arc<std::sync::Mutex<Vec<(String, std::collections::HashMap<String, String>)>>>;
async fn spawn_header_recording_upstream(body: &'static str) -> (String, SeenHeaders) {
    use axum::routing::any;
    let rec: SeenHeaders = Arc::new(std::sync::Mutex::new(Vec::new()));
    let rec_srv = rec.clone();
    let app = axum::Router::new().fallback(any(move |req: HttpRequest<Body>| {
        let rec = rec_srv.clone();
        async move {
            let headers = req
                .headers()
                .iter()
                .map(|(k, v)| (k.as_str().to_string(), v.to_str().unwrap_or("").to_string()))
                .collect();
            rec.lock().unwrap().push((req.uri().to_string(), headers));
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

fn request_with(uri: &str, headers: &[(&str, &str)]) -> Request {
    let mut b = HttpRequest::builder()
        .method("POST")
        .uri(uri)
        .header("content-type", "application/json");
    for (k, v) in headers {
        b = b.header(*k, *v);
    }
    b.body(Body::from(
        r#"{"model":"claude-3","messages":[{"role":"user","content":"hi"}]}"#.to_string(),
    ))
    .unwrap()
}

/// 用户插话（2026-10-22）：开着「记录用户请求」时，未匹配分组也要记全请求与回给客户端的 404。
/// 404 文本只进用户侧两列，上游侧留空（没发生上游请求）。
#[tokio::test]
async fn no_group_404_records_user_side_request_and_response() {
    let state = make_state(test_db().await).await;
    set_log_settings(
        &state,
        ProxyLogSettings {
            log_user_request: true,
            log_upstream_request: false,
            ..Default::default()
        },
    )
    .await;
    let resp = handle_proxy(AxumState(state.clone()), messages_request("ghost", r#"{"model":"m"}"#)).await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);

    let row = last_log_for(&state, "").await;
    assert_eq!(row.status_code, 404);
    assert!(row.request_headers.contains("authorization"), "{}", row.request_headers);
    assert!(!row.request_headers.contains("ghost"), "密钥须脱敏: {}", row.request_headers);
    assert_eq!(row.request_body, r#"{"model":"m"}"#);
    assert!(row.user_response_body.contains("no matching group"), "{}", row.user_response_body);
    assert!(!row.user_response_body.contains("ghost"));
    assert!(row.user_response_headers.contains("content-type"));
    assert!(row.response_body.is_empty(), "上游侧不应有内容: {}", row.response_body);
}

/// WS 握手路径（不经 handle_proxy_core）也要识别入站客户端并记原始 UA。
#[tokio::test]
async fn ws_handshake_records_client_and_user_agent() {
    let state = make_state(test_db().await).await;
    let req = HttpRequest::builder()
        .method("GET")
        .uri("/v1/responses")
        .header("authorization", "Bearer ghost")
        .header("upgrade", "websocket")
        .header("connection", "Upgrade")
        .header("user-agent", "codex-tui/0.38.0 (Mac OS 15.0; arm64)")
        .body(Body::empty())
        .unwrap();
    let resp = handle_proxy(AxumState(state.clone()), req).await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    let row = last_log_for(&state, "").await;
    assert_eq!(row.client, "codex_tui");
    assert_eq!(row.user_agent, "codex-tui/0.38.0 (Mac OS 15.0; arm64)");
}

/// ADR 0009 决策 1：各默认通道都能定组；多个通道同时出现时第一个对上分组的胜
/// （Authorization 里是客户端自带的上游 key，x-api-key 才是分组密钥）。
#[tokio::test]
async fn default_key_channels_resolve_group_first_match() {
    let upstream = spawn_stub_upstream(200, ANTHROPIC_OK).await;
    let state = make_state(test_db().await).await;
    setup_group_with_upstream(&state, "gkch", &upstream).await;

    let cases: [(&str, &[(&str, &str)]); 8] = [
        ("/v1/messages", &[("authorization", "bearer gkch")]),
        ("/v1/messages", &[("authorization", "Key gkch")]),
        ("/v1/messages", &[("authorization", "gkch")]),
        ("/v1/messages", &[("api-key", "gkch")]),
        ("/v1/messages", &[("x-goog-api-key", "gkch")]),
        ("/v1/messages?key=gkch", &[]),
        ("/v1/messages?api_key=gkch", &[]),
        (
            "/v1/messages",
            &[("authorization", "Bearer sk-upstream-own"), ("x-api-key", "gkch")],
        ),
    ];
    for (uri, headers) in cases {
        let resp = handle_proxy(AxumState(state.clone()), request_with(uri, headers)).await;
        assert_eq!(resp.status(), StatusCode::OK, "{uri} {headers:?}");
    }
}

/// ADR 0009 决策 3：透传平台未显式配置认证头 → 客户端从哪个通道传来就在同一通道换成平台 key，
/// 其它凭证头一律剥。
#[tokio::test]
async fn passthrough_replaces_key_in_the_channel_client_used() {
    let (upstream, seen) = spawn_header_recording_upstream(r#"{"ok":true}"#).await;
    let state = make_state(test_db().await).await;
    setup_passthrough_platform_group(&state, "gkpt", &upstream).await;

    let last = || seen.lock().unwrap().last().cloned().expect("upstream not hit");

    let resp = handle_proxy(AxumState(state.clone()), request_with("/v1/x", &[("x-goog-api-key", "gkpt")])).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let (_, h) = last();
    assert_eq!(h.get("x-goog-api-key").map(String::as_str), Some("sk-up"));
    assert!(!h.contains_key("authorization"));

    let resp = handle_proxy(AxumState(state.clone()), request_with("/v1/x", &[("authorization", "Key gkpt")])).await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(last().1.get("authorization").map(String::as_str), Some("Key sk-up"));

    let resp = handle_proxy(
        AxumState(state.clone()),
        request_with("/v1/x?a=1&key=gkpt", &[("authorization", "Bearer sk-client-own")]),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let (uri, h) = last();
    assert_eq!(uri, "/v1/x?a=1&key=sk-up");
    assert!(!h.contains_key("authorization"), "未定组的凭证头须剥: {h:?}");

    let resp = handle_proxy(
        AxumState(state.clone()),
        request_with("/v1/x", &[("authorization", "Bearer sk-client-own"), ("x-api-key", "gkpt")]),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let (_, h) = last();
    assert_eq!(h.get("x-api-key").map(String::as_str), Some("sk-up"));
    assert!(!h.contains_key("authorization"));
}

/// ADR 0009 决策 2：透传平台显式配置的认证头兼作入站密钥通道；出站按配置注入，
/// 日志里该头值脱敏。
#[tokio::test]
async fn passthrough_auth_config_doubles_as_inbound_key_channel() {
    let (upstream, seen) = spawn_header_recording_upstream(r#"{"ok":true}"#).await;
    let state = make_state(test_db().await).await;
    set_log_settings(
        &state,
        ProxyLogSettings {
            log_user_request: true,
            log_upstream_request: true,
            ..Default::default()
        },
    )
    .await;
    setup_passthrough_platform_group_extra(
        &state,
        "gkcustom",
        &upstream,
        r#"{"auth_header":"X-Token","auth_template":"Token {key}"}"#,
    )
    .await;

    let resp = handle_proxy(AxumState(state.clone()), request_with("/v1/x", &[("x-token", "token gkcustom")])).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let (_, h) = seen.lock().unwrap().last().cloned().unwrap();
    assert_eq!(h.get("x-token").map(String::as_str), Some("Token sk-up"));

    let row = last_log_for(&state, "gkcustom").await;
    assert!(!row.request_headers.contains("gkcustom"), "{}", row.request_headers);
    assert!(row.request_headers.contains("x-token"));
}

/// ADR 0009 决策 4：自定义通道头与 api-key 不得透给转换路径的上游（装的是分组密钥）。
#[tokio::test]
async fn convert_path_strips_key_channel_headers() {
    let (upstream, seen) = spawn_header_recording_upstream(ANTHROPIC_OK).await;
    let state = make_state(test_db().await).await;
    setup_group_with_upstream(&state, "gkconv", &upstream).await;
    // 另一个分组的透传平台声明了 X-Token 通道；运行时对所有请求生效
    setup_passthrough_platform_group_extra(
        &state,
        "gkother",
        "http://127.0.0.1:9",
        r#"{"auth_header":"X-Token","auth_template":"{key}"}"#,
    )
    .await;

    for headers in [&[("x-token", "gkconv")][..], &[("api-key", "gkconv")][..]] {
        let resp = handle_proxy(AxumState(state.clone()), request_with("/v1/messages", headers)).await;
        assert_eq!(resp.status(), StatusCode::OK, "{headers:?}");
        let (_, h) = seen.lock().unwrap().last().cloned().unwrap();
        assert!(!h.contains_key("x-token"), "{h:?}");
        assert!(!h.contains_key("api-key"), "{h:?}");
        assert_eq!(h.get("x-api-key").map(String::as_str), Some("sk-up"));
    }
}

// ── 票 01：路由决策快照 ──

async fn create_plain_platform(
    state: &Arc<ProxyState>,
    name: &str,
    base_url: &str,
) -> aidog_db::models::Platform {
    aidog_db::create_platform(
        &state.db,
        CreatePlatform {
            name: name.into(),
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
            expires_at: None,
            quota_source: None,
        },
    )
    .await
    .unwrap()
}

/// 隔离实例跑真实请求（票 01 acceptance）：快照随主记录落库，直接查库可见，
/// 候选数与分组配置一致；手动禁用候选带 disabled-manual 标签。
/// 候选须含两个 Enabled 平台，否则命中 sole_platform 单启用短路。
#[tokio::test]
async fn routing_snapshot_records_excluded_candidates() {
    let upstream = spawn_stub_upstream(200, ANTHROPIC_OK).await;
    let state = make_state(test_db().await).await;
    let live = create_plain_platform(&state, "live", &upstream).await;
    let live2 = create_plain_platform(&state, "live2", "http://127.0.0.1:9").await;
    let off = create_plain_platform(&state, "off", "http://127.0.0.1:9").await;
    aidog_db::update_platform(
        &state.db,
        aidog_db::models::UpdatePlatform {
            id: off.id,
            status: Some(aidog_db::models::PlatformStatus::Disabled),
            name: None,
            platform_type: None,
            base_url: None,
            api_key: None,
            extra: None,
            models: None,
            available_models: None,
            endpoints: None,
            enabled: None,
            manual_budgets: None,
            join_group_ids: None,
            expires_at: None,
            quota_source: None,
        },
    )
    .await
    .unwrap();
    let group = aidog_db::create_group(&state.db, aidog_db::test_support::sample_group("gksnap", vec![]))
        .await
        .unwrap();
    aidog_db::set_group_platforms(
        &state.db,
        group.id,
        &[
            GroupPlatformInput { platform_id: live.id, priority: Some(0), weight: Some(1), level_priority: Some(5) },
            GroupPlatformInput { platform_id: live2.id, priority: Some(1), weight: Some(1), level_priority: Some(5) },
            GroupPlatformInput { platform_id: off.id, priority: Some(2), weight: Some(1), level_priority: Some(5) },
        ],
    )
    .await
    .unwrap();

    let req = messages_request(
        "gksnap",
        r#"{"model":"claude-3","messages":[{"role":"user","content":"hi"}]}"#,
    );
    let resp = handle_proxy(AxumState(state.clone()), req).await;
    assert_eq!(resp.status(), StatusCode::OK);

    let log = last_log_for(&state, "gksnap").await;
    let rs = log.routing.expect("routing snapshot must be recorded");
    assert!(rs.mode.is_some());
    assert_eq!(rs.entries.len(), 3, "候选数应与分组配置一致");
    let e_live = rs.entries.iter().find(|e| e.platform_id == live.id).unwrap();
    let e_off = rs.entries.iter().find(|e| e.platform_id == off.id).unwrap();
    assert_eq!(e_live.reason, aidog_db::models::RouteReason::Selected);
    assert_eq!(e_live.order, 1);
    assert_eq!(
        e_off.reason,
        aidog_db::models::RouteReason::DisabledManual
    );
}

/// 死站跳过补记（票 01）：同 base_url（connect 拒绝死站）第二候选跳过时标
/// skipped-dead-upstream。用真 connect 失败（127.0.0.1:1 RST）：reset 上游是
/// 连接建立后被重置，不算 connect_failed，不触发 R3 死站联动。
#[tokio::test]
async fn routing_snapshot_marks_skipped_dead_upstream() {
    let state = make_state(test_db().await).await;
    let p1 = create_plain_platform(&state, "d1", "http://127.0.0.1:1").await;
    let p2 = create_plain_platform(&state, "d2", "http://127.0.0.1:1").await;
    let group = aidog_db::create_group(&state.db, aidog_db::test_support::sample_group("gkdead", vec![]))
        .await
        .unwrap();
    aidog_db::set_group_platforms(
        &state.db,
        group.id,
        &[
            GroupPlatformInput { platform_id: p1.id, priority: Some(0), weight: Some(1), level_priority: Some(5) },
            GroupPlatformInput { platform_id: p2.id, priority: Some(0), weight: Some(1), level_priority: Some(5) },
        ],
    )
    .await
    .unwrap();

    let req = messages_request(
        "gkdead",
        r#"{"model":"claude-3","messages":[{"role":"user","content":"hi"}]}"#,
    );
    let resp = handle_proxy(AxumState(state.clone()), req).await;
    assert_eq!(resp.status(), StatusCode::SERVICE_UNAVAILABLE);

    let log = last_log_for(&state, "gkdead").await;
    let rs = log.routing.expect("routing snapshot must be recorded");
    assert_eq!(rs.entries.len(), 2);
    let reasons: Vec<_> = rs.entries.iter().map(|e| e.reason).collect();
    // 同站 connect 失败：先试的标 selected（尝试过、失败），同站兄弟标 skipped-dead-upstream
    assert_eq!(
        reasons.iter().filter(|r| **r == aidog_db::models::RouteReason::Selected).count(),
        1
    );
    assert_eq!(
        reasons.iter().filter(|r| **r == aidog_db::models::RouteReason::SkippedDeadUpstream).count(),
        1
    );
}

/// 全组额度用完（票 03）：多平台组两平台均额度冷却 → 402 + 文案 + 审计行
/// blocked_by='router', blocked_reason='quota_exhausted', status_code=402, est_cost=0。
/// 三文件库 fixture（platform 冷却状态跨库，db-pool-routing.md）；上游为不可达占位，路由层即拦。
#[tokio::test]
async fn chat_all_quota_cooled_402_and_audit_log() {
    let (db, _dir) = aidog_db::test_support::three_file_db().await;
    let state = make_state(db).await;
    let p1 = create_plain_platform(&state, "q1", "http://127.0.0.1:1").await;
    let p2 = create_plain_platform(&state, "q2", "http://127.0.0.1:1").await;
    let group = aidog_db::create_group(&state.db, aidog_db::test_support::sample_group("gkquota", vec![]))
        .await
        .unwrap();
    aidog_db::set_group_platforms(
        &state.db,
        group.id,
        &[
            GroupPlatformInput { platform_id: p1.id, priority: Some(0), weight: Some(1), level_priority: Some(5) },
            GroupPlatformInput { platform_id: p2.id, priority: Some(0), weight: Some(1), level_priority: Some(5) },
        ],
    )
    .await
    .unwrap();
    let until = aidog_db::now() + 3_600_000;
    state.scheduler.set_quota_cooldown(p1.id, until);
    state.scheduler.set_quota_cooldown(p2.id, until + 60_000);

    let resp = handle_proxy(
        AxumState(state.clone()),
        messages_request("gkquota", r#"{"model":"claude-3","max_tokens":10,"messages":[{"role":"user","content":"hi"}]}"#),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::PAYMENT_REQUIRED);
    let body = axum::body::to_bytes(resp.into_body(), 1 << 20).await.unwrap();
    let v: Value = serde_json::from_slice(&body).expect("json error body");
    // anthropic 入站（/v1/messages）→ body 带 anthropic 错误外壳
    assert_eq!(v["type"], "error");
    assert_eq!(v["error"]["type"], "quota_exhausted");
    assert!(
        v["error"]["message"].as_str().is_some_and(|m| m.contains("Insufficient balance") || m.contains("余额不足")),
        "non coding-plan group says balance insufficient: {v}"
    );
    let log = last_log_for(&state, "gkquota").await;
    assert_eq!(log.status_code, 402);
    assert_eq!(log.blocked_by, "router");
    assert_eq!(log.blocked_reason, "quota_exhausted");
    assert_eq!(log.est_cost, 0.0);
}


/// 同上但 openai 入站（/v1/chat/completions）→ body 无 anthropic 外壳，`{"error":{...}}`。
#[tokio::test]
async fn chat_all_quota_cooled_402_openai_shape() {
    let (db, _dir) = aidog_db::test_support::three_file_db().await;
    let state = make_state(db).await;
    let p1 = create_plain_platform(&state, "qo1", "http://127.0.0.1:1").await;
    let p2 = create_plain_platform(&state, "qo2", "http://127.0.0.1:1").await;
    let group = aidog_db::create_group(&state.db, aidog_db::test_support::sample_group("gkquotaopen", vec![]))
        .await
        .unwrap();
    aidog_db::set_group_platforms(
        &state.db,
        group.id,
        &[
            GroupPlatformInput { platform_id: p1.id, priority: Some(0), weight: Some(1), level_priority: Some(5) },
            GroupPlatformInput { platform_id: p2.id, priority: Some(0), weight: Some(1), level_priority: Some(5) },
        ],
    )
    .await
    .unwrap();
    let until = aidog_db::now() + 3_600_000;
    state.scheduler.set_quota_cooldown(p1.id, until);
    state.scheduler.set_quota_cooldown(p2.id, until + 60_000);

    let req = HttpRequest::builder()
        .method("POST")
        .uri("/v1/chat/completions")
        .header("authorization", "Bearer gkquotaopen")
        .header("content-type", "application/json")
        .body(Body::from(r#"{"model":"claude-3","max_tokens":10,"messages":[{"role":"user","content":"hi"}]}"#))
        .unwrap();
    let resp = handle_proxy(AxumState(state.clone()), req).await;
    assert_eq!(resp.status(), StatusCode::PAYMENT_REQUIRED);
    let body = axum::body::to_bytes(resp.into_body(), 1 << 20).await.unwrap();
    let v: Value = serde_json::from_slice(&body).expect("json error body");
    assert_eq!(v["error"]["type"], "quota_exhausted");
    assert!(v.get("type").is_none(), "openai 入站不带 anthropic 外壳: {v}");
}

/// 直通模型（直接模型转发）：声明该模型的平台全部额度冷却 → 同样 402 quota_exhausted（不是 503）。
/// 组 ≥2、两平台均声明 m1（直通链 len>1 guard）。
#[tokio::test]
async fn chat_direct_model_all_quota_cooled_402() {
    let (db, _dir) = aidog_db::test_support::three_file_db().await;
    let state = make_state(db).await;
    setup_group_with_upstream(&state, "gkdirquota", "https://example.invalid").await;
    let second = aidog_db::create_platform(
        &state.db,
        aidog_db::test_support::sample_platform("dsecond"),
    )
    .await
    .unwrap();
    aidog_db::update_platform(&state.db, aidog_db::models::UpdatePlatform {
        id: second.id,
        name: None, platform_type: None, base_url: None, api_key: None,
        extra: Some(r#"{"direct_models":["m1"]}"#.to_string()),
        models: None, available_models: None, endpoints: None,
        enabled: None, status: None, manual_budgets: None, join_group_ids: None, expires_at: None, quota_source: None,
    })
    .await
    .unwrap();
    let group = aidog_db::list_groups(&state.db)
        .await
        .unwrap()
        .into_iter()
        .find(|g| g.group_key == "gkdirquota")
        .unwrap();
    let existing = aidog_db::get_group_platforms(&state.db, group.id).await.unwrap();
    // 首个平台也声明 m1
    aidog_db::update_platform(&state.db, aidog_db::models::UpdatePlatform {
        id: existing[0].platform.id,
        name: None, platform_type: None, base_url: None, api_key: None,
        extra: Some(r#"{"direct_models":["m1"]}"#.to_string()),
        models: None, available_models: None, endpoints: None,
        enabled: None, status: None, manual_budgets: None, join_group_ids: None, expires_at: None, quota_source: None,
    })
    .await
    .unwrap();
    aidog_db::set_group_platforms(&state.db, group.id, &[
        GroupPlatformInput { platform_id: existing[0].platform.id, priority: Some(0), weight: Some(1), level_priority: Some(5) },
        GroupPlatformInput { platform_id: second.id, priority: Some(1), weight: Some(1), level_priority: Some(5) },
    ])
    .await
    .unwrap();
    state.db.invalidate_group_details_cache();
    let until = aidog_db::now() + 3_600_000;
    state.scheduler.set_quota_cooldown(existing[0].platform.id, until);
    state.scheduler.set_quota_cooldown(second.id, until + 60_000);

    let resp = handle_proxy(
        AxumState(state.clone()),
        messages_request("gkdirquota", r#"{"model":"m1","max_tokens":10,"messages":[{"role":"user","content":"hi"}]}"#),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::PAYMENT_REQUIRED);
    let log = last_log_for(&state, "gkdirquota").await;
    assert_eq!(log.status_code, 402);
    assert_eq!(log.blocked_reason, "quota_exhausted");
    assert_eq!(log.est_cost, 0.0);
}

/// 直通模型混合原因（额度冷却 + 手动 Disabled）→ 仍 503 direct_model_unavailable。
#[tokio::test]
async fn chat_direct_model_mixed_reason_still_503() {
    let (db, _dir) = aidog_db::test_support::three_file_db().await;
    let state = make_state(db).await;
    setup_group_with_upstream(&state, "gkdirmix", "https://example.invalid").await;
    let second = aidog_db::create_platform(
        &state.db,
        aidog_db::test_support::sample_platform("dmix"),
    )
    .await
    .unwrap();
    aidog_db::update_platform(&state.db, aidog_db::models::UpdatePlatform {
        id: second.id,
        name: None, platform_type: None, base_url: None, api_key: None,
        extra: Some(r#"{"direct_models":["m1"]}"#.to_string()),
        models: None, available_models: None, endpoints: None,
        enabled: None, status: None, manual_budgets: None, join_group_ids: None, expires_at: None, quota_source: None,
    })
    .await
    .unwrap();
    let group = aidog_db::list_groups(&state.db)
        .await
        .unwrap()
        .into_iter()
        .find(|g| g.group_key == "gkdirmix")
        .unwrap();
    let existing = aidog_db::get_group_platforms(&state.db, group.id).await.unwrap();
    aidog_db::update_platform(&state.db, aidog_db::models::UpdatePlatform {
        id: existing[0].platform.id,
        name: None, platform_type: None, base_url: None, api_key: None,
        extra: Some(r#"{"direct_models":["m1"]}"#.to_string()),
        models: None, available_models: None, endpoints: None,
        enabled: None, status: Some(aidog_db::models::PlatformStatus::Disabled),
        manual_budgets: None, join_group_ids: None, expires_at: None, quota_source: None,
    })
    .await
    .unwrap();
    aidog_db::set_group_platforms(&state.db, group.id, &[
        GroupPlatformInput { platform_id: existing[0].platform.id, priority: Some(0), weight: Some(1), level_priority: Some(5) },
        GroupPlatformInput { platform_id: second.id, priority: Some(1), weight: Some(1), level_priority: Some(5) },
    ])
    .await
    .unwrap();
    state.db.invalidate_group_details_cache();
    state.scheduler.set_quota_cooldown(second.id, aidog_db::now() + 3_600_000);

    let resp = handle_proxy(
        AxumState(state.clone()),
        messages_request("gkdirmix", r#"{"model":"m1","max_tokens":10,"messages":[{"role":"user","content":"hi"}]}"#),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::SERVICE_UNAVAILABLE);
    let log = last_log_for(&state, "gkdirmix").await;
    assert_eq!(log.blocked_reason, "direct_model_unavailable");
}

/// 直接模型转发验收 #4：声明平台全禁用 → handler route-fail 路径落审计行
/// blocked_by='router', blocked_reason='direct_model_unavailable', status_code=503, est_cost=0。
/// 注：直通链入口有 `len() > 1` guard（candidates.rs），字面单平台组不走直通链，需 ≥2 平台组。
/// 三文件库 fixture（proxy_log / platform split 表，db-pool-routing.md）。
#[tokio::test]
async fn chat_direct_model_unavailable_503_and_audit_log() {
    let (db, _dir) = aidog_db::test_support::three_file_db().await;
    let state = make_state(db).await;
    setup_group_with_upstream(&state, "gkdirectfail", "https://example.invalid").await;
    // 补第二个同组平台（组 ≥2 才进直通链）；声明平台 Disabled，它不声明 m1
    let second = aidog_db::create_platform(
        &state.db,
        aidog_db::test_support::sample_platform("second"),
    )
    .await
    .unwrap();
    let group = aidog_db::list_groups(&state.db)
        .await
        .unwrap()
        .into_iter()
        .find(|g| g.group_key == "gkdirectfail")
        .unwrap();
    let existing = aidog_db::get_group_platforms(&state.db, group.id).await.unwrap();
    let mut gps: Vec<crate::gateway::models::GroupPlatformInput> = existing
        .iter()
        .map(|g| crate::gateway::models::GroupPlatformInput {
            platform_id: g.platform.id,
            priority: Some(g.priority),
            weight: Some(1),
            level_priority: Some(g.level_priority),
        })
        .collect();
    gps.push(crate::gateway::models::GroupPlatformInput {
        platform_id: second.id,
        priority: Some(1),
        weight: Some(1),
        level_priority: Some(1),
    });
    aidog_db::set_group_platforms(&state.db, group.id, &gps).await.unwrap();
    // 唯一声明 m1 的平台手动 Disabled → 组内声明平台全不可用
    aidog_db::update_platform(&state.db, aidog_db::models::UpdatePlatform {
        id: existing[0].platform.id,
        name: None, platform_type: None, base_url: None, api_key: None,
        extra: Some(r#"{"direct_models":["m1"]}"#.to_string()),
        models: None, available_models: None, endpoints: None,
        enabled: None, status: Some(aidog_db::models::PlatformStatus::Disabled),
        manual_budgets: None, join_group_ids: None, expires_at: None, quota_source: None,
    })
    .await
    .unwrap();
    state.db.invalidate_group_details_cache();

    let resp = handle_proxy(
        AxumState(state.clone()),
        messages_request("gkdirectfail", r#"{"model":"m1","max_tokens":10,"messages":[{"role":"user","content":"hi"}]}"#),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::SERVICE_UNAVAILABLE);
    let log = last_log_for(&state, "gkdirectfail").await;
    assert_eq!(log.status_code, 503);
    assert_eq!(log.blocked_by, "router");
    assert_eq!(log.blocked_reason, "direct_model_unavailable");
    assert_eq!(log.est_cost, 0.0, "route-fail must not be priced");
}

// ══ embedding：/v1/embeddings 专用槽位转发端到端（本地假上游，禁外呼）══

type EmbHits = std::sync::Arc<StdMutex<Vec<(String, String, String)>>>;

/// 记录 (path, body, authorization) 的假上游，按队列返回 (status, body)。
async fn spawn_embedding_upstream(responses: Vec<(u16, &'static str)>) -> (String, EmbHits) {
    let hits: EmbHits = std::sync::Arc::new(StdMutex::new(Vec::new()));
    let queue = std::sync::Arc::new(StdMutex::new(std::collections::VecDeque::from(responses)));
    let h2 = hits.clone();
    let app = axum::Router::new().fallback(axum::routing::any(move |req: Request| {
        let hits = h2.clone();
        let queue = queue.clone();
        async move {
            let path = req.uri().path().to_string();
            let auth = req
                .headers()
                .get("authorization")
                .and_then(|v| v.to_str().ok())
                .unwrap_or("")
                .to_string();
            let body = axum::body::to_bytes(req.into_body(), 1024 * 1024)
                .await
                .map(|b| String::from_utf8_lossy(&b).to_string())
                .unwrap_or_default();
            hits.lock().unwrap().push((path, body, auth));
            let (status, resp) = queue.lock().unwrap().pop_front().unwrap_or((200, "{}"));
            (
                axum::http::StatusCode::from_u16(status).unwrap(),
                [("content-type", "application/json")],
                resp,
            )
        }
    }));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.ok(); });
    (format!("http://{addr}"), hits)
}

const EMB_RESPONSE: &str = r#"{"object":"list","data":[{"object":"embedding","index":0,"embedding":[0.1,0.2]}],"model":"emb-slot","usage":{"prompt_tokens":8,"total_tokens":8}}"#;

/// 建 group `gk` 并按序挂入平台：(上游地址, 槽位 embedding, 平台协议/endpoint 协议, 平台名)。
async fn setup_embedding_group(
    state: &Arc<ProxyState>,
    gk: &str,
    plats: &[(&str, Option<&str>, Protocol, &str)],
) {
    let plats: Vec<_> = plats
        .iter()
        .map(|(b, slot, p, n)| {
            (*b, PlatformModels { embedding: slot.map(String::from).into_iter().collect(), ..Default::default() }, p.clone(), *n)
        })
        .collect();
    setup_aux_group(state, gk, &plats).await;
}

/// 同上，但槽位整份 PlatformModels 由调用方给（embedding + rerank 并存用例）。
async fn setup_aux_group(
    state: &Arc<ProxyState>,
    gk: &str,
    plats: &[(&str, PlatformModels, Protocol, &str)],
) {
    let group = aidog_db::create_group(&state.db, aidog_db::test_support::sample_group(gk, vec![]))
        .await
        .unwrap();
    let mut inputs = Vec::new();
    for (i, (base, slot, proto, name)) in plats.iter().enumerate() {
        let plat = aidog_db::create_platform(
            &state.db,
            CreatePlatform {
                name: format!("{gk}-{name}"),
                platform_type: proto.clone(),
                base_url: base.to_string(),
                api_key: format!("sk-{name}"),
                extra: String::new(),
                models: Some(slot.clone()),
                available_models: None,
                endpoints: Some(vec![crate::gateway::models::PlatformEndpoint {
                    protocol: proto.clone(),
                    base_url: format!("{base}/v1"),
                    client_type: "default".into(),
                    coding_plan: false,
                }]),
                manual_budgets: None,
                auto_group: None,
                join_group_ids: None,
                expires_at: None,
                quota_source: None,
            },
        )
        .await
        .unwrap();
        inputs.push(GroupPlatformInput {
            platform_id: plat.id,
            priority: Some(i as i32),
            weight: Some(1),
            level_priority: Some(0),
        });
    }
    aidog_db::set_group_platforms(&state.db, group.id, &inputs).await.unwrap();
}

fn embedding_request(gk: &str, body: &str, path: &str) -> Request {
    HttpRequest::builder()
        .method("POST")
        .uri(path)
        .header("authorization", format!("Bearer {gk}"))
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .unwrap()
}

const EMB_REQ: &str = r#"{"model":"text-embedding-3-small","input":["hello","world"],"encoding_format":"float","dimensions":256}"#;

fn embedding_entry(price: &str) -> ModelEntry {
    ModelEntry {
        platform_code: "openai".into(),
        model_id: "emb-slot".into(),
        display_name: "emb-slot".into(),
        canonical_model: "emb-slot".into(),
        family: String::new(),
        version: String::new(),
        predecessor: String::new(),
        capabilities: vec!["embedding".into()],
        builtin_tools_excluded: vec![],
        max_input_tokens: None,
        max_output_tokens: None,
        context_window: None,
        official: true,
        price_data: format!(r#"{{"price":{price}}}"#),
        updated_at: aidog_db::now(),
    }
}

/// 路径 /embeddings、model 被槽位覆盖、其它字段原样、Authorization 为平台 key、响应 model 回写、usage 入账。
#[tokio::test]
async fn embedding_request_end_to_end() {
    let (up, hits) = spawn_embedding_upstream(vec![(200, EMB_RESPONSE)]).await;
    let state = make_state(test_db().await).await;
    aidog_db::upsert_model_entries(&state.db, vec![embedding_entry(r#"{"input":2e-8,"output":0}"#)])
        .await
        .unwrap();
    setup_embedding_group(&state, "gkemb", &[(&up, Some("emb-slot"), Protocol::OpenAI, "a")]).await;

    let resp = handle_proxy(
        AxumState(state.clone()),
        embedding_request("gkemb", EMB_REQ, "/v1/embeddings"),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body = axum::body::to_bytes(resp.into_body(), 1024 * 1024).await.unwrap();
    let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(v["model"], "text-embedding-3-small", "响应 model 回写客户端原名");
    assert_eq!(v["data"][0]["embedding"], serde_json::json!([0.1, 0.2]), "其它字段不变");
    assert_eq!(v["usage"]["prompt_tokens"], 8);

    let (path, req_body, auth) = hits.lock().unwrap()[0].clone();
    assert_eq!(path, "/v1/embeddings", "base_url(/v1) + /embeddings，不是 /chat/completions");
    let sent: serde_json::Value = serde_json::from_str(&req_body).unwrap();
    assert_eq!(sent["model"], "emb-slot", "客户端模型名被槽位覆盖");
    assert_eq!(sent["input"], serde_json::json!(["hello", "world"]));
    assert_eq!(sent["encoding_format"], "float");
    assert_eq!(sent["dimensions"], 256);
    assert_eq!(auth, "Bearer sk-a");

    let log = last_log_for(&state, "gkemb").await;
    assert_eq!(log.status_code, 200);
    assert_eq!(log.source_protocol, "embeddings");
    assert_eq!(log.target_protocol, "embeddings");
    assert_eq!(log.model, "text-embedding-3-small");
    assert_eq!(log.actual_model, "emb-slot");
    assert_eq!((log.input_tokens, log.output_tokens), (8, 0));
    assert!((log.est_cost - 8.0 * 2e-8).abs() < 1e-15, "got {}", log.est_cost);
}

/// 计费口径：registry 价 / free:true / 全 0 价未标 free → 兜底价。
#[tokio::test]
async fn embedding_cost_registry_free_and_fallback() {
    for (gk, price, expected) in [
        ("gke1", r#"{"input":2e-8,"output":0}"#, 8.0 * 2e-8),
        ("gke2", r#"{"free":true,"input":0,"output":0}"#, 0.0),
        ("gke3", r#"{"input":0,"output":0}"#, 8.0 * 3.0 / 1_000_000.0),
    ] {
        let (up, _hits) = spawn_embedding_upstream(vec![(200, EMB_RESPONSE)]).await;
        let state = make_state(test_db().await).await;
        aidog_db::upsert_model_entries(&state.db, vec![embedding_entry(price)]).await.unwrap();
        setup_embedding_group(&state, gk, &[(&up, Some("emb-slot"), Protocol::OpenAI, "a")]).await;
        let resp = handle_proxy(
            AxumState(state.clone()),
            embedding_request(gk, EMB_REQ, "/v1/embeddings"),
        )
        .await;
        assert_eq!(resp.status(), StatusCode::OK);
        let log = last_log_for(&state, gk).await;
        assert!((log.est_cost - expected).abs() < 1e-12, "{gk}: want {expected}, got {}", log.est_cost);
    }
}

/// 无候选 → 503 + blocked_reason=no_embedding_platform（三文件库，读回）。
/// 三种成因：槽位未配 / 槽位纯空白 / 非 OpenAI 系协议平台。
#[tokio::test]
async fn embedding_no_candidate_503_and_audit_log() {
    let (db, _dir) = aidog_db::test_support::three_file_db().await;
    let state = make_state(db).await;
    let (up, hits) = spawn_embedding_upstream(vec![]).await;
    setup_embedding_group(&state, "gkn1", &[(&up, None, Protocol::OpenAI, "a")]).await;
    setup_embedding_group(&state, "gkn2", &[(&up, Some("  "), Protocol::OpenAI, "a")]).await;
    setup_embedding_group(&state, "gkn3", &[(&up, Some("emb"), Protocol::Anthropic, "a")]).await;
    for gk in ["gkn1", "gkn2", "gkn3"] {
        let resp = handle_proxy(
            AxumState(state.clone()),
            embedding_request(gk, EMB_REQ, "/v1/embeddings"),
        )
        .await;
        assert_eq!(resp.status(), StatusCode::SERVICE_UNAVAILABLE, "{gk}");
        let log = last_log_for(&state, gk).await;
        assert_eq!(log.status_code, 503);
        assert_eq!(log.blocked_by, "router");
        assert_eq!(log.blocked_reason, "no_embedding_platform");
        assert_eq!(log.est_cost, 0.0);
    }
    assert!(hits.lock().unwrap().is_empty(), "不得打到任何上游");
}

/// 三平台：槽位为空的被排除；首个 529 故障转移到第二个。
#[tokio::test]
async fn embedding_skips_empty_slot_and_fails_over() {
    let (up_empty, hits_empty) = spawn_embedding_upstream(vec![]).await;
    let (up1, hits1) = spawn_embedding_upstream(vec![(529, r#"{"error":"overloaded"}"#)]).await;
    let (up2, hits2) = spawn_embedding_upstream(vec![(200, EMB_RESPONSE)]).await;
    let state = make_state(test_db().await).await;
    setup_embedding_group(
        &state,
        "gkfo",
        &[
            (&up_empty, None, Protocol::OpenAI, "empty"),
            (&up1, Some("emb-1"), Protocol::OpenAI, "p1"),
            (&up2, Some("emb-2"), Protocol::OpenAI, "p2"),
        ],
    )
    .await;
    let resp = handle_proxy(
        AxumState(state.clone()),
        embedding_request("gkfo", EMB_REQ, "/proxy/v1/embeddings"),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK, "首个失败应转移到第二个");
    assert!(hits_empty.lock().unwrap().is_empty(), "槽位为空的平台不参与");
    assert_eq!(hits1.lock().unwrap().len(), 1);
    assert_eq!(hits2.lock().unwrap().len(), 1);
    let sent: serde_json::Value = serde_json::from_str(&hits2.lock().unwrap()[0].1).unwrap();
    assert_eq!(sent["model"], "emb-2", "各平台用各自槽位");
    assert_eq!(last_log_for(&state, "gkfo").await.retry_count, 1);
}

/// 非 JSON body → 400（不走 openai messages 解析）。
#[tokio::test]
async fn embedding_non_json_body_400() {
    let (up, hits) = spawn_embedding_upstream(vec![]).await;
    let state = make_state(test_db().await).await;
    setup_embedding_group(&state, "gkbad", &[(&up, Some("emb"), Protocol::OpenAI, "a")]).await;
    let resp = handle_proxy(
        AxumState(state.clone()),
        embedding_request("gkbad", "not json", "/v1/embeddings"),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    assert!(hits.lock().unwrap().is_empty());
}


// ══ rerank：/v1/rerank 专用槽位转发端到端（本地假上游，禁外呼）══

const RR_REQ: &str = r#"{"model":"client-rerank","query":"q","documents":["a","b"],"top_n":1,"return_documents":true}"#;
const RR_JINA: &str = r#"{"model":"rr-slot","usage":{"total_tokens":100},"results":[{"index":1,"relevance_score":0.9}]}"#;
const RR_SILICON: &str = r#"{"id":"x","results":[{"index":0,"relevance_score":0.5}],"meta":{"tokens":{"input_tokens":40,"output_tokens":0}}}"#;
const RR_COHERE: &str = r#"{"id":"x","results":[{"index":0,"relevance_score":0.5}],"meta":{"billed_units":{"search_units":2}}}"#;
const RR_COHERE_NO_UNITS: &str = r#"{"id":"x","results":[],"meta":{"billed_units":{}}}"#;

fn rerank_slot(v: &str) -> PlatformModels {
    PlatformModels { rerank: vec![v.into()], ..Default::default() }
}

fn rerank_entry(price: &str) -> ModelEntry {
    ModelEntry {
        model_id: "rr-slot".into(),
        display_name: "rr-slot".into(),
        canonical_model: "rr-slot".into(),
        capabilities: vec!["rerank".into()],
        ..embedding_entry(price)
    }
}

/// 路径 /rerank、model 被槽位覆盖、query/documents/top_n/return_documents 原样、Authorization 为平台 key、
/// 响应 model 回写、usage 入账、日志协议标签 rerank。
#[tokio::test]
async fn rerank_request_end_to_end() {
    let (up, hits) = spawn_embedding_upstream(vec![(200, RR_JINA)]).await;
    let state = make_state(test_db().await).await;
    aidog_db::upsert_model_entries(&state.db, vec![rerank_entry(r#"{"input":3e-8,"output":0}"#)])
        .await
        .unwrap();
    setup_aux_group(&state, "gkrr", &[(&up, rerank_slot("rr-slot"), Protocol::OpenAI, "a")]).await;

    let resp = handle_proxy(
        AxumState(state.clone()),
        embedding_request("gkrr", RR_REQ, "/v2/rerank"),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body = axum::body::to_bytes(resp.into_body(), 1024 * 1024).await.unwrap();
    let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(v["model"], "client-rerank", "响应 model 回写客户端原名");
    assert_eq!(v["results"][0]["relevance_score"], 0.9);

    let (path, req_body, auth) = hits.lock().unwrap()[0].clone();
    assert_eq!(path, "/v1/rerank", "base_url(/v1) + /rerank");
    let sent: serde_json::Value = serde_json::from_str(&req_body).unwrap();
    assert_eq!(sent["model"], "rr-slot");
    assert_eq!(sent["query"], "q");
    assert_eq!(sent["documents"], serde_json::json!(["a", "b"]));
    assert_eq!(sent["top_n"], 1);
    assert_eq!(sent["return_documents"], true);
    assert_eq!(auth, "Bearer sk-a");

    let log = last_log_for(&state, "gkrr").await;
    assert_eq!((log.source_protocol.as_str(), log.target_protocol.as_str()), ("rerank", "rerank"));
    assert_eq!(log.actual_model, "rr-slot");
    assert_eq!((log.input_tokens, log.output_tokens), (100, 0));
    assert!((log.est_cost - 100.0 * 3e-8).abs() < 1e-15, "got {}", log.est_cost);
}

/// 费用口径：token × price.input / SiliconFlow meta.tokens / free / 全 0 未标 free → 兜底价 /
/// unit:"request" → unit_price × search_units（缺省 1 次），Cohere tokens 记 0，上游若返回 token 照记。
#[tokio::test]
async fn rerank_cost_matrix() {
    let cases: [(&str, &str, &str, f64, i32); 7] = [
        ("gkr1", r#"{"input":3e-8,"output":0}"#, RR_JINA, 100.0 * 3e-8, 100),
        ("gkr2", r#"{"input":2e-8,"output":0}"#, RR_SILICON, 40.0 * 2e-8, 40),
        ("gkr3", r#"{"free":true,"input":0,"output":0}"#, RR_JINA, 0.0, 100),
        ("gkr4", r#"{"input":0,"output":0}"#, RR_JINA, 100.0 * 3.0 / 1_000_000.0, 100),
        ("gkr5", r#"{"unit":"request","unit_price":0.002}"#, RR_COHERE, 0.004, 0),
        ("gkr6", r#"{"unit":"request","unit_price":0.002}"#, RR_COHERE_NO_UNITS, 0.002, 0),
        ("gkr7", r#"{"unit":"request","unit_price":0.002}"#, RR_JINA, 0.002, 100),
    ];
    for (gk, price, resp_body, expected, tokens) in cases {
        let (up, _h) = spawn_embedding_upstream(vec![(200, resp_body)]).await;
        let state = make_state(test_db().await).await;
        aidog_db::upsert_model_entries(&state.db, vec![rerank_entry(price)]).await.unwrap();
        setup_aux_group(&state, gk, &[(&up, rerank_slot("rr-slot"), Protocol::OpenAI, "a")]).await;
        let resp = handle_proxy(
            AxumState(state.clone()),
            embedding_request(gk, RR_REQ, "/v1/rerank"),
        )
        .await;
        assert_eq!(resp.status(), StatusCode::OK, "{gk}");
        let log = last_log_for(&state, gk).await;
        assert_eq!(log.input_tokens, tokens, "{gk}");
        assert!((log.est_cost - expected).abs() < 1e-12, "{gk}: want {expected}, got {}", log.est_cost);
    }
}

/// 无候选 → 503 + no_rerank_platform（槽位空 / 非 OpenAI 系 / 只配了 embedding 槽）。
#[tokio::test]
async fn rerank_no_candidate_503_and_audit_log() {
    let (db, _dir) = aidog_db::test_support::three_file_db().await;
    let state = make_state(db).await;
    let (up, hits) = spawn_embedding_upstream(vec![]).await;
    let emb_only = PlatformModels { embedding: vec!["emb".into()], ..Default::default() };
    setup_aux_group(&state, "gkq1", &[(&up, PlatformModels::default(), Protocol::OpenAI, "a")]).await;
    setup_aux_group(&state, "gkq2", &[(&up, rerank_slot("rr"), Protocol::Anthropic, "a")]).await;
    setup_aux_group(&state, "gkq3", &[(&up, emb_only, Protocol::OpenAI, "a")]).await;
    for gk in ["gkq1", "gkq2", "gkq3"] {
        let resp = handle_proxy(
            AxumState(state.clone()),
            embedding_request(gk, RR_REQ, "/v1/rerank"),
        )
        .await;
        assert_eq!(resp.status(), StatusCode::SERVICE_UNAVAILABLE, "{gk}");
        let log = last_log_for(&state, gk).await;
        assert_eq!((log.blocked_by.as_str(), log.blocked_reason.as_str()), ("router", "no_rerank_platform"));
        assert_eq!(log.est_cost, 0.0);
    }
    assert!(hits.lock().unwrap().is_empty(), "不得打到任何上游");
}

/// 同一平台两槽都配：embedding / rerank 请求各取各自槽位，互不串。
#[tokio::test]
async fn embedding_and_rerank_slots_do_not_cross() {
    let (up, hits) = spawn_embedding_upstream(vec![(200, EMB_RESPONSE), (200, RR_JINA)]).await;
    let state = make_state(test_db().await).await;
    let both = PlatformModels {
        embedding: vec!["emb-slot".into()],
        rerank: vec!["rr-slot".into()],
        ..Default::default()
    };
    setup_aux_group(&state, "gkboth", &[(&up, both, Protocol::OpenAI, "a")]).await;
    for (body, path) in [(EMB_REQ, "/v1/embeddings"), (RR_REQ, "/v1/rerank")] {
        let resp = handle_proxy(AxumState(state.clone()), embedding_request("gkboth", body, path)).await;
        assert_eq!(resp.status(), StatusCode::OK, "{path}");
    }
    let h = hits.lock().unwrap();
    let model_of = |i: usize| serde_json::from_str::<serde_json::Value>(&h[i].1).unwrap()["model"].clone();
    assert_eq!((h[0].0.as_str(), model_of(0)), ("/v1/embeddings", serde_json::json!("emb-slot")));
    assert_eq!((h[1].0.as_str(), model_of(1)), ("/v1/rerank", serde_json::json!("rr-slot")));
}

/// 请求流向 票 02 端到端：`User-Agent: claude-cli/x` 经 handle_proxy_core 全链路（本地假上游）后，
/// proxy_log.client='claude_code'，stats_agg_hourly 行 client='claude_code' 且 req_model=请求模型
/// （≠ actual_model）。用三文件库 + 另开一个 Db 句柄读回（写后重开级断言，防错库路由）。
#[tokio::test]
async fn client_and_req_model_land_in_log_and_stats_agg() {
    let upstream = spawn_stub_upstream(200, ANTHROPIC_OK).await; // 响应 model = "claude-3"
    let (db, dir) = aidog_db::test_support::three_file_db().await;
    let state = make_state(db).await;
    setup_group_with_upstream(&state, "gk_flow", &upstream).await;

    let req = HttpRequest::builder()
        .method("POST")
        .uri("/v1/messages")
        .header("authorization", "Bearer gk_flow")
        .header("content-type", "application/json")
        .header("user-agent", "claude-cli/2.0.1 (external, cli)")
        .body(Body::from(
            r#"{"model":"request-model","messages":[{"role":"user","content":"hi"}]}"#,
        ))
        .unwrap();
    let resp = handle_proxy(AxumState(state.clone()), req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    flush_log_queue(&state).await;

    let reopened = aidog_db::Db::new(dir.path().join("aidog.db").to_str().unwrap())
        .await
        .unwrap();
    let log_client: String = reopened
        .call_read_proxy_log_traced(None, std::panic::Location::caller(), |c| {
            Ok(c.query_row(
                "SELECT client FROM proxy_log WHERE group_key = 'gk_flow'",
                [],
                |r| r.get(0),
            )?)
        })
        .await
        .unwrap();
    assert_eq!(log_client, "claude_code");
    let (client, req_model, model): (String, String, String) = reopened
        .call_read_traced(None, std::panic::Location::caller(), |c| {
            Ok(c.query_row(
                "SELECT client, req_model, model FROM stats_agg_hourly WHERE group_key = 'gk_flow'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )?)
        })
        .await
        .unwrap();
    assert_eq!(client, "claude_code");
    assert_eq!(req_model, "request-model");
    assert_eq!(model, "claude-3", "model 列仍是 actual_model 优先");
}

// ── 票 04：客户端白名单端到端（多平台组普通过滤路径；本地假上游，无真实外呼）──

/// 建一个多平台组（同 group_key），每平台 (名, base_url, extra)。
async fn setup_multi_platform_group(state: &Arc<ProxyState>, gk: &str, plats: &[(&str, &str, &str)]) {
    let mut inputs = Vec::new();
    for (i, (name, base_url, extra)) in plats.iter().enumerate() {
        let plat = aidog_db::create_platform(
            &state.db,
            CreatePlatform {
                name: (*name).into(),
                platform_type: Protocol::Anthropic,
                base_url: (*base_url).to_string(),
                api_key: "sk-up".into(),
                extra: (*extra).to_string(),
                models: None,
                available_models: None,
                endpoints: None,
                manual_budgets: None,
                auto_group: None,
                join_group_ids: None,
                expires_at: None,
                quota_source: None,
            },
        )
        .await
        .unwrap();
        inputs.push(GroupPlatformInput {
            platform_id: plat.id,
            priority: Some(i as i32),
            weight: Some(1),
            level_priority: Some(0),
        });
    }
    let group = aidog_db::create_group(&state.db, aidog_db::test_support::sample_group(gk, vec![]))
        .await
        .unwrap();
    aidog_db::set_group_platforms(&state.db, group.id, &inputs).await.unwrap();
}

fn messages_request_ua(gk: &str, ua: Option<&str>) -> Request {
    let mut b = HttpRequest::builder()
        .method("POST")
        .uri("/v1/messages")
        .header("authorization", format!("Bearer {gk}"))
        .header("content-type", "application/json");
    if let Some(ua) = ua {
        b = b.header("user-agent", ua);
    }
    b.body(Body::from(
        r#"{"model":"claude-3","max_tokens":10,"messages":[{"role":"user","content":"hi"}]}"#,
    ))
    .unwrap()
}

const WL_CC_EXTRA: &str = r#"{"allowed_clients":["claude_code"]}"#;
const PEAK_ALL_DAY_EXTRA: &str =
    r#"{"disable_during_peak":true,"peak":[{"start_hour":0,"end_hour":24,"multiplier":1.5}]}"#;

/// 全部候选被白名单拦下 → 417 + 审计行（router / client_not_allowed / est_cost 0 / 原始 UA）；
/// claude_code 与其变体命中 → 200；无 UA → 417。
#[tokio::test]
async fn whitelist_all_blocked_417_and_hit_matrix() {
    let state = make_state(test_db().await).await;
    let up = spawn_stub_upstream(200, ANTHROPIC_OK).await;
    setup_multi_platform_group(&state, "gkwl", &[("A", &up, WL_CC_EXTRA), ("B", &up, WL_CC_EXTRA)]).await;

    let resp = handle_proxy(AxumState(state.clone()), messages_request_ua("gkwl", Some("curl/8.4.0"))).await;
    assert_eq!(resp.status(), StatusCode::EXPECTATION_FAILED);
    let body = axum::body::to_bytes(resp.into_body(), 1 << 20).await.unwrap();
    assert!(String::from_utf8_lossy(&body).contains("client not allowed"), "417 body states reason");
    let log = last_log_for(&state, "gkwl").await;
    assert_eq!(log.status_code, 417);
    assert_eq!(log.blocked_by, "router");
    assert_eq!(log.blocked_reason, "client_not_allowed");
    assert_eq!(log.est_cost, 0.0);
    assert_eq!(log.user_agent, "curl/8.4.0");
    assert_eq!(log.client, "curl");
    let snap = log.routing.expect("routing snapshot");
    assert!(snap.entries.iter().all(|e| e.reason == aidog_db::models::RouteReason::ClientNotAllowed));

    // 无 UA → 认不出 → 417
    let resp = handle_proxy(AxumState(state.clone()), messages_request_ua("gkwl", None)).await;
    assert_eq!(resp.status(), StatusCode::EXPECTATION_FAILED);

    // claude_code 本体与变体（勾大类 = 全变体）→ 200
    for ua in ["claude-cli/2.0.1 (external, cli)", "claude-cli/2.0 (external, claude-vscode)"] {
        let resp = handle_proxy(AxumState(state.clone()), messages_request_ua("gkwl", Some(ua))).await;
        assert_eq!(resp.status(), StatusCode::OK, "{ua}");
    }
}

/// A 白名单拦下、B 未配 → 请求落到 B；未配白名单的存量组行为不变。
#[tokio::test]
async fn whitelist_falls_through_to_unrestricted_platform() {
    let state = make_state(test_db().await).await;
    let up = spawn_stub_upstream(200, ANTHROPIC_OK).await;
    setup_multi_platform_group(&state, "gkfall", &[("A", &up, WL_CC_EXTRA), ("B", &up, "")]).await;
    let b_id = aidog_db::list_platforms(&state.db).await.unwrap().into_iter().find(|p| p.name == "B").unwrap().id;
    let resp = handle_proxy(AxumState(state.clone()), messages_request_ua("gkfall", Some("curl/8.4.0"))).await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(last_log_for(&state, "gkfall").await.platform_id, b_id);

    // 回归：整组无白名单，curl 照常放行
    setup_multi_platform_group(&state, "gkplain", &[("C", &up, ""), ("D", &up, "")]).await;
    let resp = handle_proxy(AxumState(state.clone()), messages_request_ua("gkplain", Some("curl/8.4.0"))).await;
    assert_eq!(resp.status(), StatusCode::OK);
}

/// 白名单与其他排除原因（高峰禁用）并存 → 503，blocked_reason 为 client_not_allowed_mixed。
#[tokio::test]
async fn whitelist_mixed_with_peak_disabled_503() {
    let state = make_state(test_db().await).await;
    let up = spawn_stub_upstream(200, ANTHROPIC_OK).await;
    setup_multi_platform_group(&state, "gkmix", &[("A", &up, WL_CC_EXTRA), ("B", &up, PEAK_ALL_DAY_EXTRA)]).await;
    let resp = handle_proxy(AxumState(state.clone()), messages_request_ua("gkmix", Some("curl/8.4.0"))).await;
    assert_eq!(resp.status(), StatusCode::SERVICE_UNAVAILABLE);
    let log = last_log_for(&state, "gkmix").await;
    assert_eq!(log.status_code, 503);
    assert_eq!(log.blocked_by, "router");
    assert_eq!(log.blocked_reason, "client_not_allowed_mixed");
    assert_eq!(log.est_cost, 0.0);
}

async fn set_platform_status(state: &Arc<ProxyState>, name: &str, status: aidog_db::models::PlatformStatus) {
    let id = aidog_db::list_platforms(&state.db).await.unwrap().into_iter().find(|p| p.name == name).unwrap().id;
    aidog_db::update_platform(&state.db, aidog_db::models::UpdatePlatform {
        id, name: None, platform_type: None, base_url: None, api_key: None, extra: None,
        models: None, available_models: None, endpoints: None,
        enabled: None, status: Some(status), manual_budgets: None,
        join_group_ids: None, expires_at: None, quota_source: None,
    })
    .await
    .unwrap();
    state.db.invalidate_group_details_cache();
}

/// 断言最近一条日志是白名单 417 审计行（router / client_not_allowed / est_cost 0 / 原始 UA）。
async fn assert_client_417_log(state: &Arc<ProxyState>, gk: &str, ua: &str) {
    let log = last_log_for(state, gk).await;
    assert_eq!(log.status_code, 417, "{gk}");
    assert_eq!(log.blocked_by, "router");
    assert_eq!(log.blocked_reason, "client_not_allowed");
    assert_eq!(log.est_cost, 0.0);
    assert_eq!(log.user_agent, ua);
}

/// 等效退化形态（票 05）：多平台组只剩 1 个 Enabled 平台 → sole_platform 单平台短路，
/// guard 在短路之内：白名单拦下 → 417，不得静默打到唯一平台；放行 UA 照常 200。
#[tokio::test]
async fn whitelist_degenerate_single_enabled_returns_417() {
    let state = make_state(test_db().await).await;
    let up = spawn_stub_upstream(200, ANTHROPIC_OK).await;
    setup_multi_platform_group(&state, "gkdeg", &[("A", &up, WL_CC_EXTRA), ("B", &up, "")]).await;
    set_platform_status(&state, "B", aidog_db::models::PlatformStatus::Disabled).await;
    let resp = handle_proxy(AxumState(state.clone()), messages_request_ua("gkdeg", Some("curl/8.4.0"))).await;
    assert_eq!(resp.status(), StatusCode::EXPECTATION_FAILED);
    assert_client_417_log(&state, "gkdeg", "curl/8.4.0").await;
    let resp = handle_proxy(AxumState(state.clone()), messages_request_ua("gkdeg", Some("claude-cli/2.0.1 (external, cli)"))).await;
    assert_eq!(resp.status(), StatusCode::OK);
}

/// 字面单平台组（len==1）：417；放行 UA 200；无白名单存量不变。与等效退化形态分开验证。
#[tokio::test]
async fn whitelist_literal_single_platform_group_417() {
    let state = make_state(test_db().await).await;
    let up = spawn_stub_upstream(200, ANTHROPIC_OK).await;
    setup_multi_platform_group(&state, "gklit", &[("A", &up, WL_CC_EXTRA)]).await;
    let resp = handle_proxy(AxumState(state.clone()), messages_request_ua("gklit", Some("curl/8.4.0"))).await;
    assert_eq!(resp.status(), StatusCode::EXPECTATION_FAILED);
    assert_client_417_log(&state, "gklit", "curl/8.4.0").await;
    let resp = handle_proxy(AxumState(state.clone()), messages_request_ua("gklit", None)).await;
    assert_eq!(resp.status(), StatusCode::EXPECTATION_FAILED, "no UA = unrecognized");
    let resp = handle_proxy(AxumState(state.clone()), messages_request_ua("gklit", Some("claude-cli/2.0.1 (external, cli)"))).await;
    assert_eq!(resp.status(), StatusCode::OK);

    setup_multi_platform_group(&state, "gklitplain", &[("P", &up, "")]).await;
    let resp = handle_proxy(AxumState(state.clone()), messages_request_ua("gklitplain", Some("curl/8.4.0"))).await;
    assert_eq!(resp.status(), StatusCode::OK, "regression: no whitelist");
}

/// status bypass：单平台 auto_disabled 本会无视状态强制请求；白名单优先级高于 bypass。
#[tokio::test]
async fn whitelist_beats_status_bypass_single_platform() {
    let state = make_state(test_db().await).await;
    let up = spawn_stub_upstream(200, ANTHROPIC_OK).await;
    setup_multi_platform_group(&state, "gkbyp", &[("A", &up, WL_CC_EXTRA)]).await;
    set_platform_status(&state, "A", aidog_db::models::PlatformStatus::AutoDisabled).await;
    let resp = handle_proxy(AxumState(state.clone()), messages_request_ua("gkbyp", Some("curl/8.4.0"))).await;
    assert_eq!(resp.status(), StatusCode::EXPECTATION_FAILED);
    assert_client_417_log(&state, "gkbyp", "curl/8.4.0").await;
    let resp = handle_proxy(AxumState(state.clone()), messages_request_ua("gkbyp", Some("claude-cli/2.0.1 (external, cli)"))).await;
    assert_eq!(resp.status(), StatusCode::OK, "regression: allowed client still bypasses status");
}

/// 单平台组 + 高峰禁用 + 白名单：既有硬停先判（与多平台路径同位次），错误码仍为 peak_disabled 503。
#[tokio::test]
async fn whitelist_single_platform_with_peak_disabled_keeps_peak_error() {
    let state = make_state(test_db().await).await;
    let up = spawn_stub_upstream(200, ANTHROPIC_OK).await;
    let extra = r#"{"allowed_clients":["claude_code"],"disable_during_peak":true,"peak":[{"start_hour":0,"end_hour":24,"multiplier":1.5}]}"#;
    setup_multi_platform_group(&state, "gksp", &[("A", &up, extra)]).await;
    let resp = handle_proxy(AxumState(state.clone()), messages_request_ua("gksp", Some("curl/8.4.0"))).await;
    assert_eq!(resp.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(last_log_for(&state, "gksp").await.blocked_reason, "peak");
}

/// 直通模型链：声明平台全被白名单拦 → 417；放行 UA 命中声明平台；声明平台一部分被拦、
/// 其余不可用 → 沿用 direct_model_unavailable（503）。
#[tokio::test]
async fn whitelist_direct_model_chain() {
    let state = make_state(test_db().await).await;
    let up = spawn_stub_upstream(200, ANTHROPIC_OK).await;
    let direct_wl = r#"{"direct_models":["claude-3"],"allowed_clients":["claude_code"]}"#;
    setup_multi_platform_group(&state, "gkdir", &[("A", &up, direct_wl), ("B", &up, "")]).await;
    let resp = handle_proxy(AxumState(state.clone()), messages_request_ua("gkdir", Some("curl/8.4.0"))).await;
    assert_eq!(resp.status(), StatusCode::EXPECTATION_FAILED, "B 未声明 direct_models：不得借道落到 B");
    assert_client_417_log(&state, "gkdir", "curl/8.4.0").await;
    let a_id = aidog_db::list_platforms(&state.db).await.unwrap().into_iter().find(|p| p.name == "A").unwrap().id;
    let resp = handle_proxy(AxumState(state.clone()), messages_request_ua("gkdir", Some("claude-cli/2.0.1 (external, cli)"))).await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(last_log_for(&state, "gkdir").await.platform_id, a_id);

    // 混合：A 声明 + 白名单拦，C 声明 + 手动禁用 → 原错误 direct_model_unavailable
    let direct = r#"{"direct_models":["claude-3"]}"#;
    setup_multi_platform_group(&state, "gkdir2", &[("A2", &up, direct_wl), ("C2", &up, direct), ("B2", &up, "")]).await;
    set_platform_status(&state, "C2", aidog_db::models::PlatformStatus::Disabled).await;
    let resp = handle_proxy(AxumState(state.clone()), messages_request_ua("gkdir2", Some("curl/8.4.0"))).await;
    assert_eq!(resp.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(last_log_for(&state, "gkdir2").await.blocked_reason, "direct_model_unavailable");
}

/// 透传平台：聊天路径、body 解析失败路径、协议子端点路径都受白名单约束；放行 UA 照常；
/// models 探测端点不受约束；无白名单存量不变。
#[tokio::test]
async fn whitelist_constrains_passthrough_platform() {
    const OK: &str = r#"{"id":"x"}"#;
    let (upstream, seen) = spawn_recording_upstream(OK).await;
    let state = make_state(test_db().await).await;
    setup_passthrough_platform_group_extra(&state, "gkpt", &upstream, WL_CC_EXTRA).await;
    let send = |method: &'static str, path: &'static str, body: &'static str, ua: Option<&'static str>| {
        let mut b = HttpRequest::builder()
            .method(method)
            .uri(path)
            .header("authorization", "Bearer gkpt")
            .header("content-type", "application/json");
        if let Some(ua) = ua {
            b = b.header("user-agent", ua);
        }
        b.body(Body::from(body)).unwrap()
    };
    let chat = r#"{"model":"claude-3","max_tokens":10,"messages":[{"role":"user","content":"hi"}]}"#;
    for (method, path, body) in [
        ("POST", "/v1/messages", chat),
        ("POST", "/v1/messages", "not json at all"),
        ("POST", "/v1/messages/count_tokens", r#"{"model":"m","messages":[]}"#),
        ("POST", "/v1/embeddings", r#"{"model":"m","input":"x"}"#),
    ] {
        let resp = handle_proxy(AxumState(state.clone()), send(method, path, body, Some("curl/8.4.0"))).await;
        assert_eq!(resp.status(), StatusCode::EXPECTATION_FAILED, "{method} {path} {body}");
        assert_client_417_log(&state, "gkpt", "curl/8.4.0").await;
    }
    assert_eq!(seen.lock().unwrap().len(), 0, "blocked requests must not reach upstream");

    let resp = handle_proxy(AxumState(state.clone()), send("POST", "/v1/messages", chat, Some("claude-cli/2.0.1 (external, cli)"))).await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(seen.lock().unwrap().len(), 1);

    // models 探测端点：定组前分流，不受约束
    let resp = handle_proxy(AxumState(state.clone()), send("GET", "/v1/models", "", Some("curl/8.4.0"))).await;
    assert_eq!(resp.status(), StatusCode::OK);
}

/// 嵌入 / rerank / 决策端点：各 handler 自己的选平台入口接了白名单闸门。平台带槽位（过得了
/// 槽位筛选），无 UA（认不出）→ 整组被白名单拦下 → 417 + 审计行；放行 UA 不在本例（无头请求）。
#[tokio::test]
async fn whitelist_constrains_aux_and_decision_routes() {
    let (up, hits) = spawn_embedding_upstream(vec![(200, EMB_RESPONSE)]).await;
    let state = make_state(test_db().await).await;
    let both = PlatformModels {
        embedding: vec!["emb-slot".into()],
        rerank: vec!["rr-slot".into()],
        ..Default::default()
    };
    setup_aux_group(&state, "gkaux", &[(&up, both, Protocol::OpenAI, "wa")]).await;
    set_platform_extra(&state, "gkaux-wa", WL_CC_EXTRA).await;
    for (body, path) in [(EMB_REQ, "/v1/embeddings"), (RR_REQ, "/v1/rerank")] {
        let resp = handle_proxy(AxumState(state.clone()), embedding_request("gkaux", body, path)).await;
        assert_eq!(resp.status(), StatusCode::EXPECTATION_FAILED, "{path}");
        let log = last_log_for(&state, "gkaux").await;
        assert_eq!((log.status_code, log.blocked_by.as_str(), log.blocked_reason.as_str()), (417, "router", "client_not_allowed"), "{path}");
    }
    assert!(hits.lock().unwrap().is_empty(), "不得打到上游");

    let (dup, _) = spawn_decision_upstream(vec![(200, OR_RESPONSE)]).await;
    let jev = PlatformModels { jev: vec!["jev-latest".into()], ..Default::default() };
    setup_decision_group(&state, "gkaux-dec", &dup, jev, None, Protocol::TypeSafe).await;
    set_platform_extra(&state, "stub-gkaux-dec", WL_CC_EXTRA).await;
    let resp = handle_proxy(AxumState(state.clone()), decision_request("gkaux-dec", "whatever")).await;
    assert_eq!(resp.status(), StatusCode::EXPECTATION_FAILED, "decision");
    assert_eq!(last_log_for(&state, "gkaux-dec").await.blocked_reason, "client_not_allowed");
}

async fn set_platform_extra(state: &Arc<ProxyState>, name: &str, extra: &str) {
    let id = aidog_db::list_platforms(&state.db).await.unwrap().into_iter().find(|p| p.name == name).unwrap().id;
    aidog_db::update_platform(&state.db, aidog_db::models::UpdatePlatform {
        id, name: None, platform_type: None, base_url: None, api_key: None, extra: Some(extra.to_string()),
        models: None, available_models: None, endpoints: None,
        enabled: None, status: None, manual_budgets: None,
        join_group_ids: None, expires_at: None, quota_source: None,
    })
    .await
    .unwrap();
    state.db.invalidate_group_details_cache();
}

// ══ 票 06：同平台模型链换模型（本地假上游，禁外呼）══
// 嵌入层次：forward_attempt 内、非 2xx 之后；单平台 / 等效单平台 / 多平台组均在其内生效。
// 透传平台（handler 在 forward_attempt 之前拦截）与直通模型（备选恒空）不换模型。

const CHAIN_REQ: &str = r#"{"model":"claude-x","max_tokens":16,"messages":[{"role":"user","content":"hi"}]}"#;
const CHAIN_REQ_STREAM: &str = r#"{"model":"claude-x","max_tokens":16,"stream":true,"messages":[{"role":"user","content":"hi"}]}"#;
const MB_OK: &str = r#"{"id":"msg_1","type":"message","role":"assistant","model":"m-b","content":[{"type":"text","text":"hi"}],"stop_reason":"end_turn","usage":{"input_tokens":5,"output_tokens":3}}"#;
const E404_MODEL: &str = r#"{"type":"error","error":{"type":"not_found_error","message":"model: m-a"}}"#;
const E400_INVALID_MODEL: &str = r#"{"error":{"message":"Invalid model: m-a"}}"#;
const E404_WEIRD: &str = r#"{"error":{"message":"weird"}}"#;
const E429: &str = r#"{"error":{"message":"rate limited"}}"#;
const E402: &str = r#"{"error":{"message":"insufficient balance"}}"#;
const E401: &str = r#"{"error":{"message":"invalid api key"}}"#;
const E503_NO_CHANNEL: &str = r#"{"error":{"code":"model_not_found","message":"No available channel for model m-a under group default"}}"#;
const E503_NO_CHANNEL_ZH: &str = r#"{"error":{"message":"当前分组 default 下对于模型 m-a 无可用渠道"}}"#;
const E503_PLAIN: &str = r#"{"error":{"message":"service unavailable"}}"#;
const CHAIN_SSE_OK: &str = "event: message_start\ndata: {\"type\":\"message_start\",\"message\":{\"usage\":{\"input_tokens\":5,\"output_tokens\":0}}}\n\nevent: message_delta\ndata: {\"type\":\"message_delta\",\"usage\":{\"output_tokens\":3}}\n\nevent: message_stop\ndata: {\"type\":\"message_stop\"}\n\n";

fn chain_models(chain: &[&str]) -> PlatformModels {
    PlatformModels { default: chain.iter().map(|s| s.to_string()).collect(), ..Default::default() }
}

/// 上游实际收到的 model 序列。
fn hit_models(h: &EmbHits) -> Vec<String> {
    h.lock()
        .unwrap()
        .iter()
        .map(|(_, body, _)| serde_json::from_str::<Value>(body).unwrap()["model"].as_str().unwrap().to_string())
        .collect()
}

async fn chain_send(state: &Arc<ProxyState>, gk: &str, body: &str) -> axum::response::Response {
    handle_proxy(AxumState(state.clone()), messages_request(gk, body)).await
}

fn anthropic_price_entry(model: &str, price: &str) -> ModelEntry {
    ModelEntry {
        platform_code: "anthropic".into(),
        model_id: model.into(),
        display_name: model.into(),
        canonical_model: model.into(),
        capabilities: vec![],
        official: true,
        price_data: format!(r#"{{"price":{price}}}"#),
        updated_at: aidog_db::now(),
        ..embedding_entry("{}")
    }
}

/// 首模型 404 model_not_found → 次模型 200：上游收到 [m-a, m-b]，attempts 两项各带模型，
/// est_cost 只计成功那次（m-a 价故意不同，计错即红）。单平台组（len==1）形态。
#[tokio::test]
async fn chain_model_not_found_switches_to_next_model() {
    let (up, hits) = spawn_embedding_upstream(vec![(404, E404_MODEL), (200, MB_OK)]).await;
    let state = make_state(test_db().await).await;
    aidog_db::upsert_model_entries(
        &state.db,
        vec![
            anthropic_price_entry("m-a", r#"{"input":1e-3,"output":1e-3}"#),
            anthropic_price_entry("m-b", r#"{"input":1e-6,"output":2e-6}"#),
        ],
    )
    .await
    .unwrap();
    setup_aux_group(&state, "gkc1", &[(&up, chain_models(&["m-a", "m-b"]), Protocol::Anthropic, "p")]).await;
    let resp = chain_send(&state, "gkc1", CHAIN_REQ).await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(hit_models(&hits), ["m-a", "m-b"]);
    let log = last_log_for(&state, "gkc1").await;
    assert_eq!(log.attempts.len(), 2, "同平台换模型每次尝试各一项，不新增 proxy_log 行");
    assert_eq!(log.attempts[0].model.as_deref(), Some("m-a"));
    assert_eq!(log.attempts[0].status_code, 404);
    assert_eq!(log.attempts[1].model.as_deref(), Some("m-b"));
    assert_eq!(log.attempts[1].status_code, 200);
    assert_eq!(log.actual_model, "m-b", "展示模型 = 实际成功的模型（上游自报）");
    assert_eq!(log.status_code, 200);
    assert!((log.est_cost - (5.0 * 1e-6 + 3.0 * 2e-6)).abs() < 1e-12, "只计成功那次: {}", log.est_cost);
}

/// 429 / 402 / 401 是上游服务类错误：不换模型，直接换平台；上游 1 只收到一次请求。
#[tokio::test]
async fn chain_service_errors_switch_platform_not_model() {
    for (i, (status, body)) in [(429u16, E429), (402, E402), (401, E401)].into_iter().enumerate() {
        let (up1, hits1) = spawn_embedding_upstream(vec![(status, body)]).await;
        let (up2, hits2) = spawn_embedding_upstream(vec![(200, ANTHROPIC_OK)]).await;
        let state = make_state(test_db().await).await;
        let gk = format!("gkc2{i}");
        setup_aux_group(
            &state,
            &gk,
            &[
                (&up1, chain_models(&["m-a", "m-b"]), Protocol::Anthropic, "p1"),
                (&up2, chain_models(&["m-c", "m-d"]), Protocol::Anthropic, "p2"),
            ],
        )
        .await;
        let resp = chain_send(&state, &gk, CHAIN_REQ).await;
        assert_eq!(resp.status(), StatusCode::OK, "{status}");
        assert_eq!(hit_models(&hits1), ["m-a"], "{status}: 同平台只试一次");
        assert_eq!(hit_models(&hits2), ["m-c"], "{status}");
        let log = last_log_for(&state, &gk).await;
        assert_eq!(log.attempts.len(), 2, "{status}");
        assert_eq!(log.attempts[0].chain_stop.as_deref(), Some("service"), "{status}: 链上还有 m-b 未尝试，记终止原因");
        assert_eq!(log.attempts[1].chain_stop, None);
    }
}

/// 链耗尽：模型类失败不碰平台健康（逐次重算，旧「链尽记一次」规则已废）；
/// 三模型全 404 → 终态回包是最后一次错误，降权连败 0、last_error 空。
#[tokio::test]
async fn chain_exhausted_records_platform_failure_once() {
    let (up, hits) = spawn_embedding_upstream(vec![(404, E404_MODEL), (404, E404_MODEL), (404, E404_MODEL)]).await;
    let state = make_state(test_db().await).await;
    setup_aux_group(&state, "gkc3", &[(&up, chain_models(&["m-a", "m-b", "m-c"]), Protocol::Anthropic, "p")]).await;
    let resp = chain_send(&state, "gkc3", CHAIN_REQ).await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    assert_eq!(hit_models(&hits), ["m-a", "m-b", "m-c"]);
    let p = aidog_db::list_platforms(&state.db).await.unwrap().remove(0);
    assert_eq!(state.scheduler.penalty_consecutive(p.id), 0, "模型类失败不降权");
    let log = last_log_for(&state, "gkc3").await;
    assert_eq!(log.attempts.len(), 3);
    assert!(log.attempts.iter().all(|a| a.chain_stop.is_none()), "耗尽不是提前终止");
    assert!(p.last_error.is_empty(), "模型类失败不写 last_error: {:?}", p.last_error);
}

/// 连接失败（transport 错）属服务类：不换模型，链上剩余模型记 chain_stop=service，画布显示「跳过」而非「未选」。
#[tokio::test]
async fn chain_transport_error_records_chain_stop_service() {
    // 绑端口后立即释放 → 本地必拒连，不碰外网
    let dead = {
        let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        format!("http://{}", l.local_addr().unwrap())
    };
    let state = make_state(test_db().await).await;
    setup_aux_group(&state, "gkc3t", &[(&dead, chain_models(&["m-a", "m-b"]), Protocol::Anthropic, "p")]).await;
    let resp = chain_send(&state, "gkc3t", CHAIN_REQ).await;
    assert_eq!(resp.status(), StatusCode::BAD_GATEWAY);
    let log = last_log_for(&state, "gkc3t").await;
    let last = log.attempts.last().unwrap();
    assert_eq!(last.status_code, 0);
    assert_eq!(last.model.as_deref(), Some("m-a"));
    assert_eq!(last.chain_stop.as_deref(), Some("service"), "链上 m-b 未尝试，须记终止原因");
}

/// 链中途成功：失败过的前序模型不留降权（连败为 0）。
#[tokio::test]
async fn chain_success_after_model_failures_leaves_no_penalty() {
    let (up, _hits) = spawn_embedding_upstream(vec![(404, E404_MODEL), (200, ANTHROPIC_OK)]).await;
    let state = make_state(test_db().await).await;
    setup_aux_group(&state, "gkc3b", &[(&up, chain_models(&["m-a", "m-b"]), Protocol::Anthropic, "p")]).await;
    assert_eq!(chain_send(&state, "gkc3b", CHAIN_REQ).await.status(), StatusCode::OK);
    let pid = aidog_db::list_platforms(&state.db).await.unwrap()[0].id;
    assert_eq!(state.scheduler.penalty_consecutive(pid), 0);
}

/// Unknown 类 4xx 默认换模型；Model 类 400（invalid model）链尽后：非末位候选换下个平台且不记平台错误
/// （无 last_error / 降权 / 熔断计数），末位候选仍把上游 400 回给客户端。
#[tokio::test]
async fn chain_unknown_4xx_switches_model_and_model_400_exhausted_fails_over_without_blame() {
    // Unknown：404 无特征 → 换模型成功
    let (up, hits) = spawn_embedding_upstream(vec![(404, E404_WEIRD), (200, ANTHROPIC_OK)]).await;
    let state = make_state(test_db().await).await;
    setup_aux_group(&state, "gkc4a", &[(&up, chain_models(&["m-a", "m-b"]), Protocol::Anthropic, "p")]).await;
    assert_eq!(chain_send(&state, "gkc4a", CHAIN_REQ).await.status(), StatusCode::OK);
    assert_eq!(hit_models(&hits), ["m-a", "m-b"]);

    // Model 类 400：平台 1 两个模型都 400 → 换到平台 2 成功；平台 1 不背锅
    let (up1, hits1) = spawn_embedding_upstream(vec![(400, E400_INVALID_MODEL), (400, E400_INVALID_MODEL)]).await;
    let (up2, hits2) = spawn_embedding_upstream(vec![(200, ANTHROPIC_OK)]).await;
    let state = make_state(test_db().await).await;
    setup_aux_group(
        &state,
        "gkc4b",
        &[
            (&up1, chain_models(&["m-a", "m-b"]), Protocol::Anthropic, "p1"),
            (&up2, chain_models(&["m-c"]), Protocol::Anthropic, "p2"),
        ],
    )
    .await;
    assert_eq!(chain_send(&state, "gkc4b", CHAIN_REQ).await.status(), StatusCode::OK);
    assert_eq!(hit_models(&hits1), ["m-a", "m-b"]);
    assert_eq!(hit_models(&hits2), ["m-c"], "Model 类 400 链尽后换平台");
    let p1 = aidog_db::list_platforms(&state.db).await.unwrap().into_iter().find(|p| p.name == "gkc4b-p1").unwrap();
    assert!(p1.last_error.is_empty(), "不写 last_error: {}", p1.last_error);
    assert!(!state.scheduler.penalty_active(p1.id, aidog_db::now()), "不降权");
    assert_eq!(state.scheduler.penalty_consecutive(p1.id), 0);
    let log = last_log_for(&state, "gkc4b").await;
    assert_eq!(log.attempts.len(), 3, "两次模型 400 + 平台 2 成功各留痕");
    assert_eq!(log.attempts[1].status_code, 400);

    // 末位候选：Model 类 400 照旧回给客户端
    let (up3, hits3) = spawn_embedding_upstream(vec![(400, E400_INVALID_MODEL), (400, E400_INVALID_MODEL)]).await;
    let state = make_state(test_db().await).await;
    setup_aux_group(&state, "gkc4c", &[(&up3, chain_models(&["m-a", "m-b"]), Protocol::Anthropic, "p")]).await;
    assert_eq!(chain_send(&state, "gkc4c", CHAIN_REQ).await.status(), StatusCode::BAD_REQUEST);
    assert_eq!(hit_models(&hits3), ["m-a", "m-b"]);
}

/// 命中槽位被关闭 → 整组无候选：503 + blocked_reason='slot_disabled'，上游零请求，est_cost 0。
#[tokio::test]
async fn chat_all_slots_disabled_503_and_audit_log() {
    let (up, hits) = spawn_embedding_upstream(vec![(200, ANTHROPIC_OK)]).await;
    let state = make_state(test_db().await).await;
    setup_aux_group(
        &state,
        "gkslotoff",
        &[
            (&up, chain_models(&["m-a"]), Protocol::Anthropic, "p1"),
            (&up, chain_models(&["m-b"]), Protocol::Anthropic, "p2"),
        ],
    )
    .await;
    for p in aidog_db::list_platforms(&state.db).await.unwrap() {
        aidog_db::update_platform(&state.db, aidog_db::models::UpdatePlatform {
            id: p.id, name: None, platform_type: None, base_url: None, api_key: None,
            extra: Some(r#"{"disabled_slots":["default"]}"#.to_string()),
            models: None, available_models: None, endpoints: None,
            enabled: None, status: None, manual_budgets: None,
            join_group_ids: None, expires_at: None, quota_source: None,
        })
        .await
        .unwrap();
    }
    let resp = chain_send(&state, "gkslotoff", CHAIN_REQ).await;
    assert_eq!(resp.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert!(hits.lock().unwrap().is_empty(), "槽位关闭不应请求上游");
    let log = last_log_for(&state, "gkslotoff").await;
    assert_eq!(log.status_code, 503);
    assert_eq!(log.blocked_by, "router");
    assert_eq!(log.blocked_reason, "slot_disabled");
    assert_eq!(log.est_cost, 0.0);
    for p in aidog_db::list_platforms(&state.db).await.unwrap() {
        assert!(p.last_error.is_empty());
    }
}

/// 5xx 例外：message 命中模型类特征（new-api 503 无可用渠道，中英）→ 换模型不换平台；
/// 普通 5xx → 服务类，直接换平台。
#[tokio::test]
async fn chain_5xx_with_model_marker_switches_model_plain_5xx_switches_platform() {
    for (i, body) in [E503_NO_CHANNEL, E503_NO_CHANNEL_ZH].into_iter().enumerate() {
        let (up, hits) = spawn_embedding_upstream(vec![(503, body), (200, ANTHROPIC_OK)]).await;
        let state = make_state(test_db().await).await;
        let gk = format!("gkc5a{i}");
        setup_aux_group(&state, &gk, &[(&up, chain_models(&["m-a", "m-b"]), Protocol::Anthropic, "p")]).await;
        assert_eq!(chain_send(&state, &gk, CHAIN_REQ).await.status(), StatusCode::OK);
        assert_eq!(hit_models(&hits), ["m-a", "m-b"], "{body}");
    }

    let (up1, hits1) = spawn_embedding_upstream(vec![(503, E503_PLAIN)]).await;
    let (up2, hits2) = spawn_embedding_upstream(vec![(200, ANTHROPIC_OK)]).await;
    let state = make_state(test_db().await).await;
    setup_aux_group(
        &state,
        "gkc5b",
        &[
            (&up1, chain_models(&["m-a", "m-b"]), Protocol::Anthropic, "p1"),
            (&up2, chain_models(&["m-c"]), Protocol::Anthropic, "p2"),
        ],
    )
    .await;
    assert_eq!(chain_send(&state, "gkc5b", CHAIN_REQ).await.status(), StatusCode::OK);
    assert_eq!(hit_models(&hits1), ["m-a"]);
    assert_eq!(hit_models(&hits2), ["m-c"]);
}

/// error_rule 标 non-retryable：不换模型也不换平台，直接回客户端；终止项带 chain_stop=rule。
#[tokio::test]
async fn chain_error_rule_non_retryable_blocks_model_switch() {
    use aidog_db::models::{
        ActionKind, ActionParams, ActionStep, AppliesTo, ConditionLeaf, ConditionNode, MatchType, MiddlewareRule, Target,
    };
    let (up, hits) = spawn_embedding_upstream(vec![(404, E404_MODEL), (200, ANTHROPIC_OK)]).await;
    let state = make_state(test_db().await).await;
    state.middleware.rebuild_from_rules(vec![MiddlewareRule {
        id: 1,
        name: "no-retry-404".into(),
        description: String::new(),
        conditions: ConditionNode::Leaf(ConditionLeaf {
            target: Target::Status,
            field: String::new(),
            match_type: MatchType::Regex,
            pattern: "404".into(),
            validator: String::new(),
        }),
        actions: vec![ActionStep {
            kind: ActionKind::Classify,
            params: ActionParams { category: "x".into(), retryable: false, ..Default::default() },
        }],
        applies_to: AppliesTo::default(),
        priority: 0,
        enabled: true,
        is_builtin: false,
        failed: false,
        created_at: 0,
        updated_at: 0,
    }]);
    setup_aux_group(&state, "gkc6", &[(&up, chain_models(&["m-a", "m-b"]), Protocol::Anthropic, "p")]).await;
    assert_eq!(chain_send(&state, "gkc6", CHAIN_REQ).await.status(), StatusCode::NOT_FOUND);
    assert_eq!(hit_models(&hits), ["m-a"]);
    let log = last_log_for(&state, "gkc6").await;
    assert_eq!(log.attempts.len(), 1);
    assert_eq!(log.attempts[0].chain_stop.as_deref(), Some("rule"));
}

/// 流式：非 2xx 且尚无字节写给客户端 → 可换模型；2xx 流一旦开始就不再换。
#[tokio::test]
async fn chain_stream_switches_before_first_byte_not_after() {
    let (up, hits) = spawn_embedding_upstream(vec![(404, E404_MODEL), (200, CHAIN_SSE_OK)]).await;
    let state = make_state(test_db().await).await;
    setup_aux_group(&state, "gkc7a", &[(&up, chain_models(&["m-a", "m-b"]), Protocol::Anthropic, "p")]).await;
    let resp = chain_send(&state, "gkc7a", CHAIN_REQ_STREAM).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let _ = axum::body::to_bytes(resp.into_body(), 1 << 20).await.unwrap();
    assert_eq!(hit_models(&hits), ["m-a", "m-b"]);

    // 2xx 流已开始：上游只收到首模型一次
    let (up, hits) = spawn_embedding_upstream(vec![(200, CHAIN_SSE_OK), (200, CHAIN_SSE_OK)]).await;
    let state = make_state(test_db().await).await;
    setup_aux_group(&state, "gkc7b", &[(&up, chain_models(&["m-a", "m-b"]), Protocol::Anthropic, "p")]).await;
    let resp = chain_send(&state, "gkc7b", CHAIN_REQ_STREAM).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let _ = axum::body::to_bytes(resp.into_body(), 1 << 20).await.unwrap();
    assert_eq!(hit_models(&hits), ["m-a"]);
}

/// 等效单平台（多平台组仅剩 1 个 Enabled）与字面单平台组（len==1）各一例：
/// 都换模型；两模型都 400 invalid model 时终态回包一致（400，不再做别的）。
#[tokio::test]
async fn chain_degenerate_and_literal_single_platform_behave_alike() {
    // 字面单平台组
    let (up, hits) = spawn_embedding_upstream(vec![(400, E400_INVALID_MODEL), (400, E400_INVALID_MODEL)]).await;
    let state = make_state(test_db().await).await;
    setup_aux_group(&state, "gkc8a", &[(&up, chain_models(&["m-a", "m-b"]), Protocol::Anthropic, "A")]).await;
    assert_eq!(chain_send(&state, "gkc8a", CHAIN_REQ).await.status(), StatusCode::BAD_REQUEST);
    assert_eq!(hit_models(&hits), ["m-a", "m-b"]);

    // 等效单平台：B 被禁用，只剩 A
    let (up, hits) = spawn_embedding_upstream(vec![(400, E400_INVALID_MODEL), (400, E400_INVALID_MODEL)]).await;
    let (upb, hitsb) = spawn_embedding_upstream(vec![(200, ANTHROPIC_OK)]).await;
    let state = make_state(test_db().await).await;
    setup_aux_group(
        &state,
        "gkc8b",
        &[
            (&up, chain_models(&["m-a", "m-b"]), Protocol::Anthropic, "A"),
            (&upb, chain_models(&["m-c"]), Protocol::Anthropic, "B"),
        ],
    )
    .await;
    set_platform_status(&state, "gkc8b-B", aidog_db::models::PlatformStatus::Disabled).await;
    assert_eq!(chain_send(&state, "gkc8b", CHAIN_REQ).await.status(), StatusCode::BAD_REQUEST);
    assert_eq!(hit_models(&hits), ["m-a", "m-b"]);
    assert!(hitsb.lock().unwrap().is_empty());

    // 等效单平台换模型成功
    let (up, hits) = spawn_embedding_upstream(vec![(404, E404_MODEL), (200, ANTHROPIC_OK)]).await;
    let (upb, _) = spawn_embedding_upstream(vec![(200, ANTHROPIC_OK)]).await;
    let state = make_state(test_db().await).await;
    setup_aux_group(
        &state,
        "gkc8c",
        &[
            (&up, chain_models(&["m-a", "m-b"]), Protocol::Anthropic, "A"),
            (&upb, chain_models(&["m-c"]), Protocol::Anthropic, "B"),
        ],
    )
    .await;
    set_platform_status(&state, "gkc8c-B", aidog_db::models::PlatformStatus::Disabled).await;
    assert_eq!(chain_send(&state, "gkc8c", CHAIN_REQ).await.status(), StatusCode::OK);
    assert_eq!(hit_models(&hits), ["m-a", "m-b"]);
}

/// 链长 1 回归：无备选 → 与现状一致（404 换平台）；直通模型（direct_models）备选为空，不换模型。
#[tokio::test]
async fn chain_len1_and_direct_models_do_not_switch_model() {
    let (up1, hits1) = spawn_embedding_upstream(vec![(404, E404_MODEL)]).await;
    let (up2, hits2) = spawn_embedding_upstream(vec![(200, ANTHROPIC_OK)]).await;
    let state = make_state(test_db().await).await;
    setup_aux_group(
        &state,
        "gkc9a",
        &[
            (&up1, chain_models(&["m-a"]), Protocol::Anthropic, "p1"),
            (&up2, chain_models(&["m-c"]), Protocol::Anthropic, "p2"),
        ],
    )
    .await;
    assert_eq!(chain_send(&state, "gkc9a", CHAIN_REQ).await.status(), StatusCode::OK);
    assert_eq!(hit_models(&hits1), ["m-a"]);
    assert_eq!(hit_models(&hits2), ["m-c"]);

    let (up, hits) = spawn_embedding_upstream(vec![(404, E404_MODEL), (200, ANTHROPIC_OK)]).await;
    let state = make_state(test_db().await).await;
    setup_multi_platform_group(&state, "gkc9b", &[("D", &up, r#"{"direct_models":["claude-x"]}"#)]).await;
    assert_eq!(chain_send(&state, "gkc9b", CHAIN_REQ).await.status(), StatusCode::NOT_FOUND);
    assert_eq!(hit_models(&hits), ["claude-x"]);
}

// ══ 票 07：embedding / rerank / jev 专用槽位同平台换模型（复用 with_model_chain，本地假上游）══

#[derive(Clone, Copy, Debug)]
enum AuxKind {
    Emb,
    Rerank,
    Jev,
}

const AUX_KINDS: [AuxKind; 3] = [AuxKind::Emb, AuxKind::Rerank, AuxKind::Jev];

impl AuxKind {
    fn ok_body(self) -> &'static str {
        match self {
            AuxKind::Emb => EMB_RESPONSE,
            AuxKind::Rerank => RR_JINA,
            AuxKind::Jev => OR_RESPONSE,
        }
    }
    fn models(self, chain: &[&str]) -> PlatformModels {
        let v: Vec<String> = chain.iter().map(|s| s.to_string()).collect();
        match self {
            AuxKind::Emb => PlatformModels { embedding: v, ..Default::default() },
            AuxKind::Rerank => PlatformModels { rerank: v, ..Default::default() },
            AuxKind::Jev => PlatformModels { jev: v, ..Default::default() },
        }
    }
    fn request(self, gk: &str) -> Request {
        match self {
            AuxKind::Emb => embedding_request(gk, EMB_REQ, "/v1/embeddings"),
            AuxKind::Rerank => embedding_request(gk, RR_REQ, "/v2/rerank"),
            AuxKind::Jev => decision_request(gk, "jev-x"),
        }
    }
}

/// 每个 (上游, 槽位链) 一个平台；空链 = 该槽位为空，被候选过滤剔除（等效单平台形态）。
async fn setup_aux_chain(state: &Arc<ProxyState>, gk: &str, kind: AuxKind, plats: &[(&str, &[&str])]) {
    let plats: Vec<_> = plats
        .iter()
        .enumerate()
        .map(|(i, (up, chain))| (*up, kind.models(chain), Protocol::OpenAI, ["pa", "pb", "pc"][i]))
        .collect();
    setup_aux_group(state, gk, &plats).await;
}

/// 首模型模型类错误 → 次模型成功；上游收到 [m-a, m-b]，attempts 两项各带模型；
/// 单平台组与等效单平台（第二个平台槽位为空被剔除）各一遍。
#[tokio::test]
async fn aux_model_error_switches_to_next_model() {
    for kind in AUX_KINDS {
        for equivalent in [false, true] {
            let (up, hits) = spawn_embedding_upstream(vec![(404, E404_MODEL), (200, kind.ok_body())]).await;
            let state = make_state(test_db().await).await;
            let gk = format!("gka7{}{equivalent}", kind as u8);
            let mut plats: Vec<(&str, &[&str])> = vec![(&up, &["m-a", "m-b"])];
            if equivalent {
                plats.push((&up, &[]));
            }
            setup_aux_chain(&state, &gk, kind, &plats).await;
            let resp = handle_proxy(AxumState(state.clone()), kind.request(&gk)).await;
            assert_eq!(resp.status(), StatusCode::OK, "{kind:?} eq={equivalent}");
            assert_eq!(hit_models(&hits), ["m-a", "m-b"], "{kind:?} eq={equivalent}");
            let log = last_log_for(&state, &gk).await;
            assert_eq!(log.attempts.len(), 2, "{kind:?}");
            assert_eq!(log.attempts[0].model.as_deref(), Some("m-a"));
            assert_eq!(log.attempts[0].status_code, 404);
            assert_eq!(log.attempts[1].model.as_deref(), Some("m-b"));
            assert_eq!(log.attempts[1].status_code, 200);
            assert_eq!(log.actual_model, "m-b", "{kind:?}");
            assert_eq!(log.status_code, 200);
        }
    }
}

/// 服务类错误（429）不换模型，直接换平台：上游 1 只收到 m-a 一次，上游 2 从 m-a 起。
#[tokio::test]
async fn aux_service_error_switches_platform_not_model() {
    for kind in AUX_KINDS {
        let (up1, hits1) = spawn_embedding_upstream(vec![(429, E429)]).await;
        let (up2, hits2) = spawn_embedding_upstream(vec![(200, kind.ok_body())]).await;
        let state = make_state(test_db().await).await;
        let gk = format!("gka7s{}", kind as u8);
        setup_aux_chain(&state, &gk, kind, &[(&up1, &["m-a", "m-b"]), (&up2, &["m-a", "m-b"])]).await;
        let resp = handle_proxy(AxumState(state.clone()), kind.request(&gk)).await;
        assert_eq!(resp.status(), StatusCode::OK, "{kind:?}");
        assert_eq!(hit_models(&hits1), ["m-a"], "{kind:?}");
        assert_eq!(hit_models(&hits2), ["m-a"], "{kind:?}");
        let log = last_log_for(&state, &gk).await;
        assert_eq!(log.attempts.len(), 2, "{kind:?}");
    }
}

/// 链长 1 回归：模型类错误无下一个模型可换，行为同改造前（上游只收到一次，错误回客户端）。
#[tokio::test]
async fn aux_chain_len_one_does_not_switch() {
    for kind in AUX_KINDS {
        let (up, hits) = spawn_embedding_upstream(vec![(404, E404_MODEL), (200, kind.ok_body())]).await;
        let state = make_state(test_db().await).await;
        let gk = format!("gka7l{}", kind as u8);
        setup_aux_chain(&state, &gk, kind, &[(&up, &["m-a"])]).await;
        let resp = handle_proxy(AxumState(state.clone()), kind.request(&gk)).await;
        assert_ne!(resp.status(), StatusCode::OK, "{kind:?}");
        assert_eq!(hit_models(&hits), ["m-a"], "{kind:?}");
        let log = last_log_for(&state, &gk).await;
        assert_eq!(log.attempts.len(), 1, "{kind:?}");
    }
}

// ── 票 02：网关执行平台限频（extra.rate_limits；本地假上游，零真实外呼）──

async fn platform_id_of(state: &Arc<ProxyState>, name: &str) -> u64 {
    aidog_db::list_platforms(&state.db).await.unwrap().into_iter().find(|p| p.name == name).unwrap().id
}

async fn rl_status(state: &Arc<ProxyState>, gk: &str) -> StatusCode {
    handle_proxy(AxumState(state.clone()), messages_request_ua(gk, None)).await.status()
}

/// 假上游：每个请求先睡 `delay_ms` 再回 ANTHROPIC_OK（制造在途重叠）。
async fn spawn_slow_upstream(delay_ms: u64) -> String {
    use axum::routing::any;
    let app = axum::Router::new().fallback(any(move || async move {
        tokio::time::sleep(std::time::Duration::from_millis(delay_ms)).await;
        (axum::http::StatusCode::OK, [("content-type", "application/json")], ANTHROPIC_OK)
    }));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.ok(); });
    format!("http://{addr}")
}

/// 假上游：发 SSE 头 + 一个 message_start 后挂起不结束（流式在途 / 客户端断连场景）。
async fn spawn_hanging_sse_upstream() -> String {
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
                let _ = stream.write_all(format!("{:x}\r\n{ev}\r\n", ev.len()).as_bytes()).await;
                let _ = stream.flush().await;
                tokio::time::sleep(std::time::Duration::from_secs(30)).await;
            });
        }
    });
    format!("http://{addr}")
}

fn messages_request_stream(gk: &str) -> Request {
    messages_request(gk, r#"{"model":"claude-3","stream":true,"max_tokens":10,"messages":[{"role":"user","content":"hi"}]}"#)
}

const RPM1: &str = r#"{"rate_limits":{"rpm":1}}"#;

/// 用户故事 1/7：RPM 第 N+1 个请求被排除，多平台组自动换平台（不报错）。
#[tokio::test]
async fn ratelimit_rpm_nplus1_switches_platform() {
    let state = make_state(test_db().await).await;
    let up = spawn_stub_upstream(200, ANTHROPIC_OK).await;
    setup_multi_platform_group(&state, "gkrl1", &[("A", &up, r#"{"rate_limits":{"rpm":2}}"#), ("B", &up, "")]).await;
    let (a, b) = (platform_id_of(&state, "A").await, platform_id_of(&state, "B").await);
    for i in 0..2 {
        assert_eq!(rl_status(&state, "gkrl1").await, StatusCode::OK);
        assert_eq!(last_log_for(&state, "gkrl1").await.platform_id, a, "req {i} on A");
        state.db.invalidate_group_details_cache();
    }
    assert_eq!(rl_status(&state, "gkrl1").await, StatusCode::OK);
    assert_eq!(last_log_for(&state, "gkrl1").await.platform_id, b, "3rd request excluded A -> B");
}

/// 用户故事 8：整组超限 → 503 + blocked_reason='rate_limit' 落库。
#[tokio::test]
async fn ratelimit_all_over_limit_503_and_audit_log() {
    let state = make_state(test_db().await).await;
    let up = spawn_stub_upstream(200, ANTHROPIC_OK).await;
    setup_multi_platform_group(&state, "gkrl2", &[("A", &up, RPM1), ("B", &up, RPM1)]).await;
    assert_eq!(rl_status(&state, "gkrl2").await, StatusCode::OK);
    assert_eq!(rl_status(&state, "gkrl2").await, StatusCode::OK);
    assert_eq!(rl_status(&state, "gkrl2").await, StatusCode::SERVICE_UNAVAILABLE);
    let log = last_log_for(&state, "gkrl2").await;
    assert_eq!(log.status_code, 503);
    assert_eq!(log.blocked_by, "router");
    assert_eq!(log.blocked_reason, "rate_limit");
    assert_eq!(log.est_cost, 0.0);
}

/// 用户故事 9：字面单平台组超限同样 503，不 bypass（guard 在单平台短路之内）。
#[tokio::test]
async fn ratelimit_literal_single_platform_group_503() {
    let state = make_state(test_db().await).await;
    let up = spawn_stub_upstream(200, ANTHROPIC_OK).await;
    setup_multi_platform_group(&state, "gkrl3", &[("A", &up, RPM1)]).await;
    assert_eq!(rl_status(&state, "gkrl3").await, StatusCode::OK);
    assert_eq!(rl_status(&state, "gkrl3").await, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(last_log_for(&state, "gkrl3").await.blocked_reason, "rate_limit");
}

/// 用户故事 10：多平台组只剩 1 个 Enabled 平台（等效单平台）超限 → 503，不静默打上游。
#[tokio::test]
async fn ratelimit_degenerate_single_enabled_503() {
    let state = make_state(test_db().await).await;
    let up = spawn_stub_upstream(200, ANTHROPIC_OK).await;
    setup_multi_platform_group(&state, "gkrl4", &[("A", &up, RPM1), ("B", &up, "")]).await;
    set_platform_status(&state, "B", aidog_db::models::PlatformStatus::Disabled).await;
    assert_eq!(rl_status(&state, "gkrl4").await, StatusCode::OK);
    assert_eq!(rl_status(&state, "gkrl4").await, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(last_log_for(&state, "gkrl4").await.blocked_reason, "rate_limit");
}

/// 用户故事 4：并发上限 + 释放。两个重叠请求：一个占 A、另一个换 B；结束后 A 释放可再用。
#[tokio::test]
async fn ratelimit_concurrency_limits_and_releases() {
    let state = make_state(test_db().await).await;
    let up = spawn_slow_upstream(300).await;
    setup_multi_platform_group(&state, "gkrl5", &[("A", &up, r#"{"rate_limits":{"concurrency":1}}"#), ("B", &up, "")]).await;
    let (r1, r2) = tokio::join!(
        async { handle_proxy(AxumState(state.clone()), messages_request_ua("gkrl5", None)).await },
        async {
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            handle_proxy(AxumState(state.clone()), messages_request_ua("gkrl5", None)).await
        },
    );
    assert_eq!((r1.status(), r2.status()), (StatusCode::OK, StatusCode::OK));
    drop((r1, r2)); // 响应体随 Access 持有并发，丢弃 = 客户端读完
    let logs = aidog_logs::list_proxy_logs(&state.db, 100, 0).await.unwrap();
    let a = platform_id_of(&state, "A").await;
    let on_a = logs.iter().filter(|l| l.group_key == "gkrl5" && l.platform_id == a).count();
    assert_eq!(on_a, 1, "并发 1：重叠的第二个请求不进 A");
    // 全部结束后释放：下一个请求回到 A（priority 最高）。
    state.db.invalidate_group_details_cache();
    assert_eq!(rl_status(&state, "gkrl5").await, StatusCode::OK);
    assert_eq!(last_log_for(&state, "gkrl5").await.platform_id, a, "并发已释放");
}

/// 并发释放覆盖流式断连：单平台组并发 1，流式请求在途 → 第二个 503；客户端丢弃响应后释放 → 再次 200。
#[tokio::test]
async fn ratelimit_concurrency_released_on_stream_disconnect() {
    let state = make_state(test_db().await).await;
    let hang = spawn_hanging_sse_upstream().await;
    setup_multi_platform_group(&state, "gkrl6", &[("A", &hang, r#"{"rate_limits":{"concurrency":1}}"#)]).await;
    let r1 = handle_proxy(AxumState(state.clone()), messages_request_stream("gkrl6")).await;
    assert_eq!(r1.status(), StatusCode::OK);
    assert_eq!(rl_status(&state, "gkrl6").await, StatusCode::SERVICE_UNAVAILABLE, "流在途占着并发");
    drop(r1); // 客户端断连
    let r3 = handle_proxy(AxumState(state.clone()), messages_request_stream("gkrl6")).await;
    assert_eq!(r3.status(), StatusCode::OK, "断连后并发释放");
}

/// 用户故事 3：TPM 预占 + usage 修正。tpm=20：首个大请求预占远超 20，结束后按上游 usage(8) 修正，
/// 后续小请求才得以继续；累计到 >= 20 后被挡。
#[tokio::test]
async fn ratelimit_tpm_reserve_then_settle_to_usage() {
    let state = make_state(test_db().await).await;
    let up = spawn_stub_upstream(200, ANTHROPIC_OK).await;
    setup_multi_platform_group(&state, "gkrl7", &[("A", &up, r#"{"rate_limits":{"tpm":20}}"#)]).await;
    let big = format!(
        r#"{{"model":"claude-3","max_tokens":10,"messages":[{{"role":"user","content":"{}"}}]}}"#,
        "hello world ".repeat(200)
    );
    let r1 = handle_proxy(AxumState(state.clone()), messages_request("gkrl7", &big)).await;
    assert_eq!(r1.status(), StatusCode::OK, "空窗口放行大请求");
    assert_eq!(rl_status(&state, "gkrl7").await, StatusCode::OK, "预占已按 usage(8) 修正，否则这里被挡");
    assert_eq!(rl_status(&state, "gkrl7").await, StatusCode::OK, "16 + 小估算 <= 20");
    assert_eq!(rl_status(&state, "gkrl7").await, StatusCode::SERVICE_UNAVAILABLE, "24 >= 20");
    assert_eq!(last_log_for(&state, "gkrl7").await.blocked_reason, "rate_limit");
}

/// 用户故事 11：windows 命中时用窗口上限（全天窗口 rpm=1 压过平台 rpm=100）。
#[tokio::test]
async fn ratelimit_window_overrides_platform_limit() {
    let state = make_state(test_db().await).await;
    let up = spawn_stub_upstream(200, ANTHROPIC_OK).await;
    let extra = r#"{"rate_limits":{"rpm":100,"windows":[{"start_hour":0,"end_hour":24,"multiplier":1.0,"rpm":1}]}}"#;
    setup_multi_platform_group(&state, "gkrl8", &[("A", &up, extra)]).await;
    assert_eq!(rl_status(&state, "gkrl8").await, StatusCode::OK);
    assert_eq!(rl_status(&state, "gkrl8").await, StatusCode::SERVICE_UNAVAILABLE);
}

/// 模型级上限：extra.rate_limits.models.<请求模型> 超限 → 该平台被排除（候选筛选阶段）。
#[tokio::test]
async fn ratelimit_model_level_rpm_excludes_platform() {
    let state = make_state(test_db().await).await;
    let up = spawn_stub_upstream(200, ANTHROPIC_OK).await;
    let extra = r#"{"rate_limits":{"models":{"claude-3":{"rpm":1}}}}"#;
    setup_multi_platform_group(&state, "gkrl9", &[("A", &up, extra)]).await;
    assert_eq!(rl_status(&state, "gkrl9").await, StatusCode::OK);
    assert_eq!(rl_status(&state, "gkrl9").await, StatusCode::SERVICE_UNAVAILABLE);
}

/// 回归：没配 rate_limits 的平台不受影响。
#[tokio::test]
async fn ratelimit_absent_config_unaffected() {
    let state = make_state(test_db().await).await;
    let up = spawn_stub_upstream(200, ANTHROPIC_OK).await;
    setup_multi_platform_group(&state, "gkrl10", &[("A", &up, "")]).await;
    for _ in 0..5 {
        assert_eq!(rl_status(&state, "gkrl10").await, StatusCode::OK);
    }
}

fn oversize_chat_body() -> String {
    format!(
        r#"{{"model":"claude-3","max_tokens":10,"messages":[{{"role":"user","content":"{}"}}]}}"#,
        "hello world ".repeat(200)
    )
}

/// 筛选与 begin 同一估算：窗口已有占用时，估算超出剩余 TPM 的大请求在筛选阶段就被排除 → 503 + rate_limit
/// （旧：筛选估算 0 放行，begin 必败，末候选回无 blocked_reason 的通用 503）。
#[tokio::test]
async fn ratelimit_tpm_oversize_request_single_platform_503_rate_limit() {
    let state = make_state(test_db().await).await;
    let up = spawn_stub_upstream(200, ANTHROPIC_OK).await;
    setup_multi_platform_group(&state, "gkrl11", &[("A", &up, r#"{"rate_limits":{"tpm":100}}"#)]).await;
    assert_eq!(rl_status(&state, "gkrl11").await, StatusCode::OK, "小请求占用窗口 (usage 8)");
    let r = handle_proxy(AxumState(state.clone()), messages_request("gkrl11", &oversize_chat_body())).await;
    assert_eq!(r.status(), StatusCode::SERVICE_UNAVAILABLE);
    let log = last_log_for(&state, "gkrl11").await;
    assert_eq!((log.blocked_by.as_str(), log.blocked_reason.as_str()), ("router", "rate_limit"));
}

/// 同上，等效单平台组（多平台组只剩 1 个 Enabled）。
#[tokio::test]
async fn ratelimit_tpm_oversize_request_equivalent_single_503_rate_limit() {
    let state = make_state(test_db().await).await;
    let up = spawn_stub_upstream(200, ANTHROPIC_OK).await;
    setup_multi_platform_group(&state, "gkrl12", &[("A", &up, r#"{"rate_limits":{"tpm":100}}"#), ("B", &up, "")]).await;
    set_platform_status(&state, "B", aidog_db::models::PlatformStatus::Disabled).await;
    assert_eq!(rl_status(&state, "gkrl12").await, StatusCode::OK);
    let r = handle_proxy(AxumState(state.clone()), messages_request("gkrl12", &oversize_chat_body())).await;
    assert_eq!(r.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(last_log_for(&state, "gkrl12").await.blocked_reason, "rate_limit");
}

/// aux（embedding / rerank / jev）同样占限频名额：rpm=1 → 第二个请求 503 + rate_limit（路由层整组限频同语义）。
#[tokio::test]
async fn ratelimit_embedding_takes_slot_rpm1() {
    let (up, _hits) = spawn_embedding_upstream(vec![(200, EMB_RESPONSE), (200, EMB_RESPONSE)]).await;
    let state = make_state(test_db().await).await;
    setup_embedding_group(&state, "gkrl13", &[(&up, Some("emb-slot"), Protocol::OpenAI, "a")]).await;
    set_platform_extra(&state, "gkrl13-a", RPM1).await;
    let send = || handle_proxy(AxumState(state.clone()), embedding_request("gkrl13", EMB_REQ, "/v1/embeddings"));
    assert_eq!(send().await.status(), StatusCode::OK);
    assert_eq!(send().await.status(), StatusCode::SERVICE_UNAVAILABLE);
    let log = last_log_for(&state, "gkrl13").await;
    assert_eq!((log.blocked_by.as_str(), log.blocked_reason.as_str()), ("router", "rate_limit"));
}

// ── registry 错误码码义（票 ratelimit-03）：glm 的 1302=rate_limit / 1305=overloaded 由 preset 声明 ──

async fn mk_platform_of(state: &Arc<ProxyState>, ptype: Protocol, base_url: String) -> u64 {
    // 同时挂 Anthropic 端点：客户端是 anthropic 协议。
    aidog_db::test_support::create_platform_at_fake_upstream(&state.db, ptype.clone(), &base_url, &[ptype, Protocol::Anthropic]).await
}

/// 组内按序挂平台，第一个优先。
async fn group_of(state: &Arc<ProxyState>, gk: &str, ids: &[u64]) {
    let group = aidog_db::create_group(&state.db, aidog_db::test_support::sample_group(gk, vec![]))
        .await
        .unwrap();
    let inputs: Vec<_> = ids
        .iter()
        .enumerate()
        .map(|(i, id)| GroupPlatformInput {
            platform_id: *id,
            priority: Some(i as i32),
            weight: Some(1),
            level_priority: Some(1),
        })
        .collect();
    aidog_db::set_group_platforms(&state.db, group.id, &inputs).await.unwrap();
}

const HI: &str = r#"{"model":"claude-3","messages":[{"role":"user","content":"hi"}]}"#;

#[tokio::test]
async fn registry_code_rate_limit_cools_platform_to_minute_boundary() {
    let up = spawn_stub_upstream(429, r#"{"error":{"code":"1302","message":"您的账户已达到速率限制"}}"#).await;
    let state = make_state(test_db().await).await;
    let pid = mk_platform_of(&state, Protocol::Glm, up).await;
    group_of(&state, "gkrl1302", &[pid]).await;
    let _ = handle_proxy(AxumState(state.clone()), messages_request("gkrl1302", HI)).await;
    assert!(state.scheduler.quota_cooled(pid, aidog_db::now() + 1), "1302 应进每分钟冷却");
    assert!(!state.scheduler.quota_cooled(pid, aidog_db::now() + 61_000), "冷却只到下一整分边界");
}

#[tokio::test]
async fn registry_code_overloaded_is_service_class_fails_over_without_cooldown() {
    let bad = spawn_stub_upstream(400, r#"{"error":{"code":"1305","message":"该模型当前访问量过大"}}"#).await;
    let ok = spawn_stub_upstream(200, ANTHROPIC_OK).await;
    let state = make_state(test_db().await).await;
    let first = mk_platform_of(&state, Protocol::Glm, bad).await;
    let second = mk_platform_of(&state, Protocol::Anthropic, ok).await;
    group_of(&state, "gkov1305", &[first, second]).await;
    let resp = handle_proxy(AxumState(state.clone()), messages_request("gkov1305", HI)).await;
    assert_eq!(resp.status(), StatusCode::OK, "1305 归 Service，应换平台");
    assert!(!state.scheduler.quota_cooled(first, aidog_db::now() + 1), "1305 不进限频冷却");
    assert!(
        !matches!(state.scheduler.breaker_state(first), crate::gateway::scheduling::BreakerState::Closed { fails: 0 }),
        "1305 计入熔断失败"
    );
}

#[tokio::test]
async fn undeclared_code_keeps_old_behaviour() {
    // glm 未声明 9999；anthropic 平台没有任何码义——同样的 1302 body 不得触发冷却
    let up = spawn_stub_upstream(429, r#"{"error":{"code":"9999","message":"x"}}"#).await;
    let up2 = spawn_stub_upstream(429, r#"{"error":{"code":"1302","message":"x"}}"#).await;
    let state = make_state(test_db().await).await;
    let g = mk_platform_of(&state, Protocol::Glm, up).await;
    let a = mk_platform_of(&state, Protocol::Anthropic, up2).await;
    group_of(&state, "gkund1", &[g]).await;
    group_of(&state, "gkund2", &[a]).await;
    let _ = handle_proxy(AxumState(state.clone()), messages_request("gkund1", HI)).await;
    let _ = handle_proxy(AxumState(state.clone()), messages_request("gkund2", HI)).await;
    assert!(!state.scheduler.quota_cooled(g, aidog_db::now() + 1));
    assert!(!state.scheduler.quota_cooled(a, aidog_db::now() + 1));
}

// ── 路由画布 01：路由阶段补记快照（本地假上游，无外呼）──

const RC01_BODY: &str = r#"{"model":"claude-3","max_tokens":10,"messages":[{"role":"user","content":"hi"}]}"#;

/// 单平台短路（字面 / 等效）端到端：快照带 shortcut 标记。
#[tokio::test]
async fn rc01_single_platform_shortcut_in_log_snapshot() {
    let up = spawn_stub_upstream(200, ANTHROPIC_OK).await;
    let state = make_state(test_db().await).await;
    setup_group_with_upstream(&state, "gkrc1", &up).await;
    let resp = handle_proxy(AxumState(state.clone()), messages_request("gkrc1", RC01_BODY)).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let sc = last_log_for(&state, "gkrc1").await.routing.unwrap().shortcut.expect("literal shortcut");
    assert!(sc.literal);

    // 等效单平台：两平台，其中一个手动禁用
    setup_multi_platform_group(&state, "gkrc1b", &[("EA", &up, ""), ("EB", &up, "")]).await;
    set_platform_status(&state, "EB", aidog_db::models::PlatformStatus::Disabled).await;
    let resp = handle_proxy(AxumState(state.clone()), messages_request("gkrc1b", RC01_BODY)).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let sc = last_log_for(&state, "gkrc1b").await.routing.unwrap().shortcut.expect("equivalent shortcut");
    assert!(!sc.literal);
}

/// 最大尝试次数=2 + 4 个全失败平台：第 3、4 个入选候选因总次数用尽未尝试，快照可读出上限与原因。
#[tokio::test]
async fn rc01_max_retries_truncation_in_log_snapshot() {
    let up = spawn_stub_upstream(500, r#"{"error":"boom"}"#).await;
    let state = make_state(test_db().await).await;
    setup_multi_platform_group(&state, "gkrc2", &[("T1", &up, ""), ("T2", &up, ""), ("T3", &up, ""), ("T4", &up, "")]).await;
    set_group_max_attempts(&state, "gkrc2", 2).await;
    let _ = handle_proxy(AxumState(state.clone()), messages_request("gkrc2", RC01_BODY)).await;
    let r = last_log_for(&state, "gkrc2").await.routing.unwrap();
    assert_eq!(r.max_retries, Some(2));
    for name in ["T3", "T4"] {
        let e = r.entries.iter().find(|e| e.platform == name).unwrap();
        assert_eq!(e.reason, aidog_db::models::RouteReason::RetryLimit, "{name}");
    }
    assert!(r.entries.iter().filter(|e| e.platform != "T3" && e.platform != "T4").all(|e| e.reason == aidog_db::models::RouteReason::Selected));
    assert_eq!(last_log_for(&state, "gkrc2").await.attempts.len(), 2, "总次数 2 → 只调用 2 个平台");
}

/// 路由失败（全部候选手动禁用）：快照带 eval_ms。
#[tokio::test]
async fn rc01_route_failure_snapshot_has_eval_ms() {
    let up = spawn_stub_upstream(200, ANTHROPIC_OK).await;
    let state = make_state(test_db().await).await;
    setup_multi_platform_group(&state, "gkrc3", &[("FA", &up, ""), ("FB", &up, "")]).await;
    set_platform_status(&state, "FA", aidog_db::models::PlatformStatus::Disabled).await;
    set_platform_status(&state, "FB", aidog_db::models::PlatformStatus::Disabled).await;
    let resp = handle_proxy(AxumState(state.clone()), messages_request("gkrc3", RC01_BODY)).await;
    assert_ne!(resp.status(), StatusCode::OK);
    let r = last_log_for(&state, "gkrc3").await.routing.expect("route-fail snapshot");
    assert!(r.eval_ms.is_some(), "eval_ms missing on failure path");
    assert_eq!(r.entries.len(), 2);
}

/// 透传平台认领成功（解析失败分流 + count_tokens 子端点分流两条路径）：写快照。
#[tokio::test]
async fn rc01_passthrough_claim_writes_snapshot() {
    const OK: &str = r#"{"id":"x","usage":{"prompt_tokens":7,"completion_tokens":2}}"#;
    let (upstream, _seen) = spawn_recording_upstream(OK).await;
    let state = make_state(test_db().await).await;
    setup_passthrough_platform_group(&state, "gkrc4", &upstream).await;
    for path in ["/v1/messages", "/v1/messages/count_tokens"] {
        let req = HttpRequest::builder()
            .method("POST")
            .uri(path)
            .header("authorization", "Bearer gkrc4")
            .header("content-type", "application/json")
            .body(Body::from(if path.ends_with("tokens") { r#"{"model":"m","messages":[]}"# } else { "not json" }))
            .unwrap();
        let resp = handle_proxy(AxumState(state.clone()), req).await;
        assert!(resp.status().is_success(), "{path}: {}", resp.status());
        let r = last_log_for(&state, "gkrc4").await.routing.unwrap_or_else(|| panic!("{path}: no snapshot"));
        assert_eq!(r.entries.len(), 1, "{path}");
        assert_eq!(r.entries[0].reason, aidog_db::models::RouteReason::Selected);
        assert!(r.eval_ms.is_some());
        // 下一轮 last_log_for 取同 group 最新一条前清掉旧行的歧义：不同 path 各自断言即可
    }
}

// ── 路由画布 02：转发尝试补记种类 / 错误分类 / 副作用（本地假上游，无外呼）──

/// 传输错误：同平台原地重试那一项 kind=retry；末次耗尽失败项归服务类并带降权副作用
/// （单候选组触发本地网络保护，不计熔断）。
#[tokio::test]
async fn rc02_transport_retry_kind_and_exhausted_class() {
    let up = spawn_flaky_upstream(1).await;
    let state = make_state(test_db().await).await;
    setup_group_with_upstream(&state, "gkrc02a", &up).await;
    assert_eq!(handle_proxy(AxumState(state.clone()), messages_request("gkrc02a", RC01_BODY)).await.status(), StatusCode::OK);
    let log = last_log_for(&state, "gkrc02a").await;
    assert_eq!(log.attempts.len(), 2);
    assert_eq!(log.attempts[0].kind.as_deref(), Some("retry"));
    assert_eq!(log.attempts[1].kind, None);

    let dead = spawn_flaky_upstream(usize::MAX).await;
    setup_group_with_upstream(&state, "gkrc02b", &dead).await;
    assert_eq!(handle_proxy(AxumState(state.clone()), messages_request("gkrc02b", RC01_BODY)).await.status(), StatusCode::BAD_GATEWAY);
    let last = last_log_for(&state, "gkrc02b").await.attempts.pop().unwrap();
    assert_eq!(last.kind, None);
    assert_eq!(last.error_class.as_deref(), Some("service"));
    assert_eq!(last.effects, ["penalty_server"]);
}

/// anthropic-beta 400 剔头重试：被降级的那次 400 标 beta_downgrade，重试成功项为正常。
#[tokio::test]
async fn rc02_beta_downgrade_kind() {
    use axum::routing::any;
    let app = axum::Router::new().fallback(any(|req: HttpRequest<Body>| async move {
        if req.headers().contains_key("anthropic-beta") {
            (axum::http::StatusCode::BAD_REQUEST, [("content-type", "application/json")], r#"{"error":{"message":"bad"}}"#)
        } else {
            (axum::http::StatusCode::OK, [("content-type", "application/json")], ANTHROPIC_OK)
        }
    }));
    let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let up = format!("http://{}", l.local_addr().unwrap());
    tokio::spawn(async move { axum::serve(l, app).await.ok(); });
    let state = make_state(test_db().await).await;
    setup_group_with_upstream(&state, "gkrc02c", &up).await;
    let req = HttpRequest::builder()
        .method("POST").uri("/v1/messages")
        .header("authorization", "Bearer gkrc02c")
        .header("content-type", "application/json")
        .header("anthropic-beta", "x-new-beta")
        .body(Body::from(RC01_BODY)).unwrap();
    assert_eq!(handle_proxy(AxumState(state.clone()), req).await.status(), StatusCode::OK);
    let log = last_log_for(&state, "gkrc02c").await;
    assert_eq!(log.attempts.len(), 2);
    assert_eq!(log.attempts[0].kind.as_deref(), Some("beta_downgrade"));
    assert_eq!(log.attempts[0].status_code, 400);
    assert_eq!(log.attempts[1].kind, None);
}

/// 200 空响应（决策 B）：kind=empty_200、服务类、降权副作用；换平台时链上剩余模型记 chain_stop=service。
#[tokio::test]
async fn rc02_empty_200_kind_class_and_chain_stop() {
    let empty = spawn_stub_upstream(200, "").await;
    let ok = spawn_stub_upstream(200, ANTHROPIC_OK).await;
    let state = make_state(test_db().await).await;
    setup_aux_group(&state, "gkrc02d", &[
        (&empty, chain_models(&["m-a", "m-b"]), Protocol::Anthropic, "p1"),
        (&ok, chain_models(&["m-c"]), Protocol::Anthropic, "p2"),
    ]).await;
    assert_eq!(chain_send(&state, "gkrc02d", CHAIN_REQ).await.status(), StatusCode::OK);
    let log = last_log_for(&state, "gkrc02d").await;
    assert_eq!(log.attempts.len(), 2, "m-b 未尝试，不出现");
    let a = &log.attempts[0];
    assert_eq!((a.kind.as_deref(), a.status_code), (Some("empty_200"), 200));
    assert_eq!(a.error_class.as_deref(), Some("service"));
    assert_eq!(a.effects, ["penalty_server"]);
    assert_eq!(a.chain_stop.as_deref(), Some("service"), "空 200 换平台，m-b 未试");
    assert_eq!(log.attempts[1].kind, None);
}

/// 错误类别 / registry 码义 / 副作用：每行 (协议, 状态码, body, 期望类别, 期望码义, 期望副作用)。
#[tokio::test]
async fn rc02_error_class_code_meaning_and_effects() {
    #[allow(clippy::type_complexity)] // 测试表行，拆 type 反而难读
    let cases: [(Protocol, u16, &'static str, &str, Option<&str>, &[&str]); 5] = [
        (Protocol::Glm, 429, r#"{"error":{"code":"1302","message":"x"}}"#, "service", Some("rate_limit"), &["cooldown_minute"]),
        (Protocol::Glm, 400, r#"{"error":{"code":"1305","message":"x"}}"#, "service", Some("overloaded"), &["breaker_count", "penalty_client"]),
        (Protocol::Anthropic, 401, r#"{"error":{"message":"invalid api key"}}"#, "service", None, &["penalty_client", "cooldown_auth"]),
        (Protocol::Anthropic, 402, r#"{"error":{"message":"insufficient balance"}}"#, "service", None, &["penalty_client", "cooldown_balance"]),
        (Protocol::Anthropic, 500, r#"{"error":{"message":"service unavailable"}}"#, "service", None, &["penalty_server"]),
    ];
    for (i, (proto, status, body, class, meaning, effects)) in cases.into_iter().enumerate() {
        let up = spawn_stub_upstream(status, body).await;
        let state = make_state(test_db().await).await;
        let pid = mk_platform_of(&state, proto, up).await;
        let gk = format!("gkrc02e{i}");
        group_of(&state, &gk, &[pid]).await;
        let _ = handle_proxy(AxumState(state.clone()), messages_request(&gk, HI)).await;
        let a = last_log_for(&state, &gk).await.attempts.remove(0);
        assert_eq!(a.kind, None, "case {i}");
        assert_eq!(a.error_class.as_deref(), Some(class), "case {i}");
        assert_eq!(a.code_meaning.as_deref(), meaning, "case {i}");
        assert_eq!(a.effects, effects, "case {i}");
    }
}

/// 模型类失败（换模型，未记账）：带 model 类别、无副作用；命中 error_rule 时带规则标识。
#[tokio::test]
async fn rc02_model_class_no_effects_and_error_rule_recorded() {
    use aidog_db::models::{
        ActionKind, ActionParams, ActionStep, AppliesTo, ConditionLeaf, ConditionNode, MatchType, MiddlewareRule, Target,
    };
    let (up, _hits) = spawn_embedding_upstream(vec![(404, E404_MODEL), (200, ANTHROPIC_OK)]).await;
    let state = make_state(test_db().await).await;
    setup_aux_group(&state, "gkrc02f", &[(&up, chain_models(&["m-a", "m-b"]), Protocol::Anthropic, "p")]).await;
    assert_eq!(chain_send(&state, "gkrc02f", CHAIN_REQ).await.status(), StatusCode::OK);
    let a = last_log_for(&state, "gkrc02f").await.attempts.remove(0);
    assert_eq!((a.error_class.as_deref(), a.error_rule.as_deref()), (Some("model"), None));
    assert!(a.effects.is_empty(), "换模型未记账，无副作用: {:?}", a.effects);

    let (up, _hits) = spawn_embedding_upstream(vec![(404, E404_MODEL), (200, ANTHROPIC_OK)]).await;
    state.middleware.rebuild_from_rules(vec![MiddlewareRule {
        id: 1,
        name: "no-retry-404".into(),
        description: String::new(),
        conditions: ConditionNode::Leaf(ConditionLeaf {
            target: Target::Status,
            field: String::new(),
            match_type: MatchType::Regex,
            pattern: "404".into(),
            validator: String::new(),
        }),
        actions: vec![ActionStep {
            kind: ActionKind::Classify,
            params: ActionParams { category: "x".into(), retryable: false, ..Default::default() },
        }],
        applies_to: AppliesTo::default(),
        priority: 0,
        enabled: true,
        is_builtin: false,
        failed: false,
        created_at: 0,
        updated_at: 0,
    }]);
    setup_aux_group(&state, "gkrc02g", &[(&up, chain_models(&["m-a", "m-b"]), Protocol::Anthropic, "p")]).await;
    assert_eq!(chain_send(&state, "gkrc02g", CHAIN_REQ).await.status(), StatusCode::NOT_FOUND);
    let a = last_log_for(&state, "gkrc02g").await.attempts.remove(0);
    assert!(a.error_rule.as_deref().is_some_and(|r| r.contains("no-retry-404")), "{:?}", a.error_rule);
    assert_eq!(a.chain_stop.as_deref(), Some("rule"));
}

/// 模型级限频跳过：不再无痕，推为 kind=rate_limit_model 的一次尝试，链继续下一个模型。
#[tokio::test]
async fn rc02_model_rate_limit_skip_recorded() {
    let (up, hits) = spawn_embedding_upstream(vec![(200, ANTHROPIC_OK), (200, ANTHROPIC_OK)]).await;
    let state = make_state(test_db().await).await;
    setup_aux_group(&state, "gkrc02h", &[(&up, chain_models(&["m-a", "m-b"]), Protocol::Anthropic, "p")]).await;
    set_platform_extra(&state, "gkrc02h-p", r#"{"rate_limits":{"models":{"m-a":{"rpm":1}}}}"#).await;
    assert_eq!(chain_send(&state, "gkrc02h", CHAIN_REQ).await.status(), StatusCode::OK);
    state.db.invalidate_group_details_cache();
    assert_eq!(chain_send(&state, "gkrc02h", CHAIN_REQ).await.status(), StatusCode::OK);
    assert_eq!(hit_models(&hits), ["m-a", "m-b"], "第二次 m-a 被限频跳过，直接用 m-b");
    let log = last_log_for(&state, "gkrc02h").await;
    assert_eq!(log.attempts.len(), 2);
    assert_eq!(log.attempts[0].kind.as_deref(), Some("rate_limit_model"));
    assert_eq!(log.attempts[0].model.as_deref(), Some("m-a"));
    assert_eq!(log.attempts[1].kind, None);
    assert_eq!(log.attempts[1].model.as_deref(), Some("m-b"));
}

/// 重排尊重限频：A 的假上游在收到请求时顺手占满 B 的并发位，A 500 后重排挡掉 B，终态 503 + blocked_reason=rate_limit。
#[tokio::test]
async fn rc02_platform_rate_limit_skip_recorded() {
    use axum::routing::any;
    let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let up_a = format!("http://{}", l.local_addr().unwrap());
    let up_b = spawn_stub_upstream(200, ANTHROPIC_OK).await;
    let state = make_state(test_db().await).await;
    setup_aux_group(&state, "gkrc02i", &[
        (&up_a, chain_models(&["m-a"]), Protocol::Anthropic, "pa"),
        (&up_b, chain_models(&["m-b1", "m-b2"]), Protocol::Anthropic, "pb"),
    ]).await;
    let b_extra = r#"{"rate_limits":{"concurrency":1}}"#;
    set_platform_extra(&state, "gkrc02i-pb", b_extra).await;
    let b_id = platform_id_of(&state, "gkrc02i-pb").await;
    let held: Arc<std::sync::Mutex<Vec<Arc<crate::gateway::rate_limit::Access>>>> = Default::default();
    let (st, held2) = (state.clone(), held.clone());
    let app = axum::Router::new().fallback(any(move || {
        let (st, held) = (st.clone(), held2.clone());
        async move {
            let now = aidog_db::now();
            let lim = crate::gateway::rate_limit::RateLimitsCfg::parse(b_extra).platform_limits(now, "");
            if let Some(a) = st.scheduler.rate.begin(b_id, &lim, now, 0) {
                held.lock().unwrap().push(a);
            }
            (axum::http::StatusCode::INTERNAL_SERVER_ERROR, [("content-type", "application/json")], r#"{"error":{"message":"boom"}}"#)
        }
    }));
    tokio::spawn(async move { axum::serve(l, app).await.ok(); });
    let resp = chain_send(&state, "gkrc02i", CHAIN_REQ).await;
    assert_eq!(resp.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(held.lock().unwrap().len(), 1, "B 并发位应已被占");
    let log = last_log_for(&state, "gkrc02i").await;
    // 逐次重算：A 失败后重排时 B 已被并发占满，直接被路由层挡掉（不再进到 begin 失败），
    // 只留 A 的一条尝试；终态 503 沿用限频审计标记。
    assert_eq!(log.attempts.len(), 1);
    assert_eq!(log.attempts[0].status_code, 500);
    assert_eq!(log.attempts[0].exclusion.as_deref(), Some("platform"));
    assert_ne!(log.attempts[0].platform_id, b_id);
    assert_eq!(log.blocked_reason, "rate_limit");
}

// ══ 请求尝试-逐次重算 票 02：每次调用前按当前调度状态重排 + 按错误类排除 ══

const E500_PLAIN: &str = r#"{"error":{"message":"boom"}}"#;

/// 排除范围落在尝试记录上：模型类 → pair，服务类 → platform；成功 / 直返不带。
#[tokio::test]
async fn ra02_attempt_records_exclusion_scope() {
    let (up1, _h1) = spawn_embedding_upstream(vec![(404, E404_MODEL), (500, E500_PLAIN)]).await;
    let (up2, _h2) = spawn_embedding_upstream(vec![(200, ANTHROPIC_OK)]).await;
    let state = make_state(test_db().await).await;
    setup_aux_group(
        &state,
        "gkra02a",
        &[
            (&up1, chain_models(&["m-a", "m-b"]), Protocol::Anthropic, "p1"),
            (&up2, chain_models(&["m-c"]), Protocol::Anthropic, "p2"),
        ],
    )
    .await;
    assert_eq!(chain_send(&state, "gkra02a", CHAIN_REQ).await.status(), StatusCode::OK);
    let log = last_log_for(&state, "gkra02a").await;
    let scopes: Vec<_> = log.attempts.iter().map(|a| a.exclusion.as_deref()).collect();
    assert_eq!(scopes, [Some("pair"), Some("platform"), None]);
}

/// 失败组合本次请求内不重复；每个组合恰好试一次。
#[tokio::test]
async fn ra02_failed_pair_never_repeated() {
    let (up1, h1) = spawn_embedding_upstream(vec![(404, E404_MODEL); 6]).await;
    let (up2, h2) = spawn_embedding_upstream(vec![(404, E404_MODEL); 6]).await;
    let state = make_state(test_db().await).await;
    setup_aux_group(
        &state,
        "gkra02b",
        &[
            (&up1, chain_models(&["m-a", "m-b"]), Protocol::Anthropic, "p1"),
            (&up2, chain_models(&["m-c", "m-d"]), Protocol::Anthropic, "p2"),
        ],
    )
    .await;
    let resp = chain_send(&state, "gkra02b", CHAIN_REQ).await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    assert_eq!(hit_models(&h1), ["m-a", "m-b"]);
    assert_eq!(hit_models(&h2), ["m-c", "m-d"]);
    assert_eq!(last_log_for(&state, "gkra02b").await.attempts.len(), 4);
}

/// 服务类失败每次记一次：两个平台各 500 → 各降权一次、attempts 两条（首次排除整平台）；
/// 各自链上第二个模型不再被选。
#[tokio::test]
async fn ra02_service_failures_account_once_each() {
    let (up1, h1) = spawn_embedding_upstream(vec![(500, E500_PLAIN); 4]).await;
    let (up2, h2) = spawn_embedding_upstream(vec![(500, E500_PLAIN); 4]).await;
    let state = make_state(test_db().await).await;
    setup_aux_group(
        &state,
        "gkra02c",
        &[
            (&up1, chain_models(&["m-a", "m-b"]), Protocol::Anthropic, "p1"),
            (&up2, chain_models(&["m-c", "m-d"]), Protocol::Anthropic, "p2"),
        ],
    )
    .await;
    let _ = chain_send(&state, "gkra02c", CHAIN_REQ).await;
    assert_eq!(hit_models(&h1), ["m-a"]);
    assert_eq!(hit_models(&h2), ["m-c"]);
    for n in ["gkra02c-p1", "gkra02c-p2"] {
        let id = platform_id_of(&state, n).await;
        assert_eq!(state.scheduler.penalty_consecutive(id), 1, "{n}");
    }
    let log = last_log_for(&state, "gkra02c").await;
    assert_eq!(log.attempts.len(), 2);
    assert_eq!(log.attempts[0].exclusion.as_deref(), Some("platform"));
    assert_eq!(log.attempts[1].exclusion, None, "末次直接回客户端，没有后续可排除");
}

/// 模型类失败不碰平台健康（真三库读回）：breaker 计数、last_error、降权都不变；
/// 同平台下一个模型被选中。
#[tokio::test]
async fn ra02_model_failure_leaves_platform_health_untouched() {
    let (db, _dir) = aidog_db::test_support::three_file_db().await;
    let (up, hits) = spawn_embedding_upstream(vec![(404, E404_MODEL), (200, ANTHROPIC_OK)]).await;
    let state = make_state(db).await;
    setup_aux_group(&state, "gkra02d", &[(&up, chain_models(&["m-a", "m-b"]), Protocol::Anthropic, "p")]).await;
    assert_eq!(chain_send(&state, "gkra02d", CHAIN_REQ).await.status(), StatusCode::OK);
    assert_eq!(hit_models(&hits), ["m-a", "m-b"]);
    let p = aidog_db::list_platforms(&state.db).await.unwrap().remove(0);
    assert!(p.last_error.is_empty(), "{:?}", p.last_error);
    assert!(matches!(state.scheduler.breaker_state(p.id), crate::gateway::scheduling::BreakerState::Closed { fails: 0 }));
    assert_eq!(state.scheduler.penalty_consecutive(p.id), 0);
}

/// 第 1 次尝试期间调度状态变了（这里：P2 熔断打开）→ 第 2 次尝试重排，跳过 P2 直接选 P3。
/// 旧实现按首次排序固定顺序会去撞 P2。
#[tokio::test]
async fn ra02_state_change_between_attempts_reranks() {
    use axum::routing::any;
    let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let up1 = format!("http://{}", l.local_addr().unwrap());
    let (up2, h2) = spawn_embedding_upstream(vec![(200, ANTHROPIC_OK)]).await;
    let (up3, h3) = spawn_embedding_upstream(vec![(200, ANTHROPIC_OK)]).await;
    let state = make_state(test_db().await).await;
    setup_aux_group(
        &state,
        "gkra02e",
        &[
            (&up1, chain_models(&["m-a"]), Protocol::Anthropic, "p1"),
            (&up2, chain_models(&["m-b"]), Protocol::Anthropic, "p2"),
            (&up3, chain_models(&["m-c"]), Protocol::Anthropic, "p3"),
        ],
    )
    .await;
    let p2 = platform_id_of(&state, "gkra02e-p2").await;
    let st = state.clone();
    let app = axum::Router::new().fallback(any(move || {
        let st = st.clone();
        async move {
            let th = crate::gateway::scheduling::BreakerThresholds { failure_threshold: 1, open_secs: 60, half_open_max: 1 };
            st.scheduler.record_failure(p2, &th, aidog_db::now());
            (axum::http::StatusCode::INTERNAL_SERVER_ERROR, [("content-type", "application/json")], E500_PLAIN)
        }
    }));
    tokio::spawn(async move { axum::serve(l, app).await.ok(); });
    assert_eq!(chain_send(&state, "gkra02e", CHAIN_REQ).await.status(), StatusCode::OK);
    assert!(h2.lock().unwrap().is_empty(), "P2 熔断后不应再被选");
    assert_eq!(hit_models(&h3), ["m-c"]);
}

/// 不可重试错误（400 硬错、非模型类）立即回客户端，不换平台也不换模型。
#[tokio::test]
async fn ra02_hard_400_returns_immediately() {
    let (up1, h1) = spawn_embedding_upstream(vec![(400, r#"{"error":{"message":"bad field"}}"#); 3]).await;
    let (up2, h2) = spawn_embedding_upstream(vec![(200, ANTHROPIC_OK)]).await;
    let state = make_state(test_db().await).await;
    setup_aux_group(
        &state,
        "gkra02f",
        &[
            (&up1, chain_models(&["m-a"]), Protocol::Anthropic, "p1"),
            (&up2, chain_models(&["m-c"]), Protocol::Anthropic, "p2"),
        ],
    )
    .await;
    assert_eq!(chain_send(&state, "gkra02f", CHAIN_REQ).await.status(), StatusCode::BAD_REQUEST);
    assert_eq!(hit_models(&h1), ["m-a"]);
    assert!(h2.lock().unwrap().is_empty());
}

// ══ 请求尝试-逐次重算 票 03：向量化 / 重排序 / 决策 接入同一尝试编排（搜索转发见 test_tools.rs）══

/// 绑一个端口再释放：连接必被拒（传输错误，connect 失败）。
async fn ra03_dead_base() -> String {
    let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    format!("http://{}", l.local_addr().unwrap())
}

/// 第 1 次尝试期间 P2 熔断打开 → 第 2 次尝试重排，跳过 P2 直接选 P3（旧固定顺序会去撞 P2）。
#[tokio::test]
async fn ra03_aux_state_change_between_attempts_reranks() {
    use axum::routing::any;
    for kind in AUX_KINDS {
        let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let up1 = format!("http://{}", l.local_addr().unwrap());
        let (up2, h2) = spawn_embedding_upstream(vec![(200, kind.ok_body())]).await;
        let (up3, h3) = spawn_embedding_upstream(vec![(200, kind.ok_body())]).await;
        let state = make_state(test_db().await).await;
        let gk = format!("gkra03a{}", kind as u8);
        setup_aux_chain(&state, &gk, kind, &[(&up1, &["m-a"]), (&up2, &["m-b"]), (&up3, &["m-c"])]).await;
        let p2 = platform_id_of(&state, &format!("{gk}-pb")).await;
        let st = state.clone();
        let app = axum::Router::new().fallback(any(move || {
            let st = st.clone();
            async move {
                let th = crate::gateway::scheduling::BreakerThresholds { failure_threshold: 1, open_secs: 60, half_open_max: 1 };
                st.scheduler.record_failure(p2, &th, aidog_db::now());
                (axum::http::StatusCode::INTERNAL_SERVER_ERROR, [("content-type", "application/json")], E500_PLAIN)
            }
        }));
        tokio::spawn(async move { axum::serve(l, app).await.ok(); });
        let resp = handle_proxy(AxumState(state.clone()), kind.request(&gk)).await;
        assert_eq!(resp.status(), StatusCode::OK, "{kind:?}");
        assert!(h2.lock().unwrap().is_empty(), "{kind:?}: P2 熔断后不应再被选");
        assert_eq!(hit_models(&h3), ["m-c"], "{kind:?}");
    }
}

/// 连接失败属服务类：排除整个平台并在尝试记录上标 platform；共享同一 base_url 的兄弟平台在本次请求内跳过（R3）。
#[tokio::test]
async fn ra03_aux_connect_failure_excludes_platform_and_dead_siblings() {
    for kind in AUX_KINDS {
        let dead = ra03_dead_base().await;
        let (up3, h3) = spawn_embedding_upstream(vec![(200, kind.ok_body())]).await;
        let state = make_state(test_db().await).await;
        let gk = format!("gkra03b{}", kind as u8);
        setup_aux_chain(&state, &gk, kind, &[(&dead, &["m-a", "m-x"]), (&dead, &["m-b"]), (&up3, &["m-c"])]).await;
        let resp = handle_proxy(AxumState(state.clone()), kind.request(&gk)).await;
        assert_eq!(resp.status(), StatusCode::OK, "{kind:?}");
        let log = last_log_for(&state, &gk).await;
        let scopes: Vec<_> = log.attempts.iter().map(|a| (a.model.as_deref(), a.exclusion.as_deref())).collect();
        assert_eq!(scopes, [(Some("m-a"), Some("platform")), (Some("m-c"), None)], "{kind:?}");
        assert_eq!(hit_models(&h3), ["m-c"], "{kind:?}");
    }
}

/// 服务类失败排除整个平台（链上第二个模型不再试）+ 每次记一次；模型类失败换同平台下一个模型且不碰平台健康（三库读回）。
#[tokio::test]
async fn ra03_aux_service_excludes_platform_model_switches_same_platform() {
    for kind in AUX_KINDS {
        let (db, _dir) = aidog_db::test_support::three_file_db().await;
        let (up1, h1) = spawn_embedding_upstream(vec![(500, E500_PLAIN); 3]).await;
        let (up2, h2) = spawn_embedding_upstream(vec![(404, E404_MODEL), (200, kind.ok_body())]).await;
        let state = make_state(db).await;
        let gk = format!("gkra03c{}", kind as u8);
        setup_aux_chain(&state, &gk, kind, &[(&up1, &["m-a", "m-b"]), (&up2, &["m-c", "m-d"])]).await;
        let resp = handle_proxy(AxumState(state.clone()), kind.request(&gk)).await;
        assert_eq!(resp.status(), StatusCode::OK, "{kind:?}");
        assert_eq!(hit_models(&h1), ["m-a"], "{kind:?}: 服务类失败后该平台其他模型不再选");
        assert_eq!(hit_models(&h2), ["m-c", "m-d"], "{kind:?}");
        let log = last_log_for(&state, &gk).await;
        let scopes: Vec<_> = log.attempts.iter().map(|a| a.exclusion.as_deref()).collect();
        assert_eq!(scopes, [Some("platform"), Some("pair"), None], "{kind:?}");
        let (p1, p2) = (platform_id_of(&state, &format!("{gk}-pa")).await, platform_id_of(&state, &format!("{gk}-pb")).await);
        assert_eq!(state.scheduler.penalty_consecutive(p1), 1, "{kind:?}: 服务类记一次");
        assert_eq!(state.scheduler.penalty_consecutive(p2), 0, "{kind:?}: 模型类不降权");
        let plats = aidog_db::list_platforms(&state.db).await.unwrap();
        let pb = plats.iter().find(|p| p.id == p2).unwrap();
        assert!(pb.last_error.is_empty(), "{kind:?}: 模型类不写 last_error: {:?}", pb.last_error);
        assert!(matches!(state.scheduler.breaker_state(p2), crate::gateway::scheduling::BreakerState::Closed { fails: 0 }), "{kind:?}");
    }
}

/// 最大尝试次数 3：首平台链 4 个模型全模型类失败 → 只调用 3 次，次平台零调用。
#[tokio::test]
async fn ra03_aux_budget_cap_counts_model_switches() {
    for kind in AUX_KINDS {
        let (up1, h1) = spawn_embedding_upstream(vec![(404, E404_MODEL); 6]).await;
        let (up2, h2) = spawn_embedding_upstream(vec![(200, kind.ok_body())]).await;
        let state = make_state(test_db().await).await;
        let gk = format!("gkra03d{}", kind as u8);
        setup_aux_chain(&state, &gk, kind, &[(&up1, &["m-a", "m-b", "m-c", "m-d"]), (&up2, &["m-e"])]).await;
        set_group_max_attempts(&state, &gk, 3).await;
        let resp = handle_proxy(AxumState(state.clone()), kind.request(&gk)).await;
        assert_ne!(resp.status(), StatusCode::OK, "{kind:?}");
        assert_eq!(hit_models(&h1), ["m-a", "m-b", "m-c"], "{kind:?}");
        assert!(h2.lock().unwrap().is_empty(), "{kind:?}: 上限用尽，次平台不被调用");
        assert_eq!(last_log_for(&state, &gk).await.attempts.len(), 3, "{kind:?}");
    }
}

// ─── 管理接口（ADR 0010）：/api/rpc/* 挂在代理端口 ───────────────────────────

/// 装一个只带 `/rpc/ping` 的测试路由（真路由表在 `aidog_rpc`，由内核测试覆盖）。
fn install_mgmt_test_router() {
    super::install_management_router(
        axum::Router::new().route(
            "/rpc/ping",
            axum::routing::post(|| async { axum::Json(serde_json::json!({"pong": true})) }),
        ),
    );
}

async fn set_mgmt(state: &Arc<ProxyState>, api_enabled: bool, token: &str) {
    crate::kernel_settings::save_kernel_settings(
        &state.db,
        &crate::kernel_settings::KernelSettings {
            api_enabled,
            auth_token: token.into(),
        },
    )
    .await
    .unwrap();
}

fn mgmt_client() -> reqwest::Client {
    reqwest::Client::builder().no_proxy().build().unwrap()
}

/// 开关缺省开、凭据空：本机直接调通，返回命令的 JSON。
#[tokio::test]
async fn mgmt_rpc_default_on_returns_command_json() {
    install_mgmt_test_router();
    let state = make_state(test_db().await).await;
    let base = spawn_proxy_router(state.clone()).await;
    let r = mgmt_client().post(format!("{base}/api/rpc/ping")).send().await.unwrap();
    assert_eq!(r.status(), 200);
    assert_eq!(r.json::<serde_json::Value>().await.unwrap(), serde_json::json!({"pong": true}));
}

/// 开关关：明确的「未启用」404 + JSON，且不进转发（不落 proxy_log）。
#[tokio::test]
async fn mgmt_rpc_disabled_returns_not_enabled_and_does_not_forward() {
    install_mgmt_test_router();
    let state = make_state(test_db().await).await;
    set_mgmt(&state, false, "").await;
    let base = spawn_proxy_router(state.clone()).await;
    for path in ["/api/rpc/ping", "/api/events", "/api/ui/", "/api/ui/x.js"] {
        let r = mgmt_client().request(
            if path == "/api/rpc/ping" { reqwest::Method::POST } else { reqwest::Method::GET },
            format!("{base}{path}"),
        ).send().await.unwrap();
        assert_eq!(r.status(), 404, "{path}");
        let v: serde_json::Value = r.json().await.unwrap();
        assert_eq!(v["error"], "management_api_disabled", "{path}");
    }
    flush_log_queue(&state).await;
    assert!(aidog_logs::list_proxy_logs(&state.db, 50, 0).await.unwrap().is_empty(), "未启用不得落入转发");
}

/// 凭据非空：缺 / 错 Bearer → 401，对的 → 200。
#[tokio::test]
async fn mgmt_rpc_optional_token_is_enforced() {
    install_mgmt_test_router();
    let state = make_state(test_db().await).await;
    set_mgmt(&state, true, "s3cret").await;
    let base = spawn_proxy_router(state.clone()).await;
    let url = format!("{base}/api/rpc/ping");
    let c = mgmt_client();
    assert_eq!(c.post(&url).send().await.unwrap().status(), 401);
    assert_eq!(c.post(&url).bearer_auth("nope").send().await.unwrap().status(), 401);
    assert_eq!(c.post(&url).bearer_auth("s3cret").send().await.unwrap().status(), 200);
}

/// 非回环来源一律 403（bind_lan=true 监听 0.0.0.0 时同样只放行本机）。
#[tokio::test]
async fn mgmt_rpc_rejects_non_loopback_peer() {
    install_mgmt_test_router();
    let state = make_state(test_db().await).await;
    let req = axum::http::Request::builder()
        .method("POST")
        .uri("/api/rpc/ping")
        .body(axum::body::Body::empty())
        .unwrap();
    let r = super::handle_management(
        AxumState(state),
        axum::extract::ConnectInfo("10.1.2.3:5555".parse().unwrap()),
        req,
    )
    .await;
    assert_eq!(r.status(), 403);
}

/// 未登记的命令走管理接口的 404，不落入转发；其他路径照常进转发（根健康端点仍在）。
#[tokio::test]
async fn mgmt_rpc_unknown_command_is_404_and_other_paths_still_route() {
    install_mgmt_test_router();
    let state = make_state(test_db().await).await;
    let base = spawn_proxy_router(state.clone()).await;
    let c = mgmt_client();
    assert_eq!(c.post(format!("{base}/api/rpc/nope")).send().await.unwrap().status(), 404);
    flush_log_queue(&state).await;
    assert!(aidog_logs::list_proxy_logs(&state.db, 50, 0).await.unwrap().is_empty());
    let root: serde_json::Value = c.get(format!("{base}/")).send().await.unwrap().json().await.unwrap();
    assert_eq!(root["service"], "aidog");
}

/// 调用记录（API 票 03）：落行、字段正确、来源可区分、正文（请求体）不入库。
#[tokio::test]
async fn mgmt_call_log_records_metadata_without_bodies() {
    install_mgmt_test_router();
    let state = make_state(test_db().await).await;
    let base = spawn_proxy_router(state.clone()).await;
    let c = mgmt_client();
    let url = format!("{base}/api/rpc/ping");
    let r = c.post(&url).header("user-agent", "my-script/1").body("BODY-MARKER-9f3a").send().await.unwrap();
    assert_eq!(r.status(), 200);
    let r = c.post(&url).header(super::SOURCE_HEADER, super::SETTINGS_TEST_SOURCE).send().await.unwrap();
    assert_eq!(r.status(), 200);

    let rows = aidog_db::list_api_call_logs(&state.db, 10).await.unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[1].command, "ping");
    assert_eq!(rows[1].status_code, 200);
    assert!(rows[1].success);
    assert_eq!(rows[1].user_agent, "my-script/1");
    assert_eq!(rows[1].source, "third_party");
    assert!(rows[1].created_at > 0);
    assert_eq!(rows[0].source, "settings_test");
    // 全表任意列都不含请求 / 响应正文。
    let dump: String = state
        .db
        .call_read_proxy_log_traced(None, std::panic::Location::caller(), |conn| {
            Ok(conn.query_row(
                "SELECT group_concat(command || user_agent || source, '|') FROM api_call_log",
                [],
                |r| r.get::<_, String>(0),
            )?)
        })
        .await
        .unwrap();
    assert!(!dump.contains("BODY-MARKER-9f3a") && !dump.contains("pong"));
}

/// 被拒请求（未启用 404 / 凭据错 401）也记录状态与失败标记；放行后的 404 同样记。非回环 403 不记（见下一条）。
#[tokio::test]
async fn mgmt_call_log_records_rejected_requests() {
    install_mgmt_test_router();
    let state = make_state(test_db().await).await;
    let base = spawn_proxy_router(state.clone()).await;
    let c = mgmt_client();
    let url = format!("{base}/api/rpc/ping");

    set_mgmt(&state, false, "").await;
    assert_eq!(c.post(&url).send().await.unwrap().status(), 404);
    set_mgmt(&state, true, "s3cret").await;
    assert_eq!(c.post(&url).bearer_auth("nope").send().await.unwrap().status(), 401);
    let req = axum::http::Request::builder()
        .method("POST")
        .uri("/api/rpc/ping")
        .body(axum::body::Body::empty())
        .unwrap();
    let r = super::handle_management(
        AxumState(state.clone()),
        axum::extract::ConnectInfo("10.1.2.3:5555".parse().unwrap()),
        req,
    )
    .await;
    assert_eq!(r.status(), 403);
    assert_eq!(c.post(format!("{base}/api/rpc/nope")).bearer_auth("s3cret").send().await.unwrap().status(), 404);

    let rows = aidog_db::list_api_call_logs(&state.db, 10).await.unwrap();
    let got: Vec<(String, i32, bool)> =
        rows.iter().rev().map(|r| (r.command.clone(), r.status_code, r.success)).collect();
    assert_eq!(
        got,
        [
            ("ping".to_string(), 404, false),
            ("ping".to_string(), 401, false),
            ("nope".to_string(), 404, false),
        ]
    );
}

fn rc_chat_req(gk: &str) -> Request {
    HttpRequest::builder()
        .method("POST")
        .uri("/v1/messages")
        .header("authorization", format!("Bearer {gk}"))
        .header("content-type", "application/json")
        .body(Body::from(r#"{"model":"claude-3","messages":[{"role":"user","content":"hi"}]}"#))
        .unwrap()
}

/// 路由画布修复 1：透传平台转发成功 → attempts 记一条真实 ProxyAttempt（platform / status / 耗时），retry_count 仍 0。
#[tokio::test]
async fn rcfix_passthrough_platform_success_records_attempt() {
    let state = make_state(test_db().await).await;
    let up = spawn_stub_upstream(200, r#"{"ok":true}"#).await;
    setup_passthrough_platform_group(&state, "gkrcpa", &up).await;
    let resp = handle_proxy(AxumState(state.clone()), rc_chat_req("gkrcpa")).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let row = last_log_for(&state, "gkrcpa").await;
    assert_eq!(row.attempts.len(), 1, "passthrough must record its attempt");
    let a = &row.attempts[0];
    assert_eq!(a.platform_id, row.platform_id);
    assert_eq!(a.platform_name, "pt");
    assert_eq!(a.status_code, 200);
    assert!(a.error.is_empty());
    assert!(a.ts > 0 && a.ttfb_ms.is_some());
    assert_eq!(row.retry_count, 0);
}

/// 同上：上游 500 与连接失败也各记一条（500 附 `HTTP 500`，连接失败 status=0 + 错误文本）。
#[tokio::test]
async fn rcfix_passthrough_platform_failure_records_attempt() {
    let state = make_state(test_db().await).await;
    let up = spawn_stub_upstream(500, r#"{"e":1}"#).await;
    setup_passthrough_platform_group(&state, "gkrcpf", &up).await;
    let _ = handle_proxy(AxumState(state.clone()), rc_chat_req("gkrcpf")).await;
    let row = last_log_for(&state, "gkrcpf").await;
    assert_eq!(row.attempts.len(), 1);
    assert_eq!(row.attempts[0].status_code, 500);
    assert_eq!(row.attempts[0].error, "HTTP 500");

    let dead = spawn_reset_upstream().await;
    setup_passthrough_platform_group(&state, "gkrcpd", &dead).await;
    let _ = handle_proxy(AxumState(state.clone()), rc_chat_req("gkrcpd")).await;
    let row = last_log_for(&state, "gkrcpd").await;
    assert_eq!(row.attempts.len(), 1);
    assert_eq!(row.attempts[0].status_code, 0);
    assert!(!row.attempts[0].error.is_empty());
    assert!(row.attempts[0].ttfb_ms.is_none());
}

/// 非回环请求只回 403、不写 api_call_log（LAN 上的任何人都能刷表）；超长命令名入库截 64 字符。
#[tokio::test]
async fn mgmt_call_log_skips_non_loopback_and_truncates_command() {
    install_mgmt_test_router();
    let state = make_state(test_db().await).await;
    let req = axum::http::Request::builder()
        .method("POST")
        .uri("/api/rpc/ping")
        .body(axum::body::Body::empty())
        .unwrap();
    let r = super::handle_management(
        AxumState(state.clone()),
        axum::extract::ConnectInfo("10.1.2.3:5555".parse().unwrap()),
        req,
    )
    .await;
    assert_eq!(r.status(), 403);
    assert!(aidog_db::list_api_call_logs(&state.db, 10).await.unwrap().is_empty(), "非回环请求不得落行");

    let base = spawn_proxy_router(state.clone()).await;
    let long = "x".repeat(300);
    assert_eq!(mgmt_client().post(format!("{base}/api/rpc/{long}")).send().await.unwrap().status(), 404);
    let rows = aidog_db::list_api_call_logs(&state.db, 10).await.unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].command.chars().count(), 64);
}

/// 成员平台「最近一次结果」：失败写入的 last_error 必须被下一次成功清掉，
/// 即便路由缓存里的 platform 快照在失败写入前就已填充（旧 guard 读缓存快照 last_error 为空而跳过清除）。
#[tokio::test]
async fn success_clears_last_error_written_after_route_cache_filled() {
    let (up, _hits) = spawn_embedding_upstream(vec![(200, MB_OK), (200, MB_OK)]).await;
    let state = make_state(test_db().await).await;
    setup_aux_group(&state, "gklast", &[(&up, chain_models(&["m-a"]), Protocol::Anthropic, "p")]).await;
    // 第一次成功请求：路由缓存填入 last_error 为空的 platform 快照。
    assert_eq!(chain_send(&state, "gklast", CHAIN_REQ).await.status(), StatusCode::OK);
    let pid = aidog_db::list_platforms(&state.db).await.unwrap().remove(0).id;
    // 之后一次失败（别的路径）写入 last_error；缓存不失效。
    aidog_db::set_platform_last_error(&state.db, pid, Some("HTTP 500 boom".into())).await.unwrap();
    assert_eq!(chain_send(&state, "gklast", CHAIN_REQ).await.status(), StatusCode::OK);
    let p = aidog_db::get_platform(&state.db, pid).await.unwrap().unwrap();
    assert_eq!(p.last_error, "", "成功后 last_error 必须清空");
    assert_eq!(p.last_error_at, 0);
}

// ══ 中间件可还原 mask 测试公用件（本地假上游，禁外呼）══
// 各 mw 测试共持一把锁串行（共享设置缓存 / 全局状态）。

static PRIVACY_TEST_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

/// 测试邮箱原值。
const PRIVACY_EMAIL: &str = "bob@example.com";

/// 起一个按给定字节块分片下发 SSE 的假上游（跨 chunk 测试用，content-type: text/event-stream）。
async fn spawn_privacy_sse_upstream(chunks: Vec<Vec<u8>>) -> String {
    use axum::routing::any;
    let app = axum::Router::new().fallback(any(move || async move {
        let body = Body::from_stream(futures::stream::iter(
            chunks.into_iter().map(|c| Ok::<_, std::io::Error>(Bytes::from(c))),
        ));
        axum::response::Response::builder()
            .status(axum::http::StatusCode::OK)
            .header("content-type", "text/event-stream")
            .body(body)
            .unwrap()
    }));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.ok(); });
    format!("http://{addr}")
}

// ══ 中间件可还原 mask（票 01）：上游收占位符、客户端收原文、四侧日志无原文 ══

fn mw_mask_rule(
    id: i64,
    pattern: &str,
    priority: i64,
    mode: aidog_db::models::MaskMode,
    groups: &[&str],
) -> aidog_db::models::MiddlewareRule {
    use aidog_db::models::*;
    MiddlewareRule {
        id,
        name: format!("mask-{id}"),
        description: String::new(),
        conditions: ConditionNode::Leaf(ConditionLeaf {
            target: Target::RequestBody,
            field: String::new(),
            match_type: MatchType::Regex,
            pattern: pattern.into(),
            validator: String::new(),
        }),
        actions: vec![ActionStep { kind: ActionKind::Mask, params: ActionParams { mode, ..Default::default() } }],
        applies_to: AppliesTo { groups: groups.iter().map(|s| s.to_string()).collect(), ..Default::default() },
        priority,
        enabled: true,
        is_builtin: false,
        failed: false,
        created_at: 0,
        updated_at: 0,
    }
}

const MW_EMAIL_RE: &str = r"[a-z]+@[a-z]+\.com";

fn mw_placeholder() -> String {
    aidog_middleware::placeholder::placeholder_for("MASK", PRIVACY_EMAIL)
}

async fn log_all_sides(state: &Arc<ProxyState>) {
    set_log_settings(
        state,
        ProxyLogSettings { log_user_request: true, log_upstream_request: true, ..Default::default() },
    )
    .await;
}

#[tokio::test]
async fn mw_reversible_nonstream_roundtrip_and_four_side_logs() {
    use aidog_db::models::MaskMode;
    let _g = PRIVACY_TEST_LOCK.lock().await;
    let ph = mw_placeholder();
    let upstream_body: &'static str = Box::leak(
        format!(
            r#"{{"id":"msg_1","type":"message","role":"assistant","model":"claude-3","content":[{{"type":"text","text":"echo {ph}"}}],"stop_reason":"end_turn","usage":{{"input_tokens":5,"output_tokens":3}}}}"#
        )
        .into_boxed_str(),
    );
    let (up, hits) = spawn_embedding_upstream(vec![(200, upstream_body)]).await;
    let state = make_state(test_db().await).await;
    state.middleware.rebuild_from_rules(vec![mw_mask_rule(1, MW_EMAIL_RE, 0, MaskMode::Reversible, &[])]);
    setup_group_with_upstream(&state, "gkm1", &up).await;
    log_all_sides(&state).await;

    let req = format!(r#"{{"model":"claude-3","messages":[{{"role":"user","content":"mail {PRIVACY_EMAIL} hi"}}]}}"#);
    let resp = handle_proxy(AxumState(state.clone()), messages_request("gkm1", &req)).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let client = String::from_utf8_lossy(&axum::body::to_bytes(resp.into_body(), 1 << 20).await.unwrap()).to_string();
    assert!(client.contains(PRIVACY_EMAIL), "客户端应拿回原文: {client}");
    assert!(!client.contains(&ph), "客户端不应见占位符: {client}");

    let sent = hits.lock().unwrap()[0].1.clone();
    assert!(sent.contains(&ph), "上游应收占位符: {sent}");
    assert!(!sent.contains(PRIVACY_EMAIL), "原文禁出站: {sent}");

    let log = last_log_for(&state, "gkm1").await;
    for (side, body) in [
        ("request_body", &log.request_body),
        ("upstream_request_body", &log.upstream_request_body),
        ("response_body", &log.response_body),
        ("user_response_body", &log.user_response_body),
    ] {
        assert!(!body.contains(PRIVACY_EMAIL), "{side} 含原文: {body}");
        assert!(body.contains(&ph), "{side} 应为占位符形态: {body}");
    }
}

#[tokio::test]
async fn mw_reversible_stream_restores_across_chunks() {
    use aidog_db::models::MaskMode;
    let _g = PRIVACY_TEST_LOCK.lock().await;
    let ph = mw_placeholder();
    let sse = format!(
        "event: message_start\ndata: {{\"type\":\"message_start\",\"message\":{{\"usage\":{{\"input_tokens\":5,\"output_tokens\":0}}}}}}\n\nevent: content_block_delta\ndata: {{\"type\":\"content_block_delta\",\"index\":0,\"delta\":{{\"type\":\"text_delta\",\"text\":\"token {ph} done\"}}}}\n\nevent: message_stop\ndata: {{\"type\":\"message_stop\"}}\n\n"
    );
    let cut = sse.find(&ph).unwrap() + ph.len() / 2;
    let up = spawn_privacy_sse_upstream(vec![sse.as_bytes()[..cut].to_vec(), sse.as_bytes()[cut..].to_vec()]).await;
    let state = make_state(test_db().await).await;
    state.middleware.rebuild_from_rules(vec![mw_mask_rule(1, MW_EMAIL_RE, 0, MaskMode::Reversible, &[])]);
    setup_group_with_upstream(&state, "gkm2", &up).await;
    log_all_sides(&state).await;

    let req = format!(r#"{{"model":"claude-3","stream":true,"messages":[{{"role":"user","content":"mail {PRIVACY_EMAIL}"}}]}}"#);
    let resp = handle_proxy(AxumState(state.clone()), messages_request("gkm2", &req)).await;
    let client = String::from_utf8_lossy(&axum::body::to_bytes(resp.into_body(), 1 << 20).await.unwrap()).to_string();
    assert!(client.contains(PRIVACY_EMAIL), "流式客户端应拿原文: {client}");
    assert!(!client.contains(&ph), "流式客户端不应见占位符: {client}");
    let log = last_log_for(&state, "gkm2").await;
    assert!(!log.user_response_body.contains(PRIVACY_EMAIL), "流式用户响应日志含原文: {}", log.user_response_body);
    assert!(!log.request_body.contains(PRIVACY_EMAIL));
    assert!(!log.upstream_request_body.contains(PRIVACY_EMAIL));
}

/// 「全部内容」范围：规则 = 内置密钥正则（含通用长随机 token 分支）+ reversible + fields=all。
fn mw_all_secret_rule() -> aidog_db::models::MiddlewareRule {
    use aidog_db::models::*;
    let mut r = mw_mask_rule(1, aidog_db::BUILTIN_SECRET_PATTERN, 0, MaskMode::Reversible, &[]);
    r.actions[0].params.fields = vec!["all".into()];
    r
}

#[tokio::test]
async fn mw_all_scope_masks_tool_result_and_restores_to_client() {
    let _g = PRIVACY_TEST_LOCK.lock().await;
    let secret = "sk-proj-abcdefghijklmnop1234";
    let ph = aidog_middleware::placeholder::placeholder_for("MASK", secret);
    let upstream_body: &'static str = Box::leak(
        format!(
            r#"{{"id":"m","type":"message","role":"assistant","model":"claude-3","content":[{{"type":"text","text":"saw {ph}"}}],"stop_reason":"end_turn","usage":{{"input_tokens":5,"output_tokens":3}}}}"#
        )
        .into_boxed_str(),
    );
    let (up, hits) = spawn_embedding_upstream(vec![(200, upstream_body)]).await;
    let state = make_state(test_db().await).await;
    state.middleware.rebuild_from_rules(vec![mw_all_secret_rule()]);
    setup_group_with_upstream(&state, "gka1", &up).await;
    log_all_sides(&state).await;
    let req = format!(
        r#"{{"model":"claude-3","messages":[{{"role":"user","content":"run"}},{{"role":"assistant","content":[{{"type":"tool_use","id":"toolu_1","name":"cat","input":{{"path":"/x"}}}}]}},{{"role":"user","content":[{{"type":"tool_result","tool_use_id":"toolu_1","content":"file has {secret} inside"}}]}}]}}"#
    );
    let resp = handle_proxy(AxumState(state.clone()), messages_request("gka1", &req)).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let client = String::from_utf8_lossy(&axum::body::to_bytes(resp.into_body(), 1 << 20).await.unwrap()).to_string();
    let sent = hits.lock().unwrap()[0].1.clone();
    assert!(sent.contains(&ph) && !sent.contains(secret), "tool_result 内密钥应以占位符出站: {sent}");
    assert!(client.contains(secret) && !client.contains(&ph), "客户端应拿回原文: {client}");
    let log = last_log_for(&state, "gka1").await;
    assert!(!log.upstream_request_body.contains(secret), "{}", log.upstream_request_body);
    assert!(!log.request_body.contains(secret), "{}", log.request_body);
}

#[tokio::test]
async fn mw_all_scope_skip_list_reaches_upstream_unchanged() {
    let _g = PRIVACY_TEST_LOCK.lock().await;
    // 每个值都能被密钥通用长随机 token 分支命中（字母数字前缀 + `_`/`-` + 15+ 含数字），
    // 若不在跳过名单里就会被替换。
    let model = "claude-opus-4-20250514-extended";
    let sig = "sig_0123456789abcdefghijklmnopqr";
    let tid = "toolu_01A09q90qiusdhfusdhf12";
    let b64 = "iVBOR_w0KGgoAAAANSUhEUgAAAAEAAAAB01234567";
    // redacted_thinking / image_url / input_audio / call_id 等形态由 test_middleware_body 的遍历单测覆盖
    // （anthropic 转换层会丢弃这些块，到不了上游）。
    let re = regex::Regex::new(aidog_db::BUILTIN_SECRET_PATTERN).unwrap();
    for v in [model, sig, tid, b64] {
        assert!(re.is_match(v), "夹具值必须会被密钥正则命中: {v}");
    }
    let secret = "sk-proj-abcdefghijklmnop1234";
    let (up, hits) = spawn_embedding_upstream(vec![(200, ANTHROPIC_OK)]).await;
    let state = make_state(test_db().await).await;
    state.middleware.rebuild_from_rules(vec![mw_all_secret_rule()]);
    setup_group_with_upstream(&state, "gka2", &up).await;
    let req = serde_json::json!({
        "model": model,
        "messages": [
            {"role": "user", "content": [
                {"type": "text", "text": format!("leak {secret}")},
                {"type": "image", "source": {"type": "base64", "media_type": "image/png", "data": b64}}
            ]},
            {"role": "assistant", "content": [
                {"type": "thinking", "thinking": "t", "signature": sig},
                {"type": "tool_use", "id": tid, "name": "cat", "input": {}}
            ]},
            {"role": "user", "content": [{"type": "tool_result", "tool_use_id": tid, "content": "ok"}]}
        ]
    })
    .to_string();
    let resp = handle_proxy(AxumState(state.clone()), messages_request("gka2", &req)).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let sent: serde_json::Value = serde_json::from_str(&hits.lock().unwrap()[0].1).unwrap();
    let c0 = &sent["messages"][0]["content"];
    assert!(!c0[0]["text"].as_str().unwrap().contains(secret), "对照：普通文本里的密钥必须被替换: {c0}");
    assert_eq!(sent["model"], model);
    assert_eq!(c0[1]["source"]["data"], b64);
    let c1 = &sent["messages"][1]["content"];
    assert_eq!(c1[0]["signature"], sig);
    assert_eq!(c1[1]["id"], tid);
    assert_eq!(sent["messages"][2]["content"][0]["tool_use_id"], tid);
}

#[tokio::test]
async fn mw_fixed_mask_behaviour_unchanged() {
    use aidog_db::models::MaskMode;
    let _g = PRIVACY_TEST_LOCK.lock().await;
    let (up, hits) = spawn_embedding_upstream(vec![(200, ANTHROPIC_OK)]).await;
    let state = make_state(test_db().await).await;
    state.middleware.rebuild_from_rules(vec![mw_mask_rule(1, MW_EMAIL_RE, 0, MaskMode::Fixed, &[])]);
    setup_group_with_upstream(&state, "gkm3", &up).await;
    let req = format!(r#"{{"model":"claude-3","messages":[{{"role":"user","content":"mail {PRIVACY_EMAIL}"}}]}}"#);
    let resp = handle_proxy(AxumState(state.clone()), messages_request("gkm3", &req)).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let sent = hits.lock().unwrap()[0].1.clone();
    assert!(sent.contains("****") && !sent.contains("{{PII_") && !sent.contains(PRIVACY_EMAIL), "{sent}");
}

#[tokio::test]
async fn mw_two_reversible_rules_second_leaves_placeholder_alone() {
    use aidog_db::models::MaskMode;
    let _g = PRIVACY_TEST_LOCK.lock().await;
    let ph = mw_placeholder();
    let upstream_body: &'static str = Box::leak(
        format!(
            r#"{{"id":"m","type":"message","role":"assistant","model":"claude-3","content":[{{"type":"text","text":"echo {ph}"}}],"stop_reason":"end_turn","usage":{{"input_tokens":5,"output_tokens":3}}}}"#
        )
        .into_boxed_str(),
    );
    let (up, hits) = spawn_embedding_upstream(vec![(200, upstream_body)]).await;
    let state = make_state(test_db().await).await;
    state.middleware.rebuild_from_rules(vec![
        mw_mask_rule(1, MW_EMAIL_RE, 1, MaskMode::Reversible, &[]),
        mw_mask_rule(2, "PII|MASK|example", 2, MaskMode::Reversible, &[]),
    ]);
    setup_group_with_upstream(&state, "gkm4", &up).await;
    let req = format!(r#"{{"model":"claude-3","messages":[{{"role":"user","content":"mail {PRIVACY_EMAIL}"}}]}}"#);
    let resp = handle_proxy(AxumState(state.clone()), messages_request("gkm4", &req)).await;
    let client = String::from_utf8_lossy(&axum::body::to_bytes(resp.into_body(), 1 << 20).await.unwrap()).to_string();
    assert!(hits.lock().unwrap()[0].1.contains(&ph), "上游应见第一条的占位符");
    assert!(client.contains(PRIVACY_EMAIL) && !client.contains("{{PII_"), "{client}");
}

#[tokio::test]
async fn mw_reversible_failure_is_503_zero_upstream_and_scoped() {
    use aidog_db::models::MaskMode;
    let _g = PRIVACY_TEST_LOCK.lock().await;
    let (up, hits) = spawn_embedding_upstream(vec![(200, ANTHROPIC_OK)]).await;
    let state = make_state(test_db().await).await;
    state.middleware.rebuild_from_rules(vec![mw_mask_rule(1, MW_EMAIL_RE, 0, MaskMode::Reversible, &["gkm5"])]);
    setup_group_with_upstream(&state, "gkm5", &up).await;
    setup_group_with_upstream(&state, "gkm5-other", &up).await;
    let req = format!(r#"{{"model":"claude-3","messages":[{{"role":"user","content":"mail {PRIVACY_EMAIL}"}}]}}"#);

    aidog_middleware::set_reversible_force_fail(true);
    let resp = handle_proxy(AxumState(state.clone()), messages_request("gkm5", &req)).await;
    let other = handle_proxy(AxumState(state.clone()), messages_request("gkm5-other", &req)).await;
    aidog_middleware::set_reversible_force_fail(false);

    assert_eq!(resp.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(hits.lock().unwrap().len(), 1, "仅作用域外那次到达上游");
    assert_eq!(other.status(), StatusCode::OK, "作用域外请求不受影响");
    let full = last_log_for(&state, "gkm5").await;
    assert_eq!(full.status_code, 503);
    assert_eq!(full.blocked_by, "router");
    assert_eq!(full.blocked_reason, "privacy_scrub_unavailable");
}

// ── Claude Code 网关提示头（request-class / agent-type）落 proxy_log（本地假上游，无真实外呼）──

fn messages_request_hints(gk: &str, hints: &[(&str, &str)]) -> Request {
    let mut b = HttpRequest::builder()
        .method("POST")
        .uri("/v1/messages")
        .header("authorization", format!("Bearer {gk}"))
        .header("content-type", "application/json");
    for (k, v) in hints {
        b = b.header(*k, *v);
    }
    b.body(Body::from(
        r#"{"model":"claude-3","max_tokens":10,"messages":[{"role":"user","content":"hi"}]}"#,
    ))
    .unwrap()
}

#[tokio::test]
async fn claude_hint_headers_recorded_in_proxy_log() {
    let state = make_state(test_db().await).await;
    let up = spawn_stub_upstream(200, ANTHROPIC_OK).await;
    setup_passthrough_group(&state, "gkhint", &up).await;

    // 头名大小写不敏感（HeaderMap 归一小写；此处混写），值 trim。
    let resp = handle_proxy(
        AxumState(state.clone()),
        messages_request_hints(
            "gkhint",
            &[("X-Claude-Code-Request-Class", " subagent "), ("x-claude-code-agent-type", "Explore")],
        ),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let log = last_log_for(&state, "gkhint").await;
    assert_eq!((log.request_class.as_str(), log.agent_type.as_str()), ("subagent", "Explore"));

    // 仅 request-class；未知值原样存。
    handle_proxy(
        AxumState(state.clone()),
        messages_request_hints("gkhint", &[("x-claude-code-request-class", "brand_new")]),
    )
    .await;
    let log = last_log_for(&state, "gkhint").await;
    assert_eq!((log.request_class.as_str(), log.agent_type.as_str()), ("brand_new", ""));

    // 无头 → 空串。
    handle_proxy(AxumState(state.clone()), messages_request_hints("gkhint", &[])).await;
    let log = last_log_for(&state, "gkhint").await;
    assert_eq!((log.request_class.as_str(), log.agent_type.as_str()), ("", ""));
}

#[tokio::test]
async fn claude_hint_ids_recorded_in_proxy_log() {
    let state = make_state(test_db().await).await;
    let up = spawn_stub_upstream(200, ANTHROPIC_OK).await;
    setup_passthrough_group(&state, "gkhint2", &up).await;

    let resp = handle_proxy(
        AxumState(state.clone()),
        messages_request_hints(
            "gkhint2",
            &[
                ("x-claude-code-prompt-id", "11111111-2222-3333-4444-555555555555"),
                ("x-claude-code-session-id", "sess-9"),
                ("x-claude-code-agent-id", "ag-2"),
                ("x-claude-code-parent-agent-id", "ag-1"),
                ("x-claude-code-compaction", "manual"),
                ("x-claude-code-context-compacted", "auto"),
                ("x-claude-code-prev-tool-durations", "Bash=742;Read=9"),
            ],
        ),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let log = last_log_for(&state, "gkhint2").await;
    assert_eq!(log.prompt_id, "11111111-2222-3333-4444-555555555555");
    assert_eq!((log.session_id.as_str(), log.agent_id.as_str(), log.parent_agent_id.as_str()), ("sess-9", "ag-2", "ag-1"));
    assert_eq!((log.compaction.as_str(), log.context_compacted.as_str()), ("manual", "auto"));
    assert_eq!(log.prev_tool_durations, "Bash=742;Read=9");

    // 无头 → 全空串。
    handle_proxy(AxumState(state.clone()), messages_request_hints("gkhint2", &[])).await;
    let log = last_log_for(&state, "gkhint2").await;
    assert_eq!(
        [&log.prompt_id, &log.session_id, &log.agent_id, &log.parent_agent_id, &log.compaction, &log.context_compacted, &log.prev_tool_durations]
            .iter().filter(|s| !s.is_empty()).count(),
        0
    );
}

// ═══════════════ X-Aidog-Group 显式选组头 ═══════════════

/// 两个 Anthropic 分组各配一个记录头的假上游；返回 (A 上游记录, B 上游记录)。
async fn setup_two_groups_recording(state: &Arc<ProxyState>) -> (SeenHeaders, SeenHeaders) {
    let (up_a, seen_a) = spawn_header_recording_upstream(ANTHROPIC_OK).await;
    let (up_b, seen_b) = spawn_header_recording_upstream(ANTHROPIC_OK).await;
    setup_group_with_upstream(state, "hga", &up_a).await;
    setup_group_with_upstream(state, "hgb", &up_b).await;
    (seen_a, seen_b)
}

/// 头指向已存在分组 → 该组胜过 token 定的组；头不转发上游；值 trim。
#[tokio::test]
async fn group_header_overrides_token_and_is_not_forwarded() {
    let state = make_state(test_db().await).await;
    let (seen_a, seen_b) = setup_two_groups_recording(&state).await;
    let resp = handle_proxy(
        AxumState(state.clone()),
        request_with("/v1/messages", &[("authorization", "Bearer hga"), ("x-aidog-group", "  hgb  ")]),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(seen_a.lock().unwrap().len(), 0, "token 组不应被命中");
    {
        let seen = seen_b.lock().unwrap();
        assert_eq!(seen.len(), 1, "头指向的组应被命中");
        assert!(!seen[0].1.contains_key("x-aidog-group"), "头不得转发上游: {:?}", seen[0].1);
    }
    assert_eq!(last_log_for(&state, "hgb").await.status_code, 200);
}

/// 头指向不存在的分组 → 同未知 token：404，不回落 token 的组，上游零命中。
#[tokio::test]
async fn group_header_unknown_group_404_without_token_fallback() {
    let state = make_state(test_db().await).await;
    let (seen_a, seen_b) = setup_two_groups_recording(&state).await;
    let resp = handle_proxy(
        AxumState(state.clone()),
        request_with("/v1/messages", &[("authorization", "Bearer hga"), ("x-aidog-group", "nope")]),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    assert_eq!(seen_a.lock().unwrap().len() + seen_b.lock().unwrap().len(), 0);
}

/// 头缺席 / 空 / 全空白 → 行为不变，仍按 token 定组。
#[tokio::test]
async fn group_header_absent_or_blank_keeps_token_behaviour() {
    let state = make_state(test_db().await).await;
    let (seen_a, seen_b) = setup_two_groups_recording(&state).await;
    for hdrs in [
        vec![("authorization", "Bearer hga")],
        vec![("authorization", "Bearer hga"), ("x-aidog-group", "")],
        vec![("authorization", "Bearer hga"), ("x-aidog-group", "   ")],
    ] {
        let resp = handle_proxy(AxumState(state.clone()), request_with("/v1/messages", &hdrs)).await;
        assert_eq!(resp.status(), StatusCode::OK, "{hdrs:?}");
    }
    assert_eq!(seen_a.lock().unwrap().len(), 3);
    assert_eq!(seen_b.lock().unwrap().len(), 0);
    // 空头不转发
    assert!(seen_a.lock().unwrap().iter().all(|(_, h)| !h.contains_key("x-aidog-group")));
}

/// 无任何 token，只带头也能定组（订阅 OAuth Bearer 场景：Authorization 是别的东西）。
#[tokio::test]
async fn group_header_alone_selects_group_with_foreign_bearer() {
    let state = make_state(test_db().await).await;
    let (_seen_a, seen_b) = setup_two_groups_recording(&state).await;
    let resp = handle_proxy(
        AxumState(state.clone()),
        request_with("/v1/messages", &[("authorization", "Bearer sk-oauth-xyz"), ("x-aidog-group", "hgb")]),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(seen_b.lock().unwrap().len(), 1);
}

/// 透传平台：头选组后不转发该头；Authorization 里的非分组 key 被剥，换成平台 key。
#[tokio::test]
async fn group_header_passthrough_strips_header_and_foreign_auth() {
    let (upstream, seen) = spawn_header_recording_upstream(r#"{"ok":true}"#).await;
    let state = make_state(test_db().await).await;
    setup_passthrough_platform_group(&state, "hgpt", &upstream).await;
    let resp = handle_proxy(
        AxumState(state.clone()),
        request_with("/v1/x", &[("authorization", "Bearer sk-client-own"), ("x-aidog-group", "hgpt")]),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let h = seen.lock().unwrap().last().cloned().expect("upstream not hit").1;
    assert!(!h.contains_key("x-aidog-group"), "{h:?}");
    assert_ne!(h.get("authorization").map(String::as_str), Some("Bearer sk-client-own"), "{h:?}");
}

/// /v1/models：头指向的组决定清单来源（透传组 → 原样转发上游），头不转发。
#[tokio::test]
async fn group_header_selects_group_for_models_list() {
    let (upstream, seen) = spawn_header_recording_upstream(r#"{"data":[]}"#).await;
    let state = make_state(test_db().await).await;
    setup_passthrough_platform_group(&state, "hgm", &upstream).await;
    let req = HttpRequest::builder()
        .method("GET")
        .uri("/v1/models")
        .header("x-aidog-group", "hgm")
        .body(Body::empty())
        .unwrap();
    let resp = handle_proxy(AxumState(state.clone()), req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let seen = seen.lock().unwrap();
    assert_eq!(seen.len(), 1, "透传组应转发上游");
    assert!(!seen[0].1.contains_key("x-aidog-group"));
}

/// WS 握手同一套：头指向未知组 → 404，不回落 token。
#[tokio::test]
async fn group_header_unknown_group_ws_404() {
    let state = make_state(test_db().await).await;
    let (_a, _b) = setup_two_groups_recording(&state).await;
    let req = HttpRequest::builder()
        .method("GET")
        .uri("/v1/responses")
        .header("authorization", "Bearer hga")
        .header("x-aidog-group", "nope")
        .header("upgrade", "websocket")
        .header("connection", "Upgrade")
        .body(Body::empty())
        .unwrap();
    let resp = handle_proxy(AxumState(state.clone()), req).await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

// ── anthropic-ratelimit-unified-*：响应头 → 平台卡片读的 est_coding_plan（本地假上游）──

async fn spawn_unified_upstream(status: u16, ctype: &'static str, body: &'static str) -> String {
    use axum::routing::any;
    let app = axum::Router::new().fallback(any(move || async move {
        (
            axum::http::StatusCode::from_u16(status).unwrap(),
            [
                ("content-type", ctype),
                ("anthropic-ratelimit-unified-status", "allowed"),
                ("anthropic-ratelimit-unified-5h-utilization", "0.42"),
                ("anthropic-ratelimit-unified-5h-reset", "4102444800"),
                ("anthropic-ratelimit-unified-7d-utilization", "0.07"),
                ("anthropic-ratelimit-unified-7d-reset", "4103000000"),
            ],
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

/// 写库在后台 spawn：轮询至 est_coding_plan 出现 tier（上限 ~3s）。
async fn wait_unified_plan(state: &Arc<ProxyState>) -> crate::gateway::estimate::EstCodingPlan {
    for _ in 0..60 {
        let plats = aidog_db::list_platforms(&state.db).await.unwrap();
        let plan = crate::gateway::estimate::EstCodingPlan::from_json(&plats[0].est_coding_plan);
        if !plan.tiers.is_empty() {
            return plan;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    crate::gateway::estimate::EstCodingPlan::default()
}

fn assert_unified_plan(plan: &crate::gateway::estimate::EstCodingPlan) {
    let five = plan.tiers.iter().find(|t| t.name == "five_hour").expect("five_hour tier");
    assert!((five.est_utilization - 42.0).abs() < 1e-9, "got {}", five.est_utilization);
    let wk = plan.tiers.iter().find(|t| t.name == "weekly_limit").expect("weekly_limit tier");
    assert!((wk.est_utilization - 7.0).abs() < 1e-9);
}

#[tokio::test]
async fn unified_headers_nonstream_200_update_subscription_usage() {
    let upstream = spawn_unified_upstream(200, "application/json", ANTHROPIC_OK).await;
    let state = make_state(test_db().await).await;
    setup_group_with_upstream(&state, "gku1", &upstream).await;
    let req = messages_request("gku1", r#"{"model":"claude-3","messages":[{"role":"user","content":"hi"}]}"#);
    let resp = handle_proxy(AxumState(state.clone()), req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert_unified_plan(&wait_unified_plan(&state).await);
}

#[tokio::test]
async fn unified_headers_stream_200_update_subscription_usage() {
    let sse = "event: message_start\ndata: {\"type\":\"message_start\",\"message\":{\"usage\":{\"input_tokens\":5,\"output_tokens\":0}}}\n\nevent: message_stop\ndata: {\"type\":\"message_stop\"}\n\n";
    let upstream = spawn_unified_upstream(200, "text/event-stream", sse).await;
    let state = make_state(test_db().await).await;
    setup_group_with_upstream(&state, "gku2", &upstream).await;
    let req = messages_request("gku2", r#"{"model":"claude-3","stream":true,"messages":[{"role":"user","content":"hi"}]}"#);
    let resp = handle_proxy(AxumState(state.clone()), req).await;
    let _ = axum::body::to_bytes(resp.into_body(), 1 << 20).await.unwrap();
    assert_unified_plan(&wait_unified_plan(&state).await);
}

#[tokio::test]
async fn unified_headers_on_429_update_subscription_usage() {
    let upstream = spawn_unified_upstream(429, "application/json", r#"{"error":"rate limited"}"#).await;
    let state = make_state(test_db().await).await;
    setup_group_with_upstream(&state, "gku3", &upstream).await;
    let req = messages_request("gku3", r#"{"model":"claude-3","messages":[{"role":"user","content":"hi"}]}"#);
    let _ = handle_proxy(AxumState(state.clone()), req).await;
    assert_unified_plan(&wait_unified_plan(&state).await);
}

// ── 工具按需加载模拟（ticket 05）：非官方上游出站 body ──

const TS_REQ: &str = r#"{"model":"claude-3","max_tokens":8,"tools":[{"name":"ToolSearch","description":"d","input_schema":{"type":"object","properties":{}}},{"name":"mcp__a__loaded","description":"d","input_schema":{"type":"object","properties":{}},"defer_loading":true},{"name":"mcp__a__idle","description":"d","input_schema":{"type":"object","properties":{}},"defer_loading":true}],"messages":[{"role":"user","content":"go"},{"role":"assistant","content":[{"type":"tool_use","id":"t1","name":"ToolSearch","input":{"query":"x"}}]},{"role":"user","content":[{"type":"tool_result","tool_use_id":"t1","content":[{"type":"tool_reference","tool_name":"mcp__a__loaded"}]}]}]}"#;
const OPENAI_OK_MIN: &str = r#"{"id":"c1","object":"chat.completion","model":"m","choices":[{"index":0,"message":{"role":"assistant","content":"hi"},"finish_reason":"stop"}],"usage":{"prompt_tokens":1,"completion_tokens":1,"total_tokens":2}}"#;

/// 第三方 Anthropic 协议平台（同协议透传）：未点名延迟工具不下发，已点名去 defer_loading，
/// tool_reference 换文字；非流式与流式请求侧一致。
#[tokio::test]
async fn tool_search_emulation_anthropic_third_party_passthrough() {
    let (upstream, rec) = spawn_recording_upstream(ANTHROPIC_OK).await;
    let state = make_state(test_db().await).await;
    setup_group_with_upstream(&state, "gkts1", &upstream).await;
    for body in [TS_REQ.to_string(), TS_REQ.replacen(r#""max_tokens":8"#, r#""max_tokens":8,"stream":true"#, 1)] {
        let _ = handle_proxy(AxumState(state.clone()), messages_request("gkts1", &body)).await;
    }
    let captured = rec.lock().unwrap().clone();
    assert_eq!(captured.len(), 2);
    for (_, b) in &captured {
        let sent: serde_json::Value = serde_json::from_str(b).unwrap();
        let names: Vec<&str> = sent["tools"].as_array().unwrap().iter().map(|t| t["name"].as_str().unwrap()).collect();
        assert_eq!(names, ["ToolSearch", "mcp__a__loaded"], "{b}");
        assert!(!b.contains("defer_loading") && !b.contains("tool_reference"), "{b}");
        assert!(b.contains("已加载工具 mcp__a__loaded"), "{b}");
    }
}

/// OpenAI 系平台（转换分支）：同样的剔除/降级在协议转换之前生效。
#[tokio::test]
async fn tool_search_emulation_openai_convert() {
    let (upstream, rec) = spawn_recording_upstream(OPENAI_OK_MIN).await;
    let state = make_state(test_db().await).await;
    setup_group_with_platform(&state, "gkts2", Protocol::OpenAI, &upstream, "").await;
    let _ = handle_proxy(AxumState(state.clone()), messages_request("gkts2", TS_REQ)).await;
    let captured = rec.lock().unwrap().clone();
    assert_eq!(captured.len(), 1);
    let b = &captured[0].1;
    assert!(b.contains("mcp__a__loaded") && !b.contains("mcp__a__idle"), "{b}");
    assert!(!b.contains("defer_loading") && !b.contains("tool_reference"), "{b}");
    assert!(b.contains("已加载工具 mcp__a__loaded"), "{b}");
}

// ══ 时段档 embedding / rerank：命中整套替换，整组都不接 → 503（本地假上游）══

/// 给名为 `name` 的平台写入全天命中的时段档（models 整套替换）。
async fn set_all_day_rule(state: &Arc<ProxyState>, name: &str, models: serde_json::Value) {
    let id = aidog_db::list_platforms(&state.db).await.unwrap().into_iter().find(|p| p.name == name).unwrap().id;
    let extra = serde_json::json!({"time_windows":[{"windows":[{"start_hour":0,"end_hour":24}],"models":models}]}).to_string();
    aidog_db::update_platform(&state.db, aidog_db::models::UpdatePlatform {
        id, name: None, platform_type: None, base_url: None, api_key: None, extra: Some(extra),
        models: None, available_models: None, endpoints: None,
        enabled: None, status: None, manual_budgets: None,
        join_group_ids: None, expires_at: None, quota_source: None,
    })
    .await
    .unwrap();
    state.db.invalidate_group_details_cache();
}

/// 时段内整组档里都没有 embedding / rerank → 503 + 对应 blocked_reason，且不打上游；
/// 档里有值的平台照常用档值发往上游。
#[tokio::test]
async fn time_rule_without_embedding_or_rerank_503_and_with_uses_rule() {
    let (db, _dir) = aidog_db::test_support::three_file_db().await;
    let state = make_state(db).await;
    let (up, hits) = spawn_embedding_upstream(vec![(200, EMB_RESPONSE)]).await;
    let both = PlatformModels { embedding: vec!["emb-day".into()], rerank: vec!["rr-day".into()], ..Default::default() };
    setup_aux_group(&state, "gkt1", &[(&up, both.clone(), Protocol::OpenAI, "a")]).await;
    setup_aux_group(&state, "gkt2", &[(&up, both.clone(), Protocol::OpenAI, "a")]).await;
    set_all_day_rule(&state, "gkt1-a", serde_json::json!({"default":["gpt"]})).await;
    for (path, req, reason) in [
        ("/v1/embeddings", EMB_REQ, "no_embedding_platform"),
        ("/v1/rerank", RR_REQ, "no_rerank_platform"),
    ] {
        let resp = handle_proxy(AxumState(state.clone()), embedding_request("gkt1", req, path)).await;
        assert_eq!(resp.status(), StatusCode::SERVICE_UNAVAILABLE, "{path}");
        let log = last_log_for(&state, "gkt1").await;
        assert_eq!((log.blocked_by.as_str(), log.blocked_reason.as_str()), ("router", reason), "{path}");
    }
    assert!(hits.lock().unwrap().is_empty(), "时段内被排除，不得打到上游");

    set_all_day_rule(&state, "gkt2-a", serde_json::json!({"embedding":["emb-night"]})).await;
    let resp = handle_proxy(AxumState(state.clone()), embedding_request("gkt2", EMB_REQ, "/v1/embeddings")).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let sent = hits.lock().unwrap();
    assert_eq!(sent.len(), 1);
    assert!(sent[0].1.contains("emb-night") && !sent[0].1.contains("emb-day"), "发往上游的是档值: {}", sent[0].1);
}

// ══ 请求尝试-逐次重算 票 01：最大尝试次数 = 一次请求的上游调用总数（换平台 + 同平台换模型都计）══

async fn set_group_max_attempts(state: &Arc<ProxyState>, gk: &str, n: u32) {
    let g = aidog_db::list_groups(&state.db).await.unwrap().into_iter().find(|g| g.group_key == gk).unwrap();
    aidog_db::update_group(&state.db, aidog_db::models::UpdateGroup {
        id: g.id, name: None, routing_mode: None, request_timeout_secs: 0, connect_timeout_secs: 0,
        source_protocol: None, max_retries: Some(n), model_mappings: g.model_mappings.clone(),
        env_vars: g.env_vars.clone(), is_default: None,
    })
    .await
    .unwrap();
    state.db.invalidate_group_details_cache();
}

/// 上限 3，首平台模型链 3 个模型全部模型类失败 → 共 3 次调用后停止：次平台不被调用，尝试记录 3 条。
#[tokio::test]
async fn ra01_chat_cap_counts_model_switches() {
    let (up1, hits1) = spawn_embedding_upstream(vec![(404, E404_MODEL); 6]).await;
    let (up2, hits2) = spawn_embedding_upstream(vec![(200, MB_OK)]).await;
    let state = make_state(test_db().await).await;
    setup_aux_group(
        &state,
        "gkra1",
        &[
            (&up1, chain_models(&["m-a", "m-b", "m-c"]), Protocol::Anthropic, "p1"),
            (&up2, chain_models(&["m-d"]), Protocol::Anthropic, "p2"),
        ],
    )
    .await;
    set_group_max_attempts(&state, "gkra1", 3).await;
    let resp = chain_send(&state, "gkra1", CHAIN_REQ).await;
    assert_ne!(resp.status(), StatusCode::OK);
    assert_eq!(hit_models(&hits1), ["m-a", "m-b", "m-c"]);
    assert!(hits2.lock().unwrap().is_empty(), "总次数用尽，次平台不得被调用");
    assert_eq!(last_log_for(&state, "gkra1").await.attempts.len(), 3);
}

/// 上限 3：首平台链 2 个模型失败（2 次）+ 次平台第 1 次失败（第 3 次）→ 第三个平台不被调用。
#[tokio::test]
async fn ra01_chat_cap_counts_across_platforms() {
    let (up1, hits1) = spawn_embedding_upstream(vec![(404, E404_MODEL); 6]).await;
    let (up2, hits2) = spawn_embedding_upstream(vec![(500, E503_PLAIN); 6]).await;
    let (up3, hits3) = spawn_embedding_upstream(vec![(200, MB_OK)]).await;
    let state = make_state(test_db().await).await;
    setup_aux_group(
        &state,
        "gkra2",
        &[
            (&up1, chain_models(&["m-a", "m-b"]), Protocol::Anthropic, "p1"),
            (&up2, chain_models(&["m-c", "m-d"]), Protocol::Anthropic, "p2"),
            (&up3, chain_models(&["m-e"]), Protocol::Anthropic, "p3"),
        ],
    )
    .await;
    set_group_max_attempts(&state, "gkra2", 3).await;
    let resp = chain_send(&state, "gkra2", CHAIN_REQ).await;
    assert_ne!(resp.status(), StatusCode::OK);
    assert_eq!(hits1.lock().unwrap().len() + hits2.lock().unwrap().len(), 3);
    assert!(hits3.lock().unwrap().is_empty(), "第 4 次调用不得发生");
    assert_eq!(last_log_for(&state, "gkra2").await.attempts.len(), 3);
}

/// 向量化 / 重排序：上限 1 → 首平台失败后不换平台。
#[tokio::test]
async fn ra01_aux_cap_stops_platform_switch() {
    let (up1, hits1) = spawn_embedding_upstream(vec![(529, r#"{"error":"overloaded"}"#)]).await;
    let (up2, hits2) = spawn_embedding_upstream(vec![(200, EMB_RESPONSE)]).await;
    let state = make_state(test_db().await).await;
    setup_embedding_group(
        &state,
        "gkra3",
        &[(&up1, Some("emb-1"), Protocol::OpenAI, "p1"), (&up2, Some("emb-2"), Protocol::OpenAI, "p2")],
    )
    .await;
    set_group_max_attempts(&state, "gkra3", 1).await;
    let resp = handle_proxy(AxumState(state.clone()), embedding_request("gkra3", EMB_REQ, "/v1/embeddings")).await;
    assert_ne!(resp.status(), StatusCode::OK);
    assert_eq!(hits1.lock().unwrap().len(), 1);
    assert!(hits2.lock().unwrap().is_empty(), "上限 1：不得换平台");
}

/// 向量化同平台换模型也计数：上限 2，链 3 个模型全 404 → 2 次调用。
#[tokio::test]
async fn ra01_aux_cap_counts_model_switches() {
    let (up, hits) = spawn_embedding_upstream(vec![(404, E404_MODEL); 6]).await;
    let state = make_state(test_db().await).await;
    let slot = PlatformModels { embedding: vec!["e-a".into(), "e-b".into(), "e-c".into()], ..Default::default() };
    setup_aux_group(&state, "gkra3b", &[(&up, slot, Protocol::OpenAI, "p")]).await;
    set_group_max_attempts(&state, "gkra3b", 2).await;
    let resp = handle_proxy(AxumState(state.clone()), embedding_request("gkra3b", EMB_REQ, "/v1/embeddings")).await;
    assert_ne!(resp.status(), StatusCode::OK);
    assert_eq!(hits.lock().unwrap().len(), 2);
}

/// 决策请求：上限 2，jev 链 3 个模型全 404 → 2 次调用。
#[tokio::test]
async fn ra01_decision_cap_counts_model_switches() {
    let (up, hits) = spawn_decision_upstream(vec![(404, E404_MODEL), (404, E404_MODEL), (404, E404_MODEL), (200, OR_RESPONSE)]).await;
    let state = make_state(test_db().await).await;
    let jev = PlatformModels { jev: vec!["j-a".into(), "j-b".into(), "j-c".into()], ..Default::default() };
    setup_decision_group(&state, "gkra4", &up, jev, None, Protocol::TypeSafe).await;
    set_group_max_attempts(&state, "gkra4", 2).await;
    let resp = handle_proxy(AxumState(state.clone()), decision_request("gkra4", "whatever")).await;
    assert_ne!(resp.status(), StatusCode::OK);
    assert_eq!(hits.lock().unwrap().len(), 2);
}
