//! 独立搜索/tools 转发（票 19）单元测试：
//! 纯函数（tools_url_matches / tools_success_ok 逐平台 strict 表）+ 端到端
//! （three_file_db 真 split 库 + 本地 stub 上游 + preset cache 重定向 → handle_proxy 全链，
//! 落库断言经 flush 后**重开 DB** 读回——split 表错路由在内存库测不出，见 db-pool-routing.md）。
//! 转发目标一律本地 stub（test-no-real-calls.md）；URL 重定向经 store_presets_cache 装
//! 「bundled 克隆 + tools_endpoints 指向 stub」的文档（其余协议字段原样，不污染其它测试），
//! TOOLS_E2E_LOCK 串行化端到端（全局 cache 互斥）。

use super::*;
use crate::gateway::models::{CreatePlatform, GroupPlatformInput, Protocol};
use aidog_db::test_support::{sample_group, three_file_db};
use aidog_middleware::MiddlewareEngine;
use axum::body::Body;
use axum::http::Request as HttpRequest;
use axum::routing::any;
use std::sync::Arc;
use std::sync::Mutex as StdMutex;

/// 端到端测试互斥：store_presets_cache 是进程级全局，stub 文档不能与其它端到端并发。
static TOOLS_E2E_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

// ── 纯函数：URL 匹配 ──

#[test]
fn tools_url_path_extracts_path() {
    assert_eq!(
        crate::gateway::billing::tools_url_path("https://api.deepseek.com/v1/tools/search"),
        "/v1/tools/search"
    );
    assert_eq!(crate::gateway::billing::tools_url_path("https://host.example"), "/");
}

#[test]
fn tools_url_matches_exact_and_variants() {
    let u = "https://api.deepseek.com/v1/tools/search";
    assert!(crate::gateway::billing::tools_url_matches(u, "/v1/tools/search"));
    // 尾斜杠与 /proxy 前缀变体
    assert!(crate::gateway::billing::tools_url_matches(u, "/v1/tools/search/"));
    assert!(crate::gateway::billing::tools_url_matches(u, "/proxy/v1/tools/search"));
    // 不同 path 不命中（语义 2 精确匹配）
    assert!(!crate::gateway::billing::tools_url_matches(u, "/v1/tools/fetch"));
    assert!(!crate::gateway::billing::tools_url_matches(u, "/v1/tools/searchx"));
}

#[test]
fn tools_endpoint_predicate_follows_registry() {
    // bundled registry 已声明（票 18）
    assert!(is_tools_endpoint("/v1/tools/search"));
    assert!(is_tools_endpoint("/v1/tools/fetch"));
    assert!(is_tools_endpoint("/api/paas/v4/web_search"));
    assert!(is_tools_endpoint("/v2/ai_search/chat/completions"));
    // 未声明 path → false（语义 11 零回归）
    assert!(!is_tools_endpoint("/v1/tools/other"));
    assert!(!is_tools_endpoint("/v1/messages"));
}

// ── 纯函数：strict 成功判定逐平台表 ──

#[test]
fn success_deepseek_search_requires_nonempty_results() {
    let ok = br#"{"results":[{"title":"t","url":"https://x"}]}"#;
    assert!(tools_success_ok(&Protocol::DeepSeek, "search", ok));
    // 官方明文「搜索无结果不计费」：空数组不算成功
    assert!(!tools_success_ok(&Protocol::DeepSeek, "search", br#"{"results":[]}"#));
    assert!(!tools_success_ok(&Protocol::DeepSeek, "search", b"not json"));
    // fetch 看正文非空
    assert!(tools_success_ok(
        &Protocol::DeepSeek,
        "fetch",
        br#"{"title":"t","content":"body text"}"#
    ));
    assert!(!tools_success_ok(&Protocol::DeepSeek, "fetch", br#"{"title":"t","content":""}"#));
}

#[test]
fn success_kimi_family_fields() {
    let ok = br#"{"search_results":[{"title":"t"}]}"#;
    assert!(tools_success_ok(&Protocol::Kimi, "search", ok));
    assert!(tools_success_ok(&Protocol::KimiEn, "search_pro", ok));
    assert!(!tools_success_ok(&Protocol::Kimi, "search", br#"{"search_results":[]}"#));
    assert!(tools_success_ok(
        &Protocol::Kimi,
        "fetch",
        br#"{"url":"https://x","markdown":"hello body"}"#
    ));
    // 404 markdown_not_found 形态：markdown 空/缺 → 不计
    assert!(!tools_success_ok(&Protocol::Kimi, "fetch", br#"{"url":"https://x"}"#));
}

#[test]
fn success_glm_stepfun_qianfan_fields() {
    assert!(tools_success_ok(
        &Protocol::Glm,
        "web_search",
        br#"{"search_intent":[],"search_result":[{"title":"t"}]}"#
    ));
    assert!(!tools_success_ok(&Protocol::GlmEn, "web_search", br#"{"search_result":[]}"#));
    assert!(tools_success_ok(
        &Protocol::StepFun,
        "search",
        br#"{"results":[{"url":"https://x"}]}"#
    ));
    assert!(!tools_success_ok(&Protocol::StepFun, "search", br#"{"results":[]}"#));
    assert!(tools_success_ok(
        &Protocol::QianFan,
        "ai_search_chat",
        br#"{"choices":[{"message":{"content":"a"}}]}"#
    ));
    assert!(!tools_success_ok(&Protocol::QianFan, "ai_search_chat", br#"{"choices":[]}"#));
}

#[test]
fn no_tools_platform_message_is_user_facing() {
    assert!(kind_route_error_message("no_tools_platform").contains("搜索平台"));
}

// ── 端到端 helpers ──

/// stub 上游：所有 POST 返回 (status, body)，并把收到的鉴权头 / path / body 记入捕获 JSON。
async fn spawn_capture_upstream(status: u16, body: &'static str) -> (String, Arc<StdMutex<Value>>) {
    let captured: Arc<StdMutex<Value>> = Arc::new(StdMutex::new(Value::Null));
    let cap = captured.clone();
    let app = axum::Router::new().fallback(
        any(move |headers: axum::http::HeaderMap, req: axum::body::Body| async move {
            let body_bytes = axum::body::to_bytes(req, 1024 * 1024)
                .await
                .unwrap_or_default();
            *cap.lock().unwrap() = serde_json::json!({
                "authorization": headers.get("authorization").and_then(|v| v.to_str().ok()),
                "appbuilder": headers.get("x-appbuilder-authorization").and_then(|v| v.to_str().ok()),
                "path": "/captured",
                "body": String::from_utf8_lossy(&body_bytes).to_string(),
            });
            (
                axum::http::StatusCode::from_u16(status).unwrap(),
                [("content-type", "application/json")],
                body,
            )
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.ok();
    });
    (format!("http://{addr}"), captured)
}

/// 装「bundled 克隆 + 指定协议 tools_endpoints 重定向到 stub」的 preset 文档（保留其余字段，
/// 其它测试读到与 bundled 等价数据）。测完须 `invalidate_presets_cache()` 恢复回落。
fn store_tools_preset_redirect(stub: &str, codes: &[&str]) {
    let mut doc = crate::gateway::registry::presets().clone();
    let protos = doc.get_mut("protocols").and_then(|p| p.as_object_mut()).unwrap();
    for c in codes {
        let po = protos.get_mut(*c).and_then(|p| p.as_object_mut()).unwrap();
        let te = po
            .get_mut("tools_endpoints")
            .and_then(|t| t.as_object_mut())
            .unwrap();
        let te: &mut serde_json::Map<String, Value> = te;
        let redirected: serde_json::Map<String, Value> = te
            .iter()
            .map(|(k, v)| {
                let path = crate::gateway::billing::tools_url_path(v.as_str().unwrap_or(""));
                (k.clone(), Value::String(format!("{stub}{path}")))
            })
            .collect();
        *te = redirected;
    }
    crate::gateway::registry::store_presets_cache(doc);
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

/// 注册平台（指定协议 / extra / 优先级）+ group 并关联，返回平台 id。
async fn setup_platform_group(
    state: &Arc<ProxyState>,
    gk: &str,
    name: &str,
    pt: Protocol,
    extra: &str,
    base_url: &str,
    priority: i32,
) -> u64 {
    let plat = aidog_db::create_platform(
        &state.db,
        CreatePlatform {
            name: name.to_string(),
            platform_type: pt,
            base_url: base_url.to_string(),
            api_key: "sk-tools".into(),
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
    let group = aidog_db::create_group(&state.db, sample_group(gk, vec![]))
        .await
        .unwrap();
    aidog_db::set_group_platforms(
        &state.db,
        group.id,
        &[GroupPlatformInput {
            platform_id: plat.id,
            priority: Some(priority),
            weight: Some(1),
            level_priority: Some(0),
        }],
    )
    .await
    .unwrap();
    plat.id
}

fn tools_request(gk: &str, path: &str, body: &str) -> Request {
    HttpRequest::builder()
        .method("POST")
        .uri(path)
        .header("authorization", format!("Bearer {gk}"))
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .unwrap()
}

/// 独立重开同一三文件库读回（写后重开级别断言；split 表错路由在这里显形）。
async fn reopen_and_read(dir: &tempfile::TempDir) -> Db {
    let main = dir.path().join("aidog.db");
    Db::new(main.to_str().unwrap()).await.unwrap()
}

const SEARCH_OK: &str = r#"{"results":[{"title":"t","url":"https://x","snippet":"s"}]}"#;
const SEARCH_EMPTY: &str = r#"{"results":[]}"#;

/// 用例 1：成功计费含汇率换算。deepseek search 成功 → est_cost = 5/1000 ÷ 6.6675（CNY→USD），
/// model 列 = tools:search。proxy_log 行写后重开 DB 读回验证。
#[tokio::test]
async fn e2e_success_bills_per_call_with_fx() {
    let _guard = TOOLS_E2E_LOCK.lock().await;
    let (db, dir) = three_file_db().await;
    let state = make_state(db).await;
    let (stub, _) = spawn_capture_upstream(200, SEARCH_OK).await;
    store_tools_preset_redirect(&stub, &["deepseek"]);
    let plat_id = setup_platform_group(&state, "g1", "ds", Protocol::DeepSeek, "", &stub, 0).await;

    let resp = handle_proxy(
        AxumState(state.clone()),
        tools_request("g1", "/v1/tools/search", r#"{"query":"rust"}"#),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    flush_log_queue(&state).await;
    crate::gateway::registry::invalidate_presets_cache();

    let fresh = reopen_and_read(&dir).await;
    let logs = aidog_logs::list_proxy_logs(&fresh, 10, 0).await.unwrap();
    assert_eq!(logs.len(), 1, "one proxy_log row");
    let row = aidog_logs::get_proxy_log(&fresh, &logs[0].id).await.unwrap().unwrap();
    assert_eq!(row.platform_id, plat_id as u64);
    assert_eq!(row.model, "tools:search");
    assert_eq!(row.status_code, 200);
    let expected = 5.0 / 1000.0 / crate::gateway::billing::CNY_PER_USD;
    assert!(
        (row.est_cost - expected).abs() < 1e-9,
        "est_cost = per_1000/1000 / CNY_PER_USD, got {}",
        row.est_cost
    );
}

/// 用例 2：200 空结果不计费（官方明文「搜索无结果不计费」）→ est_cost 0，客户端仍拿 200。
#[tokio::test]
async fn e2e_empty_results_not_billed() {
    let _guard = TOOLS_E2E_LOCK.lock().await;
    let (db, dir) = three_file_db().await;
    let state = make_state(db).await;
    let (stub, _) = spawn_capture_upstream(200, SEARCH_EMPTY).await;
    store_tools_preset_redirect(&stub, &["deepseek"]);
    setup_platform_group(&state, "g1", "ds", Protocol::DeepSeek, "", &stub, 0).await;

    let resp = handle_proxy(
        AxumState(state.clone()),
        tools_request("g1", "/v1/tools/search", r#"{"query":"nothing"}"#),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    flush_log_queue(&state).await;
    crate::gateway::registry::invalidate_presets_cache();

    let fresh = reopen_and_read(&dir).await;
    let logs = aidog_logs::list_proxy_logs(&fresh, 10, 0).await.unwrap();
    assert_eq!(logs.len(), 1);
    let row = aidog_logs::get_proxy_log(&fresh, &logs[0].id).await.unwrap().unwrap();
    assert_eq!(row.est_cost, 0.0, "empty results must not be billed");
    assert_eq!(row.status_code, 200);
}

/// 用例 3：非 200 不计费且走共享熔断（500 → Server penalty 计数，penalty_active 可查）。
#[tokio::test]
async fn e2e_upstream_500_not_billed_and_counts_breaker() {
    let _guard = TOOLS_E2E_LOCK.lock().await;
    let (db, dir) = three_file_db().await;
    let state = make_state(db).await;
    let (stub, _) = spawn_capture_upstream(500, r#"{"error":"boom"}"#).await;
    store_tools_preset_redirect(&stub, &["deepseek"]);
    let plat_id = setup_platform_group(&state, "g1", "ds", Protocol::DeepSeek, "", &stub, 0).await;

    let resp = handle_proxy(
        AxumState(state.clone()),
        tools_request("g1", "/v1/tools/search", r#"{"query":"x"}"#),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::INTERNAL_SERVER_ERROR, "last candidate: upstream status passthrough");
    // 共享熔断：tools 失败计入同一 scheduler（Server tier penalty 立即生效）。
    assert!(
        state
            .scheduler
            .penalty_active(plat_id as u64, aidog_db::now()),
        "tools failure must count into shared breaker penalty"
    );
    flush_log_queue(&state).await;
    crate::gateway::registry::invalidate_presets_cache();

    let fresh = reopen_and_read(&dir).await;
    let logs = aidog_logs::list_proxy_logs(&fresh, 10, 0).await.unwrap();
    assert_eq!(logs.len(), 1);
    let row = aidog_logs::get_proxy_log(&fresh, &logs[0].id).await.unwrap().unwrap();
    assert_eq!(row.est_cost, 0.0, "non-200 must not be billed");
    assert_eq!(row.status_code, 500, "last candidate passthrough upstream status");
}

/// 用例 4：failover 次序。声明同 path 的 deepseek（500）+ kimi（200）→ 换平台成功；
/// kimi 无 tools_pricing 声明 → est_cost 0（语义 8 兜底），model 列 = tools:search。
#[tokio::test]
async fn e2e_failover_to_second_platform() {
    let _guard = TOOLS_E2E_LOCK.lock().await;
    let (db, dir) = three_file_db().await;
    let state = make_state(db).await;
    let (stub500, _) = spawn_capture_upstream(500, r#"{"error":"down"}"#).await;
    // kimi strict 判定看 search_results（deepseek 形状的 SEARCH_OK 会被判不计费）
    let (stub200, _) = spawn_capture_upstream(200, r#"{"search_results":[{"title":"t"}]}"#).await;
    store_tools_preset_redirect(&stub500, &["deepseek"]);
    store_tools_preset_redirect(&stub200, &["kimi"]);
    let p1 = aidog_db::create_platform(&state.db, CreatePlatform {
        name: "ds".into(), platform_type: Protocol::DeepSeek, base_url: stub500.clone(),
        api_key: "sk-tools".into(), extra: String::new(), models: None,
        available_models: None, endpoints: None, manual_budgets: None, auto_group: None,
        join_group_ids: None, expires_at: None, quota_source: None,
    }).await.unwrap();
    let p2 = aidog_db::create_platform(&state.db, CreatePlatform {
        name: "kimi".into(), platform_type: Protocol::Kimi, base_url: stub200.clone(),
        api_key: "sk-tools".into(), extra: String::new(), models: None,
        available_models: None, endpoints: None, manual_budgets: None, auto_group: None,
        join_group_ids: None, expires_at: None, quota_source: None,
    }).await.unwrap();
    let group = aidog_db::create_group(&state.db, sample_group("g1", vec![])).await.unwrap();
    aidog_db::set_group_platforms(&state.db, group.id, &[
        GroupPlatformInput { platform_id: p1.id, priority: Some(0), weight: Some(1), level_priority: Some(0) },
        GroupPlatformInput { platform_id: p2.id, priority: Some(1), weight: Some(1), level_priority: Some(0) },
    ]).await.unwrap();

    let resp = handle_proxy(
        AxumState(state.clone()),
        tools_request("g1", "/v1/tools/search", r#"{"text_query":"x"}"#),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    flush_log_queue(&state).await;
    crate::gateway::registry::invalidate_presets_cache();

    let fresh = reopen_and_read(&dir).await;
    let logs = aidog_logs::list_proxy_logs(&fresh, 10, 0).await.unwrap();
    assert_eq!(logs.len(), 1);
    let row = aidog_logs::get_proxy_log(&fresh, &logs[0].id).await.unwrap().unwrap();
    assert_eq!(row.status_code, 200);
    assert_eq!(row.model, "tools:search", "key follows winning platform");
    // kimi tools_pricing 已上（票 21 落库 bc6628dee，agent 基点 da764097d 尚无）：
    // search ¥10/1000 per-call ÷ CNY_PER_USD 换算 USD。
    let kimi_expected = 10.0 / 1000.0 / crate::gateway::billing::CNY_PER_USD;
    assert!(
        (row.est_cost - kimi_expected).abs() < 1e-9,
        "kimi billed at per_1000 10 CNY, got {}",
        row.est_cost
    );
    assert_eq!(row.retry_count, 1, "first attempt failed, second succeeded");
}

/// 用例 5：组内无声明平台 → 400 + no_tools_platform 白名单 reason，落审计 proxy_log。
#[tokio::test]
async fn e2e_no_tools_platform_returns_400() {
    let _guard = TOOLS_E2E_LOCK.lock().await;
    let (db, dir) = three_file_db().await;
    let state = make_state(db).await;
    // anthropic 平台无 tools_endpoints 声明；分流仍触发（bundled registry 有该 path 声明），
    // 但组内候选被 kind 过滤全拒 → no_tools_platform。
    setup_platform_group(
        &state,
        "g1",
        "claude",
        Protocol::Anthropic,
        "",
        "http://127.0.0.1:1",
        0,
    )
    .await;

    let resp = handle_proxy(
        AxumState(state.clone()),
        tools_request("g1", "/v1/tools/search", r#"{"query":"x"}"#),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    flush_log_queue(&state).await;
    crate::gateway::registry::invalidate_presets_cache();

    let fresh = reopen_and_read(&dir).await;
    let logs = aidog_logs::list_proxy_logs(&fresh, 10, 0).await.unwrap();
    assert_eq!(logs.len(), 1);
    assert_eq!(logs[0].blocked_by, "router");
    assert_eq!(logs[0].blocked_reason, "no_tools_platform");
    let row = aidog_logs::get_proxy_log(&fresh, &logs[0].id).await.unwrap().unwrap();
    assert_eq!(row.est_cost, 0.0);
}

/// 用例 6：qianfan 例外 header 与 extra 凭据：X-Appbuilder-Authorization: Bearer
/// <extra.appbuilder_key>，不带 Authorization；body 原样透传（自带 model 不改写）。
#[tokio::test]
async fn e2e_qianfan_appbuilder_header_and_passthrough_body() {
    let _guard = TOOLS_E2E_LOCK.lock().await;
    let (db, dir) = three_file_db().await;
    let state = make_state(db).await;
    let qianfan_ok = r#"{"choices":[{"message":{"content":"answer"}}],"references":[]}"#;
    let (stub, captured) = spawn_capture_upstream(200, qianfan_ok).await;
    store_tools_preset_redirect(&stub, &["qianfan"]);
    setup_platform_group(
        &state,
        "g1",
        "qf",
        Protocol::QianFan,
        r#"{"appbuilder_key":"qbk-test-123"}"#,
        &stub,
        0,
    )
    .await;

    let req_body = r#"{"messages":[{"content":"hi","role":"user"}],"model":"test-model-x"}"#;
    let resp = handle_proxy(
        AxumState(state.clone()),
        tools_request("g1", "/v2/ai_search/chat/completions", req_body),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    flush_log_queue(&state).await;
    crate::gateway::registry::invalidate_presets_cache();

    let cap = captured.lock().unwrap().clone();
    assert_eq!(
        cap.get("appbuilder").and_then(|v| v.as_str()),
        Some("Bearer qbk-test-123"),
        "qianfan must auth via X-Appbuilder-Authorization with extra.appbuilder_key"
    );
    assert_eq!(
        cap.get("authorization").and_then(|v| v.as_str()),
        None,
        "qianfan must not send platform.api_key Authorization"
    );
    assert_eq!(
        cap.get("body").and_then(|v| v.as_str()),
        Some(req_body),
        "body must pass through verbatim (no model rewrite)"
    );

    let fresh = reopen_and_read(&dir).await;
    let logs = aidog_logs::list_proxy_logs(&fresh, 10, 0).await.unwrap();
    assert_eq!(logs.len(), 1);
    let row = aidog_logs::get_proxy_log(&fresh, &logs[0].id).await.unwrap().unwrap();
    assert_eq!(row.model, "tools:ai_search_chat");
    // qianfan 定性按次已定（票 21 落库）：ai_search_chat ¥36/1000 per-call ÷ CNY_PER_USD。
    let qf_expected = 36.0 / 1000.0 / crate::gateway::billing::CNY_PER_USD;
    assert!(
        (row.est_cost - qf_expected).abs() < 1e-9,
        "qianfan billed at per_1000 36 CNY, got {}",
        row.est_cost
    );
}

/// 用例 7：未声明 path 零回归——/v1/tools/other 不在声明集 → is_tools_endpoint false，
/// 不进 tools 分流（行为与现状一致，落聊天链既有 4xx/404 路径）。
#[tokio::test]
async fn e2e_undeclared_path_not_intercepted() {
    let _guard = TOOLS_E2E_LOCK.lock().await;
    let (db, dir) = three_file_db().await;
    let state = make_state(db).await;
    let (stub, _) = spawn_capture_upstream(200, SEARCH_OK).await;
    store_tools_preset_redirect(&stub, &["deepseek"]);
    // base_url 死端口：若误进 tools 分流会打 stub（200）；走聊天链则连不上 → 非 200。
    setup_platform_group(&state, "g1", "ds", Protocol::DeepSeek, "", "http://127.0.0.1:1", 0).await;

    let resp = handle_proxy(
        AxumState(state.clone()),
        tools_request("g1", "/v1/tools/other", r#"{"query":"x"}"#),
    )
    .await;
    assert_ne!(
        resp.status(),
        StatusCode::OK,
        "undeclared path must not reach tools forward (stub would 200)"
    );
    flush_log_queue(&state).await;
    crate::gateway::registry::invalidate_presets_cache();
    let fresh = reopen_and_read(&dir).await;
    let logs = aidog_logs::list_proxy_logs(&fresh, 10, 0).await.unwrap();
    for r in &logs {
        let row = aidog_logs::get_proxy_log(&fresh, &r.id).await.unwrap().unwrap();
        assert!(
            !row.upstream_request_url.contains(&stub),
            "no upstream call to the tools stub for undeclared path"
        );
    }
}
