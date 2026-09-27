//! ST8 端到端 MITM 测试：完整 TLS + CA + client mock 链路。
//!
//! 补 ST5 单测未覆盖的「真实 TLS 握手 + hyper auto Builder over TLS + 客户端解密验明文」
//! 完整链路。ST5 的 `mitm_forward_plaintext_request_hits_ai_path` 直灌明文 Request 给
//! handle_proxy_core（绕过 TLS 层）；本文件加回 TLS 层（rustls accept_client 假证书 +
//! rustls client 信任 CA + hyper h1 over TLS round-trip）。
//!
//! **覆盖链路**（design ST8 验收）：
//! ```text
//! mock client (rustls, 信任 ST1 假 CA)
//!   → TCP connect
//!   → rustls TLS 握手（client connect ↔ accept_client 用假 CA 签 leaf）
//!   → 明文 HTTP/1.1 POST /v1/messages（hyper h1 client over TLS）
//!   → serve_plaintext auto Builder 解帧
//!   → handle_proxy_core（middleware / 路由 / forward_attempt / 采集）
//!   → stub 上游 axum（http://127.0.0.1，Anthropic 协议 200）
//!   → 响应回写（hyper 写回 TLS stream，rustls 自动加密回 client）
//!   → client 解密验明文（hyper h1 读 Response）
//! ```
//!
//! **CONNECT 隧道建立本身**（axum upgrade 机制）由 test_connect.rs 的
//! `connect_authority_form_resolves_target` / `connect_tunnel_via_real_proxy_env` 覆盖；
//! 本文件聚焦「隧道建后的 TLS + 明文 forward」端到端，不重复 CONNECT 隧道测试。
//!
//! **h2 端到端**：见 `mitm_e2e_h2_remaining_risk` 测试注释的剩余风险说明。
//!
//! ponytail: 真实 TcpListener/TcpStream（禁 tokio duplex —— ST6 已证 h2 handshake 死锁；
//! h1 over TLS 在 duplex 上理论上可行但 rustls + hyper 组合对 backpressure 敏感，
//! 真实 TCP 更稳）。

use super::*;
use crate::gateway::models::{CreatePlatform, GroupPlatformInput, Protocol};
use crate::gateway::scheduling;
use aidog_db::test_support::{sample_group, test_db};
use aidog_middleware::MiddlewareEngine;
use aidog_mitm::ca::{RootCa, create_and_store_root_ca};
use aidog_mitm::cert_signer::CertSigner;
use aidog_mitm::tls::accept_client;
use axum::body::Body;
use hyper_util::rt::{TokioExecutor, TokioIo};
use rustls::pki_types::ServerName;
use std::sync::Arc;
use tokio::io::{AsyncRead, AsyncWrite};

/// 测试用 ProxyState（内存 DB + 空 middleware/scheduler）。
async fn make_state_with_ca() -> (Arc<ProxyState>, RootCa) {
    let db = test_db().await;
    let ca = create_and_store_root_ca(&db)
        .await
        .expect("create root CA in test db");
    let (log_tx, log_rx) = tokio::sync::mpsc::channel(1024);
    let state = Arc::new(ProxyState {
        db: Arc::new(db),
        middleware: Arc::new(MiddlewareEngine::new()),
        scheduler: Arc::new(scheduling::SchedulerState::new()),
        sticky: Arc::new(scheduling::StickyTable::new()),
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
    (state, ca)
}

/// 起 stub 上游 axum server（Anthropic 协议 200 响应），返回 base_url（http://127.0.0.1:port）。
///
/// ponytail: 用 http（非 https）→ forward_attempt 的 reqwest 无 TLS 验证问题，
/// 聚焦测 client↔AirDog TLS 段（ST8 核心），上游段（AirDog↔upstream）已由 test_integration 覆盖。
async fn spawn_stub_upstream() -> String {
    let upstream_body = r#"{"id":"msg_e2e","type":"message","role":"assistant","model":"claude-3","content":[{"type":"text","text":"mitm e2e ok"}],"stop_reason":"end_turn","usage":{"input_tokens":7,"output_tokens":4}}"#;
    let body_clone = upstream_body.to_string();
    let app = axum::Router::new().fallback(axum::routing::any(move || async move {
        (
            axum::http::StatusCode::OK,
            [("content-type", "application/json")],
            body_clone,
        )
    }));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.ok() });
    format!("http://{addr}")
}

/// 解析 PEM 证书链为 DER vec（rustls root store 加载用）。
fn parse_cert_chain_pem(cert_pem: &str) -> Vec<rustls::pki_types::CertificateDer<'static>> {
    use rustls_pemfile::Item;
    let mut chain = Vec::new();
    let mut cursor = std::io::Cursor::new(cert_pem.as_bytes());
    while let Ok(item) = rustls_pemfile::read_one(&mut cursor) {
        match item {
            Some(Item::X509Certificate(der)) => chain.push(der),
            Some(_) => continue,
            None => break,
        }
    }
    chain
}

/// 构造信任指定 CA 的 rustls client config（ALPN advertise http/1.1 only）。
fn client_config_trusting_ca(ca: &RootCa) -> rustls::ClientConfig {
    let mut root_store = rustls::RootCertStore::empty();
    for c in parse_cert_chain_pem(&ca.cert_pem) {
        root_store.add(c).expect("add CA to root store");
    }
    let mut cfg = rustls::ClientConfig::builder()
        .with_root_certificates(root_store)
        .with_no_client_auth();
    // h1 only（h2 端到端风险见下方测试）
    cfg.alpn_protocols = vec![b"http/1.1".to_vec()];
    cfg
}

/// ST8 端到端 http/1.1 TLS 链路：mock client → TLS 握手（假 CA）→ 明文 Request →
/// handle_proxy_core → stub 上游 → 响应回传 → client 解密验明文。
///
/// 这是 ST8 核心断言：完整 TLS + CA + client mock 链路 round-trip，证明 MITM 解密隧道
/// 端到端可工作（ST1 CA + ST3 TLS + ST5 serve_plaintext + forward_attempt 全套协同）。
///
/// **与 ST5 `mitm_forward_plaintext_request_hits_ai_path` 的区别**：ST5 直灌明文 Request
/// 给 handle_proxy_core（绕过 TLS 层 + auto Builder）；本测试加回完整 TLS 层（rustls
/// accept_client + client connect + hyper auto Builder over TLS stream），验「客户端
/// 发密文 → AirDog 解密 → 走 AI 路径 → 响应重新加密 → 客户端解密」全链路。
#[tokio::test]
async fn mitm_e2e_h1_tls_round_trip() {
    let _ = rustls::crypto::ring::default_provider().install_default();

    // 1. stub 上游 axum server（http，Anthropic 协议 200）。
    let upstream_url = spawn_stub_upstream().await;

    // 2. ProxyState + CA（DB 存）+ group + Anthropic 平台（base_url=stub）。
    let (state, ca) = make_state_with_ca().await;
    let plat = aidog_db::create_platform(
        &state.db,
        CreatePlatform {
            name: "mitm-e2e-stub".into(),
            platform_type: Protocol::Anthropic,
            base_url: upstream_url.clone(),
            api_key: "sk-e2e-up".into(),
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
    .expect("create platform");
    let group = aidog_db::create_group(&state.db, sample_group("mitm-e2e-gk", vec![]))
        .await
        .expect("create group");
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
    .expect("set group platforms");

    // 3. MITM server 端：真实 TcpListener（模拟 CONNECT 后的 client TCP 连接）。
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let mitm_addr = listener.local_addr().unwrap();
    let signer = Arc::new(CertSigner::new(ca.clone()));
    let state_for_server = state.clone();
    let server_host = "api.anthropic.com".to_string();
    tokio::spawn(async move {
        let (tcp_stream, _) = listener.accept().await.expect("accept client");
        // rustls accept（假 CA 签 leaf，SNI fallback=server_host）→ 明文 TLS stream。
        let client_tls = accept_client(signer, tcp_stream, server_host.clone())
            .await
            .expect("TLS accept");
        // serve_plaintext：auto Builder 在明文 TLS stream 上服务 HTTP，每 Request 灌 handle_proxy_core。
        connect::serve_plaintext(state_for_server, client_tls, &server_host, None).await;
    });

    // 4. mock client：rustls client（信任 CA）→ TCP connect → TLS 握手 → hyper h1 发请求。
    let tcp = tokio::net::TcpStream::connect(mitm_addr)
        .await
        .expect("connect MITM");
    let connector = tokio_rustls::TlsConnector::from(Arc::new(client_config_trusting_ca(&ca)));
    let server_name = ServerName::try_from("api.anthropic.com".to_string()).unwrap();
    let tls_stream = connector
        .connect(server_name, tcp)
        .await
        .expect("TLS handshake");

    // hyper h1 client over TLS stream：handshake → send_request。
    let (mut sender, conn) = hyper::client::conn::http1::Builder::new()
        .handshake(TokioIo::new(tls_stream))
        .await
        .expect("h1 client handshake");
    tokio::spawn(async move {
        let _ = conn.await;
    });

    let req = hyper::Request::builder()
        .method("POST")
        .uri("/v1/messages")
        .header("authorization", "Bearer mitm-e2e-gk")
        .header("content-type", "application/json")
        .body(Body::from(
            r#"{"model":"claude-3","messages":[{"role":"user","content":"hi"}]}"#.to_string(),
        ))
        .unwrap();
    let resp = sender.send_request(req).await.expect("h1 send_request");

    // 5. 断言：client 解密后收到 200 明文响应（证明 TLS 双向 + auto Builder + forward 全链路通）。
    assert_eq!(
        resp.status(),
        StatusCode::OK,
        "MITM e2e h1: client 必须收到 200（stub 上游成功经 TLS 回写）"
    );
    // ponytail: 手写 collect hyper Incoming body（axum::body::to_bytes 锁 Body 类型不兼容 Incoming；
    // http_body_util::BodyExt 传递依赖不直接可见，与 connect.rs:421 collect_body 同款策略）。
    let body_bytes = collect_incoming_body(resp.into_body(), 64 * 1024)
        .await
        .expect("read response body");
    let body_str = std::str::from_utf8(&body_bytes).expect("body utf8");
    assert!(
        body_str.contains("mitm e2e ok"),
        "响应 body 必须含 stub 上游文本，实际: {body_str}"
    );

    // 6. 断言 proxy_log 落 AI 行（非 http-connect 盲转行）—— 明文 Request 走完整 forward 链。
    //    request_id 由 handle_proxy_core 内部生成（serve_plaintext 内 uuid::new_v4），
    //    测试拿不到具体 id；查最近 10 行找 source_protocol=anthropic 的行（test_db 空库，
    //    仅本测试写入）。
    flush_log_queue(&state).await;
    let logs = aidog_logs::list_proxy_logs(&state.db, 10, 0)
        .await
        .expect("list proxy_logs");
    // ProxyLogSummary 含 source_protocol/group_key/platform_id/input_tokens/status_code（无 est_cost）。
    let row = logs
        .into_iter()
        .find(|r| r.source_protocol == "anthropic")
        .expect("anthropic proxy_log row must exist after MITM e2e forward");
    assert_eq!(
        row.source_protocol, "anthropic",
        "MITM e2e 明文路径 source_protocol 必须是 anthropic（AI 路径），非 http-connect"
    );
    assert_eq!(row.status_code, 200, "stub 上游 200 必须记账");
    assert_eq!(
        row.group_key, "mitm-e2e-gk",
        "group_key 必须从明文 Authorization 解析"
    );
    assert_eq!(
        row.platform_id, plat.id,
        "platform_id 必须命中 stub 平台（路由生效）"
    );
    assert_eq!(
        row.input_tokens, 7,
        "input_tokens 必须从上游 usage 提取（采集生效）"
    );

    // est_cost 在 ProxyLogSummary 不返回，查完整行验 cost 记账（盲转恒 0，AI 路径非 0）。
    let full = aidog_logs::get_proxy_log(&state.db, &row.id)
        .await
        .expect("query full proxy_log")
        .expect("full proxy_log row must exist");
    assert!(
        full.est_cost > 0.0,
        "est_cost 必须非 0（cost 估算生效，盲转恒 0），实际: {}",
        full.est_cost
    );

    drop(sender); // 让 server 端 serve_connection 退出
}

/// 构造信任指定 CA 且 advertise h2 ALPN 的 rustls client config。
/// 与 `client_config_trusting_ca` 区别：ALPN 列表 [h2, http/1.1]，模拟 curl --http2 /
/// 现代浏览器在 MITM 隧道里与 rustls server 协商 h2 的真实场景。
fn client_config_trusting_ca_h2(ca: &RootCa) -> rustls::ClientConfig {
    let mut root_store = rustls::RootCertStore::empty();
    for c in parse_cert_chain_pem(&ca.cert_pem) {
        root_store.add(c).expect("add CA to root store");
    }
    let mut cfg = rustls::ClientConfig::builder()
        .with_root_certificates(root_store)
        .with_no_client_auth();
    // h2 优先（与 mitm/tls.rs SERVER_ALPN 顺序一致），rustls 按对端 advertise 选首个共同协议。
    cfg.alpn_protocols = vec![b"h2".to_vec(), b"http/1.1".to_vec()];
    cfg
}

/// 复现用户场景的 h2 CANCEL：curl -x http://127.0.0.1:<aidog>/proxy https://www.baidu.com/
/// 经 CONNECT → MITM TLS（h2 ALPN 协商成功）→ 明文 GET / 无 Authorization → 票 10 起
/// classify=Bypass → mitm_bypass::handle_mitm_observed（透明转发 + mitm_log 观测行）。
///
/// 上游（forward 内 reqwest https://www.baidu.com:443/）无法在单测里 mock 真 https 上游
/// （reqwest 用 webpki-roots 验证，禁注入自签 CA），故走 forward 的 502 错误分支
/// （上游不可达 / 任何 reqwest error）。本测试聚焦断言「h2 server 把 Response 完整回传
/// 给 h2 client，不 RST_STREAM / CANCEL」—— 这是用户报告的 CANCEL 根因定位点。
///
/// 若 forward 的 502 响应能在 h2 上干净回传（client 收 502 + body）→ 证明 h2 server 写
/// 路径正常，CANCEL 根因在 forward 的 200 成功路径（body/headers 构造）→ 进一步缩窄。
/// 若 502 也 CANCEL → 根因在 h2 server / serve_plaintext 层（非 forward 业务逻辑）。
#[tokio::test]
async fn mitm_h2_passthrough_unmatched_returns_response_not_cancel() {
    let _ = rustls::crypto::ring::default_provider().install_default();

    // 1. ProxyState + CA（DB 存）。listen_addr 必须设（否则 should_fallback_passthrough
    //    返 false → forward 不触发 → 测不到目标路径）。
    let (state, ca) = make_state_with_ca().await;
    let _ = state.listen_addr.set((
        std::net::IpAddr::V4(std::net::Ipv4Addr::new(127, 0, 0, 1)),
        9892u16,
    ));

    // 2. MITM server：真实 TCP（禁 duplex —— rustls+h2 在 duplex 上易死锁）。
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let mitm_addr = listener.local_addr().unwrap();
    let signer = Arc::new(CertSigner::new(ca.clone()));
    let state_for_server = state.clone();
    let server_host = "www.baidu.com".to_string();
    tokio::spawn(async move {
        let (tcp_stream, _) = listener.accept().await.expect("accept client");
        let client_tls = accept_client(signer, tcp_stream, server_host.clone())
            .await
            .expect("TLS accept");
        connect::serve_plaintext(state_for_server, client_tls, &server_host, None).await;
    });

    // 3. mock client：rustls client（信任 CA，advertise h2 ALPN）→ TLS 握手。
    //    **关键断言点 1**：ALPN 协商必须选出 h2（与 curl --http2 / 浏览器一致）。
    let tcp = tokio::net::TcpStream::connect(mitm_addr)
        .await
        .expect("connect MITM");
    let connector = tokio_rustls::TlsConnector::from(Arc::new(client_config_trusting_ca_h2(&ca)));
    let server_name = ServerName::try_from("www.baidu.com".to_string()).unwrap();
    let tls_stream = connector
        .connect(server_name, tcp)
        .await
        .expect("TLS handshake");
    let (_, tls_session) = tls_stream.get_ref();
    let negotiated_alpn = tls_session.alpn_protocol();
    assert_eq!(
        negotiated_alpn.map(|b| std::str::from_utf8(b).unwrap_or("")),
        Some("h2"),
        "ALPN 必须协商出 h2（rustls server advertise [h2, http/1.1] + client advertise h2 优先），\
         协商失败 = MITM h2 链路根本没建立，后续 CANCEL 无从谈起"
    );

    // 4. hyper h2 client over TLS：handshake → send_request。
    //    **关键断言点 2**：send_request 必须返回 Response（不 CANCEL）。
    let (mut sender, conn) = hyper::client::conn::http2::Builder::new(TokioExecutor::new())
        .handshake(TokioIo::new(tls_stream))
        .await
        .expect("h2 client handshake");
    tokio::spawn(async move {
        let _ = conn.await;
    });

    // 模拟 curl GET https://www.baidu.com/ —— 无 Authorization（触发 fallback passthrough），
    // Host: www.baidu.com（CONNECT target，非代理自身监听 host）。
    let req = hyper::Request::builder()
        .method("GET")
        .uri("/")
        .header("host", "www.baidu.com")
        .header("user-agent", "mitm-h2-test")
        .body(Body::empty())
        .unwrap();
    let resp = sender.send_request(req).await.expect(
        "h2 send_request 必须返回 Response —— 若此处 panic/err 即复现用户 CANCEL: \
         h2 server 在回响应前/中 RST_STREAM",
    );

    // 5. **关键断言点**：forward 把上游响应（200 真上游 / 502 上游失败）完整经 h2 回传。
    //    测试机有外网时 reqwest 直连 baidu 成功 → 200 + html；无外网 → 502 + error 文本。
    //    **两者都证明 h2 server 写路径正常**：send_request 返 Response + body 完整可读，
    //    不 RST_STREAM / 不 CANCEL。这才是定位用户 CANCEL 的关键断言（CANCEL 会让 send_request
    //    先 panic 或 body 读取 err）。
    let status = resp.status();
    let body_bytes = collect_incoming_body(resp.into_body(), 64 * 1024)
        .await
        .expect("h2 response body 必须可完整读取（CANCEL 会在此 err）");
    let body_str = String::from_utf8_lossy(&body_bytes);
    eprintln!(
        "[mitm_h2_passthrough_unmatched] status={status} body_len={} body_head={:?}",
        body_bytes.len(),
        body_str.chars().take(80).collect::<String>()
    );
    assert!(
        status == StatusCode::OK || status == StatusCode::BAD_GATEWAY,
        "MITM h2 passthrough unmatched: client 必须收到 200（真上游成功）或 502（上游失败兜底），\
         实际 {status} —— 任何其它状态 / CANCEL 都说明 h2 server 写路径异常"
    );
    assert!(
        !body_bytes.is_empty(),
        "响应 body 必须非空（200 = html 内容 / 502 = error 文本），实际空 = body 流被吞"
    );

    // 6. 票 10 起：非 API 明文请求改走 mitm_bypass —— mitm_log 落 www.baidu.com 观测行
    //    （200 真上游成功 / 502 上游失败），proxy_log 不再有「未匹配」桶行（旁路不进
    //    proxy_log 正是票 10 目标）。
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    let mitm_rows = state
        .db
        .call_read_traced(None, std::panic::Location::caller(), |conn| {
            let mut stmt =
                conn.prepare("SELECT host, status_code FROM mitm_log")?;
            let rows = stmt
                .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i32>(1)?)))?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            Ok(rows)
        })
        .await
        .expect("read mitm_log");
    let (_host, status) = mitm_rows
        .iter()
        .find(|(h, _)| h == "www.baidu.com")
        .expect("mitm_bypass 观测行必须存在（forward 路径已执行）");
    assert!(
        *status == 200 || *status == 502,
        "mitm_bypass 必须记账终态（200 真上游成功 / 502 上游失败），实际 {status}"
    );
    flush_log_queue(&state).await;
    let logs = aidog_logs::list_proxy_logs(&state.db, 10, 0)
        .await
        .expect("list proxy_logs");
    assert!(
        logs.iter().all(|r| r.group_key != "未匹配"),
        "票 10 起 MITM 非 API 流量不得落 proxy_log「未匹配」桶（旁路归 mitm_log）"
    );

    drop(sender);
}

/// ST8 验收：h2 端到端剩余风险说明 + 已覆盖的 h2 接入断言引用。
///
/// **h2 端到端未闭合的真实阻塞**：
/// 1. **Cargo.toml feature 不齐**：`hyper = { version = "1", features = ["http1"] }`
///    显式只开 http1；h2 server + client 需 `http2` feature。改 Cargo.toml 属非测试
///    源码改动（本 subtask 禁做）。
/// 2. **rustls h2 ALPN + hyper auto h2 server + multi-stream** 组合复杂度高于 h1，
///    tokio duplex 死锁（ST6 已证 h2 handshake 双方等 settings）→ 需真实 TCP + 完整
///    ALPN 协商链路。
///
/// **已覆盖的 h2 相关断言**（ST6 产物，非 0 验证）：
/// - `mitm_serve_plaintext_auto_builder_serves_http`（test_connect.rs）：验 hyper-util
///   `auto::Builder` 接入正确，h1 over auto Builder round-trip 通过。auto Builder 是
///   h1/h2 统一入口（读首字节 H2 preface `PRI * HTTP/2.0...` 分发），h1 通过即证明
///   「协议分发机制」工作；h2 仅差实际 h2 帧编解码（hyper http2 feature 提供）。
/// - ST3 `tls_handshake`（tls.rs）：rustls accept_client + client connect 双向 TLS 握手
///   通过（ALPN advertise `[h2, http/1.1]`）。
///
/// **手动验证步骤**（用户启用 MITM 后实跑）：
/// 1. 启 aidog `yarn tauri dev` + 前端装 CA 到系统信任库（ST7 UI）
/// 2. `export HTTPS_PROXY=http://127.0.0.1:<aidog_proxy_port>`
/// 3. `curl -v https://api.anthropic.com/v1/messages -H "authorization: Bearer <group>" -d '...' --http2`
/// 4. 观察日志：`mitm tls: resolved cert by SNI` + proxy_log 落 anthropic 行（非 http-connect）
/// 5. 响应 body 正确返回（h2 多流场景需多次请求验复用）
///
/// ponytail: 不为 h2 端到端改 Cargo.toml（非测试源码），标剩余风险转 main 决策是否
/// 单独开 subtask 加 hyper http2 feature + h2 真链路测试。
#[test]
fn mitm_e2e_h2_remaining_risk() {
    // 文档测试：h2 端到端的剩余风险已在上文注释说明，本函数仅作 grep-able 锚点。
    // 真实断言：auto Builder 接入 + TLS 握手已分别由 ST6/ST3 单测覆盖（见上方引用）。
    //
    // ponytail: 无运行时断言（assert!(true) 触发 clippy::assertions_on_constants）；
    // 函数存在本身即锚点，注释承载 h2 剩余风险说明。
}

/// AsRef<[u8]> 约束（serve_plaintext 内 collect_body 用）；保留 trait import 防 unused 警告。
#[allow(dead_code)]
fn _assert_traits<A: AsyncRead + AsyncWrite + Unpin + Send + 'static>() {}

/// 手写 collect hyper `Incoming` body 为 Bytes（仿 connect.rs:421 `collect_body`）。
///
/// ponytail: axum::body::to_bytes 签名锁 `axum::body::Body`，hyper client 返回的
/// `hyper::body::Incoming` 不兼容；http_body_util::BodyExt 是传递依赖不直接可见。
/// 4 行 poll_frame 循环 < 加一个 dep。
async fn collect_incoming_body(
    body: hyper::body::Incoming,
    limit: usize,
) -> Result<hyper::body::Bytes, String> {
    use hyper::body::Body as _; // poll_frame trait
    let mut buf: Vec<u8> = Vec::new();
    tokio::pin!(body);
    loop {
        let frame = std::future::poll_fn(|cx| body.as_mut().poll_frame(cx)).await;
        match frame {
            None => return Ok(hyper::body::Bytes::from(buf)),
            Some(Ok(f)) => {
                if let Ok(data) = f.into_data() {
                    if buf.len() + data.len() > limit {
                        return Err(format!("body too large (limit {limit})"));
                    }
                    buf.extend_from_slice(&data);
                }
            }
            Some(Err(e)) => return Err(format!("body read error: {e}")),
        }
    }
}

/// 票 09：MITM 解密 `/v1/messages` + CONNECT 绑定 group（订阅 OAuth Bearer）→
/// `Protocol::ClaudeCode` 平台 1:1 透传 → proxy_log 全链记账闭环：
/// group 归属（绑定注入）/ model / tokens（上游 usage 提取）/ est_cost（registry
/// claude_code 条目参考成本，非 0）。
///
/// 与 `mitm_e2e_h1_tls_round_trip` 的区别：那边走 anthropic 协议转换路径 + 请求自带
/// group token；这边是订阅透传形态 —— Authorization 是 OAuth Bearer（resolve_group 必落空），
/// 归属完全靠 serve_plaintext 的绑定注入，路由命中 claude_code 独占组的透传拦截。
#[tokio::test]
async fn mitm_bound_group_claude_code_stats_closed_loop() {
    let _ = rustls::crypto::ring::default_provider().install_default();

    // 1. stub 上游：anthropic 形状 200 + usage（claude-sonnet-5 在 registry claude_code
    //    条目带官方价 → est_cost 可算出非 0 参考值）。
    let upstream_url = spawn_stub_upstream().await;

    // 2. claude_code 订阅平台（base_url 指向 stub，端点锁死不影响主 base_url）+ 独占组。
    let (state, ca) = make_state_with_ca().await;
    let plat = aidog_db::create_platform(
        &state.db,
        CreatePlatform {
            name: "cc-stats-stub".into(),
            platform_type: Protocol::ClaudeCode,
            base_url: upstream_url.clone(),
            api_key: String::new(), // 订阅透传：aidog 不持有凭证
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
    .expect("create claude_code platform");
    let group = aidog_db::create_group(&state.db, sample_group("cc-stats-g", vec![]))
        .await
        .expect("create group");
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
    .expect("set group platforms");

    // 3. MITM server 端：TLS accept + serve_plaintext **带绑定 group**（票 08 注入门）。
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let mitm_addr = listener.local_addr().unwrap();
    let signer = Arc::new(CertSigner::new(ca.clone()));
    let state_for_server = state.clone();
    let server_host = "api.anthropic.com".to_string();
    let bound_group = group.clone();
    tokio::spawn(async move {
        let (tcp_stream, _) = listener.accept().await.expect("accept client");
        let client_tls = accept_client(signer, tcp_stream, server_host.clone())
            .await
            .expect("TLS accept");
        connect::serve_plaintext(state_for_server, client_tls, &server_host, Some(bound_group))
            .await;
    });

    // 4. mock client：订阅 OAuth Bearer（非 group token）+ /v1/messages + claude-sonnet-5。
    let tcp = tokio::net::TcpStream::connect(mitm_addr)
        .await
        .expect("connect MITM");
    let connector = tokio_rustls::TlsConnector::from(Arc::new(client_config_trusting_ca(&ca)));
    let server_name = ServerName::try_from("api.anthropic.com".to_string()).unwrap();
    let tls_stream = connector
        .connect(server_name, tcp)
        .await
        .expect("TLS handshake");
    let (mut sender, conn) = hyper::client::conn::http1::Builder::new()
        .handshake(TokioIo::new(tls_stream))
        .await
        .expect("h1 client handshake");
    tokio::spawn(async move {
        let _ = conn.await;
    });
    let req = hyper::Request::builder()
        .method("POST")
        .uri("/v1/messages")
        .header("authorization", "Bearer sk-ant-oat01-subscription-oauth")
        .header("content-type", "application/json")
        .body(Body::from(
            r#"{"model":"claude-sonnet-5","messages":[{"role":"user","content":"hi"}]}"#
                .to_string(),
        ))
        .unwrap();
    let resp = sender.send_request(req).await.expect("h1 send_request");
    assert_eq!(
        resp.status(),
        StatusCode::OK,
        "订阅透传必须 1:1 relay stub 上游 200"
    );
    let body_bytes = collect_incoming_body(resp.into_body(), 64 * 1024)
        .await
        .expect("read response body");
    assert!(
        String::from_utf8_lossy(&body_bytes).contains("mitm e2e ok"),
        "响应 body 必须是 stub 上游原文（1:1 relay）"
    );

    // 5. proxy_log 全链断言：归属 / 协议 / tokens / est_cost 参考成本。
    flush_log_queue(&state).await;
    let logs = aidog_logs::list_proxy_logs(&state.db, 10, 0)
        .await
        .expect("list proxy_logs");
    let row = logs
        .iter()
        .find(|r| r.source_protocol == "claude_code")
        .expect("claude_code 透传 proxy_log 行必须存在（归属注入命中透传拦截）");
    assert_eq!(
        row.group_key, group.group_key,
        "归属必须来自 CONNECT 绑定注入（OAuth Bearer 解不出 group）"
    );
    assert_eq!(row.platform_id, plat.id, "路由必须命中 claude_code 平台");
    assert_eq!(row.model, "claude-sonnet-5", "model 必须取自请求 body");
    assert_eq!(
        row.input_tokens, 7,
        "tokens 必须从上游 usage 提取（extract_usage）"
    );
    let full = aidog_logs::get_proxy_log(&state.db, &row.id)
        .await
        .expect("query full proxy_log")
        .expect("full row must exist");
    // registry claude_code/claude-sonnet-5 官方价：7×2e-6 + 4×1e-5 = 5.4e-5。
    // 精确值断言区分 fallback 3.0 $/M（那样是 3.3e-5）——命中 registry 条目才票 09 的参考成本链。
    assert!(
        (full.est_cost - 5.4e-5).abs() < 1e-9,
        "est_cost 必须是 registry claude_code 条目官方牌价（5.4e-5），实际: {}",
        full.est_cost
    );
    assert_eq!(
        full.blocked_reason, "",
        "解密成功的 AI 路径行不得带 mitm_opaque 标记"
    );

    drop(sender);
}
