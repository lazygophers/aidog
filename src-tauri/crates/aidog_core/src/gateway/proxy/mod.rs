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

// gateway 子模块整体 re-export：保证子模块内 `aidog_db::X` / `super::estimate::Y` 等
// 完整路径解析（原 proxy.rs 的 super=gateway，拆分后子模块 super=proxy，靠此 re-export 等价）。
pub(crate) use super::{
    estimate, http_client, log_util, manual_budget, models, router,
    scheduling, usage_color,
};

pub(crate) use aidog_adapter::{self as adapter, ChatRequest, ChatStreamEvent};
pub(crate) use aidog_db::Db;
pub(crate) use super::i18n::{self, ErrorKey, Lang};
pub(crate) use aidog_middleware::{InboundOutcome, MiddlewareEngine};
pub(crate) use super::models::{
    ClientType, Group, Protocol, ProxyAttempt, ProxyLog, ProxyLogSettings, ProxyTimeoutSettings,
};
pub(crate) use super::router::{select_candidates_ctx, RouteResult, ScheduleCtx};

mod bench;
mod count_tokens;
mod connect;
mod devin;
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
mod settings_cache;
mod stream;
mod timeout;
mod tokenizer;

#[cfg(test)]
mod test_integration;
#[cfg(test)]
mod test_group_info;
#[cfg(test)]
mod test_connect;
#[cfg(test)]
mod test_e2e_mitm;
#[cfg(test)]
mod test_agg_dedup;
#[cfg(test)]
mod test_bind;

// 对外路径保持 `gateway::proxy::X` 不变：re-export 全部对外 pub 项。
pub use endpoint::{opencode_zen_fallback, resolve_opencode_zen_key};
pub use handler::handle_proxy;
/// settings_set 写 DB 后调此重建 ProxyState 设置缓存（跨 crate：commands_config 调用）。
pub use settings_cache::refresh_proxy_settings_cache;
// ST5：明文 MITM 路径灌入用（CONNECT 分流已移至 handle_proxy_inner，core 只处理 AI 请求，
// 打破与 handle_connect 的互递归 Send 死锁）。
pub(crate) use handler::handle_proxy_core;
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
    detect_source_protocol, infer_passthrough_protocol_from_ua,
    match_platform_by_host, resolve_group, select_endpoint_for_protocol,
    should_fallback_passthrough,
};
pub(crate) use finish::{finish_nonstream, finish_stream, AttemptCtx};
pub(crate) use forward::{forward_attempt, AttemptOutcome};
pub(crate) use non_success::handle_non_success;
pub(crate) use bench::handle_bench_query;
pub(crate) use group_info::handle_group_info;
pub(crate) use headers::{
    format_pretty_json, inject_trace_header, is_sensitive_auth_header, passthrough_convert_headers,
    passthrough_headers,
};
// is_official_anthropic_host 仅 headers 内部 + 测试消费；重导出供 test_passthrough 可达。
#[allow(unused_imports)]
pub(crate) use headers::is_official_anthropic_host;
pub(crate) use health::handle_root;
// remove_log_snapshot/spawn_log_writer/LogMsg 仅测试文件 unqualified 消费（本 mod.rs 走 log:: 全限定路径）；
// 非 test cfg 下重导出未被引用，同 is_official_anthropic_host 先例 allow。
#[allow(unused_imports)]
pub(crate) use log::{
    block_inbound, get_log_settings, remove_log_snapshot, spawn_estimate, spawn_log_writer,
    upsert_connect_log, upsert_log, LogMsg,
};
#[cfg(test)]
pub(crate) use log::flush_log_queue;
pub(crate) use mock::handle_mock;
pub(crate) use notify::handle_notify;
pub(crate) use passthrough::{
    build_url_from_host, forward_passthrough_to_orig_host, handle_models_static,
    handle_passthrough, is_models_endpoint,
};
pub(crate) use responses::{handle_responses_subendpoint, is_responses_subendpoint};
pub(crate) use retry::{
    classify_429, classify_stream_first, err_chain, extract_error_message,
    filter_upstream_resp_headers, is_nonstream_body_valid, is_status_retryable,
    is_transport_retryable, resp_headers_to_log_json, transport_retry_backoff,
    truncate_attempt_error, truncate_peek_text, StreamPeek, TRANSPORT_RETRY_MAX,
};
pub(crate) use stream::{
    cap_nonstream_body, extract_usage, replace_model_in_json, replace_model_in_sse_text,
    resolve_is_stream, StreamAggregator, StreamEstCtx, StreamLogGuard, SseLineReassembler,
    Utf8ChunkReassembler,
};
pub(crate) use timeout::{get_system_timeout, resolve_timeout};
pub(crate) use settings_cache::{register as register_settings_cache, ProxySettingsCache};

/// 从 DB 读取 app locale，失败则回退英文
pub(crate) async fn get_lang(db: &Db) -> Lang {
    aidog_db::get_setting(db, "app", "locale")
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
    /// 用 DashMap 而非线程局部：流式 guard 在独立 task/Drop 路径写终态，
    /// 须与 handler 主链路共享同一 id 的快照才能正确 diff。
    /// DashMap 分片锁替代原 std::sync::Mutex<HashMap> 全局锁，降并发竞争（perf s5）。
    pub log_snapshots: dashmap::DashMap<String, aidog_logs::ProxyLogColumns>,
    /// 已聚合（写入 stats_agg_hourly）的请求 id 去重缓存，防重复计数。
    /// 背景：upsert_log 在单个请求生命周期内被调用 40+ 次（insert + 多次 update + 流式 flush），
    /// 终态后每次调用仍满足 agg gate → 同一请求被 +1 多次（实测 ~8 倍虚高）。
    /// 不能复用 log_snapshots 去重：(1) agg 写在 `!settings.enabled` 早退之前，关日志时 snapshot
    /// 根本不存在；(2) snapshot 在终态后被 remove_log_snapshot 立即移除，而终态 upsert_log 会被
    /// 反复调用（remove 后下次又见 prev=None），无法据此防止重复 agg。
    /// 用**有界 FIFO 去重缓存**（非按请求生命周期清理）：插入返回是否首次出现，首次才聚合；
    /// 容量上限 AGG_DEDUP_CAP，超限按 FIFO 淘汰最旧 id（同一请求的多次终态调用集中在极短窗口，
    /// 只要窗口覆盖住实际并发量，淘汰不会误判）。HashSet 判存 + VecDeque 记顺序。
    /// 容量取值依据见 AGG_DEDUP_CAP 定义处注释。
    pub agg_done: std::sync::Mutex<(std::collections::VecDeque<String>, std::collections::HashSet<String>)>,
    /// 代理实际监听地址（bind_ip, port）。start_proxy 绑定成功后填入，
    /// 供 fallback 直通判定识别「代理自身 host 直连」vs「MITM 解密灌入」（Host ≠ 自身）。
    /// None = 未启动 / 测试构造的 state；fallback 走保守分支（不直通，保留 404）。
    pub listen_addr: std::sync::OnceLock<(std::net::IpAddr, u16)>,
    /// 请求路径设置缓存（log_settings/lang/middleware_settings/system_timeout/proxy_client），
    /// start_proxy 初始化；settings_set 写 DB 后 refresh_proxy_settings_cache 重建。
    /// 详见 settings_cache.rs。请求路径 read().await 一借即得 typed struct，零 serde 反序列化。
    pub(crate) settings_cache: Arc<tokio::sync::RwLock<ProxySettingsCache>>,
    /// 日志异步写入队列发送端。热路径 upsert_log/upsert_connect_log 入队即返回，
    /// 单后台 writer task（spawn_log_writer）串行消费落库，见 log.rs。
    pub(crate) log_tx: tokio::sync::mpsc::Sender<log::LogMsg>,
}

/// 日志写入队列容量。终态消息用阻塞 send 保证不丢；非终态消息队满即丢（不影响最终数据）。
///
/// 定值依据（s4 proxy-hotpath-buffers，实测非估算）：只读采样真实 `~/.aidog/log.db`
/// （`proxy_log` 表，2071 行，未改动/未移动原库）算单条 `ProxyLog` 深拷贝字节量——对 8 个
/// 大 String 列 + `attempts` JSON 求 `length()` 之和：均值 576,775 B、中位数 310,677 B、
/// p90 1,396,402 B、p99 3,587,012 B、max 3,691,023 B（编码 agent 场景常见长上下文对话体）。
/// 旧值 4096 条在均值下等价 ≈ 4096×576KB ≈ 2.3GB 无界风险（`log_snapshots`/日志队列同源
/// 结构体，字节维度远超合理常驻上限）。
/// 新值按均值反推：目标常驻上限 ~300MB / 576KB ≈ 512 条。取 512（2 的幂，与 `AGG_DEDUP_CAP`
/// 同款「留够并发窗口 + 安全余量」idiom）——终态消息只在单请求生命周期尾部各发 1 次、由单
/// writer 串行快速消费，真实同时在途深度远低于历史「单机数十路并发会话」量级，512 留足冗余；
/// 中间态本就允许队满即丢（本 task 另把该分支的深拷贝挪到队满判定之后，见 `log.rs::upsert_log`）。
pub(crate) const LOG_QUEUE_CAP: usize = 512;

/// agg 去重缓存容量上限。
///
/// 单条记录字节成本（id 为 `Uuid::new_v4().simple()`，固定 32 hex 字符，见
/// `handler.rs:10`/`connect.rs:735`）：id 同时存进 `order: VecDeque<String>` 和
/// `seen: HashSet<String>` 两份，各一次堆分配 32B（macOS malloc small class 对齐，无浪费）
/// = 64B 字符串堆数据/条；外加容器骨架分摊——HashSet(hashbrown) 满载 7/8 load factor 下
/// 桶数取 cap/0.875 后上取整到 2 的幂，每桶 24B(String) + 1B 控制字节；VecDeque 背后数组
/// 按 cap 容量分摊，每槽 24B(String)。旧 cap=8192 时总占用 ≈ 8192×64B(串) + 16384×25B(哈希表)
/// + 8192×24B(双端队列) ≈ 1.08MB。
///
/// 实际窗口需求：本值只需覆盖「同一请求的全部重复终态调用」跨越的并发窗口（注释见上），
/// 不是历史总量。代码内无并发上限（scheduling.rs 的 per-platform inflight 计数器无 cap，
/// 全仓 grep 无 Semaphore/max_concurrent），按本地/小团队场景的现实并发上界估算：
/// 单机同时开的 CLI agent 会话数 ≈ 数十量级，取 200 作宽裕上界，再加 10x 安全余量 =2000，
/// 按 2 的幂取整 = 2048（hashbrown 友好）。cap=2048 时占用 ≈ 2048×64B + 4096×25B(哈希表按
/// 2048/0.875≈2341 上取整到 4096) + 2048×24B ≈ 372KB，较旧值省约 700KB 常驻。
pub(crate) const AGG_DEDUP_CAP: usize = 2048;

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

/// 代理绑定失败原因。区分「端口被占用」与其他绑定失败（权限不足 / 地址非法等），
/// 供上层（前端错误条 / 系统通知，见 s2/s3）据此选不同文案 —— 禁靠字符串前缀匹配，
/// 一律用本 enum 判别，文案本身归 i18n（见 design.md「提醒层」）。
#[derive(Debug)]
pub enum ProxyBindError {
    /// 地址已被占用（`AddrInUse`）。携带用户设定的端口号。
    AddrInUse(u16),
    /// 其他绑定失败（权限不足 / 地址非法等）。携带端口号 + 原始错误描述（英文，非用户可读文案）。
    Other(u16, String),
}

impl ProxyBindError {
    /// 触发绑定的端口号（无论哪种失败原因，上层展示错误都需要它）。
    pub fn port(&self) -> u16 {
        match self {
            Self::AddrInUse(port) => *port,
            Self::Other(port, _) => *port,
        }
    }
}

impl std::fmt::Display for ProxyBindError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::AddrInUse(port) => write!(f, "port {port} already in use"),
            Self::Other(port, msg) => write!(f, "bind failed on port {port}: {msg}"),
        }
    }
}

/// 启动代理服务器，返回 shutdown handle。
///
/// 端口是用户设定值，不是程序可协商的输出：绑定失败（含地址被占用）一律直接返回
/// `Err`，不再递增重试换端口（旧行为见 git history，会导致「设置里的端口」与「实际监听
/// 端口」漂移且单向不可逆，根因见 proxy-port-no-drift/design.md）。
pub async fn start_proxy(
    db: Arc<Db>,
    port: u16,
    app: Option<tauri::AppHandle>,
    middleware: Arc<MiddlewareEngine>,
    bind_lan: bool,
) -> Result<tokio::task::JoinHandle<()>, ProxyBindError> {
    let (log_tx, log_rx) = tokio::sync::mpsc::channel::<log::LogMsg>(LOG_QUEUE_CAP);
    let state = Arc::new({
        // 初始化设置缓存：从 DB 读一次填入 typed struct，注册到全局 weak 槽供 settings_set 重建。
        let settings_cache = Arc::new(tokio::sync::RwLock::new(
            ProxySettingsCache::load_from(&db).await,
        ));
        register_settings_cache(&settings_cache);
        ProxyState {
            db,
            app,
            middleware,
            scheduler: Arc::new(super::scheduling::SchedulerState::new()),
            sticky: Arc::new(super::scheduling::StickyTable::new()),
            log_snapshots: dashmap::DashMap::new(),
            agg_done: std::sync::Mutex::new((std::collections::VecDeque::new(), std::collections::HashSet::new())),
            listen_addr: std::sync::OnceLock::new(),
            settings_cache,
            log_tx,
        }
    });
    log::spawn_log_writer(state.clone(), log_rx);

    // bind_lan=true → 0.0.0.0（局域网其他设备可连，靠 group_key Bearer 鉴权兜底）
    // bind_lan=false → 127.0.0.1（仅本机）
    let bind_ip: [u8; 4] = if bind_lan { [0, 0, 0, 0] } else { [127, 0, 0, 1] };
    let addr = std::net::SocketAddr::from((bind_ip, port));
    let listener = match tokio::net::TcpListener::bind(addr).await {
        Ok(l) => l,
        Err(e) if e.kind() == std::io::ErrorKind::AddrInUse => {
            tracing::warn!(port, "proxy bind port in use");
            return Err(ProxyBindError::AddrInUse(port));
        }
        Err(e) => {
            tracing::error!(port, error = %e, "proxy bind failed");
            return Err(ProxyBindError::Other(port, e.to_string()));
        }
    };

    tracing::info!(port, "proxy server bound, starting");

    // 记录实际监听地址供 fallback 直通判定（识别代理自身 host vs MITM 解密灌入）。
    // OnceLock：bind 成功后地址不变，忽略 set 失败（state 被复用场景理论不存在）。
    let _ = state.listen_addr.set((
        std::net::IpAddr::V4(std::net::Ipv4Addr::new(bind_ip[0], bind_ip[1], bind_ip[2], bind_ip[3])),
        port,
    ));

    let app = build_router(state);

    let handle = crate::logging::spawn_traced("axum_serve", async move {
        axum::serve(listener, app).await.ok();
    });

    Ok(handle)
}

/// 构建代理 Router：absolute-form forward middleware 包外层，识别 `-x` forward 客户端的
/// absolute-form URI（`GET http://host/path`）→ 直接转 handle_proxy 走 fallback 直通，
/// 绕过 `.route("/")` 健康端点（axum 按 `uri.path()` 匹配会劫持 absolute-form 的 `/`）。
/// path-only URI（reverse proxy 常规请求）→ next.run 进正常路由。
fn build_router(state: Arc<ProxyState>) -> Router {
    Router::new()
        .route("/api/group-info", post(handle_group_info))
        .route("/api/notify", post(handle_notify))
        // ponytail: 量测专用调试端点，驱动固定查询走真实读连接池以复现 page cache 常驻
        // （见 sqlite-page-cache-residency/design.md「数据流」），无鉴权但 localhost-only
        // 绑定 + 只读查询零副作用，与 /api/group-info 同信任边界。
        .route("/api/debug/bench-query", post(handle_bench_query))
        // 健康端点：客户端（Claude Code / Codex 启动探测等）会命中代理根 URL（含 / 前缀），
        // 无 Authorization 不应进 handle_proxy 走 404，也不应落 proxy_log 污染统计。
        // 仅返回 200 + 身份 JSON，跳过组路由 / 日志 / 上游。
        .route("/", get(handle_root))
        .route("/proxy", get(handle_root))
        .fallback(handle_proxy)
        .with_state(state.clone())
        .layer(axum::middleware::from_fn_with_state(
            state,
            absolute_form_forward_mw,
        ))
}

/// forward proxy absolute-form 识别 middleware：HTTP forward 客户端（curl `-x`）发出
/// `GET http://www.baidu.com/` 这类 absolute-form URI（含 scheme + authority），
/// axum 按 `uri.path()`=`/` 匹配 `.route("/")` → 健康端点劫持。
/// 此处识别 scheme+host 同时存在 → 直接转 handle_proxy（进 fallback 直通原 host），
/// 不进 axum path 匹配。CONNECT 已在 handle_proxy 入口分流（不触发此 middleware 路径问题）。
///
/// ponytail: 复用 handle_proxy 完整请求生命周期（req span / RequestLogGuard / fallback），
/// 不新写 forward handler；与 MITM fallback 同语义（虚拟桶「未匹配」+ cost=0）。
async fn absolute_form_forward_mw(
    axum::extract::State(state): axum::extract::State<Arc<ProxyState>>,
    req: Request,
    next: axum::middleware::Next,
) -> Response {
    let uri = req.uri();
    if uri.scheme_str().is_some() && uri.host().is_some() {
        // absolute-form → forward proxy 客户端请求 → 走 handle_proxy（fallback 直通原 host）。
        return handle_proxy(axum::extract::State(state), req).await;
    }
    next.run(req).await
}
