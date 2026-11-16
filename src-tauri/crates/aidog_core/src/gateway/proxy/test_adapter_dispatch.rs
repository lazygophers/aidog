//! 端点来源回归护栏（issue 68 验收⑤）：出站永远按**用户平台配置**的端点
//! (protocol, base_url) 出站，registry 内置端点不参与运行时。
//! 端点读取是既有行为（运行时只读 DB platform.endpoints），本模块是回归护栏——
//! 旧码已绿，不构成新行为先红证据（测试纪律按 parallel-implement.md §22 分类）。

use super::*;
use aidog_middleware::MiddlewareEngine;
use crate::gateway::models::{CreatePlatform, GroupPlatformInput, PlatformEndpoint, Protocol};
use aidog_db::test_support::test_db;
use axum::body::Body;
use axum::http::Request as HttpRequest;
use aidog_db::models::UpdatePlatform;
use std::sync::{Arc, Mutex};

/// 起一个记录 (path, body) 的本地假上游，返回 (base_url, 记录句柄)。
async fn spawn_recording_upstream(resp_body: &'static str) -> (String, Arc<Mutex<Vec<(String, String)>>>) {
    let hits: Arc<Mutex<Vec<(String, String)>>> = Arc::new(Mutex::new(Vec::new()));
    let sink = hits.clone();
    let resp_body = resp_body.to_string();
    let app = axum::Router::new().fallback(|uri: axum::http::Uri, body: String| async move {
        sink.lock().unwrap().push((uri.path().to_string(), body));
        (
            axum::http::StatusCode::OK,
            [("content-type", "application/json")],
            resp_body,
        )
    });
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.ok();
    });
    (format!("http://{addr}"), hits)
}

/// 建一个带用户端点的平台并挂进分组。
async fn setup_user_endpoint_group(
    state: &Arc<ProxyState>,
    gk: &str,
    ep_protocol: Protocol,
    base_url: &str,
) {
    let plat = aidog_db::create_platform(
        &state.db,
        CreatePlatform {
            name: "stub".into(),
            platform_type: Protocol::OpenAI,
            base_url: base_url.to_string(),
            api_key: "sk-up".into(),
            extra: String::new(),
            models: None,
            available_models: None,
            endpoints: Some(vec![PlatformEndpoint {
                protocol: ep_protocol,
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

const OPENAI_OK: &str =
    r#"{"id":"chatcmpl_1","object":"chat.completion","model":"gpt-x","choices":[{"index":0,"message":{"role":"assistant","content":"hi"},"finish_reason":"stop"}],"usage":{"prompt_tokens":3,"completion_tokens":2,"total_tokens":5}}"#;
const ANTHROPIC_OK: &str = r#"{"id":"msg_1","type":"message","role":"assistant","model":"claude-3","content":[{"type":"text","text":"hi"}],"stop_reason":"end_turn","usage":{"input_tokens":5,"output_tokens":3}}"#;

/// 用户端点 = openai wire：入站 anthropic /v1/messages → 出站按用户配置的
/// openai 端点协议转 openai 形状、打用户 base_url 的 /chat/completions。
#[tokio::test]
async fn outbound_follows_user_endpoint_protocol_and_base_url() {
    let (upstream, hits) = spawn_recording_upstream(OPENAI_OK).await;
    let state = make_state(test_db().await).await;
    setup_user_endpoint_group(&state, "gkuep", Protocol::OpenAI, &upstream).await;

    let req = HttpRequest::builder()
        .method("POST")
        .uri("/v1/messages")
        .header("authorization", "Bearer gkuep")
        .header("content-type", "application/json")
        .body(Body::from(
            r#"{"model":"claude-3","max_tokens":32,"messages":[{"role":"user","content":"hi"}]}"#,
        ))
        .unwrap();
    let resp = handle_proxy(AxumState(state.clone()), req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    flush_log_queue(&state).await;

    let hits = hits.lock().unwrap();
    assert_eq!(hits.len(), 1, "上游应恰好收到一次请求: {hits:?}");
    let (path, body) = &hits[0];
    assert_eq!(path, "/chat/completions", "出站路径应按用户端点协议派生");
    assert!(body.contains("\"messages\""), "应转换成 openai 请求体: {body}");
    assert!(
        !body.contains("\"stop_reason\""),
        "不应是 anthropic 原体透传: {body}"
    );
}

/// 用户改端点协议（openai → anthropic）：同一入站 openai 请求出站按新协议转
/// anthropic 形状、打 /v1/messages——端点协议是运行时唯一真值。
#[tokio::test]
async fn outbound_follows_endpoint_protocol_after_user_change() {
    let (upstream, hits) = spawn_recording_upstream(ANTHROPIC_OK).await;
    let state = make_state(test_db().await).await;
    setup_user_endpoint_group(&state, "gkchg", Protocol::OpenAI, &upstream).await;

    let req = HttpRequest::builder()
        .method("POST")
        .uri("/chat/completions")
        .header("authorization", "Bearer gkchg")
        .header("content-type", "application/json")
        .body(Body::from(
            r#"{"model":"gpt-x","max_tokens":32,"messages":[{"role":"user","content":"hi"}]}"#,
        ))
        .unwrap();
    let resp = handle_proxy(AxumState(state.clone()), req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    flush_log_queue(&state).await;
    assert_eq!(hits.lock().unwrap().len(), 1);

    // 用户把端点协议改成 anthropic（update_platform 全量同步 endpoints）
    let plats = aidog_db::list_platforms(&state.db).await.unwrap();
    let pid = plats.iter().find(|p| p.name == "stub").unwrap().id;
    aidog_db::update_platform(
        &state.db,
        UpdatePlatform {
            id: pid,
            endpoints: Some(vec![PlatformEndpoint {
                protocol: Protocol::Anthropic,
                base_url: upstream.clone(),
                client_type: "default".to_string(),
                coding_plan: false,
            }]),
            name: None,
            platform_type: None,
            base_url: None,
            api_key: None,
            extra: None,
            models: None,
            available_models: None,
            enabled: None,
            status: None,
            manual_budgets: None,
            join_group_ids: None,
            expires_at: None,
            quota_source: None,
        },
    )
    .await
    .unwrap();

    let req = HttpRequest::builder()
        .method("POST")
        .uri("/chat/completions")
        .header("authorization", "Bearer gkchg")
        .header("content-type", "application/json")
        .body(Body::from(
            r#"{"model":"gpt-x","max_tokens":32,"messages":[{"role":"user","content":"hi"}]}"#,
        ))
        .unwrap();
    let resp = handle_proxy(AxumState(state.clone()), req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    flush_log_queue(&state).await;

    let hits = hits.lock().unwrap();
    assert_eq!(hits.len(), 2, "两次请求都应打到假上游: {hits:?}");
    let (path, body) = &hits[1];
    assert_eq!(path, "/v1/messages", "改端点协议后出站应按新协议派生路径");
    assert!(
        body.contains("\"system\"") || body.contains("\"stop_reason\"") || body.contains("\"max_tokens\""),
        "出站体应按 anthropic wire 构造: {body}"
    );
}

/// GLM 家族四个成员 × 端点协议，入站带 aidog 扩展 `disable_thinking: true`，返回上游收到的 body。
async fn glm_outbound_body(platform_type: Protocol, ep: Protocol, gk: &str) -> serde_json::Value {
    let resp_ok = if ep == Protocol::Anthropic { ANTHROPIC_OK } else { OPENAI_OK };
    let (upstream, hits) = spawn_recording_upstream(resp_ok).await;
    let state = make_state(test_db().await).await;
    // 厂商直连协议建库端点被锁成 preset 域名，用 fixture 直写 DB 指向本地假上游
    let pid = aidog_db::test_support::create_platform_at_fake_upstream(
        &state.db,
        platform_type.clone(),
        &upstream,
        std::slice::from_ref(&ep),
    )
    .await;
    let group = aidog_db::create_group(&state.db, aidog_db::test_support::sample_group(gk, vec![]))
        .await
        .unwrap();
    aidog_db::set_group_platforms(
        &state.db,
        group.id,
        &[GroupPlatformInput {
            platform_id: pid,
            priority: Some(0),
            weight: Some(1),
            level_priority: Some(0),
        }],
    )
    .await
    .unwrap();
    let req = HttpRequest::builder()
        .method("POST")
        .uri("/chat/completions")
        .header("authorization", format!("Bearer {gk}"))
        .header("content-type", "application/json")
        .body(Body::from(
            r#"{"model":"glm-test","max_tokens":32,"disable_thinking":true,"messages":[{"role":"user","content":"hi"}]}"#,
        ))
        .unwrap();
    let resp = handle_proxy(AxumState(state.clone()), req).await;
    let status = resp.status();
    let resp_body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    assert_eq!(status, StatusCode::OK, "{platform_type:?}/{ep:?}: {}", String::from_utf8_lossy(&resp_body));
    flush_log_queue(&state).await;
    let hits = hits.lock().unwrap();
    assert_eq!(hits.len(), 1, "上游应恰好收到一次请求: {hits:?}");
    serde_json::from_str(&hits[0].1).unwrap()
}

const GLM_FAMILY: [(Protocol, &str); 4] = [
    (Protocol::Glm, "gkglm"),
    (Protocol::GlmEn, "gkglmen"),
    (Protocol::GlmCoding, "gkglmcp"),
    (Protocol::GlmCodingEn, "gkglmcpen"),
];

/// issue 75：GLM 家族 openai 端点禁思考写官方参数 `thinking.type=disabled`
/// （docs.bigmodel.cn / docs.z.ai thinking-mode），不写 vLLM 式 `chat_template_kwargs`。
#[tokio::test]
async fn glm_family_openai_endpoint_disables_thinking_with_official_param() {
    for (pt, gk) in GLM_FAMILY {
        let body = glm_outbound_body(pt.clone(), Protocol::OpenAI, gk).await;
        assert_eq!(
            body.get("thinking"),
            Some(&serde_json::json!({"type": "disabled"})),
            "{pt:?} openai 端点应写 thinking.type=disabled: {body}"
        );
        assert!(
            body.get("chat_template_kwargs").is_none(),
            "{pt:?} openai 端点不应写 chat_template_kwargs: {body}"
        );
        assert!(body.get("disable_thinking").is_none(), "{pt:?} 私有字段应剥除: {body}");
    }
}

/// issue 75：GLM 家族 anthropic 端点禁思考与基座一致（`thinking.type=disabled`）。
#[tokio::test]
async fn glm_family_anthropic_endpoint_disables_thinking() {
    for (pt, gk) in GLM_FAMILY {
        let body = glm_outbound_body(pt.clone(), Protocol::Anthropic, gk).await;
        assert_eq!(
            body.get("thinking"),
            Some(&serde_json::json!({"type": "disabled"})),
            "{pt:?} anthropic 端点应写 thinking.type=disabled: {body}"
        );
        assert!(body.get("disable_thinking").is_none(), "{pt:?} 私有字段应剥除: {body}");
    }
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
