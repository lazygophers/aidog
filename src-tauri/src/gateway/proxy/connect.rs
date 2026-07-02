//! P1 HTTP CONNECT 隧道：标准 http_proxy 行为。
//!
//! 客户端配 `http_proxy=127.0.0.1:<port>` 后任意 HTTP/HTTPS 流量经 CONNECT 隧道盲转。
//! P1 不解密 HTTPS（P2 才 MITM），只记 proxy_log 元数据（host/status/duration/platform_id），
//! 不计费、不统计字节（用户锁 YAGNI）。
//!
//! 关键技术点（research 结论 1 + 5）:
//! - axum 0.8 `axum::serve` 底层 `hyper_util auto + upgrades`，CONNECT upgrade 默认开启
//! - hyper-util 用私有 `Rewind<T>` 包 `TokioIo<TcpStream>`（axum::serve 喂入的 IO 类型），
//!   需 `auto::upgrade::downcast::<TokioIo<TcpStream>>` 取回底层流 + 预读缓冲
//!   （client 可能已发 TLS ClientHello 前若干字节，须先 flush 到上游）
//! - CONNECT 响应 `200 + 空 body`，**禁带 `Connection: upgrade` header**（hyper h1 role.rs
//!   380-384 对 CONNECT 2xx 响应禁止 content-length/transfer-encoding）
//! - `tokio::io::copy` 双向 + `tokio::join!`；字节 u64 返回但 P1 不入库

use super::*;
use hyper_util::rt::TokioIo;

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

    // 先尝试建上游 TCP 连接；失败立即返回 502 + 写 proxy_log（终态）。
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

    // 200 + 空 body 建立隧道（hyper 自动写 status line；禁 Connection/Upgrade header）。
    let resp = Response::builder()
        .status(StatusCode::OK)
        .body(Body::empty())
        .unwrap();

    // spawn 双向转发：response 已先返回，upgrade 在 task 内 await。
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
        // （axum::serve 喂入的 IO 类型）+ 拿预读 buf（client 可能已发 TLS ClientHello，
        // 须先 flush 到上游否则丢字节）。
        let parts = match hyper_util::server::conn::auto::upgrade::downcast::<TokioIo<tokio::net::TcpStream>>(upgraded) {
            Ok(p) => p,
            Err(upgraded) => {
                // downcast 失败（理论上不应：axum::serve 永远喂 TokioIo<TcpStream>）→
                // 退化为裸 Upgraded（impl hyper Read/Write）包 TokioIo 转 tokio IO，不拿预读 buf。
                tracing::warn!(target = %target, "downcast TokioIo<TcpStream> failed, falling back");
                let client = TokioIo::new(upgraded);
                let (mut cr, mut cw) = tokio::io::split(client);
                let (mut ur, mut uw) = upstream.into_split();
                let _ = tokio::join!(
                    tokio::io::copy(&mut cr, &mut uw),
                    tokio::io::copy(&mut ur, &mut cw),
                );
                let duration_ms = start.elapsed().as_millis() as i32;
                if log_enabled {
                    upsert_connect_log(
                        &st, request_id, String::new(), platform_id,
                        target, 200, duration_ms,
                    ).await;
                }
                return;
            }
        };
        // parts.io = TokioIo<TcpStream>（impl hyper Read/Write）；包一层 TokioIo 转 tokio IO。
        let mut client = TokioIo::new(parts.io);
        // 预读字节先 flush 到上游。
        if !parts.read_buf.is_empty() {
            let _ = tokio::io::AsyncWriteExt::write_all(&mut client, &parts.read_buf).await;
        }
        let (mut cr, mut cw) = tokio::io::split(client);
        let (mut ur, mut uw) = upstream.into_split();

        // 双向 copy + join（字节 u64 返回但 P1 不入库）。任一方向 EOF/err 即整体 drop 触发对端 FIN。
        let _ = tokio::join!(
            tokio::io::copy(&mut cr, &mut uw),
            tokio::io::copy(&mut ur, &mut cw),
        );
        let duration_ms = start.elapsed().as_millis() as i32;

        if log_enabled {
            upsert_connect_log(
                &st, request_id, String::new(), platform_id,
                target, 200, duration_ms,
            ).await;
        }
    });

    resp
}
