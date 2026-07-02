// 通用导入：声明为 pub(crate) use，子模块通过 `use super::*;` 复用，避免逐文件重复 import。
pub(crate) use axum::{
    body::{Body, Bytes},
    extract::{Request, State as AxumState},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
pub(crate) use futures::StreamExt;
pub(crate) use serde_json::Value;
pub(crate) use std::sync::Arc;
pub(crate) use tracing::Instrument;

// gateway 子模块整体 re-export：保证子模块内 `super::db::X` / `super::estimate::Y` 等
// 完整路径解析（原 proxy.rs 的 super=gateway，拆分后子模块 super=proxy，靠此 re-export 等价）。
pub(crate) use super::{
    adapter, db, estimate, http_client, log_util, manual_budget, models, notification, router,
    scheduling, usage_color,
};

pub(crate) use super::adapter::{ChatRequest, ChatStreamEvent};
pub(crate) use super::db::Db;
pub(crate) use super::i18n::{self, ErrorKey, Lang};
pub(crate) use super::middleware::{InboundOutcome, MiddlewareEngine};
pub(crate) use super::models::{
    ClientType, Group, Protocol, ProxyAttempt, ProxyLog, ProxyLogSettings, ProxyTimeoutSettings,
};
pub(crate) use super::router::{select_candidates_ctx, RouteResult, ScheduleCtx};

mod count_tokens;
mod connect;
mod endpoint;
mod finish;
mod forward;
mod group_info;
mod handler;
mod headers;
mod non_success;
mod health;
mod log;
mod mock;
mod notify;
mod passthrough;
mod responses;
mod retry;
mod stream;
mod timeout;

#[cfg(test)]
mod test_integration;
#[cfg(test)]
mod test_group_info;
#[cfg(test)]
mod test_connect;

// 对外路径保持 `gateway::proxy::X` 不变：re-export 全部对外 pub 项。
pub use endpoint::{opencode_zen_fallback, resolve_opencode_zen_key};
pub use handler::handle_proxy;
pub use headers::{
    apply_client_headers, build_upstream_headers, inject_coding_plan_fields,
    override_coding_plan_path,
};
// redact_key 仅 headers 内部消费，但作为对外 API 一致性保留可达路径 `gateway::proxy::redact_key`。
#[allow(unused_imports)]
pub use headers::redact_key;
pub use passthrough::{apply_models_auth, build_models_url};

// 子模块内部互用项（crate 内可见，便于 handler/各模块交叉调用）。
pub(crate) use count_tokens::{handle_count_tokens, is_count_tokens_endpoint};
pub(crate) use endpoint::{
    detect_source_protocol, infer_passthrough_protocol_from_ua, match_platform_by_host,
    resolve_group, select_endpoint_for_protocol,
};
pub(crate) use finish::{finish_nonstream, finish_stream};
pub(crate) use forward::{forward_attempt, AttemptOutcome};
pub(crate) use non_success::handle_non_success;
pub(crate) use group_info::handle_group_info;
pub(crate) use headers::{
    format_pretty_json, is_sensitive_auth_header, passthrough_convert_headers, passthrough_headers,
};
// is_official_anthropic_host 仅 headers 内部 + 测试消费；重导出供 test_passthrough 可达。
#[allow(unused_imports)]
pub(crate) use headers::is_official_anthropic_host;
pub(crate) use health::handle_root;
pub(crate) use log::{
    block_inbound, get_log_settings, remove_log_snapshot, spawn_estimate, upsert_connect_log,
    upsert_log,
};
pub(crate) use mock::handle_mock;
pub(crate) use notify::handle_notify;
pub(crate) use passthrough::{handle_models_static, handle_passthrough, is_models_endpoint};
pub(crate) use responses::{handle_responses_subendpoint, is_responses_subendpoint};
pub(crate) use retry::{
    classify_429, classify_stream_first, extract_error_message, filter_upstream_resp_headers,
    is_nonstream_body_valid, is_status_retryable, resp_headers_to_log_json, truncate_attempt_error,
    StreamPeek,
};
pub(crate) use stream::{
    extract_usage, replace_model_in_json, resolve_is_stream, StreamAggregator, StreamEstCtx,
    StreamLogGuard,
};
pub(crate) use timeout::{get_system_timeout, resolve_timeout};

/// 从 DB 读取 app locale，失败则回退英文
pub(crate) async fn get_lang(db: &Arc<Db>) -> Lang {
    super::db::get_setting(db, "app", "locale")
        .await
        .ok()
        .flatten()
        .and_then(|v| v.get("locale").and_then(|s| s.as_str()).map(String::from))
        .map(|s| Lang::from_locale(&s))
        .unwrap_or_default()
}

/// 代理服务器共享状态
pub struct ProxyState {
    /// 用 Arc<Db> 而非 Mutex<Db>：Db 内部已自带 Mutex<Connection>，
    /// Arc 便于克隆进后台预估 spawn（每次操作锁内自治，禁持锁跨 await）。
    pub db: Arc<Db>,
    /// 可选 AppHandle：预估更新后 emit "tray-refresh" 事件让主线程刷新托盘。
    /// 后台 spawn 不直接操作 tray（线程安全），改 emit 事件由主线程 setup 监听刷新。
    pub app: Option<tauri::AppHandle>,
    /// 中间件规则引擎单例（与 lib.rs app.manage 的同一 Arc，C2/C3 入站/出站执行用）。
    pub middleware: Arc<MiddlewareEngine>,
    /// 调度器状态（per-platform 熔断 + 延迟 EMA + 在途计数，内存）。
    pub scheduler: Arc<super::scheduling::SchedulerState>,
    /// Sticky session 绑定表（内存 LRU + TTL）。
    pub sticky: Arc<super::scheduling::StickyTable>,
    /// 渐进式日志的 per-id 已落库列快照（in-flight 请求各 1 份）。
    /// 首节点 INSERT 后存快照；后续节点与快照 diff，仅 UPDATE 变化列；终态写入后移除。
    /// 用 Mutex<HashMap> 而非线程局部：流式 guard 在独立 task/Drop 路径写终态，
    /// 须与 handler 主链路共享同一 id 的快照才能正确 diff。
    pub log_snapshots: std::sync::Mutex<std::collections::HashMap<String, super::db::ProxyLogColumns>>,
    /// 已聚合（写入 stats_agg_hourly）的请求 id 去重缓存，防重复计数。
    /// 背景：upsert_log 在单个请求生命周期内被调用 40+ 次（insert + 多次 update + 流式 flush），
    /// 终态后每次调用仍满足 agg gate → 同一请求被 +1 多次（实测 ~8 倍虚高）。
    /// 不能复用 log_snapshots 去重：(1) agg 写在 `!settings.enabled` 早退之前，关日志时 snapshot
    /// 根本不存在；(2) snapshot 在终态后被 remove_log_snapshot 立即移除，而终态 upsert_log 会被
    /// 反复调用（remove 后下次又见 prev=None），无法据此防止重复 agg。
    /// 用**有界 FIFO 去重缓存**（非按请求生命周期清理）：插入返回是否首次出现，首次才聚合；
    /// 容量上限 AGG_DEDUP_CAP，超限按 FIFO 淘汰最旧 id（in-flight + 已完成请求量远小于此上限，
    /// 同一请求的多次终态调用集中在极短窗口，淘汰不会误判）。HashSet 判存 + VecDeque 记顺序。
    pub agg_done: std::sync::Mutex<(std::collections::VecDeque<String>, std::collections::HashSet<String>)>,
}

/// agg 去重缓存容量上限。远大于任一时刻 in-flight + 近期完成请求数，保证同一请求的全部
/// 重复终态调用窗口内 id 不被淘汰；超限按 FIFO 淘汰最旧，内存有界。
pub(crate) const AGG_DEDUP_CAP: usize = 8192;

/// 向 agg 去重缓存登记 id；返回 true=首次（应聚合），false=已存在（应跳过）。超容量按 FIFO 淘汰。
pub(crate) fn agg_mark_first(state: &Arc<ProxyState>, id: &str) -> bool {
    let mut guard = state.agg_done.lock().unwrap();
    let (order, seen) = &mut *guard;
    if seen.contains(id) {
        return false;
    }
    seen.insert(id.to_string());
    order.push_back(id.to_string());
    while order.len() > AGG_DEDUP_CAP {
        if let Some(old) = order.pop_front() {
            seen.remove(&old);
        }
    }
    true
}

/// 启动代理服务器，返回 shutdown handle
pub async fn start_proxy(
    db: Arc<Db>,
    port: u16,
    app: Option<tauri::AppHandle>,
    middleware: Arc<MiddlewareEngine>,
    bind_lan: bool,
) -> Result<(tokio::task::JoinHandle<()>, u16), String> {
    let state = Arc::new(ProxyState {
        db,
        app,
        middleware,
        scheduler: Arc::new(super::scheduling::SchedulerState::new()),
        sticky: Arc::new(super::scheduling::StickyTable::new()),
        log_snapshots: std::sync::Mutex::new(std::collections::HashMap::new()),
        agg_done: std::sync::Mutex::new((std::collections::VecDeque::new(), std::collections::HashSet::new())),
    });

    let app = Router::new()
        .route("/api/group-info", post(handle_group_info))
        .route("/api/notify", post(handle_notify))
        // 健康端点：客户端（Claude Code / Codex 启动探测等）会命中代理根 URL（含 / 前缀），
        // 无 Authorization 不应进 handle_proxy 走 404，也不应落 proxy_log 污染统计。
        // 仅返回 200 + 身份 JSON，跳过组路由 / 日志 / 上游。
        .route("/", get(handle_root))
        .route("/proxy", get(handle_root))
        .fallback(handle_proxy)
        .with_state(state);

    // Try binding from port upward; if occupied, try port+1..port+100
    let mut actual_port = port;
    // bind_lan=true → 0.0.0.0（局域网其他设备可连，靠 group_key Bearer 鉴权兜底）
    // bind_lan=false → 127.0.0.1（仅本机）
    let bind_ip: [u8; 4] = if bind_lan { [0, 0, 0, 0] } else { [127, 0, 0, 1] };
    let listener = loop {
        let addr = std::net::SocketAddr::from((bind_ip, actual_port));
        match tokio::net::TcpListener::bind(addr).await {
            Ok(l) => break l,
            Err(e) if e.kind() == std::io::ErrorKind::AddrInUse => {
                tracing::warn!(port = actual_port, "proxy bind port in use, trying next");
                actual_port += 1;
                if actual_port > port + 100 {
                    tracing::error!(start = port, end = port + 101, "proxy bind failed: no available port in range");
                    return Err(format!("no available port in range {}..{}", port, port + 101));
                }
                continue;
            }
            Err(e) => {
                tracing::error!(port = actual_port, error = %e, "proxy bind failed");
                return Err(format!("bind failed: {e}"));
            }
        }
    };

    tracing::info!(port = actual_port, "proxy server bound, starting");

    let handle = tokio::spawn(async move {
        axum::serve(listener, app).await.ok();
    });

    Ok((handle, actual_port))
}
