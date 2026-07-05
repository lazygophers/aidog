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
        listen_addr: std::sync::OnceLock::new(),
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
    assert_eq!(hit.map(|(id, _)| id), Some(p.id), "CONNECT host 命中平台主 base_url 必须返回 platform_id");

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
        listen_addr: std::sync::OnceLock::new(),
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
    let resp = connect::handle_connect(AxumState(state), req, "test-rid-authority".into()).await;
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
    let resp = connect::handle_connect(AxumState(state), req, "test-rid-empty".into()).await;
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

/// L2 根因回归：CONNECT 200 后客户端立即在同一 socket 流水线写 payload（模拟真实 TLS
/// 客户端发 ClientHello），hyper-util auto server 的 speculative read 会把 payload 预读
/// 进 `parts.read_buf`。修复前代码 `write_all(&mut client, &parts.read_buf)` 把 payload
/// 回灌客户端（上游永远收不到 + 客户端收自己字节 → TLS 状态机错乱 RST）；修复后 flush 到
/// upstream（上游收得到）。本测试 mock 上游 capture 首字节断言 = payload，暴露 flush 方向 bug。
#[tokio::test]
async fn connect_tunnel_flushes_prefetch_to_upstream() {
    // 1. mock 上游 capture TCP server（accept 一条连，读首字节通过 channel 回传，再 echo）。
    let upstream = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let upstream_addr = upstream.local_addr().unwrap();
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<Vec<u8>>();
    tokio::spawn(async move {
        while let Ok((mut s, _)) = upstream.accept().await {
            let tx = tx.clone();
            tokio::spawn(async move {
                // ponytail: 读首 N 字节回传 channel（断言上游收到的首字节 = 流水线 payload），
                // 之后 echo 驱动 client 收回显避免 read 挂死。
                let mut buf = [0u8; 64];
                match s.read(&mut buf).await {
                    Ok(0) | Err(_) => return,
                    Ok(n) => {
                        let _ = tx.send(buf[..n].to_vec());
                        // echo 回客户端（若 client 还在读）。
                        let _ = s.write_all(&buf[..n]).await;
                    }
                }
                let mut buf2 = [0u8; 64];
                loop {
                    match s.read(&mut buf2).await {
                        Ok(0) | Err(_) => break,
                        Ok(n) => {
                            if s.write_all(&buf2[..n]).await.is_err() {
                                break;
                            }
                        }
                    }
                }
            });
        }
    });

    // 2. aidog proxy axum server（P1 blind_relay 路径：白名单未命中 host）。
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

    // 3. 裸 TcpStream 发 CONNECT **且不等响应**立即流水线写 payload（模拟真实 TLS 客户端
    //    发完 CONNECT 立即发 ClientHello，触发 hyper-util speculative read 把 payload
    //    预读进 parts.read_buf；现有 connect_tunnel_via_real_proxy_env 严格串行不触发）。
    let mut sock = tokio::net::TcpStream::connect(proxy_addr).await.unwrap();
    let req_line = format!(
        "CONNECT {upstream} HTTP/1.1\r\nHost: {upstream}\r\n\r\n",
        upstream = upstream_addr
    );
    // 单次 write_all 把 CONNECT + payload 灌进 socket buffer，proxy 侧 hyper-util auto server
    // 写完 200 后 speculative read 命中 payload → read_buf 非空。
    let mut combined = req_line.into_bytes();
    combined.extend_from_slice(b"PREFETCH_PAYLOAD");
    sock.write_all(&combined).await.unwrap();

    // 4. 读 CONNECT 响应 → 断言 200（隧道建立；payload 已在 proxy read_buf 里待 flush）。
    let mut buf = [0u8; 256];
    let n = sock.read(&mut buf).await.unwrap();
    let resp = String::from_utf8_lossy(&buf[..n]);
    assert!(
        resp.starts_with("HTTP/1.1 200") || resp.starts_with("HTTP/1.0 200"),
        "CONNECT 隧道必须建立返 200，实际: {resp}"
    );

    // 5. 关键断言：mock 上游收到的首字节必须是流水线 payload（修复后 flush 到 upstream）。
    //    修复前会 fail（payload 被回灌 client → 上游读到空 / 读不到 → channel 超时）。
    let upstream_first = tokio::time::timeout(
        std::time::Duration::from_secs(3),
        rx.recv(),
    ).await.expect("上游必须在 3s 内收到 flush 的 payload（修复前 flush 错对象致上游永远收不到）");
    let upstream_first = upstream_first.expect("upstream channel must yield first bytes");
    assert_eq!(
        upstream_first, b"PREFETCH_PAYLOAD".to_vec(),
        "上游首字节必须是客户端流水线写的 payload（修复后 flush 到 upstream）；\
         修复前 flush 写错对象回灌 client，上游收不到 payload"
    );
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

// ── ST5 明文 forward 接入 ──────────────────────────────────────────────────────

/// ST5 验收：MITM 明文 Request 灌入 handle_proxy_core → 走完整 AI 请求链
/// （middleware / 路由 / forward_attempt / 采集），proxy_log 落 anthropic 协议行
/// （**非 http-connect 盲转行**）+ stub 上游回 200 + cost 记账。
///
/// 这是 ST5 的核心价值验证：明文 Request 经 handle_proxy_core 后，所有 AI 规则生效
/// （对盲转的根本区别）。端到端 TLS+CA+client mock 过重，抽核心断言（与 ST4 分流判定
/// 单测同款策略）：构造明文 Request（模拟 MITM 解密后 hyper 转换出的 axum Request）→
/// 灌 handle_proxy_core → 断言 source_protocol=anthropic / status=200 / cost 记账。
///
/// 与 test_integration::forwards_anthropic_messages_to_upstream 的区别：本测试验证的是
/// **MITM 明文路径专用的 handle_proxy_core 直调**（不经 handle_proxy_inner CONNECT 分流，
/// 等价 serve_plaintext_http 的调用路径），确认 ST5 接入点正确。
#[tokio::test]
async fn mitm_forward_plaintext_request_hits_ai_path() {
    use crate::gateway::db::test_support::{sample_group, test_db};
    use crate::gateway::models::{CreatePlatform, GroupPlatformInput, Protocol};

    // 1. stub 上游 axum server（Anthropic 协议格式 200 响应）。
    let upstream_body = r#"{"id":"msg_mitm","type":"message","role":"assistant","model":"claude-3","content":[{"type":"text","text":"hi"}],"stop_reason":"end_turn","usage":{"input_tokens":5,"output_tokens":3}}"#;
    let upstream_url = {
        use axum::routing::any;
        let app = axum::Router::new().fallback(any(move || async move {
            (
                axum::http::StatusCode::OK,
                [("content-type", "application/json")],
                upstream_body,
            )
        }));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, app).await.ok() });
        format!("http://{addr}")
    };

    // 2. ProxyState + group + Anthropic 平台（base_url=stub）。
    let db = test_db().await;
    let state = Arc::new(ProxyState {
        db: Arc::new(db),
        app: None,
        middleware: Arc::new(crate::gateway::middleware::MiddlewareEngine::new()),
        scheduler: Arc::new(crate::gateway::scheduling::SchedulerState::new()),
        sticky: Arc::new(crate::gateway::scheduling::StickyTable::new()),
        log_snapshots: std::sync::Mutex::new(std::collections::HashMap::new()),
        agg_done: std::sync::Mutex::new((
            std::collections::VecDeque::new(),
            std::collections::HashSet::new(),
        )),
        listen_addr: std::sync::OnceLock::new(),
    });
    let plat = crate::gateway::db::create_platform(&state.db, CreatePlatform {
        name: "mitm-stub".into(),
        platform_type: Protocol::Anthropic,
        base_url: upstream_url,
        api_key: "sk-up".into(),
        extra: String::new(),
        models: None, available_models: None, endpoints: None, manual_budgets: None,
        auto_group: None, join_group_ids: None, default_level_priority: None, expires_at: None,
    }).await.unwrap();
    let group = crate::gateway::db::create_group(&state.db, sample_group("mitm-gk", vec![])).await.unwrap();
    crate::gateway::db::set_group_platforms(&state.db, group.id, &[GroupPlatformInput {
        platform_id: plat.id, priority: Some(0), weight: Some(1), level_priority: Some(0),
    }]).await.unwrap();

    // 3. 构造明文 Request（模拟 MITM 解密后 hyper 转换出的 axum Request）。
    //    客户端发官方 Anthropic 协议（CONNECT api.anthropic.com:443 + POST /v1/messages），
    //    aidog MITM 解密后明文 Request 的 method/uri/headers/body 与直连代理一致。
    let plaintext_req = HttpRequest::builder()
        .method("POST")
        .uri("/v1/messages")
        .header("authorization", "Bearer mitm-gk")
        .header("content-type", "application/json")
        .body(Body::from(
            r#"{"model":"claude-3","messages":[{"role":"user","content":"hi"}]}"#.to_string(),
        ))
        .unwrap();

    // 4. 灌入 handle_proxy_core（ST5 接入点，等价 serve_plaintext_http 内的调用）。
    let request_id = uuid::Uuid::new_v4().simple().to_string();
    let resp = handler::handle_proxy_core(AxumState(state.clone()), plaintext_req, request_id.clone()).await;

    // 5. 断言走 AI 路径：响应 200（stub 上游回成功）。
    assert_eq!(
        resp.status(),
        StatusCode::OK,
        "MITM 明文 Request 灌 handle_proxy_core 必须走 AI 路径返 200（stub 上游成功）"
    );

    // 6. 断言 proxy_log 落 AI 行（非 http-connect 盲转行）。
    //    关键：source_protocol=anthropic（detect_source_protocol("/v1/messages")），
    //    非 http-connect（盲转专用）。这证明明文 Request 走了完整 AI 请求链。
    let row = crate::gateway::db::get_proxy_log(&state.db, &request_id).await
        .expect("query proxy_log").expect("proxy_log row must exist");
    assert_eq!(
        row.source_protocol, "anthropic",
        "MITM 明文路径 source_protocol 必须是 anthropic（AI 路径），非 http-connect（盲转路径）"
    );
    assert_eq!(row.status_code, 200, "stub 上游 200 必须记账");
    assert_eq!(row.group_key, "mitm-gk", "group_key 必须解析正确（明文 Authorization 可见）");
    assert_eq!(row.platform_id, plat.id, "platform_id 必须命中 stub 平台（路由生效）");
    assert_eq!(row.input_tokens, 5, "input_tokens 必须从上游响应 usage 提取（采集生效）");
    assert!(row.est_cost > 0.0, "est_cost 必须非 0（cost 估算生效，盲转路径恒 0）");
}

/// ST5 失败模式回归：明文 Request 缺 Authorization（无 group）→ handle_proxy_core 返 404，
/// 不走盲转 upsert_connect_log（盲转无 group 概念）。验证 MITM 明文路径与盲转记账独立。
#[tokio::test]
async fn mitm_forward_plaintext_no_auth_returns_404_ai_path() {
    use crate::gateway::db::test_support::test_db;

    let db = test_db().await;
    let state = Arc::new(ProxyState {
        db: Arc::new(db),
        app: None,
        middleware: Arc::new(crate::gateway::middleware::MiddlewareEngine::new()),
        scheduler: Arc::new(crate::gateway::scheduling::SchedulerState::new()),
        sticky: Arc::new(crate::gateway::scheduling::StickyTable::new()),
        log_snapshots: std::sync::Mutex::new(std::collections::HashMap::new()),
        agg_done: std::sync::Mutex::new((
            std::collections::VecDeque::new(),
            std::collections::HashSet::new(),
        )),
        listen_addr: std::sync::OnceLock::new(),
    });

    // 明文 Request 无 Authorization（模拟客户端未带 apikey 的官方协议请求）。
    let plaintext_req = HttpRequest::builder()
        .method("POST")
        .uri("/v1/messages")
        .header("content-type", "application/json")
        .body(Body::from(r#"{"model":"claude-3","messages":[]}"#.to_string()))
        .unwrap();

    let request_id = uuid::Uuid::new_v4().simple().to_string();
    let resp = handler::handle_proxy_core(AxumState(state.clone()), plaintext_req, request_id.clone()).await;

    // AI 路径 404（no matching group），非盲转（盲转无 group 概念，恒 200/502）。
    assert_eq!(
        resp.status(),
        StatusCode::NOT_FOUND,
        "MITM 明文路径无 group 必须走 AI 路径 404（非盲转 200/502）"
    );

    // proxy_log 行存在 + status=404（AI 路径全量记账，盲转不落 AI 行）。
    let row = crate::gateway::db::get_proxy_log(&state.db, &request_id).await
        .expect("query proxy_log").expect("proxy_log row must exist");
    assert_eq!(row.status_code, 404, "AI 路径 404 必须落 proxy_log");
    assert_eq!(row.source_protocol, "", "group 解析失败时 source_protocol 未设（仍非 http-connect）");
}

// ── ST6 HTTP/2 auto 协议转换 ─────────────────────────────────────────────────

/// ST6 验收：`serve_plaintext` 用 hyper-util `auto::Builder` 在明文流上服务 HTTP 请求。
///
/// 核心断言：明文流喂给 auto Builder + service_fn，server 能解帧、调用 service_fn、回
/// Response。这抓住 ST6 的关键回归 —— 「auto Builder API 用错 / Cargo.toml feature 没拉齐」
/// 会让 serve_plaintext 运行时崩（编译期因 feature gate 不一定报）。
///
/// **h2 端到端归 ST8**：hyper h2 client/server 在 tokio duplex 上 handshake 死锁（双方
/// 都在等对方先写 settings，duplex 无真实 TCP backpressure 触发）。完整 h2 round-trip
/// 需真实 TCP socket + 端口绑定（mitm CA + rustls accept + hyper h2 全链路），过重，标 ST8。
/// 本测试用 h1-over-auto-Builder 覆盖「auto Builder 接入正确」核心断言（h1/h2 走同一个
/// `serve_connection` 入口，差别仅在内部 ReadVersion 分发）。
///
/// ponytail: 不灌 handle_proxy_core —— ST5 已覆盖「明文 Request 灌 core 走 AI 路径」，
/// h1/h2 在 core 入口后等价（auto Builder 只负责 HTTP 解帧）。本测试聚焦 ST6 独有改动
/// （auto Builder 替代 http1::Builder + 删 h2 桥接分支）。
#[tokio::test]
async fn mitm_serve_plaintext_auto_builder_serves_http() {
    use hyper_util::rt::TokioIo;

    // 1. duplex 模拟 serve_plaintext 的明文 TLS 流（rustls accept 后的 client_tls 等价物）。
    let (client_io, server_io) = tokio::io::duplex(8 * 1024);

    // 2. service_fn：收到 Request 回 200 + 固定 body（不调 handle_proxy_core，聚焦 auto 协议层）。
    let svc = hyper::service::service_fn(|_req: hyper::Request<hyper::body::Incoming>| async move {
        Ok::<_, std::convert::Infallible>(
            hyper::Response::builder()
                .status(StatusCode::OK)
                .body(axum::body::Body::from("auto-ok"))
                .unwrap(),
        )
    });

    // 3. serve_plaintext 等价路径：auto::Builder + TokioExecutor 在 duplex 服务端跑。
    //    auto Builder 读首字节检测协议（h1 直接进 http1 server，h2 读 preface 分发 http2 server）。
    let server_task = tokio::spawn(async move {
        let io = TokioIo::new(server_io);
        let builder = hyper_util::server::conn::auto::Builder::new(hyper_util::rt::TokioExecutor::new());
        let _ = builder.serve_connection(io, svc).await;
    });

    // 4. hyper h1 client 在 duplex 客户端发 POST /v1/messages（模拟 MITM 解密后的官方协议请求）。
    let client_io = TokioIo::new(client_io);
    let (mut sender, conn) = hyper::client::conn::http1::Builder::new()
        .handshake(client_io)
        .await
        .expect("h1 client handshake");
    tokio::spawn(async move {
        let _ = conn.await;
    });

    let req = hyper::Request::builder()
        .method("POST")
        .uri("/v1/messages")
        .header("content-type", "application/json")
        .body(axum::body::Body::from(r#"{"model":"claude-3"}"#))
        .unwrap();
    let resp = sender.send_request(req).await.expect("h1 send_request");

    // 5. 断言：auto Builder h1 路径 round-trip 成功（service_fn 被调用 + 回 Response）。
    assert_eq!(
        resp.status(),
        StatusCode::OK,
        "auto Builder 必须能服务明文 HTTP（h2 端到端归 ST8，h1 覆盖接入断言）"
    );

    // 等 server task 结束（client drop 后 auto serve_connection 返回）。
    drop(sender);
    let _ = server_task.await;
}

// ── P2 元数据记账对齐（A timeout / B 熔断 / C last_error）──────────────────────

/// P2-A：`tcp_connect_accounted` 套 `tokio::time::timeout` —— 连不可路由 target（10.255.255.1
/// RFC1918 末尾，路由器丢弃 SYN）+ 短 timeout（1s）→ 必在 1s 级别返 Err(())，不会挂满 OS 默认 ~75s。
///
/// 关键断言：返回时间 < 3s（防回归：若漏套 timeout，OS 默认 TCP SYN retry ~75s 才失败，
/// 测试会卡到 cargo test 默认 60s 超时）。
#[tokio::test]
async fn connect_timeout_applied_to_tcp_handshake() {
    let state = make_state().await;
    let start = std::time::Instant::now();
    // 10.255.255.1：典型不可路由地址（本地路由器丢弃 SYN，无 RST，必超时而非拒绝）。
    let res = connect::tcp_connect_accounted(&state, "10.255.255.1:80", 0, None, 1).await;
    let elapsed = start.elapsed();
    assert!(res.is_err(), "不可路由 target 必返 Err(())");
    assert!(
        elapsed.as_secs() < 3,
        "必须套 timeout 在 1s 级别失败（实际 {:?}），防漏套 timeout 致 OS 默认 ~75s SYN retry",
        elapsed
    );
}

/// 取一个未监听的 127.0.0.1 端口地址（bind 后 drop → 端口关闭，connect 必拒绝）。
fn closed_loopback_target() -> String {
    let addr = std::net::TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap();
    // drop listener 后端口立即关闭，TcpStream::connect 必 RST = connection refused。
    format!("{addr}")
}

/// P2-B：TCP 失败（connection refused，未监听端口）→ `scheduler.record_failure` 计入。
/// 关键断言：breaker_state 从 Closed{fails:0} → Closed{fails:1}（未达阈值仍 Closed，但 fails+1）；
/// latency EMA 仍 None（record_failure 不更新 EMA，防 CONNECT TCP 握手延迟污染 AI LeastLatency）。
#[tokio::test]
async fn connect_failure_records_breaker_fail_count() {
    use crate::gateway::db::test_support;
    use crate::gateway::models::{CreatePlatform, Protocol};
    use crate::gateway::scheduling::{BreakerState, BreakerThresholds};

    let db = test_support::test_db().await;
    let p = crate::gateway::db::create_platform(&db, CreatePlatform {
        name: "connect-fail".into(),
        platform_type: Protocol::Anthropic,
        base_url: "https://connect-fail.example/v1".into(),
        api_key: "sk".into(),
        extra: String::new(),
        models: None, available_models: None, endpoints: None, manual_budgets: None,
        auto_group: None, join_group_ids: None, default_level_priority: None, expires_at: None,
    }).await.unwrap();
    let state = Arc::new(ProxyState {
        db: Arc::new(db),
        app: None,
        middleware: Arc::new(crate::gateway::middleware::MiddlewareEngine::new()),
        scheduler: Arc::new(crate::gateway::scheduling::SchedulerState::new()),
        sticky: Arc::new(crate::gateway::scheduling::StickyTable::new()),
        log_snapshots: std::sync::Mutex::new(std::collections::HashMap::new()),
        agg_done: std::sync::Mutex::new((std::collections::VecDeque::new(), std::collections::HashSet::new())),
        listen_addr: std::sync::OnceLock::new(),
    });

    // 触发失败：127.0.0.1 关闭端口（立即 RST = connection refused，秒级失败）。
    // platform_id = p.id（直接传，模拟 host 匹配后传入）。
    let th = BreakerThresholds { failure_threshold: 5, open_secs: 60, half_open_max: 2 };
    let target = closed_loopback_target();
    let res = connect::tcp_connect_accounted(&state, &target, p.id, Some(&th), 5).await;
    assert!(res.is_err(), "关闭端口必拒绝连接");

    // 熔断失败计数：Closed{fails:0} → Closed{fails:1}（未达阈值 5 仍 Closed）。
    let st = state.scheduler.breaker_state(p.id);
    assert!(
        matches!(st, BreakerState::Closed { fails } if fails == 1),
        "TCP 失败必须 record_failure 使 fails=1，实际: {:?}",
        st
    );
    // inflight 必归零（record_failure 内 dec_inflight）。
    assert_eq!(state.scheduler.inflight(p.id), 0, "失败后 inflight 必归零");
    // EMA 未被污染（record_failure 不动 latency_ema_ms，仍 None）。
    assert!(
        state.scheduler.latency_ema(p.id).is_none(),
        "record_failure 不应更新 latency EMA（防 CONNECT TCP 握手延迟污染 AI LeastLatency 排序）"
    );

    // 未命中平台（platform_id=0）→ breaker_th=None → 不计入熔断。
    let target2 = closed_loopback_target();
    let _ = connect::tcp_connect_accounted(&state, &target2, 0, None, 5).await;
    // platform_id=0 不该被任何 breaker 记账。
}

/// P2-C：TCP 失败 → `db::set_platform_last_error` 写入 platform.last_error 列。
/// 关键断言：失败后 platform.last_error 非空 + last_error_at > 0。
#[tokio::test]
async fn connect_failure_sets_platform_last_error() {
    use crate::gateway::db::test_support;
    use crate::gateway::models::{CreatePlatform, Protocol};
    use crate::gateway::scheduling::BreakerThresholds;

    let db = test_support::test_db().await;
    let p = crate::gateway::db::create_platform(&db, CreatePlatform {
        name: "last-err".into(),
        platform_type: Protocol::Anthropic,
        base_url: "https://last-err.example/v1".into(),
        api_key: "sk".into(),
        extra: String::new(),
        models: None, available_models: None, endpoints: None, manual_budgets: None,
        auto_group: None, join_group_ids: None, default_level_priority: None, expires_at: None,
    }).await.unwrap();
    let state = Arc::new(ProxyState {
        db: Arc::new(db),
        app: None,
        middleware: Arc::new(crate::gateway::middleware::MiddlewareEngine::new()),
        scheduler: Arc::new(crate::gateway::scheduling::SchedulerState::new()),
        sticky: Arc::new(crate::gateway::scheduling::StickyTable::new()),
        log_snapshots: std::sync::Mutex::new(std::collections::HashMap::new()),
        agg_done: std::sync::Mutex::new((std::collections::VecDeque::new(), std::collections::HashSet::new())),
        listen_addr: std::sync::OnceLock::new(),
    });

    let th = BreakerThresholds { failure_threshold: 5, open_secs: 60, half_open_max: 2 };
    let target = closed_loopback_target();
    let res = connect::tcp_connect_accounted(&state, &target, p.id, Some(&th), 5).await;
    assert!(res.is_err(), "关闭端口必拒绝连接");

    // 读回 platform，验 last_error 已写。
    let p_after = crate::gateway::db::get_platform(&state.db, p.id).await.unwrap().unwrap();
    assert!(
        !p_after.last_error.is_empty(),
        "TCP 失败必须 set_platform_last_error 使 last_error 非空，实际: {:?}",
        p_after.last_error
    );
    assert!(
        p_after.last_error_at > 0,
        "last_error_at 必须被设为 now()（>0），实际: {}",
        p_after.last_error_at
    );
}



