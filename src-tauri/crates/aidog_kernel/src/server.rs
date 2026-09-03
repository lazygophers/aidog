//! `--ui` 形态的管理面 HTTP 服务（票 08）。
//!
//! 三块：
//!
//! 1. `/rpc/<命令>` —— 211 个命令的 HTTP 形态，语义等价前端的 `invoke(name, args)`（见
//!    `aidog_core::http_command`）。表在 [`crate::rpc`]。
//! 2. `/events` —— SSE 事件流。只有一个事件被前端消费（`proxy-log-updated`，payload 是
//!    平台 id 数字），但这里不过滤，原样广播，形状与 Tauri `emit` 一字不差。
//! 3. 静态前端资源 —— `dist/` 目录，SPA fallback 到 `index.html`。
//!
//! **纯内核形态（不带 `--ui`）根本不构造本模块的任何东西**，也就没有任何管理面在听。
//!
//! # 鉴权
//!
//! 凭据 = [`aidog_core::kernel_settings::KernelSettings::auth_token`]，Bearer 语义与既有
//! `/api/*` 一致（`Authorization: Bearer <token>`）。
//!
//! - 凭据非空 → **所有**管理面请求都校验（含静态资源）；
//! - 凭据为空 → 不校验。
//!
//! 管理面**永远只绑 127.0.0.1**（[`crate::management_bind_addr`]），所以凭据为空时够得着
//! 它的只有本机进程。配了凭据仍有意义：反向代理回连本机时，同机的其他进程同样够得着这个
//! 端口。注意 Bearer 头带不进浏览器的文档导航与 `EventSource` —— 配了凭据后浏览器界面本身
//! 不可直接使用，跨机访问的正解是让反向代理负责鉴权。

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use axum::body::Body;
use axum::extract::State;
use axum::http::{Request, StatusCode, header};
use axum::middleware::Next;
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use futures::stream::Stream;
use tokio::sync::broadcast;

use crate::ctx::{HeadlessCtx, KernelEvent};

/// 管理面共享状态：事件订阅源 + 凭据。
#[derive(Clone)]
pub struct ManagementState {
    ctx: Arc<HeadlessCtx>,
    /// 空 = 不校验（只可能是 127.0.0.1 形态）。
    auth_token: Arc<String>,
}

impl ManagementState {
    pub fn new(ctx: Arc<HeadlessCtx>, auth_token: String) -> Self {
        Self {
            ctx,
            auth_token: Arc::new(auth_token),
        }
    }
}

/// SSE 心跳间隔：穿过反向代理 / NAT 时防止空闲连接被掐。
const SSE_KEEPALIVE_SECS: u64 = 15;

/// 组装完整的管理面 Router。
///
/// `ui_dir` = 静态前端资源目录；`None` 或目录不存在 → 只有 `/rpc/*` 与 `/events`
/// （前端资源缺失不该让管理接口一起挂掉，日志里 warn 说明即可）。
pub fn management_router(state: ManagementState, ui_dir: Option<PathBuf>) -> Router {
    let mut app = crate::rpc::rpc_router().route("/events", get(events_handler));

    if let Some(dir) = ui_dir {
        if dir.is_dir() {
            let index = dir.join("index.html");
            // SPA：未命中静态文件的 GET 一律回 index.html（前端无 react-router，但
            // 刷新 / 直接开子路径时仍需要拿到同一个入口文档）。
            let serve = tower_http::services::ServeDir::new(&dir)
                .fallback(tower_http::services::ServeFile::new(index));
            app = app.fallback_service(serve);
            tracing::info!(dir = %dir.display(), "kernel: serving web UI assets");
        } else {
            tracing::warn!(
                dir = %dir.display(),
                "kernel: web UI asset directory not found, serving /rpc and /events only"
            );
        }
    }

    app.layer(axum::middleware::from_fn_with_state(state.clone(), auth_mw))
        .with_state(state)
}

/// 鉴权失败的响应体。**必须是合法 JSON**：前端 `services/transport.ts::httpInvoke` 对
/// 管理面的响应统一 `JSON.parse`，回一段裸文本（旧实现的 `"unauthorized"` 五个字母）会让
/// 它抛 `SyntaxError`，真正的 401 被掩盖成「解析失败」。这里回的是一个 JSON 字符串字面量，
/// 与 `Result<_, String>` 的 `Err` 走 `http_command` 时的 body 形状一致 —— 前端两条路拿到
/// 的都是同一个字符串。
const UNAUTHORIZED_BODY: &str = "\"unauthorized\"";

/// Bearer 鉴权。凭据为空时直接放行（见模块文档）。
async fn auth_mw(State(state): State<ManagementState>, req: Request<Body>, next: Next) -> Response {
    if state.auth_token.is_empty() {
        return next.run(req).await;
    }
    let presented = req
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .unwrap_or("")
        .trim();
    if presented == state.auth_token.as_str() {
        return next.run(req).await;
    }
    tracing::warn!(
        path = %req.uri().path(),
        "kernel: management request rejected (bad or missing bearer token)"
    );
    (
        StatusCode::UNAUTHORIZED,
        [(header::CONTENT_TYPE, "application/json")],
        UNAUTHORIZED_BODY,
    )
        .into_response()
}

/// SSE 事件流。每个连接一个 broadcast 订阅。
///
/// 事件名 = `emit` 的第一个参数（前端按 `proxy-log-updated` 过滤），data = payload 的 JSON
/// 原文（`platform_id` 是数字，形状与 Tauri 侧一字不差）。
async fn events_handler(
    State(state): State<ManagementState>,
) -> Sse<impl Stream<Item = Result<Event, std::convert::Infallible>>> {
    let rx = state.ctx.subscribe();
    Sse::new(event_stream(rx)).keep_alive(
        KeepAlive::new()
            .interval(Duration::from_secs(SSE_KEEPALIVE_SECS))
            .text("keep-alive"),
    )
}

fn event_stream(
    rx: broadcast::Receiver<KernelEvent>,
) -> impl Stream<Item = Result<Event, std::convert::Infallible>> {
    use futures::StreamExt;
    tokio_stream::wrappers::BroadcastStream::new(rx).filter_map(|item| async move {
        match item {
            Ok(ev) => Some(Ok(Event::default().event(ev.name).data(ev.payload.to_string()))),
            // 订阅者跟不上导致的丢事件：跳过这一条，连接保持。下一条事件仍会到，
            // 前端的刷新语义是幂等的（收到就重拉），丢中间态无害。
            Err(_) => None,
        }
    })
}

/// 绑定并起管理面服务，返回**实际**监听地址与服务任务句柄。
///
/// 端口写 0 时由内核选一个空闲端口（集成测试用）；返回的地址是 bind 之后读出来的真值，
/// 不是入参的回声。
pub async fn serve_management(
    app: Router,
    addr: SocketAddr,
) -> Result<(SocketAddr, tokio::task::JoinHandle<()>), String> {
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .map_err(|e| format!("kernel management bind {addr} failed: {e}"))?;
    let local = listener
        .local_addr()
        .map_err(|e| format!("kernel management local_addr failed: {e}"))?;
    let handle = tokio::spawn(async move {
        if let Err(e) = axum::serve(listener, app).await {
            tracing::error!(error = %e, "kernel: management server stopped");
        }
    });
    Ok((local, handle))
}
