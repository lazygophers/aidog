//! P1 HTTP CONNECT 隧道（标准 http_proxy）+ P3 MITM 解密分流（ST4）。
//!
//! 客户端配 `http_proxy=127.0.0.1:<port>` 后任意 HTTP/HTTPS 流量经 CONNECT 隧道。
//! - **P1 盲转**（默认）：未命中 MITM 白名单 / pinning_suspect / CA 未启用 → TCP 字节双向
//!   透传，不解密 HTTPS，只记 proxy_log 元数据（host/status/duration/platform_id），
//!   不计费、不统计字节（用户锁 YAGNI）。
//! - **P3 MITM**（ST4+ST5+ST6）：白名单命中 && 非 suspect && CA 已启用 → 上游 TLS 预检
//!   （pinning 探测）成功后 accept 客户端（假 CA 签 leaf）+ 在明文 TLS 流上用 hyper-util
//!   **auto** server（D9：H2 preface 自动分发 h1/h2）读明文 HTTP Request 灌入 handle_proxy_core
//!   （forward_attempt 全套：middleware/路由/headers/retry/采集）。h1 keep-alive 循环，
//!   h2 多路复用流（一个 TLS 连接多 Request，每流各灌 core）。
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
use base64::Engine as _;
use hyper_util::rt::TokioIo;
use tokio::io::{AsyncRead, AsyncWrite};

/// 双向 IO 桥接：`a` ↔ `b` 字节透传，任一方向 EOF/err 即整体 drop 触发对端 FIN。
/// 返回 `(a→b 字节数, b→a 字节数)`（IO err 按 0 计——隧道已断，字节计数仅观测用途）。
///
/// ponytail: 抽公共 helper —— blind_relay（client TCP ↔ upstream TCP）与 MITM 桥接
/// （client TLS ↔ upstream TLS）IO 模式一致（split + join copy），仅流类型不同。
/// 泛型覆盖 `TokioIo<TokioIo<TcpStream>>` / `ServerTlsStream<IO>` / `ClientTlsStream<TcpStream>`。
async fn bridge_bidir<A, B>(a: A, b: B) -> (u64, u64)
where
    A: AsyncRead + AsyncWrite + Unpin,
    B: AsyncRead + AsyncWrite + Unpin,
{
    let (mut ar, mut aw) = tokio::io::split(a);
    let (mut br, mut bw) = tokio::io::split(b);
    let (r1, r2) = tokio::join!(
        tokio::io::copy(&mut ar, &mut bw),
        tokio::io::copy(&mut br, &mut aw),
    );
    (r1.unwrap_or(0), r2.unwrap_or(0))
}

// ── CONNECT 层代理认证（spec cc-sub-mitm D2：Proxy-Authorization: Basic username = group 名）──

/// CONNECT 握手 `Proxy-Authorization` 解析结果。
// pub(crate) 仅为 test_connect.rs 断言变体用（编译期契约锚点）；生产消费全在本文件。
#[derive(Debug)]
pub(crate) enum ConnectAuth {
    /// 无认证头 → 现状零改动（host 匹配归属，不绑定 group）。
    Absent,
    /// 头存在但格式坏（非 Basic / base64 / UTF-8 / 缺 `:` 分隔）→ 407。
    Malformed,
    /// 格式合法但用户名非 URL-safe 或查无此 group → 不绑定；尝试值落 connect log 元数据。
    Unknown(String),
    /// 命中 group（按 name 匹配）→ 绑定本连接记账归属，注入 serve_plaintext → handle_proxy_core。
    /// Box 压 variant 尺寸差（Group ~232B vs String 24B，clippy large_enum_variant）。
    Bound(Box<Group>),
}

/// group 名 URL-safe 判定（undici 对 proxy URL userinfo 做 decodeURIComponent，非 URL-safe
/// 名字进 HTTPS_PROXY 必坏；CONNECT 端同规则收紧，非法用户名按未知 group 处理不绑定）。
pub(crate) fn is_url_safe_group_name(s: &str) -> bool {
    !s.is_empty()
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
}

/// 解析 `Proxy-Authorization: Basic <base64(username:password)>` 的 username。
/// `Ok(None)` = 无认证头；`Err(())` = 格式坏（认证失败）。
pub(crate) fn basic_proxy_username(headers: &axum::http::HeaderMap) -> Result<Option<String>, ()> {
    let Some(v) = headers
        .get("proxy-authorization")
        .and_then(|h| h.to_str().ok())
    else {
        return Ok(None);
    };
    let Some(b64) = v.strip_prefix("Basic ").map(str::trim) else {
        return Err(());
    };
    let decoded = base64::engine::general_purpose::STANDARD
        .decode(b64)
        .map_err(|_| ())?;
    let decoded = String::from_utf8(decoded).map_err(|_| ())?;
    let (username, _) = decoded.split_once(':').ok_or(())?;
    Ok(Some(username.to_string()))
}

/// CONNECT 认证解析 + group 解析（username 按 `group.name` 匹配；无头零 DB 往返）。
pub(crate) async fn resolve_connect_auth(db: &Db, headers: &axum::http::HeaderMap) -> ConnectAuth {
    let username = match basic_proxy_username(headers) {
        Ok(None) => return ConnectAuth::Absent,
        Ok(Some(u)) => u,
        Err(()) => return ConnectAuth::Malformed,
    };
    if !is_url_safe_group_name(&username) {
        tracing::warn!(username = %username, "connect auth: username not URL-safe, no group binding");
        return ConnectAuth::Unknown(username);
    }
    let groups = match aidog_db::list_groups(db).await {
        Ok(g) => g,
        Err(e) => {
            tracing::warn!(error = %e, "connect auth: list_groups failed, no group binding");
            return ConnectAuth::Unknown(username);
        }
    };
    match groups.into_iter().find(|g| g.name == username) {
        Some(g) => ConnectAuth::Bound(Box::new(g)),
        None => {
            tracing::warn!(username = %username, "connect auth: username matches no group, no binding");
            ConnectAuth::Unknown(username)
        }
    }
}

/// CONNECT handler — 在 `handle_proxy_core` 早期按 `Method::CONNECT` 分流进入
/// （axum fallback 命中 authority-form URI 走此路径），不破现有 /proxy AI 协议 path 路由。
///
/// 返回 200 + 空 body 建立隧道；实际双向转发在 spawn 的 task 内完成
/// （response 先返回，upgrade future 在 task 内 await）。
pub(crate) async fn handle_connect(
    AxumState(state): AxumState<Arc<ProxyState>>,
    req: Request,
    request_id: String,
) -> Response {
    // P2-D：复用 handle_proxy 的 req span —— CONNECT 子日志行携带 trace_id/request_id，
    // 可从 stderr 串回 proxy_log.id（与 AI 路径 upsert_log 行对齐）。
    let span = tracing::info_span!(
        "req", trace_id = %&request_id[..8], request_id = %request_id,
    );
    handle_connect_inner(state, req, request_id)
        .instrument(span)
        .await
}

async fn handle_connect_inner(
    state: Arc<ProxyState>,
    req: Request,
    request_id: String,
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
        let mut r = (StatusCode::BAD_REQUEST, "CONNECT missing target").into_response();
        inject_trace_header(&mut r);
        return r;
    }
    let host_only = target
        .rsplit_once(':')
        .map(|(h, _)| h)
        .unwrap_or(&target)
        .to_string();
    tracing::info!(target = %target, host_only = %host_only, request_id = %request_id, "connect parsed target/host");
    // CONNECT 认证解析先于 upgrade::on(req)（后者消费 req，headers 不可再读）。
    let connect_auth = resolve_connect_auth(&state.db, req.headers()).await;
    let on_upgrade = hyper::upgrade::on(req);

    // P1 平台匹配：仅 host（无 apikey，HTTPS 未解密）。未命中 → 0（无平台可挂记账）。
    let platform_id = match_platform_by_host(&state.db, &host_only)
        .await
        .map(|(id, _)| id)
        .unwrap_or(0u64);

    // 日志开关：disabled 时整条不落 proxy_log（与 upsert_log 早退语义一致）。
    // settings + system_timeout 一次缓存借齐（每请求 ≥2 次 DB 缓存读 → 1 次 read lock）。
    let (settings, system_timeout) = {
        let c = state.settings_cache.read().await;
        (c.log_settings.clone(), c.system_timeout.clone())
    };
    let log_enabled = settings.enabled;

    // P2-A：connect 阶段超时（system 级；CONNECT 无 group/model_mapping 输入，仅 system 兜底）。
    // 隧道建后 idle 不套超时（避长连接 SSE/WebSocket over TLS 误杀），仅 TCP 握手阶段超时。
    // system_timeout 已在上块从缓存借出。
    let conn_timeout_secs = if system_timeout.connect_timeout_secs > 0 {
        system_timeout.connect_timeout_secs
    } else {
        10
    };
    let start = std::time::Instant::now();

    // ── 认证结果分派：格式坏 → 407；其余不阻断隧道（未知 group 不绑定，仅落元数据）──
    if matches!(connect_auth, ConnectAuth::Malformed) {
        tracing::warn!(target = %target, "connect: malformed Proxy-Authorization, returning 407");
        ConnectLogCtx {
            request_id,
            platform_id: 0,
            conn_group_key: String::new(),
            start,
            log_enabled,
            blocked_reason: "",
            group_name: String::new(),
            req_bytes: 0,
            resp_bytes: 0,
        }
        .log_terminal(&state, target, 407)
        .await;
        let mut r = Response::builder()
            .status(StatusCode::PROXY_AUTHENTICATION_REQUIRED)
            .header("proxy-authenticate", "Basic realm=\"aidog\"")
            .body(Body::from("malformed Proxy-Authorization"))
            .unwrap();
        inject_trace_header(&mut r);
        return r;
    }
    // conn_group_key 落 connect log 元数据：绑定 → group_key（隧道级归属可观测）；
    // 未知用户名 → 尝试值原样（排障可查「谁配错了 group 名」）；无认证头 → 空（现状）。
    let (bound_group, conn_group_key) = match connect_auth {
        ConnectAuth::Bound(g) => {
            let key = g.group_key.clone();
            (Some(*g), key)
        }
        ConnectAuth::Unknown(ref u) => (None, u.clone()),
        ConnectAuth::Absent => (None, String::new()),
        ConnectAuth::Malformed => unreachable!("407 early-return above"),
    };
    tracing::info!(
        target = %target, request_id = %request_id,
        bound_group = conn_group_key,
        "connect auth resolved"
    );
    // 盲转不透明标记（票 09 / spec D4）：认证绑定了 group 的连接若最终走盲转
    // （白名单未命中 / MITM 降级 / CA 未启用），proxy_log 元数据行标 mitm_opaque（est_cost 恒 0）。
    // 未绑定（无头 / 未知 group）= 普通代理流量，盲转行不标记（现状零回归）。
    let blocked_reason: &'static str = if bound_group.is_some() {
        MITM_OPAQUE_REASON
    } else {
        ""
    };
    // 记账五元组 + blocked_reason 打包（ConnectLogCtx），以下整条 CONNECT 链共用。
    let log_ctx = ConnectLogCtx {
        request_id,
        platform_id,
        conn_group_key,
        start,
        log_enabled,
        blocked_reason,
        group_name: bound_group.as_ref().map(|g| g.name.clone()).unwrap_or_default(),
        req_bytes: 0,
        resp_bytes: 0,
    };

    // ST4 MITM 候选预判定：白名单命中 && 非 suspect。（DB 白名单匹配是 IO，suspect 查询是内存锁。）
    // 候选为 true 时跳过 P1 的「spawn 前 TCP 验证」（MITM 路径在 spawn 内自管 TCP 连接 +
    // pinning 预检，失败写终态 502 或降级 blind_relay）；候选为 false 走 P1 完整逻辑。
    let mitm_candidate = aidog_mitm::whitelist::matches_db(&state.db, &host_only).await
        && !aidog_mitm::mitm_state().is_suspect(&host_only).await;
    let mitm_state = aidog_mitm::mitm_state();
    tracing::info!(
        target = %target, host_only = %host_only, request_id = %log_ctx.request_id,
        mitm_candidate, log_enabled, platform_id,
        "connect dispatch: mitm_candidate decision",
    );

    // ── 非 MITM 候选：P1 完整逻辑（spawn 前 TCP 验证 + 502 早返，零回归）─────────────
    if !mitm_candidate {
        // P2-A/B/C：TCP 连接套 timeout + 熔断/last_error 记账（命中平台时）。
        let upstream =
            match tcp_connect_accounted(&state, &target, platform_id, conn_timeout_secs).await {
                Ok(s) => s,
                Err(()) => {
                    log_ctx.log_terminal(&state, target.clone(), 502).await;
                    let mut r = (StatusCode::BAD_GATEWAY, format!("connect {target} failed"))
                        .into_response();
                    inject_trace_header(&mut r);
                    return r;
                }
            };
        return spawn_blind_relay(state, on_upgrade, upstream, target, log_ctx);
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
    crate::logging::spawn_traced("connect_mitm", async move {
        let upgraded = match on_upgrade.await {
            Ok(u) => {
                tracing::info!(target = %target, request_id = %log_ctx.request_id, "connect upgrade ready → entering MITM/blind dispatch");
                u
            }
            Err(e) => {
                tracing::warn!(error = %e, target = %target, request_id = %log_ctx.request_id, "connect upgrade failed");
                // 隧道未建立即断（无字节盲转发生）→ 不标 mitm_opaque。
                log_ctx.no_opaque().log_terminal(&st, target, 499).await;
                return;
            }
        };
        // hyper-util auto 用私有 Rewind<T> 包 IO；downcast 回 TokioIo<TcpStream>
        // （axum::serve 喂入的 IO 类型）+ 拿预读 buf。
        let parts = match hyper_util::server::conn::auto::upgrade::downcast::<
            TokioIo<tokio::net::TcpStream>,
        >(upgraded)
        {
            Ok(p) => p,
            Err(upgraded) => {
                // downcast 失败（理论上不应）→ 退化 blind_relay（裸 Upgraded，不进 MITM）。
                tracing::warn!(target = %target, request_id = %log_ctx.request_id, "downcast TokioIo<TcpStream> failed, blind relay");
                let client = TokioIo::new(upgraded);
                blind_relay_after_connect(&st, client, &target, conn_timeout_secs, &[], log_ctx.clone())
                    .await;
                return;
            }
        };
        // parts.io = TokioIo<TcpStream>（impl hyper Read/Write）；包一层 TokioIo 转 tokio IO。
        // 客户端连接类型 = TokioIo<TokioIo<TcpStream>>（impl tokio AsyncRead/AsyncWrite）。
        let client = TokioIo::new(parts.io);
        tracing::info!(
            target = %target, request_id = %log_ctx.request_id,
            read_buf_len = parts.read_buf.len(),
            "connect upgraded: read_buf from speculative read (TLS ClientHello if any)",
        );

        // ST4 分流：MITM 候选 && read_buf 空 → 走 MITM；read_buf 非空（hyper-util speculative
        // read 预读的客户端字节，如 TLS ClientHello）→ 降级 blind_relay（blind_relay 内部
        // 已正确 flush read_buf 到 upstream，见 blind_relay_after_connect prefetch 参数）。
        //
        // ponytail: MITM 路径不处理 read_buf —— accept_client 需把预读字节 prepend 到 TLS
        // 输入流前面（组合 AsyncRead），复杂度 vs 收益失衡。read_buf 非空降级 blind_relay，
        // 行为保守正确（blind_relay 把预读字节 flush 到 upstream 非 client）。
        let client_for_blind: Option<_> = if parts.read_buf.is_empty() {
            tracing::info!(target = %target, request_id = %log_ctx.request_id, "→ handle_mitm (read_buf empty)");
            match handle_mitm(
                &st,
                mitm_state,
                client,
                &target,
                &host_only,
                bound_group,
                log_ctx.clone(),
            )
            .await
            {
                MitmOutcome::Connected => {
                    tracing::info!(target = %target, request_id = %log_ctx.request_id, "← handle_mitm Connected (MITM 隧道建/终态已写)");
                    return; // MITM 成功建隧道或终态日志已写
                }
                MitmOutcome::Degraded(reason, client_back) => {
                    // MITM 降级（CA 未启用 / pinning / IO error）→ 拿回 client 走 blind_relay
                    tracing::info!(
                        target = %target, request_id = %log_ctx.request_id,
                        reason = reason.as_str(),
                        "mitm degraded to blind relay"
                    );
                    Some(client_back)
                }
            }
        } else {
            tracing::info!(target = %target, request_id = %log_ctx.request_id, read_buf_len = parts.read_buf.len(), "→ blind_relay (read_buf non-empty, skip MITM)");
            Some(client)
        };

        // ── blind_relay 降级 / read_buf 非空 路径 ──────────────────────────────
        let client = client_for_blind.expect("client_for_blind set in both branches above");
        // read_buf（speculative read 从客户端预读的字节）通过 prefetch 参数传给
        // blind_relay_after_connect，由其在 connect upstream 成功后 flush 到 upstream
        // （写错对象回灌 client 会致 TLS 状态机错乱 RST，见 spawn_blind_relay 同款修复）。
        blind_relay_after_connect(
            &st,
            client,
            &target,
            conn_timeout_secs,
            &parts.read_buf,
            log_ctx,
        )
        .await;
    });

    // CONNECT 200 响应是 AirDog 直构（与之后 TCP 字节透传 blind_relay 无关）→ 注入 trace header。
    // 真正的 blind_relay_after_connect 是 TCP 字节透传（已加密 TLS），物理上无法注入 HTTP 层 header。
    let mut resp = resp;
    inject_trace_header(&mut resp);
    resp
}

// ── P1 blind_relay 路径 ────────────────────────────────────────────────────────

/// P1 blind_relay：上游 TCP 已连，spawn 双向 copy（含 read_buf flush 到 upstream）。
/// 记账五元组 + blocked_reason 走 `ConnectLogCtx`。
fn spawn_blind_relay(
    state: Arc<ProxyState>,
    on_upgrade: hyper::upgrade::OnUpgrade,
    upstream: tokio::net::TcpStream,
    target: String,
    mut log_ctx: ConnectLogCtx,
) -> Response {
    let resp = Response::builder()
        .status(StatusCode::OK)
        .body(Body::empty())
        .unwrap();
    crate::logging::spawn_traced("connect_blind_relay", async move {
        let upgraded = match on_upgrade.await {
            Ok(u) => u,
            Err(e) => {
                tracing::warn!(error = %e, target = %target, "connect upgrade failed");
                // upgrade 失败（客户端升级前断）→ inflight-1（不动 breaker/EMA，非上游健康信号）。
                if log_ctx.platform_id != 0 {
                    state.scheduler.record_ignored(log_ctx.platform_id);
                }
                log_ctx.log_terminal(&state, target, 499).await;
                return;
            }
        };
        let parts = match hyper_util::server::conn::auto::upgrade::downcast::<
            TokioIo<tokio::net::TcpStream>,
        >(upgraded)
        {
            Ok(p) => p,
            Err(upgraded) => {
                tracing::warn!(target = %target, "downcast TokioIo<TcpStream> failed, blind relay");
                let client = TokioIo::new(upgraded);
                let (sent, recv) = bridge_bidir(client, upstream).await;
                log_ctx.req_bytes = sent as i64;
                log_ctx.resp_bytes = recv as i64;
                // P2-B：隧道正常关闭 → inflight-1（record_ignored 仅 inflight，不动 breaker/EMA）。
                if log_ctx.platform_id != 0 {
                    state.scheduler.record_ignored(log_ctx.platform_id);
                }
                log_ctx.log_terminal(&state, target, 200).await;
                return;
            }
        };
        let client = TokioIo::new(parts.io);
        let mut upstream = upstream;
        // read_buf 是 hyper-util speculative read 从客户端预读的字节（如 TLS ClientHello），
        // 必须 flush 到 upstream（上游才收得到）—— 写错对象（回灌 client）会让上游收不到
        // ClientHello + 客户端收自己字节致 TLS 状态机错乱 RST。
        if !parts.read_buf.is_empty() {
            let _ = tokio::io::AsyncWriteExt::write_all(&mut upstream, &parts.read_buf).await;
        }
        let (sent, recv) = bridge_bidir(client, upstream).await;
        log_ctx.req_bytes = sent as i64;
        log_ctx.resp_bytes = recv as i64;
        // P2-B：隧道正常关闭 → inflight-1（record_ignored 仅 inflight，不动 breaker/EMA）。
        if log_ctx.platform_id != 0 {
            state.scheduler.record_ignored(log_ctx.platform_id);
        }
        log_ctx.log_terminal(&state, target, 200).await;
    });
    // CONNECT 200 直构响应 → 注入 trace header（后续 spawn 内双向 TCP copy 是 blind_relay 字节透传，
    // 加密 TLS 字节流，物理上无法注入 HTTP 层 header；CONNECT 200 响应本身已注入）。
    let mut resp = resp;
    inject_trace_header(&mut resp);
    resp
}

/// blind_relay 辅助：上游 TCP 未连，先 connect 再 bridge。MITM 降级路径 / downcast 失败路径共用。
///
/// `prefetch` 是已从客户端预读的字节（hyper-util speculative read 命中，如 TLS ClientHello），
/// 须 flush 到 upstream（上游才收得到）；通常空（合法 CONNECT 客户端收 200 才发数据）。
///
/// P2-A/B/C：connect 套 timeout + TCP 失败 record_ignored（网络失败不降权）+ set_platform_last_error +
/// inflight-1；成功侧 record_ignored（仅 inflight-1，**禁 record_success**，避 CONNECT TCP
/// 握手延迟污染延迟 EMA —— AI 推理秒级 vs TCP 握手毫秒级）。
///
/// ponytail: 抽出避免 blind_relay 逻辑在 handle_connect spawn 内重复（downcast 失败 +
/// MITM 降级 + read_buf 非空三路径都走 blind_relay）。签名收 `&str` target 因调用方已拥有
/// String，借用避免 move 后还要用（tracing 等）。
/// 记账五元组 + blocked_reason 走 `ConnectLogCtx`（同 spawn_blind_relay）。
async fn blind_relay_after_connect(
    st: &Arc<ProxyState>,
    client: impl AsyncRead + AsyncWrite + Unpin,
    target: &str,
    conn_timeout_secs: u64,
    prefetch: &[u8],
    mut log_ctx: ConnectLogCtx,
) {
    // blind_relay: TCP 字节透传非 AirDog 构造响应，header 物理不可注入（双向 copy 加密 TLS 字节流，
    // AirDog 看不见 / 改不了 HTTP 层）。trace header 已在 spawn 前的 CONNECT 200 响应注入，
    // 此处隧道内的客户端真实 HTTP 请求/响应不经 axum，无 inject_trace_header 调用点。
    match tcp_connect_accounted(st, target, log_ctx.platform_id, conn_timeout_secs).await {
        Ok(mut upstream) => {
            // 预读字节先 flush 到上游（read_buf 来自客户端 speculative read，上游需收得到）。
            if !prefetch.is_empty() {
                let _ = tokio::io::AsyncWriteExt::write_all(&mut upstream, prefetch).await;
            }
            let (sent, recv) = bridge_bidir(client, upstream).await;
            log_ctx.req_bytes = sent as i64;
            log_ctx.resp_bytes = recv as i64;
            // P2-B：隧道正常关闭 → inflight-1（record_ignored 仅 inflight，不动 breaker/EMA）。
            if log_ctx.platform_id != 0 {
                st.scheduler.record_ignored(log_ctx.platform_id);
            }
            log_ctx.log_terminal(st, target.to_string(), 200).await;
        }
        Err(()) => {
            log_ctx.log_terminal(st, target.to_string(), 502).await;
        }
    }
}

/// P2-A/B/C：TCP 连接 + 元数据记账封装（blind_relay 路径专用）。
///
/// - **A. timeout**：`tokio::time::timeout(conn_timeout_secs, TcpStream::connect)` —— 仅 TCP
///   握手阶段超时，隧道建后 idle 不限（避 SSE/WebSocket over TLS 长连接误杀）。
/// - **B. 在途记账**：命中平台（platform_id != 0）→ connect 前
///   `inc_inflight`；失败/超时 → `record_ignored`（网络失败不降权，仅 inflight-1）。
///   **成功侧由调用方在隧道关闭后 `record_ignored`**（仅 inflight-1，禁 record_success，
///   避 CONNECT TCP 握手延迟污染延迟 EMA —— AI 推理秒级 vs TCP 握手毫秒级）。
/// - **C. last_error**：失败 → `set_platform_last_error`（成功侧禁 recover_platform_auto_disabled，
///   CONNECT 隧道成功 ≠ 平台 AI API 健康）。
///
/// ponytail: 返 `Result<TcpStream, ()>` —— 调用方已统一走 502 log 路径，错误细节在内部已
/// tracing + record，无需回传 error 字符串（原本 format!("{e}") 仅进 502 body，调试价值低，
/// 真错误在 tracing warn 行）。最小可工作签名。
pub(crate) async fn tcp_connect_accounted(
    st: &Arc<ProxyState>,
    target: &str,
    platform_id: u64,
    conn_timeout_secs: u64,
) -> Result<tokio::net::TcpStream, ()> {
    // P2-B：connect 前 inc_inflight（仅命中平台）。
    if platform_id != 0 {
        st.scheduler.inc_inflight(platform_id);
    }
    let conn = tokio::time::timeout(
        std::time::Duration::from_secs(conn_timeout_secs),
        tokio::net::TcpStream::connect(target),
    )
    .await;
    match conn {
        Ok(Ok(s)) => Ok(s),
        Ok(Err(e)) => {
            tracing::warn!(error = %e, target, "connect: upstream TCP failed");
            record_connect_failure(st, platform_id, format!("connect TCP error: {e}")).await;
            Err(())
        }
        Err(_) => {
            tracing::warn!(
                target,
                secs = conn_timeout_secs,
                "connect: upstream TCP timeout"
            );
            record_connect_failure(
                st,
                platform_id,
                format!("connect TCP timeout ({conn_timeout_secs}s)"),
            )
            .await;
            Err(())
        }
    }
}

/// P2-B/C 失败记账：record_ignored（网络类失败不降权——不计熔断、不动 EMA，仅 inflight-1）+
/// set_platform_last_error。未命中平台（platform_id=0）→ 仅返回（无平台可挂）。
async fn record_connect_failure(st: &Arc<ProxyState>, platform_id: u64, err_msg: String) {
    if platform_id == 0 {
        return;
    }
    st.scheduler.record_ignored(platform_id);
    let _ = aidog_db::set_platform_last_error(&st.db, platform_id, Some(err_msg)).await;
}

// ── ST4 MITM 路径 ──────────────────────────────────────────────────────────────

/// MITM 路径处理结果（C8 收敛：扁平 struct → enum，按 design §C8 分支显式化）。
///
/// - **Connected**: MITM 隧道建链成功 / 客户端 TLS 已消费走终态 502 —— 调用方 return，
///   不再 blind_relay。
/// - **Degraded**: 上游失败但 client 未被 TLS accept 消费，归还 client 让调用方降级
///   blind_relay。`DegradeReason` 区分 pinning / IO / signer，tracing + 测试可断言。
///
/// ponytail: 两变体 enum + 独立 DegradeReason 子 enum（比 4 变体扁平 enum 更内聚 ——
/// Connected 路径无 client 归还字段，Degraded 路径统一 (reason, client) 结构）。原扁平
/// struct 的 `handled: bool + client_return: Option<IO>` 是 enum 的退化形式，编译期无法
/// 强制 client_return 在 handled=true 时为 None（运行时约定），enum 化消除该类不变量。
// ponytail: pub(crate) 仅为 test_connect.rs C8 测试 match 变体用（编译期契约锚点）；
// 生产调用方仍只有 handle_connect（本文件内）。
pub(crate) enum MitmOutcome<IO> {
    /// MITM 隧道建立或终态日志已写，调用方 return（不再 blind_relay）。
    Connected,
    /// 降级 blind_relay：归还未被消费的 client 流 + 原因（tracing + 测试断言用）。
    Degraded(DegradeReason, IO),
}

/// MITM 降级原因（`MitmOutcome::Degraded` 子 enum，C8 收敛）。
///
/// 调用方按 reason 写 tracing 日志（仅诊断用，行为相同 —— 都走 blind_relay）。
/// `as_str` 返静态标签供 tracing `reason = ...` 字段，与原 `&'static str` fallback_reason
/// 字段保持日志可读性。
#[derive(Debug, Clone, Copy)]
pub(crate) enum DegradeReason {
    /// 上游 TLS 握手含证书错（疑似 cert pinning）→ 标 suspect 后续跳 MITM。
    PinningSuspect,
    /// 上游 TCP / TLS 非 pinning 类 IO 错（断连 / 超时）→ 不标 suspect。
    IoError,
    /// DB 无 mitm_ca 行 / signer init 失败（用户未启用 MITM 或 CA 数据损坏）。
    SignerInit,
}

impl DegradeReason {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            DegradeReason::PinningSuspect => "upstream pinning suspect",
            DegradeReason::IoError => "upstream IO error",
            DegradeReason::SignerInit => "CA not enabled / signer init failed",
        }
    }
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
/// 日志五元组 + blocked_reason 走 `ConnectLogCtx`（CC review：散参打包）。
async fn handle_mitm<IO>(
    st: &Arc<ProxyState>,
    mitm_state: &'static aidog_mitm::MitmState,
    client: IO,
    target: &str,
    host_only: &str,
    bound_group: Option<Group>,
    log_ctx: ConnectLogCtx,
) -> MitmOutcome<IO>
where
    IO: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    // 1. 取 / 构造 CertSigner（首次从 DB load RootCa；DB 无 CA = 用户未启用 MITM → 降级）。
    let signer = match mitm_state.signer_or_init(&st.db).await {
        Ok(Some(s)) => s,
        Ok(None) => {
            return MitmOutcome::Degraded(DegradeReason::SignerInit, client);
        }
        Err(e) => {
            tracing::warn!(error = %e, host = host_only, "mitm: load signer failed, degrading");
            return MitmOutcome::Degraded(DegradeReason::SignerInit, client);
        }
    };

    // 2. 预检 connect_upstream：先连上游 TCP + TLS 握手（pinning 探测）。
    let upstream_tcp = match tokio::net::TcpStream::connect(target).await {
        Ok(s) => s,
        Err(e) => {
            tracing::warn!(error = %e, target, "mitm: upstream TCP failed, terminal 502");
            // TCP 失败非盲转（无字节透传发生）→ 不标 mitm_opaque。
            log_ctx.no_opaque().log_terminal(st, target.to_string(), 502).await;
            // TCP 失败非 pinning，不标 suspect；client 不再有用（上游连不上 blind_relay 也连不上）。
            // Connected 表示「MITM 已处理」（此处：写了终态 502），调用方 return 不 blind_relay。
            drop(client);
            return MitmOutcome::Connected;
        }
    };
    match aidog_mitm::tls::connect_upstream(host_only, upstream_tcp).await {
        aidog_mitm::tls::UpstreamTlsOutcome::Connected(upstream_tls) => {
            // 3. accept 客户端 TLS（假 CA 签 leaf，SNI fallback = CONNECT target host）。
            //    失败（client 不信任 CA / 网络断）→ client 已被 accept 消费，无法降级 blind_relay。
            //    写终态 502 + Connected（客户端 TLS 握手失败隧道断，blind_relay 也救不回）。
            let client_tls =
                match aidog_mitm::tls::accept_client(signer, client, host_only.to_string()).await {
                    Ok(s) => s,
                    Err(e) => {
                        tracing::warn!(
                            error = %e, host = host_only,
                            "mitm: client TLS handshake failed (CA not trusted?), terminal 502"
                        );
                        // 客户端 TLS 握手失败（无字节盲转发生）→ 不标 mitm_opaque。
                        log_ctx.no_opaque().log_terminal(st, target.to_string(), 502).await;
                        return MitmOutcome::Connected;
                    }
                };

            // ST5/ST6 明文 forward：在 client_tls 上读明文 HTTP Request（http/1.1 或 h2，
            // 由 hyper-util auto Builder 按 H2 preface 自动检测）→ 灌入 handle_proxy_core
            // （middleware/路由/headers/retry/采集/forward_attempt 全套，95% 复用）→ 响应明文
            // 回写 client_tls（hyper 写回，TLS 层自动加密回客户端）。上游由 forward_attempt 内部
            // http_client 自连（真证书），预检 upstream_tls 丢弃。
            // proxy_log 走 handle_proxy_core 内部 upsert_log 全量 AI 记账（body + stats_agg + cost），
            // **不走 upsert_connect_log**（盲转专用，避免污染统计）。
            //
            // D9 ALPN：ST3 SERVER_ALPN=[h2, http/1.1] 两段 advertise，rustls 按客户端偏好协商。
            // auto Builder 读明文首字节（H2 preface `PRI * HTTP/2.0...`）分发 h1/h2 server，
            // 无需手动 ALPN 分流分支（ponytail: 删分支，协议判定收敛到 hyper-util）。
            //
            // ponytail: 预检 upstream_tls 在明文路径被丢弃（forward_attempt 自连），浪费 1 条 TCP+TLS。
            // 保留预检是为 pinning 探测（探针必须先于 accept 确认上游可信，否则 client 已 accept
            // 后发现 pinning fail 无法干净降级）。pinning fail 频率低，浪费可接受。
            //
            // C8 seam 评估（design §C8「serve_plaintext 跨模块递归」）：serve_plaintext 调
            // handle_proxy_core（非 handle_proxy），后者是 ST5 切出的 CONNECT-free 入口 ——
            // 不分流 CONNECT 故与 handle_connect 无互递归。调用链 acyclic：handle_connect →
            // handle_mitm → serve_plaintext → handle_proxy_core → forward_attempt（叶）。
            // 「递归」仅为假设：若误改 serve_plaintext 调 handle_proxy 则形成
            // handle_connect → handle_proxy → handle_connect 死循环，本注释 + handle_proxy_core
            // 的存在即守护此不变量。保留当前结构 + 文档化，不重写（无真实递归可消）。
            drop(upstream_tls);
            serve_plaintext(st.clone(), client_tls, host_only, bound_group).await;
            MitmOutcome::Connected
        }
        aidog_mitm::tls::UpstreamTlsOutcome::PinningSuspect { host, error } => {
            // pinning fail → 标 suspect（带 TTL，mitm/mod.rs 内自动 expire）+ 降级 blind_relay。
            tracing::warn!(
                host = %host, error = %error,
                "mitm: upstream TLS handshake failed (pinning suspect), flagging + degrading"
            );
            mitm_state.mark_suspect(host).await;
            MitmOutcome::Degraded(DegradeReason::PinningSuspect, client)
        }
        aidog_mitm::tls::UpstreamTlsOutcome::IoError(e) => {
            // 非 pinning IO 错（TCP 断 / 超时）→ 不标 suspect + 降级 blind_relay（重试一次）。
            tracing::warn!(error = %e, target, "mitm: upstream TLS IO error, degrading");
            MitmOutcome::Degraded(DegradeReason::IoError, client)
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

/// ST5/ST6 明文 forward：在已 accept 的 client_tls（明文）流上用 hyper-util **auto**
/// server 读明文 HTTP Request，每条 Request 构造 axum Request 灌入 `handle_proxy_core`
/// （middleware / 路由 / forward_attempt 全套），响应明文回写 client_tls（hyper 写回，
/// TLS 层自动加密回客户端）。
///
/// **auto Builder（D9）**：`hyper_util::server::conn::auto::Builder` 读明文首字节检测 H2
/// preface（`PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n`），自动分发 h1 / h2 server。h1 走 keep-alive
/// 循环，h2 走多路复用流（一个 TLS 连接多 Request，每流各灌 handle_proxy_core）。
/// 无需在 connect.rs 内做 ALPN 分流分支 —— 协议判定收敛到 hyper-util，connect.rs 只负责
/// TLS accept + 灌入 core。
///
/// **proxy_log 记账**：明文路径走 AI 请求全量记账（handle_proxy_core 内 upsert_log，含 body
/// + stats_agg + cost），**不走 upsert_connect_log**（盲转专用，避免污染统计）。
///
/// ponytail: auto Builder 取代 ST5 的 http1::Builder + ST6 计划的 http2::Builder 显式分流，
/// 一个 Builder 覆盖两协议（ponytail: 删分支优先）。hyper-util server-auto feature 已含
/// hyper/http2，Cargo.toml 无需改。
/// ponytail: body 完整读（collect_body）而非 streaming 灌入 —— handle_proxy_core 内
/// 已 to_bytes(10MB) 读一次，此处再 stream 无收益且增复杂度，等价直接 collect。
/// ponytail: Infallible 错类型 —— handle_proxy_core 返 Response 不返 Err（错误已落 4xx/5xx body），
/// service_fn 不需要错误传播路径。
/// ponytail: 不复用 handle_proxy_inner 的 RequestLogGuard —— MITM 明文路径客户端断连时
/// handle_proxy_core 内部各阶段已 upsert_log 终态，499 兜底语义重叠；YAGNI 不重复 guard。
// ponytail: pub(crate) 仅为 ST8 端到端测试直调（绕过 handle_connect 的真上游预检，connect_upstream
// 写死 webpki-roots 无法 mock）；生产调用方仍只有 handle_mitm。
pub(crate) async fn serve_plaintext<S>(
    state: Arc<ProxyState>,
    client_tls: S,
    host_only: &str,
    bound_group: Option<Group>,
) where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    // TokioIo 包装：rustls server TlsStream impl tokio AsyncRead/Write，TokioIo 转 hyper Read/Write。
    let io = TokioIo::new(client_tls);
    // host_only clone owned —— auto serve_connection 要求 service 'static（h2 多流 future），
    // 闭包不能 borrow 外层 &str。每连接一次 clone（host 长度有限，开销可忽略）。
    let host_owned = host_only.to_string();
    let svc = hyper::service::service_fn(move |req: hyper::Request<hyper::body::Incoming>| {
        let st = state.clone();
        let host = host_owned.clone();
        // 每请求 clone bound group（h1 keep-alive / h2 多流，一连接多 Request 各灌 core）。
        let bound = bound_group.clone();
        async move {
            // hyper::Request<Incoming> → axum::Request<axum::body::Body>：
            // parts（method/uri/headers）通用，body 收 Bytes 后 Body::from 包装。
            let req_path = req.uri().path().to_string();
            let (parts, body) = req.into_parts();
            let bytes = match collect_body(body, 10 * 1024 * 1024).await {
                Ok(b) => b,
                Err(e) => {
                    // body 读失败返 400（与 handle_proxy_core 内同款错误语义）。
                    tracing::warn!(error = %e, host = %host, "mitm plaintext: read body failed");
                    let mut resp = hyper::Response::builder().status(StatusCode::BAD_REQUEST);
                    if let Some(h) = resp.headers_mut() {
                        h.insert(
                            axum::http::header::CONTENT_TYPE,
                            axum::http::HeaderValue::from_static("text/plain"),
                        );
                        // MITM 明文路径：AirDog 直构响应也注入 trace header（与 axum 路径一致诊断体验）。
                        if cfg!(debug_assertions) {
                            let id = crate::logging::current_trace_id()
                                .unwrap_or_else(crate::logging::new_trace_id);
                            if let Ok(hv) = axum::http::HeaderValue::from_str(&id) {
                                h.insert(axum::http::HeaderName::from_static("x-aidog-trace"), hv);
                            }
                        }
                    }
                    return Ok::<_, std::convert::Infallible>(
                        resp.body(axum::body::Body::from(format!("read body error: {e}")))
                            .expect("static response build"),
                    );
                }
            };
            // ── 票 10 分流：MITM 明文请求按 host/path 分流（mitm_bypass 单一真值源）──
            //
            // Core（AI API / hello / models）→ 灌 handle_proxy_core，走完整 AI 请求链
            // （middleware/路由/forward_attempt/采集）：
            //
            // 直调 handle_proxy_core 而非 handle_proxy：handle_proxy → handle_proxy_inner 含
            // CONNECT 分流 → handle_connect（与当前 spawn 互递归，Send 死锁）。core 不分流
            // CONNECT（分流已在 handle_proxy_inner 顶部），明文 Request method 非 CONNECT 必走
            // AI 路径，无递归。request_id + span + 499 guard 在本地构造（等价 handle_proxy_inner
            // 的 guard 语义，客户端断连时 Drop 补写终态 499）。
            //
            // 非 Core（旁路流量 / /api/oauth/* / platform.claude.com token）→ mitm_bypass：
            // 透明转发 + mitm_log 观测行（usage 蹭采样 / token 只记元数据），**不进 proxy_log**
            // （票 10 目标：防遥测行污染统计；此前这类流量灌 core 落「未匹配」桶 passthrough）。
            let route = classify_mitm_route(&host, &req_path, &parts.method);
            let req_bytes = bytes.len() as i64;
            // bound 在 Core 分支被 filter 消费（inject_group），group_name 先行拷出供观测行用。
            let bound_group_name = bound.as_ref().map(|g| g.name.clone()).unwrap_or_default();
            let resp: Response = if route == MitmRoute::Core {
                let axum_req = Request::from_parts(parts, axum::body::Body::from(bytes));
                let request_id = uuid::Uuid::new_v4().simple().to_string();
                let span = tracing::info_span!(
                    "req",
                    trace_id = %&request_id[..8],
                    request_id = %request_id,
                    mitm = %host,
                );
                // 归属注入门（票 08）：CONNECT 绑定 group 仅对 AI API 端点请求注入 core ——
                // MITM 明文请求的 Authorization 是订阅 OAuth Bearer（resolve_group 必落空），
                // 归属由隧道绑定注入（注入会使 parse_incoming_request 对非 JSON body 返 400，
                // 破坏旁路流量，故仅 API 端点注入）。
                let inject_group = bound.filter(|_| is_api_endpoint(&req_path));
                handle_proxy_core(AxumState(st.clone()), axum_req, request_id, inject_group)
                    .instrument(span)
                    .await
            } else {
                // 归属沿用 CONNECT 绑定 group（mitm_log.group_name），URL 按 CONNECT host
                // 重构（origin-form URI 只有 path 段）。
                let group_name = bound.as_ref().map(|g| g.name.clone()).unwrap_or_default();
                let pq = parts.uri.path_and_query().map(|p| p.as_str()).unwrap_or("/");
                let url = format!("https://{host}{pq}");
                let log_settings = st.settings_cache.read().await.log_settings.clone();
                handle_mitm_observed(
                    &st,
                    route,
                    url,
                    &host,
                    &req_path,
                    parts.method,
                    parts.headers,
                    bytes,
                    &group_name,
                    &log_settings,
                )
                .await
            };
            // Core anthropic.com 域观测行（2026-09-28 用户口径：mitm 日志须覆盖全部
            // anthropic.com 请求与返回）。元数据行（decrypted=true / body 恒空）——完整 body
            // 已按同一开关落 proxy_log（Core 走完整记账管线），此处双写仅补观测页可见性，
            // 不重复存 body。流式 resp_bytes 未知记 0。
            if route == MitmRoute::Core && is_anthropic_family_host(&host) {
                let group_name = bound_group_name;
                let log_settings = st.settings_cache.read().await.log_settings.clone();
                if log_settings.enabled {
                    let row = aidog_logs::MitmLogInsert {
                        group_name,
                        host: host.clone(),
                        path: req_path.clone(),
                        status_code: resp.status().as_u16() as i32,
                        req_bytes,
                        resp_bytes: 0,
                        decrypted: true,
                        request_body: String::new(),
                        response_body: String::new(),
                        created_at: aidog_db::now(),
                    };
                    if let Err(e) = aidog_logs::insert_mitm_log(&st.db, row).await {
                        tracing::warn!(error = %e, "core anthropic mitm_log insert failed (non-fatal)");
                    }
                }
            }
            Ok::<_, std::convert::Infallible>(resp)
        }
    });

    // auto Builder：按 H2 preface 自动分发 h1（keep-alive 循环）/ h2（多路复用流）。
    // TokioExecutor：h2 多流并发执行需 Executor，axum::serve 同款（hyper-util rt tokio feature）。
    // serve_connection future 在 client 关闭连接 / 协议错时返 Err（tracing 后接受）。
    let builder =
        hyper_util::server::conn::auto::Builder::new(hyper_util::rt::TokioExecutor::new());
    if let Err(e) = builder.serve_connection(io, svc).await {
        tracing::debug!(error = %e, host = host_only, "mitm plaintext: connection ended");
    }
}

// proxy_log 写入 helper：log_connect_success / log_connect_502 已并入
// `ConnectLogCtx::log_terminal`（ConnectLogCtx 定义在 log.rs，紧邻 upsert_connect_log）。
