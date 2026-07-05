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
use crate::gateway::db::test_support::{sample_group, test_db};
use crate::gateway::mitm::ca::{create_and_store_root_ca, RootCa};
use crate::gateway::mitm::cert_signer::CertSigner;
use crate::gateway::mitm::tls::accept_client;
use crate::gateway::models::{CreatePlatform, GroupPlatformInput, Protocol};
use crate::gateway::{middleware::MiddlewareEngine, scheduling};
use axum::body::Body;
use hyper_util::rt::TokioIo;
use rustls::pki_types::ServerName;
use std::sync::Arc;
use tokio::io::{AsyncRead, AsyncWrite};

/// 测试用 ProxyState（内存 DB + 空 middleware/scheduler）。
async fn make_state_with_ca() -> (Arc<ProxyState>, RootCa) {
    let db = test_db().await;
    let ca = create_and_store_root_ca(&db).await.expect("create root CA in test db");
    let state = Arc::new(ProxyState {
        db: Arc::new(db),
        app: None,
        middleware: Arc::new(MiddlewareEngine::new()),
        scheduler: Arc::new(scheduling::SchedulerState::new()),
        sticky: Arc::new(scheduling::StickyTable::new()),
        log_snapshots: std::sync::Mutex::new(std::collections::HashMap::new()),
        agg_done: std::sync::Mutex::new((
            std::collections::VecDeque::new(),
            std::collections::HashSet::new(),
        )),
        listen_addr: std::sync::OnceLock::new(),
    });
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
    let plat = crate::gateway::db::create_platform(&state.db, CreatePlatform {
        name: "mitm-e2e-stub".into(),
        platform_type: Protocol::Anthropic,
        base_url: upstream_url.clone(),
        api_key: "sk-e2e-up".into(),
        extra: String::new(),
        models: None, available_models: None, endpoints: None, manual_budgets: None,
        auto_group: None, join_group_ids: None, default_level_priority: None, expires_at: None,
    }).await.expect("create platform");
    let group = crate::gateway::db::create_group(
        &state.db, sample_group("mitm-e2e-gk", vec![]),
    ).await.expect("create group");
    crate::gateway::db::set_group_platforms(&state.db, group.id, &[GroupPlatformInput {
        platform_id: plat.id, priority: Some(0), weight: Some(1), level_priority: Some(0),
    }]).await.expect("set group platforms");

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
        connect::serve_plaintext(state_for_server, client_tls, &server_host).await;
    });

    // 4. mock client：rustls client（信任 CA）→ TCP connect → TLS 握手 → hyper h1 发请求。
    let tcp = tokio::net::TcpStream::connect(mitm_addr).await.expect("connect MITM");
    let connector = tokio_rustls::TlsConnector::from(Arc::new(client_config_trusting_ca(&ca)));
    let server_name = ServerName::try_from("api.anthropic.com".to_string()).unwrap();
    let tls_stream = connector.connect(server_name, tcp).await.expect("TLS handshake");

    // hyper h1 client over TLS stream：handshake → send_request。
    let (mut sender, conn) = hyper::client::conn::http1::Builder::new()
        .handshake(TokioIo::new(tls_stream))
        .await
        .expect("h1 client handshake");
    tokio::spawn(async move { let _ = conn.await; });

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
    let logs = crate::gateway::db::list_proxy_logs(&state.db, 10, 0)
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
    assert_eq!(row.group_key, "mitm-e2e-gk", "group_key 必须从明文 Authorization 解析");
    assert_eq!(row.platform_id, plat.id, "platform_id 必须命中 stub 平台（路由生效）");
    assert_eq!(row.input_tokens, 7, "input_tokens 必须从上游 usage 提取（采集生效）");

    // est_cost 在 ProxyLogSummary 不返回，查完整行验 cost 记账（盲转恒 0，AI 路径非 0）。
    let full = crate::gateway::db::get_proxy_log(&state.db, &row.id)
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
