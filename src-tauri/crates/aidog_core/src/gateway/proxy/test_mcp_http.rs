//! `POST /mcp` 覆盖：鉴权（401 / 403）、协议握手、tools/list、tools/call 走本地假 SearXNG
//! 并写工具日志（三文件库，错 handle 当场 no such table）、隐私开关。
use super::*;
use crate::mcp_tools::{McpToolsSettings, save_mcp_tools_settings};
use aidog_db::models::ToolLogFilter;
use aidog_db::test_support::{sample_group, three_file_db};
use aidog_middleware::MiddlewareEngine;
use axum::http::HeaderMap;
use serde_json::{Value, json};
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

async fn make_state(db: aidog_db::Db) -> Arc<ProxyState> {
    let (log_tx, _log_rx) = tokio::sync::mpsc::channel(1024);
    Arc::new(ProxyState {
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
    })
}

/// 建分组，返回 group_key。
async fn seed_group(state: &ProxyState, name: &str) -> String {
    aidog_db::create_group(&state.db, sample_group(name, vec![])).await.unwrap().group_key
}

fn bearer(token: &str) -> HeaderMap {
    let mut h = HeaderMap::new();
    h.insert("authorization", format!("Bearer {token}").parse().unwrap());
    h
}

fn ci(ip: &str) -> axum::extract::ConnectInfo<std::net::SocketAddr> {
    axum::extract::ConnectInfo(ip.parse().unwrap())
}

async fn post(state: &Arc<ProxyState>, headers: HeaderMap, ip: &str, body: Value) -> Response {
    handle_mcp(
        AxumState(state.clone()),
        headers,
        ci(ip),
        axum::body::Bytes::from(body.to_string()),
    )
    .await
}

async fn body_json(resp: Response) -> Value {
    let bytes = axum::body::to_bytes(resp.into_body(), 1 << 20).await.unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

fn rpc(id: i64, method: &str, params: Value) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params})
}

/// 本地假 SearXNG：任何请求回固定 JSON（fixtures/searxng.json 同形状）。返回 base_url。
async fn spawn_fake_searx() -> String {
    let body = r#"{"results":[{"title":"Rust Lang","url":"https://www.rust-lang.org/","content":"A language"},{"title":"Tokio","url":"https://tokio.rs/","content":"async runtime"}]}"#;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move {
        loop {
            let Ok((mut sock, _)) = listener.accept().await else { return };
            tokio::spawn(async move {
                let mut buf = [0u8; 8192];
                let _ = sock.read(&mut buf).await;
                let resp = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.len(),
                    body
                );
                let _ = sock.write_all(resp.as_bytes()).await;
                let _ = sock.shutdown().await;
            });
        }
    });
    format!("http://127.0.0.1:{port}")
}

/// 只开 searxng 引擎、指向假实例（其余免 key 引擎全关，禁真实外呼）。
async fn use_fake_searx(state: &ProxyState, base: &str) {
    let mut s = McpToolsSettings::default();
    s.search.sources = vec!["searxng".into()];
    s.search.searxng_base_urls = vec![base.to_string()];
    save_mcp_tools_settings(&state.db, &s).await.unwrap();
}

async fn set_log_user_request(state: &ProxyState, on: bool) {
    aidog_db::set_setting(
        &state.db,
        crate::gateway::models::SetSettingInput {
            scope: "proxy".into(),
            key: "logging".into(),
            value: json!({"log_user_request": on}),
        },
    )
    .await
    .unwrap();
}

#[tokio::test]
async fn missing_token_401_and_unknown_token_401() {
    let (db, _d) = three_file_db().await;
    let state = make_state(db).await;
    let r = post(&state, HeaderMap::new(), "127.0.0.1:50000", rpc(1, "ping", json!({}))).await;
    assert_eq!(r.status(), StatusCode::UNAUTHORIZED);
    let r = post(&state, bearer("gk_nope"), "127.0.0.1:50000", rpc(1, "ping", json!({}))).await;
    assert_eq!(r.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn non_loopback_is_403_even_with_valid_token() {
    let (db, _d) = three_file_db().await;
    let state = make_state(db).await;
    let gk = seed_group(&state, "g0").await;
    let r = post(&state, bearer(&gk), "192.168.1.5:50000", rpc(1, "ping", json!({}))).await;
    assert_eq!(r.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn initialize_list_and_notification() {
    let (db, _d) = three_file_db().await;
    let state = make_state(db).await;
    let gk = seed_group(&state, "g0").await;

    let r = post(&state, bearer(&gk), "127.0.0.1:50000", rpc(1, "initialize", json!({"protocolVersion":"2025-06-18"}))).await;
    assert_eq!(r.status(), StatusCode::OK);
    assert!(r.headers()["content-type"].to_str().unwrap().starts_with("application/json"));
    let v = body_json(r).await;
    assert_eq!(v["result"]["protocolVersion"], "2025-06-18");
    assert_eq!(v["result"]["serverInfo"]["name"], "aidog");

    let r = post(&state, bearer(&gk), "127.0.0.1:50000", rpc(2, "tools/list", json!({}))).await;
    let v = body_json(r).await;
    let mut names: Vec<String> = v["result"]["tools"].as_array().unwrap().iter().map(|t| t["name"].as_str().unwrap().to_string()).collect();
    names.sort();
    assert_eq!(names, ["generate_image", "web_fetch", "web_search"]);

    // 通知（无 id）→ 202 空体。
    let r = post(&state, bearer(&gk), "127.0.0.1:50000", json!({"jsonrpc":"2.0","method":"notifications/initialized"})).await;
    assert_eq!(r.status(), StatusCode::ACCEPTED);
}

#[tokio::test]
async fn malformed_body_is_parse_error() {
    let (db, _d) = three_file_db().await;
    let state = make_state(db).await;
    let gk = seed_group(&state, "g0").await;
    let r = handle_mcp(AxumState(state), bearer(&gk), ci("127.0.0.1:1"), axum::body::Bytes::from("{not json")).await;
    assert_eq!(r.status(), StatusCode::OK);
    assert_eq!(body_json(r).await["error"]["code"], -32700);
}

#[tokio::test]
async fn tools_call_web_search_hits_fake_engine_and_writes_tool_log() {
    let (db, _d) = three_file_db().await;
    let state = make_state(db).await;
    let gk = seed_group(&state, "team-a").await;
    use_fake_searx(&state, &spawn_fake_searx().await).await;
    set_log_user_request(&state, true).await;

    let r = post(&state, bearer(&gk), "127.0.0.1:50000",
        rpc(3, "tools/call", json!({"name":"web_search","arguments":{"query":"rust async"}}))).await;
    let v = body_json(r).await;
    let text = v["result"]["content"][0]["text"].as_str().unwrap();
    assert!(text.contains("https://tokio.rs/"), "text={text}");
    assert!(v["result"]["isError"].is_null());

    let page = aidog_db::list_tool_logs(&state.db, &ToolLogFilter::default(), 10, 0).await.unwrap();
    assert_eq!(page.total, 1);
    let row = &page.rows[0];
    assert_eq!((row.tool.as_str(), row.trigger.as_str(), row.group_name.as_str()), ("web_search", "mcp", "team-a"));
    assert_eq!(row.query, "rust async");
    assert_eq!(row.result_count, 2);
    assert!(row.success, "error={}", row.error);
    let engines: Value = serde_json::from_str(&row.engines).unwrap();
    assert_eq!(engines[0]["engine"], "searxng");
    assert_eq!(engines[0]["count"], 2);
    let d = aidog_db::get_tool_log(&state.db, row.id).await.unwrap().unwrap();
    assert!(d.result.contains("tokio.rs"));
}

#[tokio::test]
async fn privacy_off_blanks_query_and_result_but_keeps_metadata() {
    let (db, _d) = three_file_db().await;
    let state = make_state(db).await;
    let gk = seed_group(&state, "g0").await;
    use_fake_searx(&state, &spawn_fake_searx().await).await;
    // log_user_request 缺省 false。
    post(&state, bearer(&gk), "127.0.0.1:50000",
        rpc(3, "tools/call", json!({"name":"web_search","arguments":{"query":"secret term"}}))).await;
    let page = aidog_db::list_tool_logs(&state.db, &ToolLogFilter::default(), 10, 0).await.unwrap();
    let row = &page.rows[0];
    assert_eq!(row.query, "");
    assert_eq!(row.result_count, 2, "条数是元数据，恒记录");
    assert!(row.engines.contains("searxng"));
    let d = aidog_db::get_tool_log(&state.db, row.id).await.unwrap().unwrap();
    assert_eq!(d.result, "");
}

#[tokio::test]
async fn failed_tool_call_logs_error_row() {
    let (db, _d) = three_file_db().await;
    let state = make_state(db).await;
    let gk = seed_group(&state, "g0").await;
    // 全部引擎关闭 → 搜索失败。
    let mut s = McpToolsSettings::default();
    s.search.sources = vec![];
    save_mcp_tools_settings(&state.db, &s).await.unwrap();
    let r = post(&state, bearer(&gk), "127.0.0.1:50000",
        rpc(4, "tools/call", json!({"name":"web_search","arguments":{"query":"q"}}))).await;
    let v = body_json(r).await;
    assert_eq!(v["result"]["isError"], true);
    let row = &aidog_db::list_tool_logs(&state.db, &ToolLogFilter::default(), 10, 0).await.unwrap().rows[0];
    assert!(!row.success);
    assert!(!row.error.is_empty());
}

/// 路由接线：POST /mcp 走 build_router 到 handler；GET /mcp 405。
#[tokio::test]
async fn router_wires_post_and_get() {
    use tower::ServiceExt;
    let (db, _d) = three_file_db().await;
    let state = make_state(db).await;
    let gk = seed_group(&state, "g0").await;
    let app = super::build_router(state);
    let mut req = axum::http::Request::builder()
        .method("POST")
        .uri("/mcp")
        .header("authorization", format!("Bearer {gk}"))
        .body(axum::body::Body::from(rpc(1, "ping", json!({})).to_string()))
        .unwrap();
    req.extensions_mut().insert(ci("127.0.0.1:1"));
    let r = app.clone().oneshot(req).await.unwrap();
    assert_eq!(r.status(), StatusCode::OK);
    let mut req = axum::http::Request::builder().method("GET").uri("/mcp").body(axum::body::Body::empty()).unwrap();
    req.extensions_mut().insert(ci("127.0.0.1:1"));
    assert_eq!(app.oneshot(req).await.unwrap().status(), StatusCode::METHOD_NOT_ALLOWED);
}
