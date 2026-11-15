//! 行为锁定表测试（issue 55 票 03）：`handle_non_success` 各失败形态 × 判定/记账面。
//! 每例断言 attempt 的 `effects` / `exclusion` / `error_class`、scheduler 状态、
//! 返回 Next（failover，最终 2xx）或 Respond（终态回上游错误）。全部本地假上游，禁外呼。
//! 抽取 `judge`/`apply`（failure_verdict.rs）前后本文件必须逐字不变地保持全绿。

use super::*;
use crate::gateway::models::{CreatePlatform, GroupPlatformInput, Protocol, RoutingMode};
use aidog_db::test_support::{sample_group, test_db};
use aidog_middleware::MiddlewareEngine;
use axum::body::Body;
use axum::http::Request as HttpRequest;
use std::sync::Arc;
use std::sync::Mutex as StdMutex;

// ── 本地假上游：按序返回 (status, body, retry_after)，记录收到的 model 序列 ──

type Hits = Arc<StdMutex<Vec<(String, String, String)>>>;

async fn spawn_seq_upstream(responses: Vec<(u16, &'static str, Option<&'static str>)>) -> (String, Hits) {
    let hits: Hits = Arc::new(StdMutex::new(Vec::new()));
    let queue = Arc::new(StdMutex::new(std::collections::VecDeque::from(responses)));
    let h2 = hits.clone();
    let app = axum::Router::new().fallback(axum::routing::any(move |req: Request| {
        let hits = h2.clone();
        let queue = queue.clone();
        async move {
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
            hits.lock().unwrap().push((String::new(), body, auth));
            let (status, resp, retry_after) = queue.lock().unwrap().pop_front().unwrap_or((200, "{}", None));
            let mut headers = reqwest::header::HeaderMap::new();
            headers.insert("content-type", reqwest::header::HeaderValue::from_static("application/json"));
            if let Some(ra) = retry_after
                && let Ok(v) = reqwest::header::HeaderValue::from_str(ra)
            {
                headers.insert(reqwest::header::RETRY_AFTER, v);
            }
            (axum::http::StatusCode::from_u16(status).unwrap(), headers, resp)
        }
    }));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.ok(); });
    (format!("http://{addr}"), hits)
}

async fn spawn_stub_upstream(status: u16, body: &'static str) -> String {
    spawn_seq_upstream(vec![(status, body, None)]).await.0
}

fn hit_models(h: &Hits) -> Vec<String> {
    h.lock().unwrap().iter().map(|(_, b, _)| {
        serde_json::from_str::<serde_json::Value>(b)
            .ok()
            .and_then(|v| v.get("model").and_then(|m| m.as_str().map(String::from)))
            .unwrap_or_default()
    }).collect()
}

// ── fixtures（复制自 test_integration.rs 的同形夹具，独立成模块不可跨文件引用）──

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

async fn setup_chat_group(
    state: &Arc<ProxyState>,
    gk: &str,
    plats: &[(&str, &[&str], Protocol, &str)],
) {
    let group = aidog_db::create_group(&state.db, sample_group(gk, vec![])).await.unwrap();
    let mut inputs = Vec::new();
    for (i, (base, chain, proto, name)) in plats.iter().enumerate() {
        let slot = aidog_db::models::PlatformModels {
            default: chain.iter().map(|s| s.to_string()).collect(),
            ..Default::default()
        };
        let plat = aidog_db::create_platform(
            &state.db,
            CreatePlatform {
                name: format!("{gk}-{name}"),
                platform_type: proto.clone(),
                base_url: base.to_string(),
                api_key: format!("sk-{name}"),
                extra: String::new(),
                models: Some(slot),
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

async fn set_group_routing_mode(state: &Arc<ProxyState>, gk: &str, mode: RoutingMode) {
    let g = aidog_db::list_groups(&state.db).await.unwrap().into_iter().find(|g| g.group_key == gk).unwrap();
    aidog_db::update_group(&state.db, aidog_db::models::UpdateGroup {
        id: g.id, name: None, routing_mode: Some(mode), request_timeout_secs: 0, connect_timeout_secs: 0,
        source_protocol: None, max_retries: None, model_mappings: g.model_mappings.clone(),
        env_vars: g.env_vars.clone(), is_default: None,
    })
    .await
    .unwrap();
    state.db.invalidate_group_details_cache();
}

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

/// glm 协议平台（registry rate_limit_error_codes 1302/1305 的载体）+ 同挂 anthropic 端点。
async fn mk_glm_platform(state: &Arc<ProxyState>, base_url: String) -> u64 {
    aidog_db::test_support::create_platform_at_fake_upstream(
        &state.db,
        Protocol::Glm,
        &base_url,
        &[Protocol::Glm, Protocol::Anthropic],
    )
    .await
}

async fn group_of(state: &Arc<ProxyState>, gk: &str, ids: &[u64]) {
    let group = aidog_db::create_group(&state.db, sample_group(gk, vec![])).await.unwrap();
    let inputs: Vec<GroupPlatformInput> = ids
        .iter()
        .enumerate()
        .map(|(i, id)| GroupPlatformInput { platform_id: *id, priority: Some(i as i32), weight: Some(1), level_priority: Some(0) })
        .collect();
    aidog_db::set_group_platforms(&state.db, group.id, &inputs).await.unwrap();
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

async fn last_log_for(state: &Arc<ProxyState>, gk: &str) -> aidog_db::models::ProxyLog {
    flush_log_queue(state).await;
    let logs = aidog_logs::list_proxy_logs(&state.db, 100, 0).await.unwrap();
    let id = logs
        .iter()
        .find(|l| l.group_key == gk)
        .expect("log row missing")
        .id
        .clone();
    aidog_logs::get_proxy_log(&state.db, &id).await.unwrap().unwrap()
}

const CHAT_REQ: &str = r#"{"model":"claude-x","max_tokens":16,"messages":[{"role":"user","content":"hi"}]}"#;
const OK: &str = r#"{"id":"msg_1","type":"message","role":"assistant","model":"claude-3","content":[{"type":"text","text":"hi"}],"stop_reason":"end_turn","usage":{"input_tokens":5,"output_tokens":3}}"#;
const E404_MODEL: &str = r#"{"type":"error","error":{"type":"not_found_error","message":"model: m-a"}}"#;
const E401: &str = r#"{"error":{"message":"invalid api key"}}"#;
const E_REGION_401: &str = r#"{"error":{"message":"this service is not available in your country"}}"#;

/// 断言 attempt[0] 的 (effects, exclusion 字符串, error_class)。
fn assert_attempt(
    a: &aidog_db::models::ProxyAttempt,
    effects: &[&str],
    exclusion: Option<&str>,
    class: &str,
    ctx: &str,
) {
    let got: Vec<&str> = a.effects.iter().map(String::as_str).collect();
    assert_eq!(got, effects, "{ctx}: effects");
    assert_eq!(
        a.exclusion.as_ref().map(|e| serde_json::to_string(e).unwrap()).as_deref().map(|s| s.trim_matches('"').to_string()),
        exclusion.map(String::from),
        "{ctx}: exclusion"
    );
    assert_eq!(a.error_class.as_deref(), Some(class), "{ctx}: error_class");
}

// ══ 401 / 402 / 403 × 健康感知与否 ══

async fn auth_case(status: u16, mode: RoutingMode) -> (aidog_db::models::ProxyAttempt, Arc<ProxyState>, u64) {
    let up = spawn_stub_upstream(status, r#"{"error":{"message":"denied"}}"#).await;
    let state = make_state(test_db().await).await;
    setup_chat_group(&state, "gklk", &[(&up, &["claude-3"], Protocol::Anthropic, "p")]).await;
    set_group_routing_mode(&state, "gklk", mode).await;
    set_group_max_attempts(&state, "gklk", 1).await;
    let _ = handle_proxy(AxumState(state.clone()), messages_request("gklk", CHAT_REQ)).await;
    let log = last_log_for(&state, "gklk").await;
    let pid = aidog_db::list_platforms(&state.db).await.unwrap()[0].id;
    (log.attempts[0].clone(), state, pid)
}

#[tokio::test]
async fn lk01_401_health_aware_breaker_count() {
    let (a, state, pid) = auth_case(401, RoutingMode::HealthAware).await;
    assert_attempt(&a, &["breaker_count", "penalty_client"], None, "service", "401 HA");
    assert_eq!(state.scheduler.breaker_fails(pid), 1, "健康感知分组 401 计熔断");
    assert!(!state.scheduler.auth_cooled(pid, aidog_db::now() + 1000), "健康感知分组不走 auth 冷却");
}

#[tokio::test]
async fn lk02_401_not_health_aware_auth_cooldown() {
    let (a, state, pid) = auth_case(401, RoutingMode::Failover).await;
    assert_attempt(&a, &["penalty_client", "cooldown_auth"], None, "service", "401 non-HA");
    assert_eq!(state.scheduler.breaker_fails(pid), 0, "非健康感知分组不计熔断");
    assert!(state.scheduler.auth_cooled(pid, aidog_db::now() + 1000), "401 应设 5 分钟 auth 冷却");
}

#[tokio::test]
async fn lk03_402_balance_cooldown_both_modes() {
    for mode in [RoutingMode::HealthAware, RoutingMode::Failover] {
        let (a, state, pid) = auth_case(402, mode).await;
        assert_attempt(&a, &["penalty_client", "cooldown_balance"], None, "service", "402");
        assert_eq!(state.scheduler.breaker_fails(pid), 0, "402 不计熔断");
        assert!(state.scheduler.auth_cooled(pid, aidog_db::now() + 1000), "402 余额冷却");
    }
}

#[tokio::test]
async fn lk04_403_matches_401_both_modes() {
    let (a, state, pid) = auth_case(403, RoutingMode::HealthAware).await;
    assert_attempt(&a, &["breaker_count", "penalty_client"], None, "service", "403 HA");
    assert_eq!(state.scheduler.breaker_fails(pid), 1);
    let (a, state, pid) = auth_case(403, RoutingMode::Failover).await;
    assert_attempt(&a, &["penalty_client", "cooldown_auth"], None, "service", "403 non-HA");
    assert_eq!(state.scheduler.breaker_fails(pid), 0);
    assert!(state.scheduler.auth_cooled(pid, aidog_db::now() + 1000));
}

// ══ 429 三形态 / 529 / registry 码 / 5xx ══

async fn rate_case(
    status: u16,
    body: &'static str,
    retry_after: Option<&'static str>,
) -> (aidog_db::models::ProxyAttempt, Arc<ProxyState>, u64, u16) {
    let (up, _) = spawn_seq_upstream(vec![(status, body, retry_after)]).await;
    let state = make_state(test_db().await).await;
    setup_chat_group(&state, "gklkr", &[(&up, &["claude-3"], Protocol::Anthropic, "p")]).await;
    set_group_max_attempts(&state, "gklkr", 1).await;
    let resp = handle_proxy(AxumState(state.clone()), messages_request("gklkr", CHAT_REQ)).await;
    let log = last_log_for(&state, "gklkr").await;
    let pid = aidog_db::list_platforms(&state.db).await.unwrap()[0].id;
    (log.attempts[0].clone(), state, pid, resp.status().as_u16())
}

#[tokio::test]
async fn lk05_quota_429_with_retry_after() {
    let (a, state, pid, out) = rate_case(429, r#"{"error":{"message":"quota exhausted"}}"#, Some("120")).await;
    assert_attempt(&a, &["penalty_server", "cooldown_quota"], None, "service", "429 quota+RA");
    assert_eq!(out, 429, "单平台末位 → 终态回上游错误");
    assert!(state.scheduler.quota_cooled(pid, aidog_db::now() + 1000), "配额冷却到 Retry-After 时刻");
    assert!(!state.scheduler.rate_limit_cooled(pid, aidog_db::now() + 100), "不走限频冷却通道");
    assert_eq!(state.scheduler.breaker_fails(pid), 0, "429 不计熔断");
}

#[tokio::test]
async fn lk06_quota_429_without_reset_time() {
    let (a, state, pid, _) = rate_case(429, r#"{"error":{"message":"quota exhausted"}}"#, None).await;
    assert_attempt(&a, &["penalty_server", "cooldown_rate_limit"], None, "service", "429 quota no-reset");
    assert!(state.scheduler.rate_limit_cooled(pid, aidog_db::now() + 100), "解析不到重置时间 → 限频冷却");
    assert!(!state.scheduler.quota_cooled(pid, aidog_db::now() + 100));
    assert_eq!(state.scheduler.breaker_fails(pid), 0);
}

#[tokio::test]
async fn lk07_per_minute_429_cooldown_minute() {
    let (a, state, pid, _) = rate_case(429, r#"{"error":{"message":"rate limit 40/min exceeded"}}"#, None).await;
    assert_attempt(&a, &["cooldown_minute"], None, "service", "429 per-minute");
    assert!(state.scheduler.quota_cooled(pid, aidog_db::now() + 1), "每分钟限流冷却到下一整分边界");
    assert!(!state.scheduler.quota_cooled(pid, aidog_db::now() + 61_000));
    assert_eq!(state.scheduler.breaker_fails(pid), 0, "不计熔断");
}

#[tokio::test]
async fn lk08_plain_429_rate_limit_cooldown() {
    let (a, state, pid, _) = rate_case(429, r#"{"error":{"message":"rate limited"}}"#, None).await;
    assert_attempt(&a, &["penalty_server", "cooldown_rate_limit"], None, "service", "plain 429");
    assert!(state.scheduler.rate_limit_cooled(pid, aidog_db::now() + 100));
    assert_eq!(state.scheduler.breaker_fails(pid), 0);
}

#[tokio::test]
async fn lk09_529_rate_limit_cooldown() {
    let (a, state, pid, _) = rate_case(529, r#"{"error":{"message":"overloaded"}}"#, None).await;
    assert_attempt(&a, &["penalty_server", "cooldown_rate_limit"], None, "service", "529");
    assert!(state.scheduler.rate_limit_cooled(pid, aidog_db::now() + 100));
    assert_eq!(state.scheduler.breaker_fails(pid), 0);
}

#[tokio::test]
async fn lk10_5xx_penalty_only() {
    let (a, state, pid, out) = rate_case(500, r#"{"error":{"message":"boom"}}"#, None).await;
    assert_attempt(&a, &["penalty_server"], None, "service", "500");
    assert_eq!(out, 500);
    assert_eq!(state.scheduler.breaker_fails(pid), 0, "5xx 不计熔断");
    assert!(!state.scheduler.rate_limit_cooled(pid, aidog_db::now() + 100));
    assert!(!state.scheduler.quota_cooled(pid, aidog_db::now() + 1));
    assert!(!state.scheduler.auth_cooled(pid, aidog_db::now() + 1000));
}

// ══ 区域封锁 401 ══

#[tokio::test]
async fn lk11_region_blocked_401_no_effects() {
    let (a, state, pid, _) = rate_case(401, E_REGION_401, None).await;
    assert_attempt(&a, &["penalty_client"], None, "service", "region 401");
    assert_eq!(state.scheduler.breaker_fails(pid), 0, "区域封锁不计熔断");
    assert!(!state.scheduler.auth_cooled(pid, aidog_db::now() + 1000), "区域封锁不走 auth 冷却");
    let plats = aidog_db::list_platforms(&state.db).await.unwrap();
    assert!(plats.iter().all(|p| p.auto_disabled_until == 0), "区域封锁不自动禁用");
}

// ══ registry rate_limit / overloaded 码（glm 1302 / 1305）══

async fn glm_case(body: &'static str, status: u16) -> (aidog_db::models::ProxyAttempt, Arc<ProxyState>, u64) {
    let up = spawn_stub_upstream(status, body).await;
    let state = make_state(test_db().await).await;
    let pid = mk_glm_platform(&state, up).await;
    group_of(&state, "gklkg", &[pid]).await;
    set_group_max_attempts(&state, "gklkg", 1).await;
    let _ = handle_proxy(AxumState(state.clone()), messages_request("gklkg", CHAT_REQ)).await;
    let log = last_log_for(&state, "gklkg").await;
    (log.attempts[0].clone(), state, pid)
}

#[tokio::test]
async fn lk12_registry_rate_limit_code_cooldown_minute() {
    let (a, state, pid) = glm_case(r#"{"error":{"code":"1302","message":"您的账户已达到速率限制"}}"#, 429).await;
    assert_attempt(&a, &["cooldown_minute"], None, "service", "1302");
    assert!(state.scheduler.quota_cooled(pid, aidog_db::now() + 1), "1302 归每分钟限流桶");
    assert!(!state.scheduler.quota_cooled(pid, aidog_db::now() + 61_000));
    assert_eq!(state.scheduler.breaker_fails(pid), 0);
}

#[tokio::test]
async fn lk13_registry_overloaded_code_rate_limit_cooldown() {
    let (a, state, pid) = glm_case(r#"{"error":{"code":"1305","message":"该模型当前访问量过大"}}"#, 400).await;
    assert_attempt(&a, &["penalty_client", "cooldown_rate_limit"], None, "service", "1305");
    assert!(state.scheduler.rate_limit_cooled(pid, aidog_db::now() + 100), "1305 进限频冷却");
    assert!(!state.scheduler.quota_cooled(pid, aidog_db::now() + 1));
    assert_eq!(state.scheduler.breaker_fails(pid), 0);
}

// ══ censorship_blocked ══

#[tokio::test]
async fn lk14_censorship_disable_no_penalty() {
    let (a, state, pid, out) = rate_case(
        400,
        r#"{"error":{"message":"blocked","type":"censorship_blocked"}}"#,
        None,
    ).await;
    assert_attempt(&a, &["disabled_censorship"], None, "service", "censorship");
    assert_eq!(out, 400, "单平台末位终态回上游错误");
    let plats = aidog_db::list_platforms(&state.db).await.unwrap();
    let p = plats.iter().find(|p| p.id == pid).unwrap();
    assert!(p.auto_disabled_until > 0, "审核拒绝自动禁用一小时");
}

// ══ error_rule non-retryable ══

#[tokio::test]
async fn lk15_rule_non_retryable_blocks_all_switches() {
    use aidog_db::models::{
        ActionKind, ActionParams, ActionStep, AppliesTo, ConditionLeaf, ConditionNode, MatchType, MiddlewareRule, Target,
    };
    let (up, hits) = spawn_seq_upstream(vec![(404, E404_MODEL, None)]).await;
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
        user_modified: false,
        created_at: 0,
        updated_at: 0,
    }]);
    setup_chat_group(&state, "gklkm", &[(&up, &["m-a", "m-b"], Protocol::Anthropic, "p")]).await;
    let resp = handle_proxy(AxumState(state.clone()), messages_request("gklkm", CHAT_REQ)).await;
    assert_eq!(resp.status().as_u16(), 404, "non-retryable 直接回客户端");
    assert_eq!(hit_models(&hits), ["m-a"], "规则禁止换模型");
    let log = last_log_for(&state, "gklkm").await;
    assert_eq!(log.attempts[0].chain_stop.as_deref(), Some("rule"));
    // 模型类 + rule_blocks：无 no_blame 豁免 → penalty_client 照记（404 → Client 档）
    assert_attempt(&log.attempts[0], &["penalty_client"], None, "model", "rule non-retryable");
}

// ══ 模型类 × has_next_model 三分支 ══

#[tokio::test]
async fn lk16_model_class_switches_model_no_accounting() {
    let (up, hits) = spawn_seq_upstream(vec![(404, E404_MODEL, None), (200, OK, None)]).await;
    let state = make_state(test_db().await).await;
    setup_chat_group(&state, "gklks", &[(&up, &["m-a", "m-b"], Protocol::Anthropic, "p")]).await;
    let resp = handle_proxy(AxumState(state.clone()), messages_request("gklks", CHAT_REQ)).await;
    assert_eq!(resp.status().as_u16(), 200, "换模型后成功");
    assert_eq!(hit_models(&hits), ["m-a", "m-b"]);
    let log = last_log_for(&state, "gklks").await;
    assert_eq!(log.attempts.len(), 2);
    assert_attempt(&log.attempts[0], &[], Some("pair"), "model", "switch-model");
    assert_eq!(state.scheduler.breaker_fails(aidog_db::list_platforms(&state.db).await.unwrap()[0].id), 0, "模型类失败不碰平台健康");
}

#[tokio::test]
async fn lk17_model_chain_exhausted_switches_platform_no_blame() {
    let (up1, hits1) = spawn_seq_upstream(vec![(404, E404_MODEL, None)]).await;
    let (up2, hits2) = spawn_seq_upstream(vec![(200, OK, None)]).await;
    let state = make_state(test_db().await).await;
    setup_chat_group(
        &state,
        "gklkp",
        &[(&up1, &["m-a"], Protocol::Anthropic, "p1"), (&up2, &["m-b"], Protocol::Anthropic, "p2")],
    ).await;
    let resp = handle_proxy(AxumState(state.clone()), messages_request("gklkp", CHAT_REQ)).await;
    assert_eq!(resp.status().as_u16(), 200, "链尽换平台成功");
    assert_eq!(hit_models(&hits1), ["m-a"]);
    assert_eq!(hit_models(&hits2), ["m-b"]);
    let log = last_log_for(&state, "gklkp").await;
    assert_attempt(&log.attempts[0], &[], Some("pair"), "model", "chain-exhausted switch platform");
    let p1 = aidog_db::list_platforms(&state.db).await.unwrap().into_iter().find(|p| p.name.ends_with("p1")).unwrap();
    assert!(p1.last_error.is_empty(), "模型类失败不写平台 last_error");
}

#[tokio::test]
async fn lk18_model_chain_exhausted_last_candidate_responds_no_blame() {
    let up = spawn_stub_upstream(404, E404_MODEL).await;
    let state = make_state(test_db().await).await;
    setup_chat_group(&state, "gklkt", &[(&up, &["m-a"], Protocol::Anthropic, "p")]).await;
    set_group_max_attempts(&state, "gklkt", 1).await;
    let resp = handle_proxy(AxumState(state.clone()), messages_request("gklkt", CHAT_REQ)).await;
    assert_eq!(resp.status().as_u16(), 404, "末位直返上游错误");
    let log = last_log_for(&state, "gklkt").await;
    assert_attempt(&log.attempts[0], &[], None, "model", "chain-exhausted last");
    let pid = aidog_db::list_platforms(&state.db).await.unwrap()[0].id;
    assert_eq!(state.scheduler.breaker_fails(pid), 0);
    assert!(!state.scheduler.auth_cooled(pid, aidog_db::now() + 1000));
    assert!(!state.scheduler.rate_limit_cooled(pid, aidog_db::now() + 100));
    let p = aidog_db::list_platforms(&state.db).await.unwrap().remove(0);
    assert!(p.last_error.is_empty(), "末位模型类失败不写 last_error");
}

// ══ 服务类失败 × 多平台 failover（Next 面）══

#[tokio::test]
async fn lk19_service_error_fails_over_with_exclusion_and_penalty() {
    let up1 = spawn_stub_upstream(500, r#"{"error":{"message":"boom"}}"#).await;
    let (up2, hits2) = spawn_seq_upstream(vec![(200, OK, None)]).await;
    let state = make_state(test_db().await).await;
    setup_chat_group(
        &state,
        "gklku",
        &[(&up1, &["claude-3"], Protocol::Anthropic, "p1"), (&up2, &["claude-3"], Protocol::Anthropic, "p2")],
    ).await;
    let resp = handle_proxy(AxumState(state.clone()), messages_request("gklku", CHAT_REQ)).await;
    assert_eq!(resp.status().as_u16(), 200, "服务类失败换平台成功");
    assert_eq!(hit_models(&hits2), ["claude-3"]);
    let log = last_log_for(&state, "gklku").await;
    assert_attempt(&log.attempts[0], &["penalty_server"], Some("platform"), "service", "failover 500");
    let p1 = aidog_db::list_platforms(&state.db).await.unwrap().into_iter().find(|p| p.name.ends_with("p1")).unwrap();
    assert!(!p1.last_error.is_empty(), "服务类失败写平台 last_error");
}

// ══ 401 health-aware 计熔断后仍 failover（Next 面的 auth 分支）══

#[tokio::test]
async fn lk20_401_health_aware_fails_over_next() {
    let up1 = spawn_stub_upstream(401, E401).await;
    let (up2, _hits2) = spawn_seq_upstream(vec![(200, OK, None)]).await;
    let state = make_state(test_db().await).await;
    setup_chat_group(
        &state,
        "gklkv",
        &[(&up1, &["claude-3"], Protocol::Anthropic, "p1"), (&up2, &["claude-3"], Protocol::Anthropic, "p2")],
    ).await;
    set_group_routing_mode(&state, "gklkv", RoutingMode::HealthAware).await;
    let resp = handle_proxy(AxumState(state.clone()), messages_request("gklkv", CHAT_REQ)).await;
    assert_eq!(resp.status().as_u16(), 200);
    let log = last_log_for(&state, "gklkv").await;
    assert_attempt(&log.attempts[0], &["breaker_count", "penalty_client"], Some("platform"), "service", "401 HA failover");
    let p1 = aidog_db::list_platforms(&state.db).await.unwrap().into_iter().find(|p| p.name.ends_with("p1")).unwrap();
    assert_eq!(state.scheduler.breaker_fails(p1.id), 1);
}
