//! P1 HTTP CONNECT 隧道（标准 http_proxy）+ P3 MITM 解密分流（ST4）。
//!
//! 客户端配 `http_proxy=127.0.0.1:<port>` 后任意 HTTP/HTTPS 流量经 CONNECT 隧道。
//! - **P1 盲转**（默认）：未命中 MITM 白名单 / pinning_suspect / CA 未启用 → TCP 字节双向
//!   透传，不解密 HTTPS，只记 proxy_log 元数据（host/status/duration/platform_id），
//!   不计费、不统计字节（用户锁 YAGNI）。
//! - **P3 MITM**（ST4）：白名单命中 && 非 suspect && CA 已启用 → 上游 TLS 预检（pinning
//!   探测）成功后 accept 客户端（假 CA 签 leaf）+ 双向桥接两段 TLS 流（密文透传，不解 HTTP）。
//!   明文 Request 灌 forward_attempt 链是 ST5，本阶段只做 TLS 隧道建立。
//!
//! 关键技术点（research 结论 1 + 5）:
//! - axum 0.8 `axum::serve` 底层 `hyper_util auto + upgrades`，CONNECT upgrade 默认开启
//! - hyper-util 用私有 `Rewind<T>` 包 `TokioIo<TcpStream>`（axum::serve 喂入的 IO 类型），
//!   需 `auto::upgrade::downcast::<TokioIo<TcpStream>>` 取回底层流 + 预读缓冲
//! - CONNECT 响应 `200 + 空 body`，**禁带 `Connection: upgrade` header**（hyper h1 role.rs
//!   380-384 对 CONNECT 2xx 响应禁止 content-length/transfer-encoding）
//! - `tokio::io::copy` 双向 + `tokio::join!`；字节 u64 返回但 P1/ST4 都不入库
//!
//! ST4 分流依据：`.trellis/tasks/07-03-proxy-relay-mitm/design.md` §4 + 失败模式表。

use super::*;
use hyper_util::rt::TokioIo;
use tokio::io::{AsyncRead, AsyncWrite};

/// 双向 IO 桥接：`a` ↔ `b` 字节透传，任一方向 EOF/err 即整体 drop 触发对端 FIN。
///
/// ponytail: 抽公共 helper —— blind_relay（client TCP ↔ upstream TCP）与 MITM 桥接
/// （client TLS ↔ upstream TLS）IO 模式一致（split + join copy），仅流类型不同。
/// 泛型覆盖 `TokioIo<TokioIo<TcpStream>>` / `ServerTlsStream<IO>` / `ClientTlsStream<TcpStream>`。
async fn bridge_bidir<A, B>(a: A, b: B)
where
    A: AsyncRead + AsyncWrite + Unpin,
    B: AsyncRead + AsyncWrite + Unpin,
{
    let (mut ar, mut aw) = tokio::io::split(a);
    let (mut br, mut bw) = tokio::io::split(b);
    let _ = tokio::join!(
        tokio::io::copy(&mut ar, &mut bw),
        tokio::io::copy(&mut br, &mut aw),
    );
}

/// CONNECT handler — 在 `handle_proxy_core` 早期按 `Method::CONNECT` 分流进入
/// （axum fallback 命中 authority-form URI 走此路径），不破现有 /proxy AI 协议 path 路由。
///
/// 返回 200 + 空 body 建立隧道；实际双向转发在 spawn 的 task 内完成
/// （response 先返回，upgrade future 在 task 内 await）。
pub(crate) async fn handle_connect(
    AxumState(state): AxumState<Arc<ProxyState>>,
    req: Request,
) -> Response {
    // ponytail: CONNECT 是 authority-form URI（RFC 7231 §4.3.6），path() 返空（http 标准），
    // authority 在 uri().authority()；Host header 兜底。三源皆空 = 客户端坏请求，早返 400
    // 不进 connect 路径（避免空 target connect("") 必败 502，DB 实证 6 连发全 502 request_url 空）。
    let target = {
        let from_path = req.uri().path().trim_start_matches('/');
        let from_auth = req.uri().authority().map(|a| a.as_str()).unwrap_or("");
        let from_host = req
            .headers()
            .get(axum::http::header::HOST)
            .and_then(|h| h.to_str().ok())
            .unwrap_or("");
        [from_path, from_auth, from_host]
            .iter()
            .find(|s| !s.is_empty())
            .copied()
            .unwrap_or("")
            .to_string()
    };
    tracing::info!(uri = ?req.uri(), method = ?req.method(), target = %target, "connect recv");
    if target.is_empty() {
        tracing::warn!(uri = ?req.uri(), "connect: missing target, returning 400");
        return (StatusCode::BAD_REQUEST, "CONNECT missing target").into_response();
    }
    let host_only = target.rsplit_once(':').map(|(h, _)| h).unwrap_or(&target).to_string();
    let on_upgrade = hyper::upgrade::on(req);

    // P1 平台匹配：仅 host（无 apikey，HTTPS 未解密）。未命中 → None（落 platform_id=0）。
    let platform_id = match_platform_by_host(&state.db, &host_only).await.unwrap_or(0);

    // 日志开关：disabled 时整条不落 proxy_log（与 upsert_log 早退语义一致）。
    let settings = get_log_settings(&state.db).await;
    let log_enabled = settings.enabled;

    let request_id = uuid::Uuid::new_v4().simple().to_string();
    let start = std::time::Instant::now();

    // ST4 MITM 候选预判定：白名单命中 && 非 suspect。（DB 白名单匹配是 IO，suspect 查询是内存锁。）
    // 候选为 true 时跳过 P1 的「spawn 前 TCP 验证」（MITM 路径在 spawn 内自管 TCP 连接 +
    // pinning 预检，失败写终态 502 或降级 blind_relay）；候选为 false 走 P1 完整逻辑。
    let mitm_candidate = super::super::mitm::whitelist::matches_db(&state.db, &host_only).await
        && !super::super::mitm::mitm_state().is_suspect(&host_only).await;
    let mitm_state = super::super::mitm::mitm_state();

    // ── 非 MITM 候选：P1 完整逻辑（spawn 前 TCP 验证 + 502 早返，零回归）─────────────
    if !mitm_candidate {
        let upstream = match tokio::net::TcpStream::connect(&target).await {
            Ok(s) => s,
            Err(e) => {
                tracing::warn!(error = %e, target = %target, "connect: upstream TCP failed");
                if log_enabled {
                    upsert_connect_log(
                        &state, request_id, String::new(), platform_id,
                        target.clone(), 502, start.elapsed().as_millis() as i32,
                    ).await;
                }
                return (StatusCode::BAD_GATEWAY, format!("connect {target} failed: {e}")).into_response();
            }
        };
        return spawn_blind_relay(
            state, on_upgrade, upstream, target, request_id, platform_id, start, log_enabled,
        );
    }

    // ── MITM 候选：直接 spawn（spawn 内 pinning 预检 / accept / bridge，失败降级 blind_relay）
    // ponytail: 不做 spawn 前 TCP 验证 —— MITM 路径需先 connect_upstream（含 TCP + TLS）做
    // pinning 探测，若此处再预连 TCP 是重复；TCP 失败 / pinning fail / CA 缺失等都在 spawn
    // task 内处理 + 降级 blind_relay（blind_relay 路径自连 TCP）。响应固定 200（MITM 内部
    // 失败对客户端透明，降级后 blind_relay 正常建隧道；仅客户端不信任 CA 时握手 fail 隧道断）。
    let resp = Response::builder()
        .status(StatusCode::OK)
        .body(Body::empty())
        .unwrap();
    let st = state.clone();
    tokio::spawn(async move {
        let upgraded = match on_upgrade.await {
            Ok(u) => u,
            Err(e) => {
                tracing::warn!(error = %e, target = %target, "connect upgrade failed");
                if log_enabled {
                    upsert_connect_log(
                        &st, request_id, String::new(), platform_id,
                        target, 499, start.elapsed().as_millis() as i32,
                    ).await;
                }
                return;
            }
        };
        // hyper-util auto 用私有 Rewind<T> 包 IO；downcast 回 TokioIo<TcpStream>
        // （axum::serve 喂入的 IO 类型）+ 拿预读 buf。
        let parts = match hyper_util::server::conn::auto::upgrade::downcast::<TokioIo<tokio::net::TcpStream>>(upgraded) {
            Ok(p) => p,
            Err(upgraded) => {
                // downcast 失败（理论上不应）→ 退化 blind_relay（裸 Upgraded，不进 MITM）。
                tracing::warn!(target = %target, "downcast TokioIo<TcpStream> failed, blind relay");
                let client = TokioIo::new(upgraded);
                blind_relay_after_connect(&st, client, &target, request_id, platform_id, start, log_enabled, &[]).await;
                return;
            }
        };
        // parts.io = TokioIo<TcpStream>（impl hyper Read/Write）；包一层 TokioIo 转 tokio IO。
        // 客户端连接类型 = TokioIo<TokioIo<TcpStream>>（impl tokio AsyncRead/AsyncWrite）。
        let client = TokioIo::new(parts.io);

        // ST4 分流：MITM 候选 && read_buf 空（合法 CONNECT 客户端 read_buf 必空，RFC 7231
        // §4.3.6 要求客户端收 200 才发数据）→ 走 MITM；read_buf 非空（违规 pipelining）→
        // 降级 blind_relay（blind_relay 现有 flush 逻辑处理预读字节）。
        //
        // ponytail: MITM 路径不处理 read_buf —— accept_client 需把预读字节 prepend 到 TLS
        // 输入流前面（组合 AsyncRead），复杂度 vs 收益失衡（合法客户端 read_buf 空）。
        // read_buf 非空降级 blind_relay，行为保守正确。
        let client_for_blind: Option<_> = if parts.read_buf.is_empty() {
            let outcome = handle_mitm(
                &st, mitm_state, client, &target, &host_only,
                request_id.clone(), platform_id, start, log_enabled,
            ).await;
            if outcome.handled {
                return; // MITM 成功建隧道或终态日志已写
            }
            // MITM 降级（CA 未启用 / pinning / IO error）→ 拿回 client 走 blind_relay
            tracing::info!(target = %target, reason = outcome.fallback_reason, "mitm degraded to blind relay");
            outcome.client_return
        } else {
            Some(client)
        };

        // ── blind_relay 降级 / read_buf 非空 路径 ──────────────────────────────
        let mut client = client_for_blind.expect("client_for_blind set in both branches above");
        // 预读字节 flush（read_buf 非空时；MITM 降级路径 read_buf 必空，此 flush 为 no-op）。
        if !parts.read_buf.is_empty() {
            let _ = tokio::io::AsyncWriteExt::write_all(&mut client, &parts.read_buf).await;
        }
        blind_relay_after_connect(&st, client, &target, request_id, platform_id, start, log_enabled, &[]).await;
    });

    resp
}

// ── P1 blind_relay 路径 ────────────────────────────────────────────────────────

/// P1 blind_relay：上游 TCP 已连，spawn 双向 copy（保留 P1 完整行为含 read_buf flush）。
///
/// ponytail: 8 参数都是必要的隧道上下文（无冗余），打包 struct 仅在这几个 blind_relay
/// helper 间传递无复用价值，YAGNI；allow clippy::too_many_arguments。
#[allow(clippy::too_many_arguments)]
fn spawn_blind_relay(
    state: Arc<ProxyState>,
    on_upgrade: hyper::upgrade::OnUpgrade,
    upstream: tokio::net::TcpStream,
    target: String,
    request_id: String,
    platform_id: u64,
    start: std::time::Instant,
    log_enabled: bool,
) -> Response {
    let resp = Response::builder()
        .status(StatusCode::OK)
        .body(Body::empty())
        .unwrap();
    tokio::spawn(async move {
        let upgraded = match on_upgrade.await {
            Ok(u) => u,
            Err(e) => {
                tracing::warn!(error = %e, target = %target, "connect upgrade failed");
                if log_enabled {
                    upsert_connect_log(
                        &state, request_id, String::new(), platform_id,
                        target, 499, start.elapsed().as_millis() as i32,
                    ).await;
                }
                return;
            }
        };
        let parts = match hyper_util::server::conn::auto::upgrade::downcast::<TokioIo<tokio::net::TcpStream>>(upgraded) {
            Ok(p) => p,
            Err(upgraded) => {
                tracing::warn!(target = %target, "downcast TokioIo<TcpStream> failed, blind relay");
                let client = TokioIo::new(upgraded);
                bridge_bidir(client, upstream).await;
                log_connect_success(&state, request_id, platform_id, target, start, log_enabled).await;
                return;
            }
        };
        let mut client = TokioIo::new(parts.io);
        if !parts.read_buf.is_empty() {
            let _ = tokio::io::AsyncWriteExt::write_all(&mut client, &parts.read_buf).await;
        }
        bridge_bidir(client, upstream).await;
        log_connect_success(&state, request_id, platform_id, target, start, log_enabled).await;
    });
    resp
}

/// blind_relay 辅助：上游 TCP 未连，先 connect 再 bridge。MITM 降级路径 / downcast 失败路径共用。
///
/// `prefetch` 是已从客户端预读的字节（须先 flush 到上游）；通常空（合法 CONNECT 客户端
/// 收 200 才发数据），blind_relay 调用方负责 flush，此参数留空数组占位（接口对称）。
///
/// ponytail: 抽出避免 blind_relay 逻辑在 handle_connect spawn 内重复（downcast 失败 +
/// MITM 降级 + read_buf 非空三路径都走 blind_relay）。签名收 `&str` target 因调用方已拥有
/// String，借用避免 move 后还要用（tracing 等）。
/// ponytail: 8 参数同 spawn_blind_relay（隧道上下文），allow clippy::too_many_arguments。
#[allow(clippy::too_many_arguments)]
async fn blind_relay_after_connect(
    st: &Arc<ProxyState>,
    client: impl AsyncRead + AsyncWrite + Unpin,
    target: &str,
    request_id: String,
    platform_id: u64,
    start: std::time::Instant,
    log_enabled: bool,
    _prefetch: &[u8],
) {
    match tokio::net::TcpStream::connect(target).await {
        Ok(upstream) => {
            bridge_bidir(client, upstream).await;
            log_connect_success(st, request_id, platform_id, target.to_string(), start, log_enabled).await;
        }
        Err(e) => {
            tracing::warn!(error = %e, target, "blind relay: upstream TCP failed");
            log_connect_502(st, request_id, platform_id, target.to_string(), start, log_enabled).await;
        }
    }
}

// ── ST4 MITM 路径 ──────────────────────────────────────────────────────────────

/// MITM 路径处理结果。`handled=true` 表示 MITM 已接管（成功或终态日志已写）；
/// `handled=false` 表示降级 blind_relay，`client_return` 携带未被消费的客户端流。
struct MitmOutcome<IO> {
    handled: bool,
    /// 降级时的原因（handled=false 时填，tracing 用）。
    fallback_reason: &'static str,
    /// 降级时还给调用方的客户端流（handled=true 时为 None —— 已被 MITM 消费或 drop）。
    client_return: Option<IO>,
}

/// ST4 MITM 路径：上游 TLS 预检（pinning 探测）→ accept 客户端 → 双向桥接两段 TLS 流。
///
/// **预检顺序**（design 失败模式表）：先 connect_upstream 探测上游 TLS，pinning fail 标
/// suspect + 降级（client 未被 accept 消费，完整还给调用方）；成功才 accept_client 与
/// 客户端握手。这样 pinning 场景客户端从未见假证书，blind_relay 可正常建真隧道。
/// abort-retry 方案（先 accept 再 connect）在 pinning fail 时需重置已 accept 的客户端
/// TLS 状态机，复杂且易错；预检方案语义干净。
///
/// ponytail: signer 加载失败 / pinning / IO error 降级时 client 完整归还（未被碰），
/// blind_relay 走正常路径；accept_client 失败（client 已被 accept 消费）走 handled=true
/// 终态 502（无法降级，客户端 TLS 状态机已推进）。
/// ponytail: 9 参数是必要的 MITM 隧道上下文（state/mitm_state/client/target/host + 日志四元组
/// request_id/platform_id/start/log_enabled），打包 struct 仅在本函数传递无复用，YAGNI；
/// allow clippy::too_many_arguments。
#[allow(clippy::too_many_arguments)]
async fn handle_mitm<IO>(
    st: &Arc<ProxyState>,
    mitm_state: &'static super::super::mitm::MitmState,
    client: IO,
    target: &str,
    host_only: &str,
    request_id: String,
    platform_id: u64,
    start: std::time::Instant,
    log_enabled: bool,
) -> MitmOutcome<IO>
where
    IO: AsyncRead + AsyncWrite + Unpin + Send,
{
    // 1. 取 / 构造 CertSigner（首次从 DB load RootCa；DB 无 CA = 用户未启用 MITM → 降级）。
    let signer = match mitm_state.signer_or_init(&st.db).await {
        Ok(Some(s)) => s,
        Ok(None) => {
            return MitmOutcome {
                handled: false,
                fallback_reason: "CA not enabled (no mitm_ca row)",
                client_return: Some(client),
            };
        }
        Err(e) => {
            tracing::warn!(error = %e, host = host_only, "mitm: load signer failed, degrading");
            return MitmOutcome {
                handled: false,
                fallback_reason: "signer init error",
                client_return: Some(client),
            };
        }
    };

    // 2. 预检 connect_upstream：先连上游 TCP + TLS 握手（pinning 探测）。
    let upstream_tcp = match tokio::net::TcpStream::connect(target).await {
        Ok(s) => s,
        Err(e) => {
            tracing::warn!(error = %e, target, "mitm: upstream TCP failed, terminal 502");
            log_connect_502(st, request_id, platform_id, target.to_string(), start, log_enabled).await;
            // TCP 失败非 pinning，不标 suspect；client 不再有用（上游连不上 blind_relay 也连不上）。
            // handled=true 避免调用方 blind_relay 重试已确定连不上的 target。
            drop(client);
            return MitmOutcome { handled: true, fallback_reason: "", client_return: None };
        }
    };
    match super::super::mitm::tls::connect_upstream(host_only, upstream_tcp).await {
        super::super::mitm::tls::UpstreamTlsOutcome::Connected(upstream_tls) => {
            // 3. accept 客户端 TLS（假 CA 签 leaf，SNI fallback = CONNECT target host）。
            //    失败（client 不信任 CA / 网络断）→ client 已被 accept 消费，无法降级 blind_relay。
            //    写终态 502 + handled=true（客户端 TLS 握手失败隧道断，blind_relay 也救不回）。
            let client_tls = match super::super::mitm::tls::accept_client(
                signer, client, host_only.to_string(),
            ).await {
                Ok(s) => s,
                Err(e) => {
                    tracing::warn!(
                        error = %e, host = host_only,
                        "mitm: client TLS handshake failed (CA not trusted?), terminal 502"
                    );
                    log_connect_502(st, request_id, platform_id, target.to_string(), start, log_enabled).await;
                    return MitmOutcome { handled: true, fallback_reason: "", client_return: None };
                }
            };

            // ST5 明文路径：ALPN 协商协议决定走 http/1.1 forward 还是降级密文桥接。
            // ALPN 协商 h2 → 客户端会发 HTTP/2 帧；本 subtask 只解 http/1.1（D9 归 ST6）。
            // h2 降级：桥接 client_tls ↔ upstream_tls（都已建立），等价 ST4 字节透传；明文 copy
            // 两端解密后字节级与 blind_relay 等价。upstream_tls 在 http/1.1 路径丢弃（forward_attempt
            // 自连上游），h2 路径复用做密文桥接，避免空耗预检连接。
            let alpn = client_tls.get_ref().1.alpn_protocol();
            if alpn == Some(b"h2") {
                tracing::info!(host = host_only, "mitm: ALPN h2 negotiated, ST6 pending, fallback to byte bridge");
                bridge_bidir(client_tls, *upstream_tls).await;
                log_connect_success(st, request_id, platform_id, target.to_string(), start, log_enabled).await;
                return MitmOutcome { handled: true, fallback_reason: "", client_return: None };
            }

            // 4. ST5 明文 forward：http/1.1 协商成功 → 在 client_tls 上读明文 HTTP Request →
            //    灌入 handle_proxy_core（middleware/路由/headers/retry/采集/forward_attempt 全套，
            //    95% 复用）→ 响应明文回写 client_tls（hyper 写回，TLS 层自动加密回客户端）。
            //    上游由 forward_attempt 内部 http_client 自连（真证书），预检 upstream_tls 丢弃。
            //    proxy_log 走 handle_proxy_core 内部 upsert_log 全量 AI 记账（body + stats_agg + cost），
            //    **不走 upsert_connect_log**（盲转专用，避免污染统计）。
            //
            // ponytail: http/1.1 only — ST6 加 h2 协议转换（auto Builder + http2 feature）。
            // ponytail: 预检 upstream_tls 在明文路径被丢弃（forward_attempt 自连），浪费 1 条 TCP+TLS。
            // 保留预检是为 pinning 探测（探针必须先于 accept 确认上游可信，否则 client 已 accept
            // 后发现 pinning fail 无法干净降级）。pinning fail 频率低，浪费可接受。
            drop(upstream_tls);
            serve_plaintext_http(st.clone(), client_tls, host_only).await;
            MitmOutcome { handled: true, fallback_reason: "", client_return: None }
        }
        super::super::mitm::tls::UpstreamTlsOutcome::PinningSuspect { host, error } => {
            // pinning fail → 标 suspect（后续 CONNECT 该 host 跳过 MITM 候选）+ 降级 blind_relay。
            tracing::warn!(
                host = %host, error = %error,
                "mitm: upstream TLS handshake failed (pinning suspect), flagging + degrading"
            );
            mitm_state.mark_suspect(host).await;
            MitmOutcome {
                handled: false,
                fallback_reason: "upstream pinning suspect",
                client_return: Some(client),
            }
        }
        super::super::mitm::tls::UpstreamTlsOutcome::IoError(e) => {
            // 非 pinning IO 错（TCP 断 / 超时）→ 不标 suspect + 降级 blind_relay（重试一次）。
            tracing::warn!(error = %e, target, "mitm: upstream TLS IO error, degrading");
            MitmOutcome {
                handled: false,
                fallback_reason: "upstream TLS IO error",
                client_return: Some(client),
            }
        }
    }
}

// ── ST5 明文 forward：在 client_tls 上读明文 HTTP → 灌 handle_proxy_core ──────

/// 收集任意 `http_body::Body` 为 `Bytes`，限制总字节防 OOM。
///
/// ponytail: 手写 poll_frame 循环 —— axum::body::to_bytes 签名锁死 `axum::body::Body` 类型，
/// hyper Incoming 不兼容；http_body_util::BodyExt::collect 是传递依赖不直接可见。
/// 10 行手写 < 加一个 dep，最小可工作解。limit 触发即 err（与 handle_proxy_core 同款语义）。
async fn collect_body<B>(body: B, limit: usize) -> Result<hyper::body::Bytes, String>
where
    B: hyper::body::Body + Unpin,
    B::Error: std::fmt::Display,
    B::Data: AsRef<[u8]>,
{
    // hyper::body::BytesMut 不存在；Vec<u8> 收集 + 末尾 freeze 成 Bytes。
    // ponytail: B::Data: AsRef<[u8]> 约束让 extend_from_slice 可用（hyper::body::Bytes impl Buf，
    // 但 AsRef<[u8]> 是最简 copy 路径，避免引 bytes::Buf trait 路径）。
    let mut buf: Vec<u8> = Vec::new();
    // tokio::pin! 安全 pin（B: Unpin，pin 无 unsafe 语义；pin 后 poll_frame 需 Pin<&mut Self>）。
    tokio::pin!(body);
    loop {
        // std::future::poll_fn 避免手写 Future；cx 由 poll_fn 注入。
        let frame = std::future::poll_fn(|cx| body.as_mut().poll_frame(cx)).await;
        match frame {
            None => return Ok(hyper::body::Bytes::from(buf)),
            Some(Ok(frame)) => {
                if let Ok(data) = frame.into_data() {
                    let bytes: &[u8] = data.as_ref();
                    if buf.len() + bytes.len() > limit {
                        return Err(format!("body too large (limit {limit} bytes)"));
                    }
                    buf.extend_from_slice(bytes);
                }
                // 非 data frame（trailers 等）忽略。
            }
            Some(Err(e)) => return Err(format!("body read error: {e}")),
        }
    }
}



/// ST5 明文 forward：在已 accept 的 client_tls（明文）流上用 hyper http1 server 读
/// 明文 HTTP Request，每条 Request 构造 axum Request 灌入 `handle_proxy_core`（middleware /
/// 路由 / forward_attempt 全套），响应明文回写 client_tls（hyper 写回，TLS 层自动加密
/// 回客户端）。支持 HTTP/1.1 keep-alive（一个 TLS 会话内多 Request 循环）。
///
/// **proxy_log 记账**：明文路径走 AI 请求全量记账（handle_proxy_core 内 upsert_log，含 body
/// + stats_agg + cost），**不走 upsert_connect_log**（盲转专用，避免污染统计）。
///
/// ponytail: http/1.1 only —— ST6 加 h2（改 auto Builder + 加 hyper http2 feature）。
/// ponytail: body 完整读（collect_body）而非 streaming 灌入 —— handle_proxy_core 内
/// 已 to_bytes(10MB) 读一次，此处再 stream 无收益且增复杂度，等价直接 collect。
/// ponytail: Infallible 错类型 —— handle_proxy_core 返 Response 不返 Err（错误已落 4xx/5xx body），
/// service_fn 不需要错误传播路径。
/// ponytail: 不复用 handle_proxy_inner 的 RequestLogGuard —— MITM 明文路径客户端断连时
/// handle_proxy_core 内部各阶段已 upsert_log 终态，499 兜底语义重叠；YAGNI 不重复 guard。
async fn serve_plaintext_http<S>(
    state: Arc<ProxyState>,
    client_tls: S,
    host_only: &str,
) where
    S: AsyncRead + AsyncWrite + Unpin + Send,
{
    // TokioIo 包装：rustls server TlsStream impl tokio AsyncRead/Write，TokioIo 转 hyper Read/Write。
    let io = TokioIo::new(client_tls);
    let svc = hyper::service::service_fn(move |req: hyper::Request<hyper::body::Incoming>| {
        let st = state.clone();
        async move {
            // hyper::Request<Incoming> → axum::Request<axum::body::Body>：
            // parts（method/uri/headers）通用，body 收 Bytes 后 Body::from 包装。
            let (parts, body) = req.into_parts();
            let bytes = match collect_body(body, 10 * 1024 * 1024).await {
                Ok(b) => b,
                Err(e) => {
                    // body 读失败返 400（与 handle_proxy_core 内同款错误语义）。
                    tracing::warn!(error = %e, host = host_only, "mitm plaintext: read body failed");
                    let mut resp = hyper::Response::builder().status(StatusCode::BAD_REQUEST);
                    if let Some(h) = resp.headers_mut() {
                        h.insert(axum::http::header::CONTENT_TYPE, axum::http::HeaderValue::from_static("text/plain"));
                    }
                    return Ok::<_, std::convert::Infallible>(
                        resp.body(axum::body::Body::from(format!("read body error: {e}")))
                            .expect("static response build"),
                    );
                }
            };
            let axum_req = Request::from_parts(parts, axum::body::Body::from(bytes));
            // 灌入 handle_proxy_core —— 走完整 AI 请求链（middleware/路由/forward_attempt/采集）。
            //
            // 直调 handle_proxy_core 而非 handle_proxy：handle_proxy → handle_proxy_inner 含
            // CONNECT 分流 → handle_connect（与当前 spawn 互递归，Send 死锁）。core 不分流
            // CONNECT（分流已在 handle_proxy_inner 顶部），明文 Request method 非 CONNECT 必走
            // AI 路径，无递归。request_id + span + 499 guard 在本地构造（等价 handle_proxy_inner
            // 的 guard 语义，客户端断连时 Drop 补写终态 499）。
            let request_id = uuid::Uuid::new_v4().simple().to_string();
            let span = tracing::info_span!(
                "req",
                trace_id = %&request_id[..8],
                request_id = %request_id,
                mitm = %host_only,
            );
            let resp: Response = handle_proxy_core(AxumState(st.clone()), axum_req, request_id)
                .instrument(span)
                .await;
            Ok::<_, std::convert::Infallible>(resp)
        }
    });

    // http1 keep-alive：一个 TLS 会话内可循环多 Request（client 复用连接发多请求）。
    // serve_connection future 在 client 关闭连接 / 协议错时返 Err（tracing 后接受）。
    if let Err(e) = hyper::server::conn::http1::Builder::new()
        .serve_connection(io, svc)
        .await
    {
        tracing::debug!(error = %e, host = host_only, "mitm plaintext: http1 connection ended");
    }
}

// ── proxy_log 写入 helper（P1 + MITM 共用）─────────────────────────────────────

/// 隧道建立成功写 proxy_log（status=200）。
async fn log_connect_success(
    st: &Arc<ProxyState>,
    request_id: String,
    platform_id: u64,
    target: String,
    start: std::time::Instant,
    log_enabled: bool,
) {
    if !log_enabled {
        return;
    }
    upsert_connect_log(
        st, request_id, String::new(), platform_id,
        target, 200, start.elapsed().as_millis() as i32,
    ).await;
}

/// 上游失败写 proxy_log 终态（status=502）。
async fn log_connect_502(
    st: &Arc<ProxyState>,
    request_id: String,
    platform_id: u64,
    target: String,
    start: std::time::Instant,
    log_enabled: bool,
) {
    if !log_enabled {
        return;
    }
    upsert_connect_log(
        st, request_id, String::new(), platform_id,
        target, 502, start.elapsed().as_millis() as i32,
    ).await;
}
