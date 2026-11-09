//! 网关代执行网页搜索 / 抓网页（票 04）：经 `handle_proxy` 全链路，上游与搜索引擎均为本地假服务。
use super::*;
use crate::gateway::models::{CreatePlatform, GroupPlatformInput, Protocol};
use crate::mcp_tools::{McpToolsSettings, save_mcp_tools_settings};
use aidog_db::models::ToolLogFilter;
use aidog_db::test_support::{sample_group, three_file_db};
use aidog_middleware::MiddlewareEngine;
use axum::body::Body;
use axum::http::Request as HttpRequest;
use serde_json::{Value, json};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

const SEARX: &str = r#"{"results":[{"title":"Rust Lang","url":"https://www.rust-lang.org/learn","content":"A language"},{"title":"Tokio","url":"https://tokio.rs/","content":"async runtime"}]}"#;
const CC_PROMPT: &str = "Perform a web search for the query: rust async";

/// 本地假服务：任何请求回固定 (status, ctype, body)，`hits` 计请求数。返回 base_url。
async fn spawn_fake(status: u16, ctype: &'static str, body: &'static str) -> (String, Arc<AtomicUsize>) {
    let hits = Arc::new(AtomicUsize::new(0));
    let h = hits.clone();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move {
        loop {
            let Ok((mut sock, _)) = listener.accept().await else { return };
            h.fetch_add(1, Ordering::SeqCst);
            tokio::spawn(async move {
                let mut buf = [0u8; 8192];
                let _ = sock.read(&mut buf).await;
                let resp = format!(
                    "HTTP/1.1 {status} X\r\nContent-Type: {ctype}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = sock.write_all(resp.as_bytes()).await;
                let _ = sock.shutdown().await;
            });
        }
    });
    (format!("http://127.0.0.1:{port}"), hits)
}

async fn make_state(db: aidog_db::Db) -> Arc<ProxyState> {
    let (log_tx, log_rx) = tokio::sync::mpsc::channel(1024);
    let state = Arc::new(ProxyState {
        db: Arc::new(db),
        middleware: Arc::new(MiddlewareEngine::new()),
        scheduler: Arc::new(crate::gateway::scheduling::SchedulerState::new()),
        sticky: Arc::new(crate::gateway::scheduling::StickyTable::new()),
        log_snapshots: dashmap::DashMap::new(),
        agg_done: std::sync::Mutex::new((std::collections::VecDeque::new(), std::collections::HashSet::new())),
        listen_addr: std::sync::OnceLock::new(),
        settings_cache: Arc::new(tokio::sync::RwLock::new(Default::default())),
        log_tx,
        log_queue_bytes: std::sync::atomic::AtomicU64::new(0),
    });
    spawn_log_writer(state.clone(), log_rx);
    state
}

/// 非官方 Anthropic 平台（base_url = 假上游）+ 分组；返回 (group_key, 分组名)。
async fn setup(state: &Arc<ProxyState>, upstream: &str) -> (String, String) {
    let plat = aidog_db::create_platform(
        &state.db,
        CreatePlatform {
            name: "stub".into(),
            platform_type: Protocol::Anthropic,
            base_url: upstream.to_string(),
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
    .unwrap();
    let group = aidog_db::create_group(&state.db, sample_group("gweb", vec![])).await.unwrap();
    aidog_db::set_group_platforms(
        &state.db,
        group.id,
        &[GroupPlatformInput { platform_id: plat.id, priority: Some(0), weight: Some(1), level_priority: Some(0) }],
    )
    .await
    .unwrap();
    (group.group_key, group.name)
}

/// 只开 searxng 引擎、指向假实例（其余免 key 引擎全关，禁真实外呼）。
async fn use_fake_searx(state: &ProxyState, base: &str) {
    let mut s = McpToolsSettings::default();
    for e in aidog_search::engines::ENGINES {
        s.search.engines.insert(e.id.to_string(), e.id == "searxng");
    }
    s.search.searxng_base_urls = vec![base.to_string()];
    save_mcp_tools_settings(&state.db, &s).await.unwrap();
}

async fn set_log_user_request(state: &ProxyState, on: bool) {
    aidog_db::set_setting(
        &state.db,
        crate::gateway::models::SetSettingInput { scope: "proxy".into(), key: "logging".into(), value: json!({"log_user_request": on}) },
    )
    .await
    .unwrap();
}

fn req_body(tool: Value, text: &str, stream: bool) -> String {
    json!({"model":"claude-3","max_tokens":64,"stream":stream,"tools":[tool],
        "tool_choice":{"type":"tool","name":"web_search"},
        "messages":[{"role":"user","content":text}]})
    .to_string()
}

fn search_tool() -> Value {
    json!({"type":"web_search_20250305","name":"web_search","max_uses":8})
}

async fn send(state: &Arc<ProxyState>, gk: &str, body: String) -> Response {
    let req = HttpRequest::builder()
        .method("POST")
        .uri("/v1/messages")
        .header("authorization", format!("Bearer {gk}"))
        .header("content-type", "application/json")
        .body(Body::from(body))
        .unwrap();
    handle_proxy(AxumState(state.clone()), req).await
}

async fn json_of(r: Response) -> Value {
    serde_json::from_slice(&axum::body::to_bytes(r.into_body(), 1 << 20).await.unwrap()).unwrap()
}

/// 代理日志无行、工具日志的一页（先 flush 日志队列，保证此前入队的 proxy_log 写都已落库）。
async fn proxy_log_count_and_tool_rows(state: &Arc<ProxyState>) -> (usize, Vec<aidog_db::models::ToolLog>) {
    super::log::flush_log_queue(state).await;
    let n = aidog_logs::list_proxy_logs(&state.db, 50, 0).await.unwrap().len();
    let rows = aidog_db::list_tool_logs(&state.db, &ToolLogFilter::default(), 10, 0).await.unwrap().rows;
    (n, rows)
}

#[tokio::test]
async fn nonstream_takeover_blocks_usage_tool_log_no_proxy_log() {
    let (db, _d) = three_file_db().await;
    let state = make_state(db).await;
    let (up, up_hits) = spawn_fake(200, "application/json", "{}").await;
    let (searx, _) = spawn_fake(200, "application/json", SEARX).await;
    let (gk, group_name) = setup(&state, &up).await;
    use_fake_searx(&state, &searx).await;
    set_log_user_request(&state, true).await;

    let resp = send(&state, &gk, req_body(search_tool(), CC_PROMPT, false)).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let v = json_of(resp).await;
    let c = v["content"].as_array().unwrap();
    assert_eq!(c[0]["type"], "server_tool_use");
    assert_eq!(c[0]["name"], "web_search");
    assert_eq!(c[0]["input"]["query"], "rust async");
    assert!(c[0]["id"].as_str().unwrap().starts_with("srvtoolu_"));
    assert_eq!(c[1]["type"], "web_search_tool_result");
    assert_eq!(c[1]["tool_use_id"], c[0]["id"]);
    let items = c[1]["content"].as_array().unwrap();
    assert_eq!(items.len(), 2);
    assert_eq!(items[0]["type"], "web_search_result");
    assert_eq!(items[0]["title"], "Rust Lang");
    assert_eq!(items[0]["url"], "https://www.rust-lang.org/learn");
    assert_eq!(c[2]["type"], "text");
    assert!(c[2]["text"].as_str().unwrap().contains("tokio.rs"));
    assert_eq!(v["role"], "assistant");
    assert_eq!(v["stop_reason"], "end_turn");
    assert_eq!(v["usage"]["server_tool_use"]["web_search_requests"], 1);
    assert_eq!(v["usage"]["input_tokens"], 0);
    assert_eq!(v["usage"]["output_tokens"], 0);

    assert_eq!(up_hits.load(Ordering::SeqCst), 0, "不得调用上游模型");
    let (proxy_rows, tool_rows) = proxy_log_count_and_tool_rows(&state).await;
    assert_eq!(proxy_rows, 0, "代执行不写代理日志");
    assert_eq!(tool_rows.len(), 1);
    let t = &tool_rows[0];
    assert_eq!((t.tool.as_str(), t.trigger.as_str(), t.platform.as_str()), ("web_search", "gateway", "stub"));
    assert_eq!(t.group_name, group_name);
    assert_eq!((t.query.as_str(), t.result_count, t.success), ("rust async", 2, true));
}

#[tokio::test]
async fn stream_takeover_sse_event_sequence() {
    let (db, _d) = three_file_db().await;
    let state = make_state(db).await;
    let (up, up_hits) = spawn_fake(200, "application/json", "{}").await;
    let (searx, _) = spawn_fake(200, "application/json", SEARX).await;
    let (gk, _) = setup(&state, &up).await;
    use_fake_searx(&state, &searx).await;

    let resp = send(&state, &gk, req_body(search_tool(), CC_PROMPT, true)).await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert!(resp.headers()["content-type"].to_str().unwrap().starts_with("text/event-stream"));
    let raw = String::from_utf8(axum::body::to_bytes(resp.into_body(), 1 << 20).await.unwrap().to_vec()).unwrap();
    let events: Vec<(String, Value)> = raw
        .split("\n\n")
        .filter(|b| !b.trim().is_empty())
        .map(|b| {
            let (e, d) = b.split_once('\n').unwrap();
            (e.strip_prefix("event: ").unwrap().to_string(), serde_json::from_str(d.strip_prefix("data: ").unwrap()).unwrap())
        })
        .collect();
    let names: Vec<&str> = events.iter().map(|(e, _)| e.as_str()).collect();
    assert_eq!(
        names,
        [
            "message_start",
            "content_block_start",
            "content_block_delta",
            "content_block_stop",
            "content_block_start",
            "content_block_stop",
            "content_block_start",
            "content_block_delta",
            "content_block_stop",
            "message_delta",
            "message_stop"
        ]
    );
    assert!(events.iter().all(|(e, d)| d["type"] == *e));
    assert_eq!(events[1].1["content_block"]["type"], "server_tool_use");
    assert_eq!(events[1].1["content_block"]["input"], json!({}));
    let partial = events[2].1["delta"]["partial_json"].as_str().unwrap();
    assert_eq!(serde_json::from_str::<Value>(partial).unwrap()["query"], "rust async");
    assert_eq!(events[4].1["index"], 1);
    assert_eq!(events[4].1["content_block"]["type"], "web_search_tool_result");
    assert_eq!(events[4].1["content_block"]["content"].as_array().unwrap().len(), 2);
    assert_eq!(events[6].1["content_block"]["type"], "text");
    assert_eq!(events[9].1["delta"]["stop_reason"], "end_turn");
    assert_eq!(events[9].1["usage"]["server_tool_use"]["web_search_requests"], 1);
    assert_eq!(up_hits.load(Ordering::SeqCst), 0);
    assert_eq!(proxy_log_count_and_tool_rows(&state).await.0, 0);
}

#[tokio::test]
async fn domain_filters_apply_and_all_filtered_gives_explained_empty_result() {
    let (db, _d) = three_file_db().await;
    let state = make_state(db).await;
    let (up, _) = spawn_fake(200, "application/json", "{}").await;
    let (searx, _) = spawn_fake(200, "application/json", SEARX).await;
    let (gk, _) = setup(&state, &up).await;
    use_fake_searx(&state, &searx).await;
    let with = |k: &str, v: Value| {
        let mut t = search_tool();
        t[k] = v;
        t
    };
    // 白名单（含子域）：www.rust-lang.org 被 rust-lang.org 覆盖，tokio.rs 被滤
    let v = json_of(send(&state, &gk, req_body(with("allowed_domains", json!(["rust-lang.org"])), CC_PROMPT, false)).await).await;
    let urls: Vec<&str> = v["content"][1]["content"].as_array().unwrap().iter().map(|r| r["url"].as_str().unwrap()).collect();
    assert_eq!(urls, ["https://www.rust-lang.org/learn"]);
    // 带路径的白名单按前缀匹配
    let v = json_of(send(&state, &gk, req_body(with("allowed_domains", json!(["rust-lang.org/learn"])), CC_PROMPT, false)).await).await;
    assert_eq!(v["content"][1]["content"].as_array().unwrap().len(), 1);
    let v = json_of(send(&state, &gk, req_body(with("allowed_domains", json!(["rust-lang.org/other"])), CC_PROMPT, false)).await).await;
    assert_eq!(v["content"][1]["content"].as_array().unwrap().len(), 0);
    // 黑名单
    let v = json_of(send(&state, &gk, req_body(with("blocked_domains", json!(["tokio.rs"])), CC_PROMPT, false)).await).await;
    let urls: Vec<&str> = v["content"][1]["content"].as_array().unwrap().iter().map(|r| r["url"].as_str().unwrap()).collect();
    assert_eq!(urls, ["https://www.rust-lang.org/learn"]);
    // 全被滤掉：空结果数组（不是错误）+ 说明文字
    let v = json_of(send(&state, &gk, req_body(with("allowed_domains", json!(["nomatch.example"])), CC_PROMPT, false)).await).await;
    assert_eq!(v["content"][1]["type"], "web_search_tool_result");
    assert_eq!(v["content"][1]["content"], json!([]));
    assert!(v["content"][2]["text"].as_str().unwrap().contains("domain"));
    assert_eq!(v["usage"]["server_tool_use"]["web_search_requests"], 1);
}

#[tokio::test]
async fn all_engines_fail_returns_unavailable_error_block_not_http_error() {
    let (db, _d) = three_file_db().await;
    let state = make_state(db).await;
    let (up, _) = spawn_fake(200, "application/json", "{}").await;
    let (searx, _) = spawn_fake(500, "text/plain", "boom").await;
    let (gk, _) = setup(&state, &up).await;
    use_fake_searx(&state, &searx).await;

    for stream in [false, true] {
        let resp = send(&state, &gk, req_body(search_tool(), CC_PROMPT, stream)).await;
        assert_eq!(resp.status(), StatusCode::OK);
        let raw = axum::body::to_bytes(resp.into_body(), 1 << 20).await.unwrap();
        let raw = String::from_utf8(raw.to_vec()).unwrap();
        assert!(raw.contains("web_search_tool_result_error") && raw.contains("unavailable"), "{raw}");
    }
    let (_, rows) = proxy_log_count_and_tool_rows(&state).await;
    assert_eq!(rows.len(), 2);
    assert!(rows.iter().all(|r| !r.success && r.trigger == "gateway"));
}

#[tokio::test]
async fn web_fetch_takeover_returns_fetch_result_block() {
    let (db, _d) = three_file_db().await;
    let state = make_state(db).await;
    let (up, up_hits) = spawn_fake(200, "application/json", "{}").await;
    let (page, _) = spawn_fake(200, "text/html", "<html><body><p>hello fetched page</p></body></html>").await;
    let (gk, _) = setup(&state, &up).await;
    let body = json!({"model":"claude-3","max_tokens":64,"tools":[{"type":"web_fetch_20250910","name":"web_fetch"}],
        "messages":[{"role":"user","content":[{"type":"text","text":format!("Fetch {page}/doc and summarize")}]}]})
    .to_string();
    let v = json_of(send(&state, &gk, body).await).await;
    let c = v["content"].as_array().unwrap();
    assert_eq!(c[0]["name"], "web_fetch");
    assert_eq!(c[0]["input"]["url"], format!("{page}/doc"));
    assert_eq!(c[1]["type"], "web_fetch_tool_result");
    assert_eq!(c[1]["content"]["type"], "web_fetch_result");
    assert!(c[1]["content"]["content"]["source"]["data"].as_str().unwrap().contains("hello fetched page"));
    assert_eq!(v["usage"]["server_tool_use"]["web_fetch_requests"], 1);
    assert_eq!(up_hits.load(Ordering::SeqCst), 0);
    let (proxy_rows, rows) = proxy_log_count_and_tool_rows(&state).await;
    assert_eq!((proxy_rows, rows.len(), rows[0].tool.as_str()), (0, 1, "web_fetch"));
}

#[test]
fn official_anthropic_platform_is_not_taken_over() {
    assert!(!takes_over("https://api.anthropic.com"));
    assert!(!takes_over("https://API.anthropic.com:443/v1"));
    assert!(takes_over("https://open.bigmodel.cn/api/anthropic"));
    assert!(takes_over("http://127.0.0.1:9000"));
}

#[test]
fn detect_and_extract_rules() {
    let body: Value = serde_json::from_str(&req_body(search_tool(), CC_PROMPT, false)).unwrap();
    assert_eq!(detect(&body).unwrap().kind, Kind::Search);
    assert_eq!(last_user_text(&body), CC_PROMPT);
    assert_eq!(extract_query(CC_PROMPT), "rust async");
    assert_eq!(extract_query("  plain words "), "plain words");
    assert_eq!(extract_url("see (https://a.com/x.html). ok").as_deref(), Some("https://a.com/x.html"));
    assert_eq!(extract_url("no link"), None);
    // 无服务端网页工具 / 仅客户端工具：不接管
    let plain = json!({"tools":[{"name":"Bash","input_schema":{}}],"messages":[]});
    assert!(detect(&plain).is_none() && !has_web_server_tool(Some(&plain)));
    // web_fetch 带路径的域名条目恒不匹配
    let f = WebTool { kind: Kind::Fetch, allowed: vec!["a.com/x".into()], blocked: vec![] };
    assert!(!f.allows("https://a.com/x"));
    let f = WebTool { kind: Kind::Fetch, allowed: vec!["a.com".into()], blocked: vec![] };
    assert!(f.allows("https://docs.a.com/x") && !f.allows("https://b.com"));
}
