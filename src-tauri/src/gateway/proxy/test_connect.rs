//! CONNECT 隧道自检（ponytail：非平凡分支逻辑留一个最小可运行检查）。
//! 覆盖 host 匹配命中 / 未命中 + upsert_connect_log 落行（source_protocol=http-connect）。
//! + 根因回归门：authority-form URI target 解析（修复前 path() 返空 → 502，修复后 200）。
use super::*;
use crate::gateway::db::test_support;
use crate::gateway::middleware::MiddlewareEngine;
use crate::gateway::models::{CreatePlatform, Protocol};
use axum::body::Body;
use axum::http::Request as HttpRequest;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

/// 测试用 ProxyState（内存 DB）构造器。
async fn make_state() -> Arc<ProxyState> {
    let db = test_support::test_db().await;
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

/// match_platform_by_host：主 base_url host 命中 → 返回 platform_id；未命中 → None。
#[tokio::test]
async fn match_platform_by_host_hits_main_base_url() {
    let db = test_support::test_db().await;
    // 平台 base_url host = api.test-connect-hit.example
    let p = crate::gateway::db::create_platform(&db, CreatePlatform {
        name: "conn-hit".into(),
        platform_type: Protocol::Anthropic,
        base_url: "https://api.test-connect-hit.example/v1".into(),
        api_key: "sk-test".into(),
        extra: String::new(),
        models: None,
        available_models: None,
        endpoints: None,
        manual_budgets: None,
        auto_group: None,
        join_group_ids: None,
        default_level_priority: None,
        expires_at: None,
    }).await.expect("create platform");

    let hit = endpoint::match_platform_by_host(&db, "api.test-connect-hit.example").await;
    assert_eq!(hit, Some(p.id), "CONNECT host 命中平台主 base_url 必须返回 platform_id");

    let miss = endpoint::match_platform_by_host(&db, "api.does-not-exist.example").await;
    assert!(miss.is_none(), "未命中任何平台 base_url host 必须返回 None");
}

/// upsert_connect_log：落一行 proxy_log，source_protocol=http-connect + tokens/cost=0。
/// 关键不变量：不走 upsert_log（不污染 stats_agg），字段语义正确。
#[tokio::test]
async fn upsert_connect_log_writes_http_connect_row() {
    let db = test_support::test_db().await;
    let state = Arc::new(ProxyState {
        db: Arc::new(db),
        app: None,
        middleware: Arc::new(crate::gateway::middleware::MiddlewareEngine::default()),
        scheduler: Arc::new(crate::gateway::scheduling::SchedulerState::new()),
        sticky: Arc::new(crate::gateway::scheduling::StickyTable::new()),
        log_snapshots: std::sync::Mutex::new(std::collections::HashMap::new()),
        agg_done: std::sync::Mutex::new((std::collections::VecDeque::new(), std::collections::HashSet::new())),
    });

    log::upsert_connect_log(
        &state, "conn-log-1".into(), String::new(), 0,
        "api.example.com:443".into(), 200, 42,
    ).await;

    let row = crate::gateway::db::get_proxy_log(&state.db, "conn-log-1").await
        .expect("query proxy_log").expect("row must exist");
    assert_eq!(row.source_protocol, "http-connect", "source_protocol 标记隧道");
    assert_eq!(row.target_protocol, "http-connect");
    assert_eq!(row.platform_id, 0, "未命中 → platform_id=0");
    assert_eq!(row.status_code, 200);
    assert_eq!(row.duration_ms, 42);
    assert_eq!(row.input_tokens, 0, "隧道不计费");
    assert_eq!(row.est_cost, 0.0);
    assert_eq!(row.request_url, "api.example.com:443");
}

/// L1 根因回归门：CONNECT authority-form URI（`host:port`），http crate path() 返空、
/// authority 在 uri().authority()。修复前 path() → target="" → connect("") 必败 502；
/// 修复后多源兜底（authority）→ target 非空 → TCP 建连成功 → 200。
#[tokio::test]
async fn connect_authority_form_resolves_target() {
    // mock 上游：accept 即可（handle_connect 只需 connect 成功建链路，upgrade 在 spawn task 内 pending）。
    let upstream = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let upstream_addr = upstream.local_addr().unwrap();
    tokio::spawn(async move {
        while upstream.accept().await.is_ok() {}
    });

    let state = make_state().await;
    // authority-form URI：http crate 解析后 path 空、authority=`host:port`。
    let req = HttpRequest::builder()
        .method("CONNECT")
        .uri(format!("{upstream_addr}"))
        .body(Body::empty())
        .unwrap();
    let resp = connect::handle_connect(AxumState(state), req).await;
    assert_eq!(
        resp.status(),
        StatusCode::OK,
        "authority-form CONNECT 必须解析出 target 并建连成功（修复前会 502）"
    );
}

/// L1 三源全空早返 400：path/authority/Host 全空 = 客户端坏请求，不进 connect 路径（非 502）。
#[tokio::test]
async fn connect_triple_source_empty_returns_400() {
    let state = make_state().await;
    // uri="/" → path="/" trim_start_matches('/')="" ; authority 空 ; 无 Host header → target="" → 400。
    let req = HttpRequest::builder()
        .method("CONNECT")
        .uri("/")
        .body(Body::empty())
        .unwrap();
    let resp = connect::handle_connect(AxumState(state), req).await;
    assert_eq!(
        resp.status(),
        StatusCode::BAD_REQUEST,
        "三源全空必须早返 400（不进 connect 路径，不 connect(\"\") 必败 502）"
    );
}

/// L2 端到端：真 axum proxy + 裸 TcpStream 发 CONNECT（模拟客户端配 HTTP_PROXY 后发 HTTPS 的握手）。
/// 覆盖用户场景：客户端配 proxy env → 发 CONNECT → aidog 解 target → 建隧道返 200。
/// 修复前 6 连发 502（target 空），修复后 200。验隧道建后字节双向透传（echo 上游）。
#[tokio::test]
async fn connect_tunnel_via_real_proxy_env() {
    // 1. mock 上游 echo TCP server（双向回显，验隧道建后字节透传）。
    let upstream = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let upstream_addr = upstream.local_addr().unwrap();
    tokio::spawn(async move {
        while let Ok((mut s, _)) = upstream.accept().await {
            tokio::spawn(async move {
                // ponytail: 单流 echo（读后写回同 fd），最简字节透传验；copy(&mut s,&mut s) 双借失败用小循环。
                let mut buf = [0u8; 64];
                loop {
                    match s.read(&mut buf).await {
                        Ok(0) | Err(_) => break,
                        Ok(n) => {
                            if s.write_all(&buf[..n]).await.is_err() {
                                break;
                            }
                        }
                    }
                }
            });
        }
    });

    // 2. aidog proxy axum server（复用 start_proxy 的 Router 骨架，bind 0 端口）。
    let state = make_state().await;
    let app = axum::Router::new()
        .route("/", axum::routing::get(handle_root))
        .route("/proxy", axum::routing::get(handle_root))
        .fallback(handle_proxy)
        .with_state(state);
    let proxy_listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let proxy_addr = proxy_listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(proxy_listener, app).await.ok();
    });

    // 3. 裸 TcpStream 发 CONNECT（模拟 reqwest/HTTP 客户端配 HTTP_PROXY 后发 HTTPS 的 CONNECT 握手）。
    let mut sock = tokio::net::TcpStream::connect(proxy_addr).await.unwrap();
    let req_line = format!(
        "CONNECT {upstream} HTTP/1.1\r\nHost: {upstream}\r\n\r\n",
        upstream = upstream_addr
    );
    sock.write_all(req_line.as_bytes()).await.unwrap();

    // 4. 读 CONNECT 响应首行 → 断言 200（隧道建立；修复前会 502）。
    let mut buf = [0u8; 256];
    let n = sock.read(&mut buf).await.unwrap();
    let resp = String::from_utf8_lossy(&buf[..n]);
    assert!(
        resp.starts_with("HTTP/1.1 200") || resp.starts_with("HTTP/1.0 200"),
        "CONNECT 隧道必须建立返 200（修复前 6 连发 502），实际: {resp}"
    );

    // 5. 隧道建后续写 ping → 读回 ping（验 upgrade 后字节双向透传；echo 上游回显）。
    sock.write_all(b"ping").await.unwrap();
    let mut echo = [0u8; 4];
    let _ = sock.read_exact(&mut echo).await;
    assert_eq!(&echo, b"ping", "隧道建后字节必须双向透传（echo 上游回显）");
}

// ── ST4 MITM 分流判定单测 ──────────────────────────────────────────────────────

/// ST4 验收：mitm_candidate 分流判定 = 白名单命中 && 非 suspect。
/// 覆盖三分支：白名单命中且非 suspect → MITM 候选（true）/ 白名单未命中 → blind_relay
/// （false）/ suspect 标记后 → 降级 blind_relay（false）。
///
/// design §4 分流判定 + 失败模式表第 3 行（pinning_suspect → 后续降级）。
/// ponytail: handle_connect 含 axum spawn + TLS，端到端测过重；抽核心判定逻辑
/// （matches_db && !is_suspect）单测，验三分支组合行为正确。
/// test_db 默认 seed `*.anthropic.com` + `*.openai.com`（schema_late seed_default_whitelist），
/// 故 `api.anthropic.com` 命中白名单、`api.unknown.example` 不命中。
#[tokio::test]
async fn connect_mitm_route_split_whitelist_and_suspect() {
    use crate::gateway::db::test_support;
    use crate::gateway::mitm::mitm_state;
    use crate::gateway::mitm::whitelist::matches_db;

    let db = test_support::test_db().await;

    // 1. 白名单命中 + 非 suspect → MITM 候选 true（走 MITM 路径）。
    let hit_anthropic = matches_db(&db, "api.anthropic.com").await;
    assert!(hit_anthropic, "test_db 默认 seed *.anthropic.com，必命中");
    let suspect_anthropic = mitm_state().is_suspect("api.anthropic.com").await;
    assert!(!suspect_anthropic, "未标记 host is_suspect 必 false");
    let candidate_mitm = hit_anthropic && !suspect_anthropic;
    assert!(candidate_mitm, "白名单命中 && 非 suspect → MITM 候选 true");

    // 2. 白名单未命中 → blind_relay（candidate false，无论 suspect）。
    let miss_unknown = matches_db(&db, "api.unknown.example").await;
    assert!(!miss_unknown, "未 seed 的 host 必不命中白名单");
    let candidate_blind = miss_unknown && !mitm_state().is_suspect("api.unknown.example").await;
    assert!(!candidate_blind, "白名单未命中 → mitm_candidate 必 false（走 P1 blind_relay）");

    // 3. suspect 标记后 → 即便白名单命中也必 false（降级 blind_relay，design 失败模式表第 3 行）。
    //    用 anthropic host 模拟 pinning fail 后 mark_suspect：后续 candidate 必 false。
    mitm_state().mark_suspect("api.anthropic.com".into()).await;
    let suspect_after_mark = mitm_state().is_suspect("api.anthropic.com").await;
    assert!(suspect_after_mark, "mark_suspect 后 is_suspect 必返 true");
    let candidate_degraded = hit_anthropic && !suspect_after_mark;
    assert!(
        !candidate_degraded,
        "suspect 标记后即便白名单命中 mitm_candidate 必 false（降级 blind_relay）"
    );
}

/// ST4 验收：真实 axum proxy + 非 MITM 候选 host → P1 blind_relay 仍正常建隧道返 200
/// （不因 MITM 分流引入回归）。
///
/// 覆盖 design §4 「白名单未命中 → P1 blind_relay」路径：非候选 host 走 spawn_blind_relay
/// 完整路径（spawn 前 TCP 验证 + 502 早返 + 隧道桥接），验 ST4 分流不破 P1 行为。
#[tokio::test]
async fn connect_mitm_non_candidate_blind_relay_no_regression() {
    // mock 上游 echo TCP server。
    let upstream = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let upstream_addr = upstream.local_addr().unwrap();
    tokio::spawn(async move {
        while let Ok((mut s, _)) = upstream.accept().await {
            tokio::spawn(async move {
                let mut buf = [0u8; 64];
                loop {
                    match s.read(&mut buf).await {
                        Ok(0) | Err(_) => break,
                        Ok(n) => {
                            if s.write_all(&buf[..n]).await.is_err() {
                                break;
                            }
                        }
                    }
                }
            });
        }
    });

    let state = make_state().await;
    let app = axum::Router::new()
        .route("/", axum::routing::get(handle_root))
        .route("/proxy", axum::routing::get(handle_root))
        .fallback(handle_proxy)
        .with_state(state);
    let proxy_listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let proxy_addr = proxy_listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(proxy_listener, app).await.ok();
    });

    // 裸 TcpStream 发 CONNECT（白名单空 → 走 P1 blind_relay 路径，不进 MITM 分流）。
    let mut sock = tokio::net::TcpStream::connect(proxy_addr).await.unwrap();
    let req_line = format!(
        "CONNECT {upstream} HTTP/1.1\r\nHost: {upstream}\r\n\r\n",
        upstream = upstream_addr
    );
    sock.write_all(req_line.as_bytes()).await.unwrap();

    let mut buf = [0u8; 256];
    let n = sock.read(&mut buf).await.unwrap();
    let resp = String::from_utf8_lossy(&buf[..n]);
    assert!(
        resp.starts_with("HTTP/1.1 200") || resp.starts_with("HTTP/1.0 200"),
        "白名单空 → P1 blind_relay 路径必须建隧道返 200（ST4 分流不回归），实际: {resp}"
    );
}
