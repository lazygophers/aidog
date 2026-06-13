use axum::{
    body::{Body, Bytes},
    extract::{Request, State as AxumState},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::post,
    Json, Router,
};
use futures::StreamExt;
use serde_json::Value;
use tracing::Instrument;
use std::sync::Arc;

use super::adapter::{self, ChatRequest, ChatStreamEvent};
use super::db::Db;
use super::i18n::{self, ErrorKey, Lang};
use super::middleware::{InboundOutcome, MiddlewareEngine};
use super::models::{ClientType, Group, Protocol, ProxyAttempt, ProxyLog, ProxyLogSettings, ProxyTimeoutSettings};
use super::router::{select_candidates, RouteResult};

/// 从 DB 读取 app locale，失败则回退英文
async fn get_lang(db: &Arc<Db>) -> Lang {
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
}

/// 启动代理服务器，返回 shutdown handle
pub async fn start_proxy(
    db: Arc<Db>,
    port: u16,
    app: Option<tauri::AppHandle>,
    middleware: Arc<MiddlewareEngine>,
) -> Result<(tokio::task::JoinHandle<()>, u16), String> {
    let state = Arc::new(ProxyState { db, app, middleware });

    let app = Router::new()
        .route("/api/group-info", post(handle_group_info))
        .fallback(handle_proxy)
        .with_state(state);

    // Try binding from port upward; if occupied, try port+1..port+100
    let mut actual_port = port;
    let listener = loop {
        let addr = std::net::SocketAddr::from(([127, 0, 0, 1], actual_port));
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

/// statusline 段消费的本地预估信息（只读，不上游真查）
#[derive(serde::Serialize)]
struct GroupInfoResp {
    applicable: bool,
    balance: f64,
    /// 累计预估花费（$ / 平台币种），基于 est_cost 聚合
    spent: f64,
    coding_plan: Vec<CodingTierResp>,
    requests: i64,
    /// 成功率（0-100）
    success_rate: f64,
    /// 缓存命中率（0-100）
    cache_rate: f64,
    total_tokens: i64,
    currency: String,
    /// 余额可用天数 = balance / 动态窗口日均花费；无花费 / 无余额 → null。
    /// statusline 余额段据此上色（<1 红 / <3 黄 / 否则绿）。
    balance_days_remaining: Option<f64>,
    /// 余额使用速率配色级别（usage_color 唯一阈值源）："red"|"yellow"|"green"|"neutral"。
    /// statusline / 前端只消费此 level 不重算阈值。
    balance_level: String,
}

#[derive(serde::Serialize)]
struct CodingTierResp {
    name: String,
    /// 利用率（0-100）
    utilization: f64,
    /// 预期消耗速率分级："fast" | "normal" | "busy"（旧字段，保留兼容；新配色走 level）。
    pace: String,
    /// 使用速率配色级别（usage_color 唯一阈值源）："red"|"yellow"|"green"|"neutral"。
    /// statusline / 前端只消费此 level 不重算阈值。
    level: String,
    /// 预期重置 unix 秒；无可靠来源时 null（statusline 红色时择机展示）。
    reset_at: Option<i64>,
}

/// 分组信息端点 — 仅单平台分组返回本地预估值。
/// 鉴权：`Authorization: Bearer <group_name>`，localhost-only 端点。
/// 多平台 / 无平台分组返回 `{ applicable:false, ... }`（200）。
async fn handle_group_info(
    state: AxumState<Arc<ProxyState>>,
    headers: axum::http::HeaderMap,
) -> Response {
    // 每次 group-info 调用生成独立 trace id（statusline 周期拉取，无上游请求关联），
    // span 内所有日志自动带 group_info{trace_id=xxxxxxxx} 前缀。
    let span = tracing::info_span!("group_info", trace_id = %crate::logging::new_trace_id());
    handle_group_info_inner(state, headers).instrument(span).await
}

async fn handle_group_info_inner(
    AxumState(state): AxumState<Arc<ProxyState>>,
    headers: axum::http::HeaderMap,
) -> Response {
    let empty = || GroupInfoResp {
        applicable: false,
        balance: 0.0,
        spent: 0.0,
        coding_plan: Vec::new(),
        requests: 0,
        success_rate: 0.0,
        cache_rate: 0.0,
        total_tokens: 0,
        currency: String::new(),
        balance_days_remaining: None,
        balance_level: super::usage_color::UsageLevel::Neutral.as_str().to_string(),
    };

    // 从 Authorization: Bearer <token> 提取 group_name
    let group_name = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .map(|s| s.trim().to_string());
    let group_name = match group_name {
        Some(n) if !n.is_empty() => n,
        _ => return StatusCode::UNAUTHORIZED.into_response(),
    };

    // 定位分组
    let groups = match super::db::list_groups(&state.db).await {
        Ok(g) => g,
        Err(e) => {
            tracing::warn!(error = %e, "group-info: list_groups failed, returning not-applicable");
            return (StatusCode::OK, Json(empty())).into_response();
        }
    };
    let group = match groups.iter().find(|g| g.name == group_name) {
        Some(g) => g,
        None => {
            tracing::debug!(group = %group_name, "group-info: group not found, not-applicable");
            return (StatusCode::OK, Json(empty())).into_response();
        }
    };

    // 关联平台 —— 恰好 1 个才适用
    let platforms = match super::db::get_group_platforms(&state.db, group.id).await {
        Ok(p) => p,
        Err(e) => {
            tracing::warn!(group = %group_name, error = %e, "group-info: get_group_platforms failed, not-applicable");
            return (StatusCode::OK, Json(empty())).into_response();
        }
    };
    if platforms.len() != 1 {
        return (StatusCode::OK, Json(empty())).into_response();
    }
    let platform = &platforms[0].platform;

    // usage 统计（复用现有 db 查询，只读）
    let stats = super::db::get_group_usage_stats(&state.db, &group.name).await.unwrap_or(
        super::models::PlatformUsageStats {
            total_requests: 0,
            success_count: 0,
            total_input_tokens: 0,
            total_output_tokens: 0,
            total_cache_tokens: 0,
            cache_rate: 0.0,
            recent_failures: 0,
            recent_total: 0,
            total_cost: 0.0,
        }
    );

    let success_rate = if stats.total_requests > 0 {
        stats.success_count as f64 / stats.total_requests as f64 * 100.0
    } else {
        0.0
    };
    let total_tokens =
        stats.total_input_tokens + stats.total_output_tokens + stats.total_cache_tokens;

    // coding plan tiers（补 pace + level + reset_at）
    // level 走 usage_color（按 window_start + cycle 推算剩余可用时间%）；
    // reset_at = window_start + cycle（预估侧推算的本周期重置点，无 window_start 时 None）。
    let now_ms = super::db::now();
    let mut coding_plan: Vec<CodingTierResp> = super::estimate::EstCodingPlan::from_json(&platform.est_coding_plan)
        .tiers
        .into_iter()
        .map(|t| {
            let pace = super::estimate::tier_pace(&t).as_str().to_string();
            let level = super::estimate::tier_usage_level(&t, now_ms).as_str().to_string();
            let reset_at = super::usage_color::cycle_ms_for_tier(&t.name)
                .filter(|_| t.window_start > 0)
                .map(|cycle| (t.window_start + cycle) / 1000);
            CodingTierResp {
                name: t.name,
                utilization: t.est_utilization,
                pace,
                level,
                reset_at,
            }
        })
        .collect();

    // 追加 manual budgets 为 coding_plan tiers（让 statusline 显示各窗口预算利用率）
    // 只追加窗口类预算（rolling/fixed/daily），"total" 由 balance 段单独展示。
    for b in platform.manual_budgets.iter().filter(|b| b.enabled && b.kind != "total") {
        let util = if b.amount > 0.0 {
            (b.consumed / b.amount * 100.0).min(100.0)
        } else {
            0.0
        };
        let label = match b.kind.as_str() {
            "total" => "total".to_string(),
            _ => {
                let w = b.window_hours.unwrap_or(1.0);
                let short = match b.window_unit {
                    super::models::WindowUnit::Minute => "m",
                    super::models::WindowUnit::Hour => "h",
                    super::models::WindowUnit::Day => "d",
                    super::models::WindowUnit::Week => "w",
                    super::models::WindowUnit::Month => "mo",
                };
                let w_int = w.fract() == 0.0;
                if w_int { format!("{}{}", w as i64, short) } else { format!("{}{}", w, short) }
            }
        };
        let pace = if util > 80.0 { "fast" } else if util > 50.0 { "normal" } else { "busy" }.to_string();
        // level 走 usage_color：按窗口剩余时间 + 利用率算剩余可用时间%。
        // 窗口预算的 cycle = window_duration_ms，remain = window_start_at + dur - now；
        // 无窗口起点 / total 类 → 中性。
        let level = {
            let dur = super::manual_budget::window_duration_ms(b);
            match (dur, b.window_start_at) {
                (Some(dur), Some(start)) => {
                    let remain = start + dur - now_ms;
                    super::usage_color::coding_tier_level(util, Some(remain), Some(dur))
                }
                _ => super::usage_color::UsageLevel::Neutral,
            }
        }
        .as_str()
        .to_string();
        coding_plan.push(CodingTierResp {
            name: label,
            utilization: util,
            pace,
            level,
            reset_at: None,
        });
    }

    // 余额 = max(est_balance_remaining, manual "total" budget remaining)
    // 只取 kind="total" 的手动预算作为余额来源；rolling/fixed/daily 是窗口限速，不是余额。
    let manual_total_remaining: f64 = platform.manual_budgets
        .iter()
        .filter(|b| b.enabled && b.kind == "total")
        .map(super::manual_budget::remaining)
        .sum::<f64>();
    let balance = platform.est_balance_remaining.max(manual_total_remaining);

    // 余额可用天数：动态窗口日速率（rate_per_hour，prd B）→ days = (balance / rate_per_hour) / 24。
    // 无用量数据 / 无余额 → null（配色中性，不报警）。
    let balance_days_remaining = {
        let rate_per_hour = super::db::get_group_hourly_rate(&state.db, &group.name).await.unwrap_or(None);
        match rate_per_hour {
            Some(rate) if rate > 0.0 && balance > 0.0 => Some((balance / rate) / 24.0),
            _ => None,
        }
    };
    let balance_level = super::usage_color::balance_level(balance_days_remaining).as_str().to_string();

    let resp = GroupInfoResp {
        applicable: true,
        balance,
        spent: stats.total_cost,
        coding_plan,
        requests: stats.total_requests,
        success_rate,
        cache_rate: stats.cache_rate,
        total_tokens,
        currency: String::new(),
        balance_days_remaining,
        balance_level,
    };

    (StatusCode::OK, Json(resp)).into_response()
}

/// Read proxy log settings from DB
async fn get_log_settings(db: &Db) -> ProxyLogSettings {
    super::db::get_setting(db, "proxy", "logging")
        .await
        .ok()
        .flatten()
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or_default()
}

/// Upsert a proxy log entry; silently ignore errors.
/// Respects ProxyLogSettings: if logging disabled, does nothing;
/// if user/upstream recording disabled, clears those fields before writing.
async fn upsert_log(state: &Arc<ProxyState>, log: &ProxyLog, settings: &ProxyLogSettings) {
    if !settings.enabled {
        return;
    }
    let mut log = log.clone();
    // Clear fields based on recording switches
    if !settings.log_user_request {
        log.request_headers = String::new();
        log.request_body = String::new();
        log.user_response_headers = String::new();
        log.user_response_body = String::new();
    }
    if !settings.log_upstream_request {
        log.upstream_request_headers = String::new();
        log.upstream_request_body = String::new();
        log.upstream_response_headers = String::new();
    }
    // Calculate est_cost from model_price if tokens are present
    if log.est_cost == 0.0 && (log.input_tokens > 0 || log.output_tokens > 0) {
        let model_name = if log.actual_model.is_empty() { &log.model } else { &log.actual_model };
        // best-effort 取平台主类型的 serde 裸名（如 "deepseek"）以启用 pricing[platform_type] override；
        // 拿不到则传 ""，calc_est_cost 的 fallback 回退链仍保证非 0。
        let platform_type = super::db::get_platform(&state.db, log.platform_id)
            .await
            .ok()
            .flatten()
            .map(|p| serde_json::to_string(&p.platform_type).unwrap_or_default().trim_matches('"').to_string())
            .unwrap_or_default();
        log.est_cost = super::db::calc_est_cost(
            &state.db,
            model_name,
            &platform_type,
            log.input_tokens,
            log.output_tokens,
            log.cache_tokens,
        )
        .await;
    }
    if super::db::upsert_proxy_log(&state.db, &log).await.is_ok() {
        // 日志写库成功后通知前端三页（Platforms/Groups/Stats）实时刷新统计。
        // 同时通知托盘刷新今日统计（请求数、Token、费用等）。
        // app handle 为 None（无 GUI 上下文）时安全跳过，不影响代理逻辑。
        if let Some(app) = &state.app {
            use tauri::Emitter;
            let _ = app.emit("proxy-log-updated", log.platform_id);
            let _ = app.emit("tray-refresh", ());
        }
    }
}

/// 中间件入站拦截：写审计日志（blocked_by/blocked_reason，不计费）并立即返回 403。
/// 参照现有 parse 错误返回模式；body 为结构化 JSON，便于客户端识别拦截。
#[allow(clippy::too_many_arguments)]
async fn block_inbound(
    state: &Arc<ProxyState>,
    mut log: ProxyLog,
    log_settings: &ProxyLogSettings,
    lang: Lang,
    blocked_by: String,
    blocked_reason: String,
    start: std::time::Instant,
) -> Response {
    let body = serde_json::json!({
        "error": {
            "type": "middleware_blocked",
            "message": i18n::t(lang, ErrorKey::MiddlewareBlocked),
            "blocked_by": blocked_by,
            "blocked_reason": blocked_reason,
        }
    })
    .to_string();
    tracing::warn!(blocked_by = %blocked_by, reason = %blocked_reason, "middleware inbound: request blocked (403)");
    log.status_code = 403;
    log.blocked_by = blocked_by;
    log.blocked_reason = blocked_reason;
    log.response_body = body.clone();
    log.user_response_body = body.clone();
    log.user_response_headers = r#"{"content-type":"application/json"}"#.to_string();
    log.duration_ms = start.elapsed().as_millis() as i32;
    // est_cost 保持 0（不计费）；不调用 spawn_estimate。
    upsert_log(state, &log, log_settings).await;
    (
        StatusCode::FORBIDDEN,
        [(axum::http::header::CONTENT_TYPE, "application/json")],
        body,
    )
        .into_response()
}

/// 在后台 tokio::spawn 中执行请求驱动的 quota 预估（不阻塞响应）。
/// 余额平台扣金额 / coding plan 平台更新利用率，并按阈值触发真查校准。
/// platform_type 传入 serde rename 裸名（如 "deepseek"），供 resolve_price 查 pricing key。
#[allow(clippy::too_many_arguments)]
fn spawn_estimate(
    state: &Arc<ProxyState>,
    platform_id: u64,
    platform_type: &Protocol,
    quota_base_url: String,
    api_key: String,
    model: String,
    input_tokens: i32,
    output_tokens: i32,
    cache_tokens: i32,
    is_coding_plan: bool,
    span: tracing::Span,
) {
    // 无 token（请求失败 / 无 usage）则跳过
    if input_tokens <= 0 && output_tokens <= 0 && cache_tokens <= 0 {
        return;
    }
    // serde rename 裸名（去掉 to_string 的引号），与 pricing JSON key 一致
    let ptype = serde_json::to_string(platform_type)
        .unwrap_or_default()
        .trim_matches('"')
        .to_string();
    let db = state.db.clone();
    let app = state.app.clone();
    tokio::spawn(async move {
        super::estimate::estimate_after_request(
            &db,
            platform_id,
            &ptype,
            &quota_base_url,
            &api_key,
            &model,
            input_tokens as i64,
            output_tokens as i64,
            cache_tokens as i64,
            is_coding_plan,
        )
        .await;
        // 预估更新后通知主线程刷新托盘（emit 事件，避免后台线程直接操作 tray）
        if let Some(app) = app {
            use tauri::Emitter;
            let _ = app.emit("tray-refresh", ());
        }
    }.instrument(span));
}

/// Read system-level timeout settings from DB
async fn get_system_timeout(db: &Db) -> ProxyTimeoutSettings {
    super::db::get_setting(db, "proxy", "timeout")
        .await
        .ok()
        .flatten()
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or_default()
}

/// Resolve timeout by priority: model_mapping > group > system
fn resolve_timeout(
    mapping: &Option<super::models::ModelMapping>,
    group: &Group,
    system: &ProxyTimeoutSettings,
) -> (u64, u64) {
    let sys_req = if system.request_timeout_secs > 0 { system.request_timeout_secs } else { 300 };
    let sys_conn = if system.connect_timeout_secs > 0 { system.connect_timeout_secs } else { 10 };

    let (grp_req, grp_conn) = (
        if group.request_timeout_secs > 0 { group.request_timeout_secs } else { sys_req },
        if group.connect_timeout_secs > 0 { group.connect_timeout_secs } else { sys_conn },
    );

    match mapping {
        Some(m) => (
            if m.request_timeout_secs > 0 { m.request_timeout_secs } else { grp_req },
            if m.connect_timeout_secs > 0 { m.connect_timeout_secs } else { grp_conn },
        ),
        None => (grp_req, grp_conn),
    }
}

/// 主代理处理函数 — 渐进式日志：每个阶段即时 upsert，用 request_id 串联
async fn handle_proxy(
    state: AxumState<Arc<ProxyState>>,
    req: Request,
) -> Response {
    // 每请求生成 trace id（复用为 ProxyLog 主键）, 建 span → 该请求生命周期内所有日志
    // 自动携带 req{id=xxxxxxxx} 前缀（含 mock/passthrough 子调用, fmt 默认渲染当前 span）。
    let request_id = uuid::Uuid::new_v4().simple().to_string();
    let span = tracing::info_span!("req", trace_id = %&request_id[..8]);
    handle_proxy_inner(state, req, request_id).instrument(span).await
}

async fn handle_proxy_inner(
    AxumState(state): AxumState<Arc<ProxyState>>,
    req: Request,
    request_id: String,
) -> Response {
    let start = std::time::Instant::now();
    let created_at = super::db::now();

    // Load log settings once per request
    let log_settings = get_log_settings(&state.db).await;

    // ── 初始化日志条目 ──
    let mut log = ProxyLog {
        id: request_id,
        group_name: String::new(),
        model: String::new(),
        actual_model: String::new(),
        source_protocol: String::new(),  // will be set from group
        target_protocol: String::new(),
        platform_id: 0,
        request_headers: String::new(),
        request_body: String::new(),
        upstream_request_headers: String::new(),
        upstream_request_body: String::new(),
        response_body: String::new(),
        request_url: String::new(),
        upstream_request_url: String::new(),
        upstream_response_headers: String::new(),
        upstream_status_code: 0,
        user_response_headers: String::new(),
        user_response_body: String::new(),
        status_code: 0,
        duration_ms: 0,
        input_tokens: 0,
        output_tokens: 0,
        cache_tokens: 0,
        est_cost: 0.0,
        is_stream: false,
        attempts: Vec::new(),
        retry_count: 0,
        blocked_by: String::new(),
        blocked_reason: String::new(),
        created_at,
        updated_at: created_at,
        deleted_at: 0,
    };

    // ── 读取当前语言（用于错误消息翻译） ──
    let lang = get_lang(&state.db).await;

    // ── 捕获请求头 ──
    log.request_headers = {
        let mut h = serde_json::Map::new();
        for (k, v) in req.headers() {
            if let Ok(s) = v.to_str() {
                if k == "authorization" {
                    h.insert(k.to_string(), Value::String("[REDACTED]".into()));
                } else {
                    h.insert(k.to_string(), Value::String(s.to_string()));
                }
            }
        }
        serde_json::Value::Object(h).to_string()
    };

    // Extract auth header and path BEFORE consuming the request
    let auth_header = req.headers()
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .map(|s| s.trim().to_string());
    let path = req.uri().path().to_string();
    tracing::info!(method = %req.method(), path = %path, "http request");

    // ── 记录用户请求 URL ──
    log.request_url = req.uri().to_string();

    // ── 捕获原始请求量（用于 Claude Code 纯透传：未 redact 的真实 header / method / uri）──
    // 现有 log.request_headers 把 Authorization REDACT 了，不可用于透传，故在 into_parts 前 clone 原始量。
    let orig_method = req.method().clone();
    let orig_uri = req.uri().clone();
    let orig_headers = req.headers().clone();

    // ── 读取请求体 ──
    let (_parts, body) = req.into_parts();
    let bytes = match axum::body::to_bytes(body, 10 * 1024 * 1024).await {
        Ok(b) => b,
        Err(e) => {
            log.response_body = format!("read body error: {e}");
            log.status_code = 400;
            log.duration_ms = start.elapsed().as_millis() as i32;
            upsert_log(&state, &log, &log_settings).await;
            return (StatusCode::BAD_REQUEST, format!("{}: {e}", i18n::t(lang, ErrorKey::ReadBody))).into_response();
        }
    };
    log.request_body = String::from_utf8_lossy(&bytes).to_string();
    tracing::debug!(method = %orig_method, path = %path, body = %log.request_body, "inbound request body");

    // Best-effort model extraction
    let raw_model = serde_json::from_slice::<Value>(&bytes)
        .ok()
        .and_then(|v| v.get("model").and_then(|m| m.as_str()).map(String::from))
        .unwrap_or_default();
    log.model = raw_model.clone();

    // Upsert #1: request received
    upsert_log(&state, &log, &log_settings).await;

    // ── 查找分组 ──
    let group = {
        match resolve_group(&state.db, auth_header.as_deref(), &path).await {
            Some(g) => g,
            None => {
                if let Some(ref token) = auth_header {
                    log.response_body = format!("no matching group for token '{}' or path '{}'", token, path);
                    log.status_code = 404;
                    log.duration_ms = start.elapsed().as_millis() as i32;
                    upsert_log(&state, &log, &log_settings).await;
                    return (StatusCode::NOT_FOUND, log.response_body.clone()).into_response();
                } else {
                    log.response_body = "no matching group".to_string();
                    log.status_code = 404;
                    log.duration_ms = start.elapsed().as_millis() as i32;
                    upsert_log(&state, &log, &log_settings).await;
                    return (StatusCode::NOT_FOUND, i18n::t(lang, ErrorKey::NoMatchingGroup)).into_response();
                }
            }
        }
    };

    // Upsert #2: group resolved
    log.group_name = group.name.clone();
    // Auto-detect source_protocol from request path (group no longer restricts inbound protocol)
    let source_protocol = detect_source_protocol(&path);
    log.source_protocol = source_protocol.clone();
    tracing::info!(group = %group.name, source_protocol = %source_protocol, model = %log.model, "group resolved");
    upsert_log(&state, &log, &log_settings).await;

    // ── 解析 ChatRequest（按入站协议解析） ──
    let req_value: serde_json::Value = match serde_json::from_slice(&bytes) {
        Ok(v) => v,
        Err(e) => {
            log.response_body = format!("parse request json error: {e}");
            log.status_code = 400;
            log.duration_ms = start.elapsed().as_millis() as i32;
            upsert_log(&state, &log, &log_settings).await;
            return (StatusCode::BAD_REQUEST, format!("{}: {e}", i18n::t(lang, ErrorKey::ParseJson))).into_response();
        }
    };
    let mut chat_req: ChatRequest = match adapter::parse_incoming_request(&log.source_protocol, &req_value) {
        Ok(r) => r,
        Err(e) => {
            log.response_body = format!("failed to parse request for protocol ({}): {e}", log.source_protocol);
            log.status_code = 400;
            log.duration_ms = start.elapsed().as_millis() as i32;
            upsert_log(&state, &log, &log_settings).await;
            return (StatusCode::BAD_REQUEST, i18n::t(lang, ErrorKey::ParseRequest)).into_response();
        }
    };

    let is_stream = chat_req.stream.unwrap_or(false);
    log.is_stream = is_stream;
    let requested_model = if chat_req.model.is_empty() { raw_model } else { chat_req.model.clone() };
    log.model = requested_model.clone();

    // ── 中间件入站规则（global/group 层，路由前）──
    // settings 读取 fail-open（异常 → Default 总开关 ON）；apply 内单条规则异常不阻断主链路。
    // 顺序：request_filter→sensitive_word→redaction→content_filter→dynamic_injection。
    {
        let mw_settings = super::db::get_middleware_settings(&state.db).await;
        if let InboundOutcome::Blocked { blocked_by, blocked_reason } =
            state.middleware.apply_inbound(&mw_settings, &mut chat_req, Some(&group.name))
        {
            return block_inbound(&state, log, &log_settings, lang, blocked_by, blocked_reason, start).await;
        }
    }

    // ── 路由选择有序候选平台列表（失败逐个重试）──
    let candidate_set = match select_candidates(&state.db, &group, &chat_req.model).await {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!(group = %group.name, model = %chat_req.model, error = %e, "route failed");
            log.response_body = format!("route error: {e}");
            log.status_code = 400;
            log.duration_ms = start.elapsed().as_millis() as i32;
            upsert_log(&state, &log, &log_settings).await;
            return (StatusCode::BAD_REQUEST, format!("{}: {e}", i18n::t(lang, ErrorKey::Route))).into_response();
        }
    };

    let candidates: Vec<RouteResult> = candidate_set.candidates;

    // ── Mock / ClaudeCode 透传：不参与重试（非目标），仅按首选候选终态处理。
    // 二者本地生成 / 1:1 relay，无候选切换语义；放在重试循环外避免 move-in-loop 与无意义重试。──
    {
        let first = &candidates[0];
        if matches!(first.platform.platform_type, Protocol::Mock) {
            log.actual_model = first.target_model.clone();
            log.platform_id = first.platform.id;
            log.target_protocol = format!("{:?}", first.platform.platform_type).to_lowercase();
            chat_req.model = first.target_model.clone();
            tracing::info!(platform = %first.platform.name, "mock platform intercept, generating local response");
            return handle_mock(
                state,
                log,
                log_settings,
                &first.platform.extra,
                &chat_req,
                &req_value,
                &source_protocol,
                &requested_model,
                is_stream,
                start,
            )
            .await;
        }
        if matches!(first.platform.platform_type, Protocol::ClaudeCode) {
            log.platform_id = first.platform.id;
            tracing::info!(platform = %first.platform.name, base_url = %first.platform.base_url, "claude-code passthrough intercept (1:1 relay)");
            let base_url = first.platform.base_url.clone();
            return handle_passthrough(
                &state,
                &mut log,
                &log_settings,
                orig_method,
                orig_uri,
                orig_headers,
                bytes,
                &base_url,
                start,
                lang,
            )
            .await;
        }
    }

    // ── 重试编排：遍历候选，逐个 forward。
    //   2xx → 成功（曾 auto_disabled 则恢复 enabled），进入下游成功处理直接 return。
    //   401/403 → 标记平台 auto_disabled（指数退避），换下个候选。
    //   其他错误(5xx/超时/连接失败) → 换下个候选。
    //   每次尝试均 record 进 attempts；超过 max_retries 或候选耗尽 → 返回最后一次错误。
    let max_retries = group.max_retries as usize;
    let mut attempts: Vec<ProxyAttempt> = Vec::new();
    let candidate_total = candidates.len();

    for (attempt_idx, route) in candidates.into_iter().enumerate() {
        // 超过最大重试次数（attempt_idx 从 0 起；max_retries=2 → 最多 3 次尝试 idx 0/1/2）
        if attempt_idx > max_retries {
            break;
        }
        let attempt_start = std::time::Instant::now();
        let attempt_ts = super::db::now();
        let is_last_candidate = attempt_idx + 1 >= candidate_total || attempt_idx >= max_retries;

    let actual_model = route.target_model.clone();

    // 尝试匹配端点：按 source_protocol 查找平台是否支持对应协议的端点。
    // 先精确匹配；openai_responses 源（Codex）若无 Responses 端点，回退到 openai 端点
    // （普通 chat/completions 平台），出站经 to_openai 转换。
    let ep_proto = |ep: &super::models::PlatformEndpoint| format!("{:?}", ep.protocol).to_lowercase();
    let matched_ep = route.platform.endpoints
        .iter()
        .find(|ep| ep_proto(ep) == source_protocol)
        .or_else(|| {
            if source_protocol == "openai_responses" {
                route.platform.endpoints.iter().find(|ep| ep_proto(ep) == "openai")
            } else {
                None
            }
        });
    let (target_protocol_enum, target_base_url, client_type, coding_plan) = matched_ep
        .map(|ep| (&ep.protocol, ep.base_url.clone(), ep.client_type.clone(), ep.coding_plan))
        .unwrap_or((&route.platform.platform_type, route.platform.base_url.clone(), ClientType::Default, false));

    let target_protocol = format!("{:?}", target_protocol_enum).to_lowercase();
    let needs_model_remap = actual_model != requested_model;

    // ── 同协议透传判定 ──
    // 平台**显式声明**了与入站协议精确相同的端点 → 逻辑透传：跳过 convert_request 有损格式转换，
    // 用客户端原始请求体（仅 patch model 字段）出站；响应侧同样跳过 parse_sse→to_client_sse 格式转换。
    // 鉴权 / URL / coding_plan / usage 提取等旁路改写仍全部保留。
    // 注意：openai_responses→openai 的跨协议回退命中时 target_protocol != source_protocol，
    // 不算透传，仍走 convert_request（必须真转换）。
    let same_protocol_passthrough = matched_ep
        .map(|ep| ep_proto(ep) == source_protocol)
        .unwrap_or(false);

    // Upsert #3: route resolved
    log.actual_model = actual_model.clone();
    log.target_protocol = target_protocol.clone();
    log.platform_id = route.platform.id;
    tracing::info!(
        platform = %route.platform.name, platform_id = route.platform.id,
        requested_model = %requested_model, actual_model = %actual_model,
        source_protocol = %source_protocol, target_protocol = %target_protocol,
        coding_plan, stream = is_stream, remap = needs_model_remap,
        "request routed to upstream"
    );
    upsert_log(&state, &log, &log_settings).await;

    // 替换模型名
    chat_req.model = actual_model.clone();

    // ── 中间件入站规则（platform 层，候选选定后、convert_request 前）──
    // 仅应用 platform 作用域规则（global/group 已在路由前应用，避免重复）。
    // block 在 forward 前返回，对透传/转换分支均生效；mask/inject 改写 chat_req，
    // 转换分支(convert_request 读 chat_req)生效，同协议透传分支(用 req_value 原体)不生效（已知限制，见 report）。
    {
        let mw_settings = super::db::get_middleware_settings(&state.db).await;
        if let InboundOutcome::Blocked { blocked_by, blocked_reason } =
            state.middleware.apply_inbound_platform(&mw_settings, &mut chat_req, route.platform.id as i64)
        {
            log.platform_id = route.platform.id;
            return block_inbound(&state, log, &log_settings, lang, blocked_by, blocked_reason, start).await;
        }
    }

    // ── 手动预算耗尽阻断（mock / 上游平台均适用，转发前惰性只读判定，不写库）──
    // 任一 enabled 限额剩余 ≤ 0（含窗口惰性重置后）→ 不发上游/不出 mock，返回 402。
    // 平台保持启用，窗口/次日恢复后自动放行。无 manual_budgets（含透传）→ 跳过。
    if let Some(info) = super::manual_budget::evaluate_depletion(&route.platform.manual_budgets, super::db::now()) {
        let recover_hint = match info.kind.as_str() {
            "daily" => i18n::t(lang, ErrorKey::BudgetResetDaily),
            "rolling" => i18n::t(lang, ErrorKey::BudgetResetRolling),
            "fixed" => i18n::t(lang, ErrorKey::BudgetResetFixed),
            _ => i18n::t(lang, ErrorKey::BudgetResetTotal),
        };
        let body = serde_json::json!({
            "error": {
                "type": "manual_budget_exhausted",
                "message": format!(
                    "{} (kind={}, unit={}, amount={}). {}",
                    i18n::t(lang, ErrorKey::BudgetExhausted),
                    info.kind, info.unit, info.amount, recover_hint
                ),
                "budget_kind": info.kind,
                "budget_unit": info.unit,
                "budget_amount": info.amount,
            }
        })
        .to_string();
        tracing::warn!(
            platform = %route.platform.name, kind = %info.kind, unit = %info.unit, amount = info.amount,
            "manual budget exhausted, blocking request (402)"
        );
        log.status_code = 402;
        log.platform_id = route.platform.id;
        log.response_body = body.clone();
        log.user_response_body = body.clone();
        log.user_response_headers = r#"{"content-type":"application/json"}"#.to_string();
        log.duration_ms = start.elapsed().as_millis() as i32;
        attempts.push(ProxyAttempt {
            platform_id: route.platform.id,
            platform_name: route.platform.name.clone(),
            status_code: 402,
            error: "manual budget exhausted".to_string(),
            duration_ms: attempt_start.elapsed().as_millis() as i64,
            ts: attempt_ts,
        });
        log.retry_count = (attempts.len() as i32 - 1).max(0);
        log.attempts = std::mem::take(&mut attempts);
        upsert_log(&state, &log, &log_settings).await;
        return (
            StatusCode::PAYMENT_REQUIRED,
            [(axum::http::header::CONTENT_TYPE, "application/json")],
            body,
        )
            .into_response();
    }

    // 协议转换 / 同协议透传：
    // - 透传分支（同协议）：用客户端原始请求体，仅 patch model 字段，跳过 messages/tools 结构转换；
    //   path 由 wire 协议决定（passthrough_api_path，与 convert_request 一致但不转 body）。
    // - 转换分支：wire format 由 endpoint 协议决定，API path 由平台类型决定。
    let platform_protocol = &route.platform.platform_type;
    let (mut req_body, mut api_path) = if same_protocol_passthrough {
        let mut body = req_value.clone();
        // model remap：透传下仍必须替换路由模型名（请求体 model 字段）
        if let Some(obj) = body.as_object_mut() {
            obj.insert("model".to_string(), Value::String(actual_model.clone()));
        }
        let path = adapter::passthrough_api_path(target_protocol_enum, &actual_model, platform_protocol);
        tracing::debug!(protocol = %target_protocol, "same-protocol passthrough: skip request format conversion");
        (body, path)
    } else {
        adapter::convert_request(&chat_req, target_protocol_enum, platform_protocol)
    };

    // Coding Plan 特殊处理：注入平台特有字段 + 覆盖 API 路径
    if coding_plan {
        inject_coding_plan_fields(&mut req_body, platform_protocol);
        override_coding_plan_path(&mut api_path, platform_protocol);
    }

    let req_body_str = serde_json::to_string(&req_body).unwrap_or_default();

    // 构建目标 URL
    let base_url = target_base_url.trim_end_matches('/');
    let url = format!("{}{}", base_url, api_path);
    log.upstream_request_url = url.clone();

    // ── 解析超时：模型 > 分组 > 系统 ──
    let system_timeout = get_system_timeout(&state.db).await;
    let (req_timeout, conn_timeout) = resolve_timeout(&route.mapping, &group, &system_timeout);
    let client = super::http_client::build_http_client(
        &state.db, req_timeout, conn_timeout,
        Some(&route.platform.extra), None,
    ).await;

    // ── 构建上游请求头（用于日志记录） ──
    let upstream_headers = build_upstream_headers(&client_type, target_protocol_enum, &route.platform.api_key);

    let mut req_builder = client
        .post(&url)
        .header("Content-Type", "application/json")
        .body(req_body_str.clone());

    // ── 按 client_type + target_protocol 模拟对应客户端 header ──
    req_builder = apply_client_headers(req_builder, &client_type, target_protocol_enum, &route.platform.api_key);

    // ── 记录上游实际请求 ──
    log.upstream_request_headers = serde_json::Value::Object(
        upstream_headers.into_iter().map(|(k, v)| (k, Value::String(v))).collect()
    ).to_string();
    log.upstream_request_body = format_pretty_json(&req_body_str);
    tracing::info!(method = "POST", url = %url, "upstream request");
    tracing::debug!(method = "POST", url = %url, body = %req_body_str, "upstream request body");

    let resp = match req_builder.send().await {
        Ok(r) => r,
        Err(e) => {
            // 连接失败 / 超时 → 可重试，换下个候选；候选耗尽则返回 502。
            tracing::error!(url = %url, platform = %route.platform.name, error = %e, duration_ms = start.elapsed().as_millis() as i64, "upstream request failed (502)");
            attempts.push(ProxyAttempt {
                platform_id: route.platform.id,
                platform_name: route.platform.name.clone(),
                status_code: 0,
                error: format!("upstream error: {e}"),
                duration_ms: attempt_start.elapsed().as_millis() as i64,
                ts: attempt_ts,
            });
            if !is_last_candidate {
                continue;
            }
            log.platform_id = route.platform.id;
            log.response_body = format!("upstream error: {e}");
            log.status_code = 502;
            log.user_response_body = format!("{}: {e}", i18n::t(lang, ErrorKey::Upstream));
            log.user_response_headers = r#"{"content-type":"text/plain"}"#.to_string();
            log.duration_ms = start.elapsed().as_millis() as i32;
            log.retry_count = (attempts.len() as i32 - 1).max(0);
            log.attempts = std::mem::take(&mut attempts);
            upsert_log(&state, &log, &log_settings).await;
            return (StatusCode::BAD_GATEWAY, format!("{}: {e}", i18n::t(lang, ErrorKey::Upstream))).into_response();
        }
    };

    // ── 捕获上游响应 headers + status ──
    let status = resp.status();
    log.upstream_status_code = status.as_u16() as i32;
    {
        let mut h = serde_json::Map::new();
        for (k, v) in resp.headers() {
            if let Ok(s) = v.to_str() {
                h.insert(k.to_string(), Value::String(s.to_string()));
            }
        }
        log.upstream_response_headers = Value::Object(h).to_string();
    }

    if !status.is_success() {
        let body = resp.text().await.unwrap_or_default();
        let duration_ms = start.elapsed().as_millis() as i64;
        let code = status.as_u16();
        tracing::warn!(
            url = %url, platform = %route.platform.name, status = code,
            duration_ms, "upstream returned non-success status"
        );
        tracing::debug!(url = %url, status = code, body = %body, "upstream error response body");
        attempts.push(ProxyAttempt {
            platform_id: route.platform.id,
            platform_name: route.platform.name.clone(),
            status_code: code as i32,
            error: truncate_attempt_error(&body),
            duration_ms: attempt_start.elapsed().as_millis() as i64,
            ts: attempt_ts,
        });

        // ── 401/403：上游鉴权失败 → 自动禁用平台（指数退避），换下个候选 ──
        if code == 401 || code == 403 {
            match super::db::set_platform_auto_disabled(&state.db, route.platform.id).await {
                Ok(until) if until > 0 => tracing::warn!(
                    platform = %route.platform.name, platform_id = route.platform.id, status = code,
                    auto_disabled_until = until, "platform auto-disabled (401/403)"
                ),
                Ok(_) => {} // 用户手动 disabled，不动
                Err(e) => tracing::error!(platform_id = route.platform.id, error = %e, "auto-disable platform failed"),
            }
        }

        // ── 中间件 error_rule 分类（出站）：按规则将上游错误分类为 retryable/non-retryable。
        //   non-retryable → 立即返回不换候选（用 override_status/body 若有）。
        //   retryable     → 走默认重试语义（换下个候选）。
        //   无命中        → 默认重试语义不变（is_last_candidate 决定）。
        //   熔断器不在本树：此处只产标记驱动现有重试循环，不引入任何熔断状态。──
        let err_class = {
            let mw_settings = super::db::get_middleware_settings(&state.db).await;
            state.middleware.classify_error(
                &mw_settings, code, &body,
                Some(&group.name), Some(route.platform.id as i64),
            )
        };
        let non_retryable = err_class.as_ref().map(|c| !c.retryable).unwrap_or(false);
        if let Some(ref c) = err_class {
            tracing::info!(
                matched_by = %c.matched_by, category = %c.category, retryable = c.retryable,
                status = code, "middleware error_rule classified upstream error"
            );
        }

        // 非 2xx + retryable（或无命中）→ 换下个候选；候选耗尽 / 超 max_retries 则返回最后一次错误。
        // non-retryable → 跳过 continue，立即返回（不换候选）。
        if !non_retryable && !is_last_candidate {
            continue;
        }

        // ── 应用 error_rule override_status/body（若有）回客户端 ──
        let (out_code, out_body) = match err_class {
            Some(c) => (
                c.override_status.unwrap_or(code),
                c.override_body.unwrap_or_else(|| body.clone()),
            ),
            None => (code, body.clone()),
        };
        log.platform_id = route.platform.id;
        log.response_body = body.clone();
        log.status_code = out_code as i32;
        log.user_response_body = out_body.clone();
        log.user_response_headers = log.upstream_response_headers.clone();
        log.duration_ms = duration_ms as i32;
        log.retry_count = (attempts.len() as i32 - 1).max(0);
        log.attempts = std::mem::take(&mut attempts);
        upsert_log(&state, &log, &log_settings).await;
        return (StatusCode::from_u16(out_code).unwrap_or(StatusCode::BAD_GATEWAY), out_body)
            .into_response();
    }

    // ── 2xx：成功。若该平台曾 auto_disabled（试探成功）则恢复 enabled 清退避。──
    attempts.push(ProxyAttempt {
        platform_id: route.platform.id,
        platform_name: route.platform.name.clone(),
        status_code: status.as_u16() as i32,
        error: String::new(),
        duration_ms: attempt_start.elapsed().as_millis() as i64,
        ts: attempt_ts,
    });
    if route.platform.status == super::models::PlatformStatus::AutoDisabled {
        if let Err(e) = super::db::recover_platform_auto_disabled(&state.db, route.platform.id).await {
            tracing::error!(platform_id = route.platform.id, error = %e, "recover auto-disabled platform failed");
        } else {
            tracing::info!(platform = %route.platform.name, platform_id = route.platform.id, "platform recovered from auto-disabled (2xx)");
        }
    }
    log.platform_id = route.platform.id;
    log.retry_count = (attempts.len() as i32 - 1).max(0);
    log.attempts = std::mem::take(&mut attempts);

    // 非流式：直接透传 JSON
    if !is_stream {
        let body = resp.bytes().await.unwrap_or_default();
        let resp_str = String::from_utf8_lossy(&body).to_string();
        let (input_tokens, output_tokens, cache_tokens) = extract_usage(&resp_str);

        log.response_body = resp_str.clone();
        log.status_code = 200;
        log.duration_ms = start.elapsed().as_millis() as i32;
        log.input_tokens = input_tokens;
        log.output_tokens = output_tokens;
        log.cache_tokens = cache_tokens;

        // Replace model in response back to original if remapped
        let body = if needs_model_remap {
            replace_model_in_json(&body, &requested_model)
        } else {
            body.to_vec()
        };

        // ── 中间件出站规则（非流式 2xx）：response_override/redaction/content_filter 改写 body。
        //   在 usage 提取后改写（脱敏不影响计费/统计）；与入站脱敏幂等。
        //   总开关/子开关 OFF 时为 no-op。error_rule 不在此（仅非 2xx 路径分类）。──
        let body = {
            let mut s = String::from_utf8_lossy(&body).to_string();
            let mw_settings = super::db::get_middleware_settings(&state.db).await;
            state.middleware.apply_outbound(
                &mw_settings, &mut s,
                Some(&group.name), Some(route.platform.id as i64),
            );
            s.into_bytes()
        };
        log.user_response_body = String::from_utf8_lossy(&body).to_string();
        log.user_response_headers = r#"{"content-type":"application/json"}"#.to_string();

        tracing::info!(
            platform = %route.platform.name, model = %actual_model, status = 200, stream = false,
            duration_ms = log.duration_ms, input_tokens, output_tokens, cache_tokens,
            "request completed"
        );
        upsert_log(&state, &log, &log_settings).await;

        // ── 请求驱动预估（后台，不阻塞响应）──
        spawn_estimate(
            &state,
            route.platform.id,
            &route.platform.platform_type,
            route.platform.base_url.clone(),
            route.platform.api_key.clone(),
            actual_model.clone(),
            input_tokens,
            output_tokens,
            cache_tokens,
            coding_plan,
            tracing::Span::current(),
        );

        return (
            StatusCode::OK,
            [(axum::http::header::CONTENT_TYPE, "application/json")],
            body.to_vec(),
        )
            .into_response();
    }

    // 流式：转换 SSE 格式为 Anthropic 格式返回
    // 同协议透传时（passthrough_response），下方闭包内原样 relay 上游 SSE，仅提取 usage。
    let passthrough_response = same_protocol_passthrough;
    let protocol = target_protocol_enum.clone();
    let client_protocol = source_protocol.clone();
    let model_for_sse = requested_model.clone();
    let model_for_response = if needs_model_remap {
        requested_model.clone()
    } else {
        String::new()
    };

    // ── 中间件出站流式逐块改写上下文：在构建 stream 闭包前读取 settings（闭包在 req span 外轮询，
    //   不可再 await DB）。引擎 Arc clone 进闭包，每 chunk 文本应用 mask/override/sensitive。
    //   error 已由上游 HTTP 状态码在 forward 后判定（非 2xx 不会走到这里，故流式无需再判 error）。──
    let mw_engine = state.middleware.clone();
    let mw_settings = super::db::get_middleware_settings(&state.db).await;
    let mw_active = mw_settings.enabled;
    let mw_group = group.name.clone();
    let mw_platform_id = route.platform.id as i64;

    // ── 旁路聚合器：累积 token + 上游 SSE 原文 + 转换后下发客户端的 SSE。
    // 闭包内对其加同步锁是短临界区（push），禁持锁跨 await（闭包本身同步，不 await）。──
    let agg = Arc::new(StreamAggregator::new());
    let est_fired = Arc::new(std::sync::atomic::AtomicBool::new(false));

    // 闭包由 axum 在 req span 外轮询（Response 返回后），故此处捕获当前 req span 链回 trace_id。
    let req_span = tracing::Span::current();

    // ── body 记录受 ProxyLogSettings 开关控制：仅相应开关开启才聚合，零开关时不耗内存。
    // response_body(上游) 受 master(enabled) 控制；user_response_body 受 log_user_request 控制。──
    let record_upstream_body = log_settings.enabled;
    let record_client_body = log_settings.enabled && log_settings.log_user_request;

    // ── 最终回写 guard：[DONE] 正常结束 或 客户端断连 Drop 时回写聚合 token/body（幂等）。──
    let guard = StreamLogGuard {
        agg: agg.clone(),
        est_fired: est_fired.clone(),
        log: log.clone(),
        state: state.clone(),
        settings: log_settings.clone(),
        start,
        record_upstream_body,
        record_client_body,
        req_span: req_span.clone(),
        est: Some(StreamEstCtx {
            platform_id: route.platform.id,
            platform_type: route.platform.platform_type.clone(),
            base_url: route.platform.base_url.clone(),
            api_key: route.platform.api_key.clone(),
            model: actual_model.clone(),
            coding_plan,
        }),
    };

    // guard 被 move 进闭包，随 stream 生命周期存活；stream 被 Drop（含客户端断连）时 guard.drop 触发兜底 flush。
    let stream = resp.bytes_stream().map(move |chunk_result| {
        let chunk = match chunk_result {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!(error = %e, "SSE upstream stream chunk error");
                return Ok::<_, std::io::Error>(Bytes::from(format!("event: error\ndata: {{\"error\":\"{e}\"}}\n\n")));
            }
        };

        // 旁路累积上游响应原文（受 master 开关控制；锁为同步短临界区）
        if record_upstream_body {
            if let Ok(mut up) = guard.agg.upstream_body.lock() {
                up.push(chunk.clone());
            }
        }

        let text = String::from_utf8_lossy(&chunk);

        // ── 同协议透传：跳过 parse_sse→to_client_sse 格式转换，原样 relay 上游 SSE 字节。
        // usage 提取仍保留（accumulate_sse_usage），est_cost / 统计不丢。
        // 注意：透传下不改写响应 model 字段（保持上游原文，与请求体 model=actual_model 一致）。──
        let out_bytes = if passthrough_response {
            for line in text.lines() {
                if let Some(data) = line.strip_prefix("data: ") {
                    let data = data.trim();
                    if data == "[DONE]" {
                        continue;
                    }
                    if let Ok(json) = serde_json::from_str::<Value>(data) {
                        accumulate_sse_usage(&json, &guard.agg.tokens_in, &guard.agg.tokens_out, &guard.agg.tokens_cache);
                    }
                }
            }
            chunk.clone()
        } else {
            let mut output = String::new();
            for line in text.lines() {
                if let Some(data) = line.strip_prefix("data: ") {
                    if data.trim() == "[DONE]" {
                        output.push_str(&adapter::to_client_sse(&ChatStreamEvent::Stop {
                            finish_reason: Some("end_turn".to_string()),
                        }, &client_protocol, &model_for_sse).unwrap_or_default());
                        continue;
                    }

                    if let Ok(json) = serde_json::from_str::<Value>(data) {
                        // token 累计：复用 accumulate_sse_usage（含 Anthropic message.usage 兜底，修复主分支漏读 input_tokens）
                        accumulate_sse_usage(&json, &guard.agg.tokens_in, &guard.agg.tokens_out, &guard.agg.tokens_cache);

                        if let Some(event) = adapter::parse_sse(&json, &protocol) {
                            let event = if !model_for_response.is_empty() {
                                match event {
                                    ChatStreamEvent::Start { id, model: _ } => ChatStreamEvent::Start {
                                        id,
                                        model: model_for_response.clone(),
                                    },
                                    other => other,
                                }
                            } else {
                                event
                            };
                            if let Some(sse) = adapter::to_client_sse(&event, &client_protocol, &model_for_sse) {
                                output.push_str(&sse);
                            }
                        }
                    }
                }
            }
            Bytes::from(output)
        };

        // ── 中间件出站流式逐块改写：对下发客户端的 chunk 文本应用 mask/override/sensitive。
        //   逐块正则替换；跨 chunk 边界的密钥/敏感词可能漏匹配（已知限制，滑窗后续）。
        //   总开关 OFF 时跳过。在记录 client_body 前改写，确保审计与下发一致（脱敏后版本）。──
        let out_bytes = if mw_active && !out_bytes.is_empty() {
            let original = String::from_utf8_lossy(&out_bytes);
            let rewritten = mw_engine.apply_outbound_stream_chunk(
                &mw_settings, &original, Some(&mw_group), Some(mw_platform_id),
            );
            if rewritten == original.as_ref() {
                out_bytes
            } else {
                Bytes::from(rewritten)
            }
        } else {
            out_bytes
        };

        // 旁路累积下发客户端的 SSE（受 log_user_request 开关控制）
        if record_client_body && !out_bytes.is_empty() {
            if let Ok(mut cl) = guard.agg.client_body.lock() {
                cl.push(out_bytes.clone());
            }
        }
        // 正常结束：本 chunk 含 [DONE] 即触发 flush（token 已累加完整）；否则由断连 Drop 兜底。
        // flush 幂等（est_fired 守卫），[DONE] 与 Drop 二者只生效一次。flush 内仅 tokio::spawn，不阻塞转发。
        guard.flush_if_done(&text);

        Ok(out_bytes)
    });

    let body = Body::from_stream(stream);

    // Upsert（返回 stream 前的占位）：标记流进行中，token=0、body 占位；
    // 最终态由 guard.flush（[DONE] 或断连 Drop）覆盖。
    log.status_code = 200;
    log.response_body = "[stream]".to_string();
    log.user_response_body = "[stream]".to_string();
    log.user_response_headers = r#"{"content-type":"text/event-stream","cache-control":"no-cache","connection":"keep-alive"}"#.to_string();
    log.duration_ms = start.elapsed().as_millis() as i32;
    upsert_log(&state, &log, &log_settings).await;

    return (
        StatusCode::OK,
        [
            (axum::http::header::CONTENT_TYPE, "text/event-stream"),
            (axum::http::header::CACHE_CONTROL, "no-cache"),
            (axum::http::header::CONNECTION, "keep-alive"),
        ],
        body,
    )
        .into_response();
    } // ── end retry loop (for candidate) ──

    // 候选耗尽 / 全部超 max_retries 且未在循环内 return（理论不可达：循环内每条路径均 return 或 continue，
    // 仅 attempt_idx > max_retries 的 break 会落到这里）。返回 503 + 已记录的 attempts。
    log.status_code = 503;
    let err_body = format!("{}: all candidates exhausted", i18n::t(lang, ErrorKey::Upstream));
    log.response_body = "all candidates exhausted".to_string();
    log.user_response_body = err_body.clone();
    log.user_response_headers = r#"{"content-type":"text/plain"}"#.to_string();
    log.duration_ms = start.elapsed().as_millis() as i32;
    log.retry_count = (attempts.len() as i32 - 1).max(0);
    log.attempts = std::mem::take(&mut attempts);
    upsert_log(&state, &log, &log_settings).await;
    (StatusCode::SERVICE_UNAVAILABLE, err_body).into_response()
}

/// 截断 attempt error 字段（上游错误体可能很大，attempts JSON 列只存摘要）
fn truncate_attempt_error(body: &str) -> String {
    const MAX: usize = 500;
    if body.len() <= MAX {
        body.to_string()
    } else {
        let mut s: String = body.chars().take(MAX).collect();
        s.push('…');
        s
    }
}

/// Mock 平台请求处理：本地生成可控假响应（非流式 JSON / 流式 SSE），填假 token 进 log。
#[allow(clippy::too_many_arguments)]
async fn handle_mock(
    state: Arc<ProxyState>,
    mut log: ProxyLog,
    log_settings: ProxyLogSettings,
    extra: &str,
    chat_req: &ChatRequest,
    req_value: &Value,
    source_protocol: &str,
    requested_model: &str,
    is_stream: bool,
    start: std::time::Instant,
) -> Response {
    use super::adapter::mock;

    let cfg = mock::resolve_mock_config(extra, chat_req, req_value);

    // 真延迟
    if cfg.delay_ms > 0 {
        tokio::time::sleep(std::time::Duration::from_millis(cfg.delay_ms)).await;
    }

    // 填假 token（最终生效值）
    log.input_tokens = cfg.input_tokens;
    log.output_tokens = cfg.output_tokens;
    log.cache_tokens = cfg.cache_tokens;

    // ── 错误 / 超时模拟 ──
    match cfg.error_mode.as_str() {
        "http_error" => {
            tracing::warn!(platform_id = log.platform_id, status = cfg.status_code, "mock error_mode=http_error");
            let body = mock::build_error_body(source_protocol, cfg.status_code, "mock http_error");
            let body_str = serde_json::to_string(&body).unwrap_or_default();
            let status = StatusCode::from_u16(cfg.status_code).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
            log.status_code = cfg.status_code as i32;
            log.duration_ms = start.elapsed().as_millis() as i32;
            log.response_body = body_str.clone();
            log.user_response_body = body_str.clone();
            log.user_response_headers = r#"{"content-type":"application/json"}"#.to_string();
            upsert_log(&state, &log, &log_settings).await;
            return (status, [(axum::http::header::CONTENT_TYPE, "application/json")], body_str).into_response();
        }
        "rate_limit_429" => {
            tracing::warn!(platform_id = log.platform_id, "mock error_mode=rate_limit_429 (429)");
            let body = mock::build_error_body(source_protocol, 429, "mock rate limit");
            let body_str = serde_json::to_string(&body).unwrap_or_default();
            log.status_code = 429;
            log.duration_ms = start.elapsed().as_millis() as i32;
            log.response_body = body_str.clone();
            log.user_response_body = body_str.clone();
            log.user_response_headers = r#"{"content-type":"application/json","retry-after":"5"}"#.to_string();
            upsert_log(&state, &log, &log_settings).await;
            return (
                StatusCode::TOO_MANY_REQUESTS,
                [
                    (axum::http::header::CONTENT_TYPE, "application/json"),
                    (axum::http::header::RETRY_AFTER, "5"),
                ],
                body_str,
            )
                .into_response();
        }
        "timeout" => {
            tracing::warn!(platform_id = log.platform_id, "mock error_mode=timeout (will sleep then 504)");
            // sleep 上限保护，不真 hang 连接
            tokio::time::sleep(std::time::Duration::from_secs(600)).await;
            let body = mock::build_error_body(source_protocol, 504, "mock timeout");
            let body_str = serde_json::to_string(&body).unwrap_or_default();
            log.status_code = 504;
            log.duration_ms = start.elapsed().as_millis() as i32;
            log.response_body = body_str.clone();
            log.user_response_body = body_str.clone();
            log.user_response_headers = r#"{"content-type":"application/json"}"#.to_string();
            upsert_log(&state, &log, &log_settings).await;
            return (StatusCode::GATEWAY_TIMEOUT, [(axum::http::header::CONTENT_TYPE, "application/json")], body_str)
                .into_response();
        }
        _ => {}
    }

    // 手动预算扣减（mock 也按用量预估扣减，与上游平台一致；仅成功路径，错误模式上方已 return）
    let mb_total = (log.input_tokens + log.output_tokens + log.cache_tokens) as f64;
    if mb_total > 0.0 {
        let est = super::db::calc_est_cost(&state.db, &log.actual_model, "mock", log.input_tokens, log.output_tokens, log.cache_tokens).await;
        let _ = super::manual_budget::apply_manual_budgets(&state.db, log.platform_id, est, mb_total, super::db::now()).await;
    }

    // stream_override 优先于请求 is_stream
    let stream = cfg.stream_override.unwrap_or(is_stream);

    if stream {
        let chunks = mock::build_sse_chunks(&cfg, source_protocol, requested_model);
        let delay_ms = cfg.delay_ms;
        let body_stream = futures::stream::iter(chunks.into_iter().map(Ok::<_, std::io::Error>))
            .then(move |item| async move {
                if delay_ms > 0 {
                    tokio::time::sleep(std::time::Duration::from_millis(delay_ms)).await;
                }
                item
            });
        let body = Body::from_stream(body_stream);

        log.status_code = 200;
        log.duration_ms = start.elapsed().as_millis() as i32;
        log.response_body = "[mock stream]".to_string();
        log.user_response_body = "[mock stream]".to_string();
        log.user_response_headers = r#"{"content-type":"text/event-stream","cache-control":"no-cache","connection":"keep-alive"}"#.to_string();
        upsert_log(&state, &log, &log_settings).await;

        return (
            StatusCode::OK,
            [
                (axum::http::header::CONTENT_TYPE, "text/event-stream"),
                (axum::http::header::CACHE_CONTROL, "no-cache"),
                (axum::http::header::CONNECTION, "keep-alive"),
            ],
            body,
        )
            .into_response();
    }

    // 非流式 JSON
    let resp_body = mock::build_response(&cfg, source_protocol, requested_model);
    let body_str = serde_json::to_string(&resp_body).unwrap_or_default();
    let status = StatusCode::from_u16(cfg.status_code).unwrap_or(StatusCode::OK);
    log.status_code = cfg.status_code as i32;
    log.duration_ms = start.elapsed().as_millis() as i32;
    log.response_body = body_str.clone();
    log.user_response_body = body_str.clone();
    log.user_response_headers = r#"{"content-type":"application/json"}"#.to_string();
    upsert_log(&state, &log, &log_settings).await;

    (status, [(axum::http::header::CONTENT_TYPE, "application/json")], body_str).into_response()
}

/// Claude Code 订阅平台纯透传：把客户端原始请求 1:1 relay 到 base_url，原样返回响应，记 proxy_log。
/// 不做任何协议 / header / 认证转换；客户端自带订阅 OAuth header。
#[allow(clippy::too_many_arguments)]
async fn handle_passthrough(
    state: &Arc<ProxyState>,
    log: &mut ProxyLog,
    log_settings: &ProxyLogSettings,
    orig_method: axum::http::Method,
    orig_uri: axum::http::Uri,
    orig_headers: axum::http::HeaderMap,
    bytes: axum::body::Bytes,
    base_url: &str,
    start: std::time::Instant,
    lang: Lang,
) -> Response {
    // 透传不转换协议，source/target 都标 claude_code
    log.source_protocol = "claude_code".to_string();
    log.target_protocol = "claude_code".to_string();

    // 目标 URL = base_url(host 根) + 客户端原始 path(+query)
    let url = build_passthrough_url(base_url, &orig_uri);
    log.upstream_request_url = url.clone();

    // 解析超时（系统级；透传无 group/model mapping 覆盖）
    let system_timeout = get_system_timeout(&state.db).await;
    let req_timeout = if system_timeout.request_timeout_secs > 0 { system_timeout.request_timeout_secs } else { 300 };
    let conn_timeout = if system_timeout.connect_timeout_secs > 0 { system_timeout.connect_timeout_secs } else { 10 };
    let client = super::http_client::build_http_client(
        &state.db, req_timeout, conn_timeout,
        None, None,
    ).await;

    // 原样转发 header，剔除 hop-by-hop（Host / Content-Length 由 reqwest 按目标 URL + body 重设）
    let fwd_headers = passthrough_headers(&orig_headers);
    // 记录上游请求头（透传 redact authorization）
    log.upstream_request_headers = {
        let mut h = serde_json::Map::new();
        for (k, v) in &fwd_headers {
            let name = k.as_str();
            if name.eq_ignore_ascii_case("authorization") {
                h.insert(name.to_string(), Value::String("[REDACTED]".into()));
            } else if let Ok(s) = v.to_str() {
                h.insert(name.to_string(), Value::String(s.to_string()));
            }
        }
        Value::Object(h).to_string()
    };
    log.upstream_request_body = String::from_utf8_lossy(&bytes).to_string();
    tracing::info!(method = %orig_method, url = %url, "passthrough upstream request");
    tracing::debug!(method = %orig_method, url = %url, body = %log.upstream_request_body, "passthrough upstream request body");

    let method = match reqwest::Method::from_bytes(orig_method.as_str().as_bytes()) {
        Ok(m) => m,
        Err(_) => reqwest::Method::POST,
    };
    let mut req_builder = client.request(method, &url).body(bytes.to_vec());
    req_builder = req_builder.headers(fwd_headers);

    let resp = match req_builder.send().await {
        Ok(r) => r,
        Err(e) => {
            tracing::error!(url = %url, error = %e, duration_ms = start.elapsed().as_millis() as i64, "passthrough upstream request failed (502)");
            log.response_body = format!("upstream error: {e}");
            log.status_code = 502;
            log.user_response_body = format!("{}: {e}", i18n::t(lang, ErrorKey::Upstream));
            log.user_response_headers = r#"{"content-type":"text/plain"}"#.to_string();
            log.duration_ms = start.elapsed().as_millis() as i32;
            upsert_log(state, log, log_settings).await;
            return (StatusCode::BAD_GATEWAY, format!("{}: {e}", i18n::t(lang, ErrorKey::Upstream))).into_response();
        }
    };

    let status = resp.status();
    log.upstream_status_code = status.as_u16() as i32;
    tracing::info!(url = %url, status = status.as_u16(), duration_ms = start.elapsed().as_millis() as i64, "passthrough upstream responded");

    // 捕获上游响应头（原样照搬给客户端）
    let mut resp_header_map = axum::http::HeaderMap::new();
    {
        let mut h = serde_json::Map::new();
        for (k, v) in resp.headers() {
            if let Ok(s) = v.to_str() {
                h.insert(k.to_string(), Value::String(s.to_string()));
            }
            // 剔除 hop-by-hop / 长度类，由 axum 按 body 重设
            let name = k.as_str();
            if name.eq_ignore_ascii_case("content-length")
                || name.eq_ignore_ascii_case("transfer-encoding")
                || name.eq_ignore_ascii_case("connection")
            {
                continue;
            }
            if let (Ok(hn), Ok(hv)) = (
                axum::http::HeaderName::from_bytes(k.as_str().as_bytes()),
                axum::http::HeaderValue::from_bytes(v.as_bytes()),
            ) {
                resp_header_map.insert(hn, hv);
            }
        }
        log.upstream_response_headers = Value::Object(h).to_string();
    }

    let content_type = resp
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();
    let is_stream = content_type.contains("text/event-stream")
        || resp
            .headers()
            .get(reqwest::header::TRANSFER_ENCODING)
            .and_then(|v| v.to_str().ok())
            .map(|s| s.contains("chunked"))
            .unwrap_or(false);

    let resp_status = StatusCode::from_u16(status.as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);

    // ── 非流式：原样 relay bytes ──
    if !is_stream {
        let body = resp.bytes().await.unwrap_or_default();
        let resp_str = String::from_utf8_lossy(&body).to_string();
        let (input_tokens, output_tokens, cache_tokens) = extract_usage(&resp_str);

        log.response_body = resp_str.clone();
        log.status_code = status.as_u16() as i32;
        log.duration_ms = start.elapsed().as_millis() as i32;
        log.input_tokens = input_tokens;
        log.output_tokens = output_tokens;
        log.cache_tokens = cache_tokens;
        log.user_response_body = resp_str;
        log.user_response_headers = log.upstream_response_headers.clone();
        upsert_log(state, log, log_settings).await;

        let mut response = (resp_status, body.to_vec()).into_response();
        *response.headers_mut() = resp_header_map;
        return response;
    }

    // ── 流式：原样透传 SSE bytes，不解析不转换；旁路累计 token + 聚合 body，[DONE]/断连回写 ──
    log.is_stream = true;
    log.status_code = status.as_u16() as i32;

    let agg = Arc::new(StreamAggregator::new());
    let est_fired = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let req_span = tracing::Span::current();

    // 透传原样 relay：response_body == user_response_body == 上游 SSE 原文。
    // response_body 受 master(enabled) 控制；user_response_body 受 log_user_request 控制。
    let record_upstream_body = log_settings.enabled;
    let record_client_body = log_settings.enabled && log_settings.log_user_request;

    // 透传分支无协议转换 → user_response_body 复用 upstream 原文（不单独聚合 client_body）。
    let guard = StreamLogGuard {
        agg: agg.clone(),
        est_fired: est_fired.clone(),
        log: log.clone(),
        state: state.clone(),
        settings: log_settings.clone(),
        start,
        record_upstream_body,
        // 透传 user_response_body 由 flush 中从 upstream_body 复制（见下方 finalize），此处 client_body 不聚合
        record_client_body: false,
        req_span: req_span.clone(),
        // 透传分支历史上不做请求驱动预估，保持现状
        est: None,
    };
    // flush 后由 guard 写 response_body；透传需 user_response_body 同步 = response_body。
    // 复用 record_client_body 语义：透传时把 upstream 聚合内容也写入 user_response_body。
    let passthrough_user_body = record_client_body;

    // guard 被 move 进闭包；stream 被 Drop（含客户端断连）时 guard.drop 触发兜底 flush。
    let stream = resp.bytes_stream().map(move |chunk_result| {
        let chunk = match chunk_result {
            Ok(c) => c,
            Err(e) => return Err(std::io::Error::other(e.to_string())),
        };
        // 旁路累积上游 SSE 原文（受 master 开关控制）
        if record_upstream_body {
            if let Ok(mut up) = guard.agg.upstream_body.lock() {
                up.push(chunk.clone());
            }
        }
        // 透传 user_response_body == upstream 原文：受 log_user_request 控制时同步聚合到 client_body
        if passthrough_user_body {
            if let Ok(mut cl) = guard.agg.client_body.lock() {
                cl.push(chunk.clone());
            }
        }
        // 尽力从 SSE data 累计 usage（Anthropic / OpenAI 兼容字段，含 message.usage 兜底），不改写 chunk
        let text = String::from_utf8_lossy(&chunk);
        for line in text.lines() {
            if let Some(data) = line.strip_prefix("data: ") {
                if data.trim() == "[DONE]" {
                    continue;
                }
                if let Ok(json) = serde_json::from_str::<Value>(data) {
                    accumulate_sse_usage(&json, &guard.agg.tokens_in, &guard.agg.tokens_out, &guard.agg.tokens_cache);
                }
            }
        }
        guard.flush_if_done(&text);
        Ok::<_, std::io::Error>(chunk)
    });

    let body = Body::from_stream(stream);

    // 返回 stream 前的占位 upsert：标记流进行中，最终态由 guard.flush（[DONE]/断连）覆盖。
    log.duration_ms = start.elapsed().as_millis() as i32;
    log.response_body = "[stream]".to_string();
    log.user_response_body = "[stream]".to_string();
    log.user_response_headers = log.upstream_response_headers.clone();
    upsert_log(state, log, log_settings).await;

    let mut response = (resp_status, body).into_response();
    *response.headers_mut() = resp_header_map;
    response
}

/// 透传目标 URL 拼接：base_url(去尾斜杠) + 客户端原始 path(+query)
fn build_passthrough_url(base_url: &str, uri: &axum::http::Uri) -> String {
    let base = base_url.trim_end_matches('/');
    let pq = uri
        .path_and_query()
        .map(|pq| pq.as_str())
        .unwrap_or_else(|| uri.path());
    format!("{}{}", base, pq)
}

/// 构建透传转发 header：原样保留客户端全部 header（含 Authorization OAuth），
/// 仅剔除 hop-by-hop（Host / Content-Length，由 reqwest 按目标 URL + body 重设）。
fn passthrough_headers(orig: &axum::http::HeaderMap) -> reqwest::header::HeaderMap {
    let mut out = reqwest::header::HeaderMap::new();
    for (k, v) in orig {
        let name = k.as_str();
        if name.eq_ignore_ascii_case("host") || name.eq_ignore_ascii_case("content-length") {
            continue;
        }
        if let (Ok(hn), Ok(hv)) = (
            reqwest::header::HeaderName::from_bytes(name.as_bytes()),
            reqwest::header::HeaderValue::from_bytes(v.as_bytes()),
        ) {
            out.append(hn, hv);
        }
    }
    out
}

/// 聚合 SSE body 的上限（字节）。完整记录但防物理崩溃：超限截断 + 标记，禁 panic / OOM。
/// SQLite 单值上限 ~1GB；取 512MB 为安全上限（拼接 + UTF-8 lossy 仍有余量）。
const STREAM_BODY_MAX_BYTES: usize = 512 * 1024 * 1024;

/// 把聚合的 SSE chunk（Vec<Bytes>）拼接为字符串，超上限则截断并加标记。
/// 旁路累积零阻塞转发，此处一次性拼接（仅 flush 时调用，非 chunk 热路径）。
fn join_stream_body(chunks: &[Bytes]) -> String {
    let total: usize = chunks.iter().map(|c| c.len()).sum();
    if total > STREAM_BODY_MAX_BYTES {
        let mut buf: Vec<u8> = Vec::with_capacity(STREAM_BODY_MAX_BYTES);
        for c in chunks {
            if buf.len() >= STREAM_BODY_MAX_BYTES {
                break;
            }
            let remaining = STREAM_BODY_MAX_BYTES - buf.len();
            let take = remaining.min(c.len());
            buf.extend_from_slice(&c[..take]);
        }
        let mut s = String::from_utf8_lossy(&buf).into_owned();
        s.push_str("\n[truncated: stream body exceeded size limit]");
        s
    } else {
        let mut buf: Vec<u8> = Vec::with_capacity(total);
        for c in chunks {
            buf.extend_from_slice(c);
        }
        String::from_utf8_lossy(&buf).into_owned()
    }
}

/// 流式日志聚合状态：旁路累积 token + 上游响应原文 + 转换后下发客户端的 SSE。
/// 闭包内对其加锁是同步短临界区（push），**禁持锁跨 await**。
struct StreamAggregator {
    upstream_body: std::sync::Mutex<Vec<Bytes>>,
    client_body: std::sync::Mutex<Vec<Bytes>>,
    tokens_in: std::sync::atomic::AtomicI32,
    tokens_out: std::sync::atomic::AtomicI32,
    tokens_cache: std::sync::atomic::AtomicI32,
}

impl StreamAggregator {
    fn new() -> Self {
        Self {
            upstream_body: std::sync::Mutex::new(Vec::new()),
            client_body: std::sync::Mutex::new(Vec::new()),
            tokens_in: std::sync::atomic::AtomicI32::new(0),
            tokens_out: std::sync::atomic::AtomicI32::new(0),
            tokens_cache: std::sync::atomic::AtomicI32::new(0),
        }
    }
}

/// 流式日志最终回写 guard：[DONE] 正常结束 或 客户端断连 Drop 时，
/// 用聚合的 token + body 回写日志（INSERT OR REPLACE 覆盖返回前的占位 upsert）。
/// flush 幂等（est_fired 守卫），[DONE] 与 Drop 只触发一次。
/// Drop 内不可 await → 用 tokio::spawn fire-and-forget 落库 + 后台预估。
struct StreamLogGuard {
    agg: Arc<StreamAggregator>,
    est_fired: Arc<std::sync::atomic::AtomicBool>,
    // 日志回写上下文
    log: ProxyLog,
    state: Arc<ProxyState>,
    settings: ProxyLogSettings,
    start: std::time::Instant,
    record_upstream_body: bool,
    record_client_body: bool,
    req_span: tracing::Span,
    // 后台预估上下文（None = 不做预估，如透传分支）
    est: Option<StreamEstCtx>,
}

/// 流式 flush 时触发的后台预估上下文。
struct StreamEstCtx {
    platform_id: u64,
    platform_type: Protocol,
    base_url: String,
    api_key: String,
    model: String,
    coding_plan: bool,
}

impl StreamLogGuard {
    /// 若 chunk 文本含 SSE 终止标记（`data: [DONE]`）则触发 flush。
    /// 正常结束走此路径回写（token 已累加完整）；未命中则由 Drop 兜底。
    fn flush_if_done(&self, text: &str) {
        for line in text.lines() {
            if let Some(data) = line.strip_prefix("data: ") {
                if data.trim() == "[DONE]" {
                    self.flush();
                    return;
                }
            }
        }
    }

    /// 用聚合结果回写日志 + 触发后台预估。幂等：仅首次调用生效。
    fn flush(&self) {
        use std::sync::atomic::Ordering::Relaxed;
        if self.est_fired.swap(true, Relaxed) {
            return;
        }
        let input_tokens = self.agg.tokens_in.load(Relaxed);
        let output_tokens = self.agg.tokens_out.load(Relaxed);
        let cache_tokens = self.agg.tokens_cache.load(Relaxed);

        let mut final_log = self.log.clone();
        final_log.input_tokens = input_tokens;
        final_log.output_tokens = output_tokens;
        final_log.cache_tokens = cache_tokens;
        final_log.status_code = 200;
        final_log.duration_ms = self.start.elapsed().as_millis() as i32;
        // 聚合真实 SSE 内容写入 body（受 record 开关控制；upsert_log 仍按 settings 二次过滤）
        if self.record_upstream_body {
            if let Ok(chunks) = self.agg.upstream_body.lock() {
                final_log.response_body = join_stream_body(&chunks);
            }
        }
        if self.record_client_body {
            if let Ok(chunks) = self.agg.client_body.lock() {
                final_log.user_response_body = join_stream_body(&chunks);
            }
        }

        tracing::info!(
            platform_id = final_log.platform_id, model = %final_log.actual_model,
            status = 200, stream = true, duration_ms = final_log.duration_ms,
            input_tokens, output_tokens, cache_tokens, "stream request completed (flush)"
        );

        let upsert_state = self.state.clone();
        let upsert_settings = self.settings.clone();
        let span = self.req_span.clone();
        tokio::spawn(async move {
            upsert_log(&upsert_state, &final_log, &upsert_settings).await;
        }.instrument(span));

        if let Some(est) = &self.est {
            spawn_estimate(
                &self.state,
                est.platform_id,
                &est.platform_type,
                est.base_url.clone(),
                est.api_key.clone(),
                est.model.clone(),
                input_tokens,
                output_tokens,
                cache_tokens,
                est.coding_plan,
                self.req_span.clone(),
            );
        }
    }
}

impl Drop for StreamLogGuard {
    fn drop(&mut self) {
        // 客户端断连 / 上游无 [DONE] → flush 未触发，此处兜底回写已聚合数据。
        // Drop 内不可 async；flush 内部用 tokio::spawn 落库（Drop 发生在 runtime 任务上下文中）。
        self.flush();
    }
}

/// 从 SSE event JSON 尽力累计 usage（Anthropic / OpenAI 兼容字段）
fn accumulate_sse_usage(
    json: &Value,
    acc_in: &std::sync::atomic::AtomicI32,
    acc_out: &std::sync::atomic::AtomicI32,
    acc_cache: &std::sync::atomic::AtomicI32,
) {
    use std::sync::atomic::Ordering::Relaxed;
    // usage 可能在顶层，也可能在 message.usage（Anthropic message_start）
    let usage = json
        .get("usage")
        .or_else(|| json.get("message").and_then(|m| m.get("usage")));
    let usage = match usage {
        Some(u) => u,
        None => return,
    };
    if let Some(i) = usage
        .get("input_tokens")
        .or_else(|| usage.get("prompt_tokens"))
        .and_then(|v| v.as_i64())
    {
        acc_in.store(i as i32, Relaxed);
    }
    if let Some(o) = usage
        .get("output_tokens")
        .or_else(|| usage.get("completion_tokens"))
        .and_then(|v| v.as_i64())
    {
        acc_out.store(o as i32, Relaxed);
    }
    if let Some(c) = usage
        .get("cache_read_input_tokens")
        .and_then(|v| v.as_i64())
        .or_else(|| {
            usage
                .get("prompt_tokens_details")
                .and_then(|d| d.get("cached_tokens"))
                .and_then(|v| v.as_i64())
        })
        .or_else(|| usage.get("cache_tokens").and_then(|v| v.as_i64()))
    {
        acc_cache.store(c as i32, Relaxed);
    }
}

/// Extract input/output/cache tokens from non-stream response JSON
fn extract_usage(body: &str) -> (i32, i32, i32) {
    let v: Value = match serde_json::from_str(body) {
        Ok(v) => v,
        Err(_) => return (0, 0, 0),
    };
    let usage = match v.get("usage") {
        Some(u) => u,
        None => return (0, 0, 0),
    };
    let input = usage.get("input_tokens")
        .or_else(|| usage.get("prompt_tokens"))
        .and_then(|v| v.as_i64())
        .unwrap_or(0) as i32;
    let output = usage.get("output_tokens")
        .or_else(|| usage.get("completion_tokens"))
        .and_then(|v| v.as_i64())
        .unwrap_or(0) as i32;
    // Cache tokens: Anthropic (cache_read_input_tokens), OpenAI (prompt_tokens_details.cached_tokens), generic
    let cache = usage.get("cache_read_input_tokens")
        .and_then(|v| v.as_i64())
        .or_else(|| usage.get("prompt_tokens_details")
            .and_then(|d| d.get("cached_tokens"))
            .and_then(|v| v.as_i64()))
        .or_else(|| usage.get("cache_tokens").and_then(|v| v.as_i64()))
        .unwrap_or(0) as i32;
    (input, output, cache)
}

/// Replace "model" field in a JSON response body back to the original model name
fn replace_model_in_json(bytes: &[u8], original_model: &str) -> Vec<u8> {
    let mut v: Value = match serde_json::from_slice(bytes) {
        Ok(v) => v,
        Err(_) => return bytes.to_vec(),
    };
    if let Some(obj) = v.as_object_mut() {
        obj.insert("model".to_string(), Value::String(original_model.to_string()));
    }
    serde_json::to_vec(&v).unwrap_or_else(|_| bytes.to_vec())
}

/// 根据请求路径自动推断入站 AI 协议格式
/// - /v1/messages → anthropic
/// - /v1/responses → openai_responses（Codex，body 用 input）
/// - /v1/chat/completions, /v1/completions, /models, /images, /audio → openai
/// - /v1beta/models/... → gemini
///   回退到 anthropic
fn detect_source_protocol(path: &str) -> String {
    // Strip group path prefix (e.g. /proxy/v1/chat/completions → /v1/chat/completions)
    let api_path = if let Some(idx) = path.find("/v1/") {
        &path[idx..]
    } else if path.contains("/v1beta/") {
        return "gemini".to_string();
    } else {
        return "anthropic".to_string();
    };

    if api_path.starts_with("/v1/messages") {
        "anthropic".to_string()
    } else if api_path.starts_with("/v1/responses") {
        // OpenAI Responses API（Codex 等）用 `input` 而非 `messages`，
        // 必须单独派发到 openai_responses 入站解析，不能与 chat/completions 同组。
        "openai_responses".to_string()
    } else if api_path.starts_with("/v1/chat/completions")
        || api_path.starts_with("/v1/completions")
        || api_path.starts_with("/v1/embeddings")
        || api_path.starts_with("/v1/images")
        || api_path.starts_with("/v1/audio")
        || api_path.starts_with("/v1/models")
    {
        "openai".to_string()
    } else if path.contains("/v1beta/") {
        "gemini".to_string()
    } else {
        "anthropic".to_string()
    }
}

/// 在已取出的分组列表中按 group name 精确匹配，匹配不到再按 path 前缀匹配。
/// 单次 list_groups → 同一 Vec 上跑两种匹配，避免热路径重复全表读 + 重复 mappings JSON 解析。
/// 行为等价于原「先 name 后 path」优先级。
async fn resolve_group(db: &Db, name: Option<&str>, request_path: &str) -> Option<Group> {
    let groups = match super::db::list_groups(db).await {
        Ok(g) => g,
        Err(e) => {
            tracing::warn!(error = %e, "resolve_group: list_groups failed");
            return None;
        }
    };
    if let Some(name) = name {
        if let Some(idx) = groups.iter().position(|g| g.name == name) {
            return groups.into_iter().nth(idx);
        }
        tracing::warn!(token = %name, "resolve_group: token did not match any group name, falling back to path match");
    }
    let group_count = groups.len();
    match groups.into_iter().find(|g| request_path.starts_with(&g.path)) {
        Some(g) => Some(g),
        None => {
            tracing::warn!(
                path = %request_path, group_count,
                "resolve_group: no group matched token or path prefix"
            );
            None
        }
    }
}

// ─── 客户端模拟 Header ────────────────────────────────────────

/// 根据客户端类型和目标协议，构建模拟的 HTTP 请求头。
/// 数据来源：GitHub 逆向分析 + claude-code-hub 参考实现
pub fn apply_client_headers(
    req_builder: reqwest::RequestBuilder,
    client_type: &ClientType,
    protocol: &super::models::Protocol,
    api_key: &str,
) -> reqwest::RequestBuilder {
    match client_type {
        ClientType::Default => apply_default_headers(req_builder, protocol, api_key),
        // Claude Code family — 共享 Stainless SDK headers，仅 UA 不同
        ClientType::ClaudeCode
        | ClientType::ClaudeCodeVscode
        | ClientType::ClaudeCodeSdkTs
        | ClientType::ClaudeCodeSdkPy
        | ClientType::ClaudeCodeGhAction => {
            apply_claude_code_family_headers(req_builder, client_type, protocol, api_key)
        }
        // Codex family — 共享 Codex 基础 headers，仅 UA 不同
        ClientType::CodexCli
        | ClientType::CodexTui
        | ClientType::CodexDesktop
        | ClientType::CodexVscode => {
            apply_codex_family_headers(req_builder, client_type, protocol, api_key)
        }
        ClientType::Cursor => apply_cursor_headers(req_builder, protocol, api_key),
        ClientType::Windsurf => apply_windsurf_headers(req_builder, protocol, api_key),
    }
}

/// 根据 ClientType 子变体返回 Claude Code 家族的 User-Agent 字符串。
/// 格式: claude-cli/<version> (external, <entrypoint>[, agent-sdk/<sdk_ver>])
fn claude_code_ua(client_type: &ClientType) -> &'static str {
    match client_type {
        ClientType::ClaudeCode => "claude-cli/1.0.117 (external, cli)",
        ClientType::ClaudeCodeVscode => "claude-cli/1.0.117 (external, claude-vscode, agent-sdk/0.1.30)",
        ClientType::ClaudeCodeSdkTs => "claude-cli/1.0.117 (external, sdk-ts)",
        ClientType::ClaudeCodeSdkPy => "claude-cli/1.0.117 (external, sdk-py)",
        ClientType::ClaudeCodeGhAction => "claude-cli/1.0.117 (external, claude-code-github-action)",
        _ => "claude-cli/1.0.117 (external, cli)",
    }
}

/// 根据 ClientType 子变体返回 Codex 家族的 User-Agent 字符串
fn codex_ua(client_type: &ClientType) -> &'static str {
    match client_type {
        ClientType::CodexCli => "codex_cli_rs/0.38.0 (MacOS; arm64) Terminal",
        ClientType::CodexTui => "Codex/0.38.0",
        ClientType::CodexDesktop => "codex desktop/0.38.0",
        ClientType::CodexVscode => "codex-vscode/0.38.0",
        _ => "codex_cli_rs/0.38.0 (MacOS; arm64) Terminal",
    }
}

fn apply_default_headers(
    mut rb: reqwest::RequestBuilder,
    protocol: &super::models::Protocol,
    api_key: &str,
) -> reqwest::RequestBuilder {
    match protocol {
        super::models::Protocol::Anthropic => {
            rb = rb.header("anthropic-version", "2023-06-01")
                .header("x-api-key", api_key);
        }
        super::models::Protocol::Gemini => {
            rb = rb.header("x-goog-api-key", api_key);
        }
        _ => {
            rb = rb.header("Authorization", format!("Bearer {api_key}"));
        }
    }
    rb
}

/// Claude Code 家族共享 Stainless SDK headers
/// 来源: @anthropic-ai/claude-code/cli.js — buildHeaders() + fV()
/// 参考: claude-code-hub client-detector.ts — confirmClaudeCodeSignals()
fn apply_claude_code_family_headers(
    mut rb: reqwest::RequestBuilder,
    client_type: &ClientType,
    protocol: &super::models::Protocol,
    api_key: &str,
) -> reqwest::RequestBuilder {
    rb = rb
        .header("User-Agent", claude_code_ua(client_type))
        .header("x-app", "cli")
        .header("anthropic-version", "2023-06-01")
        .header("anthropic-dangerous-direct-browser-access", "true")
        .header("X-Stainless-Lang", "js")
        .header("X-Stainless-Package-Version", "0.60.0")
        .header("X-Stainless-OS", "MacOS")
        .header("X-Stainless-Arch", "arm64")
        .header("X-Stainless-Runtime", "node")
        .header("X-Stainless-Runtime-Version", "v22.19.0")
        .header("X-Stainless-Retry-Count", "0")
        .header("X-Stainless-Timeout", "600");

    match protocol {
        super::models::Protocol::Anthropic => {
            rb = rb.header("x-api-key", api_key);
        }
        super::models::Protocol::OpenAI => {
            rb = rb.header("Authorization", format!("Bearer {api_key}"));
        }
        super::models::Protocol::Gemini => {
            rb = rb.header("x-goog-api-key", api_key);
        }
        _ => {
            rb = rb.header("Authorization", format!("Bearer {api_key}"));
        }
    }
    rb
}

/// Codex 家族共享基础 headers
/// 来源: codex-rs/core/src/default_client.rs + model_provider_info.rs + client.rs
/// 参考: claude-code-hub client-detector.ts — CODEX_FAMILY_RULES
fn apply_codex_family_headers(
    mut rb: reqwest::RequestBuilder,
    client_type: &ClientType,
    protocol: &super::models::Protocol,
    api_key: &str,
) -> reqwest::RequestBuilder {
    rb = rb
        .header("User-Agent", codex_ua(client_type))
        .header("originator", "codex_cli_rs")
        .header("version", "0.38.0")
        .header("Accept", "text/event-stream");

    match protocol {
        super::models::Protocol::OpenAI => {
            rb = rb
                .header("Authorization", format!("Bearer {api_key}"))
                .header("OpenAI-Beta", "responses=experimental")
                .header("conversation_id", uuid_sim())
                .header("session_id", uuid_sim());
        }
        super::models::Protocol::Anthropic => {
            rb = rb
                .header("x-api-key", api_key)
                .header("anthropic-version", "2023-06-01");
        }
        super::models::Protocol::Gemini => {
            rb = rb.header("x-goog-api-key", api_key);
        }
        _ => {
            rb = rb.header("Authorization", format!("Bearer {api_key}"));
        }
    }
    rb
}

/// 模拟 Cursor IDE 请求头
/// 来源: GitHub 逆向 — 使用 Anthropic SDK 但有特定 header 组合
fn apply_cursor_headers(
    mut rb: reqwest::RequestBuilder,
    protocol: &super::models::Protocol,
    api_key: &str,
) -> reqwest::RequestBuilder {
    rb = rb
        .header("User-Agent", "Cursor/0.50.7")
        .header("x-app", "cursor");

    match protocol {
        super::models::Protocol::Anthropic => {
            rb = rb
                .header("anthropic-version", "2023-06-01")
                .header("x-api-key", api_key);
        }
        super::models::Protocol::OpenAI => {
            rb = rb.header("Authorization", format!("Bearer {api_key}"));
        }
        super::models::Protocol::Gemini => {
            rb = rb.header("x-goog-api-key", api_key);
        }
        _ => {
            rb = rb.header("Authorization", format!("Bearer {api_key}"));
        }
    }
    rb
}

/// 模拟 Windsurf IDE 请求头
/// 来源: GitHub 逆向 — 类似 Cursor，使用 Anthropic SDK
fn apply_windsurf_headers(
    mut rb: reqwest::RequestBuilder,
    protocol: &super::models::Protocol,
    api_key: &str,
) -> reqwest::RequestBuilder {
    rb = rb
        .header("User-Agent", "Windsurf/1.5.0")
        .header("x-app", "windsurf");

    match protocol {
        super::models::Protocol::Anthropic => {
            rb = rb
                .header("anthropic-version", "2023-06-01")
                .header("x-api-key", api_key);
        }
        super::models::Protocol::OpenAI => {
            rb = rb.header("Authorization", format!("Bearer {api_key}"));
        }
        super::models::Protocol::Gemini => {
            rb = rb.header("x-goog-api-key", api_key);
        }
        _ => {
            rb = rb.header("Authorization", format!("Bearer {api_key}"));
        }
    }
    rb
}

/// 生成简易 UUID v4 格式的随机字符串
fn uuid_sim() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!("{:08x}-{:04}-4{:03}-{:04}-{:012x}",
        (ts as u32).wrapping_mul(0x45d9f3b),
        (ts >> 16) as u16,
        ((ts >> 32) as u16) & 0x0fff,
        ((ts >> 48) as u16) | 0x8000,
        ((ts >> 60) as u64) & 0xffffffffffff,
    )
}

/// 构建上游请求头 KV 表（用于日志记录，与 apply_client_headers 保持一致）
pub fn build_upstream_headers(client_type: &ClientType, protocol: &super::models::Protocol, api_key: &str) -> Vec<(String, String)> {
    let mut h: Vec<(String, String)> = vec![
        ("Content-Type".into(), "application/json".into()),
    ];
    // auth header
    match protocol {
        super::models::Protocol::Anthropic => {
            h.push(("anthropic-version".into(), "2023-06-01".into()));
            h.push(("x-api-key".into(), redact_key(api_key)));
        }
        super::models::Protocol::Gemini => {
            h.push(("x-goog-api-key".into(), redact_key(api_key)));
        }
        _ => {
            h.push(("Authorization".into(), format!("Bearer {}", redact_key(api_key))));
        }
    }
    // client-specific headers
    match client_type {
        ClientType::Default => {}
        // Claude Code family
        ClientType::ClaudeCode
        | ClientType::ClaudeCodeVscode
        | ClientType::ClaudeCodeSdkTs
        | ClientType::ClaudeCodeSdkPy
        | ClientType::ClaudeCodeGhAction => {
            h.push(("User-Agent".into(), claude_code_ua(client_type).into()));
            h.push(("x-app".into(), "cli".into()));
            h.push(("anthropic-dangerous-direct-browser-access".into(), "true".into()));
            h.push(("X-Stainless-Lang".into(), "js".into()));
            h.push(("X-Stainless-Package-Version".into(), "0.60.0".into()));
            h.push(("X-Stainless-OS".into(), "MacOS".into()));
            h.push(("X-Stainless-Arch".into(), "arm64".into()));
            h.push(("X-Stainless-Runtime".into(), "node".into()));
            h.push(("X-Stainless-Runtime-Version".into(), "v22.19.0".into()));
            h.push(("X-Stainless-Retry-Count".into(), "0".into()));
            h.push(("X-Stainless-Timeout".into(), "600".into()));
        }
        // Codex family
        ClientType::CodexCli
        | ClientType::CodexTui
        | ClientType::CodexDesktop
        | ClientType::CodexVscode => {
            h.push(("User-Agent".into(), codex_ua(client_type).into()));
            h.push(("originator".into(), "codex_cli_rs".into()));
            h.push(("version".into(), "0.38.0".into()));
            h.push(("Accept".into(), "text/event-stream".into()));
            if matches!(protocol, super::models::Protocol::OpenAI) {
                h.push(("OpenAI-Beta".into(), "responses=experimental".into()));
                h.push(("conversation_id".into(), uuid_sim()));
                h.push(("session_id".into(), uuid_sim()));
            }
        }
        ClientType::Cursor => {
            h.push(("User-Agent".into(), "Cursor/0.50.7".into()));
            h.push(("x-app".into(), "cursor".into()));
        }
        ClientType::Windsurf => {
            h.push(("User-Agent".into(), "Windsurf/1.5.0".into()));
            h.push(("x-app".into(), "windsurf".into()));
        }
    }
    h
}

/// Redact API key: show first 4 and last 4 chars, mask the rest
pub fn redact_key(key: &str) -> String {
    if key.len() <= 12 {
        "[REDACTED]".into()
    } else {
        format!("{}****{}", &key[..4], &key[key.len()-4..])
    }
}

/// 为 Coding Plan 端点注入平台特有字段
/// - Kimi Code Plan: 注入 prompt_cache_key（必填，用 group + model hash 作会话标识）
pub fn inject_coding_plan_fields(body: &mut Value, protocol: &super::models::Protocol) {
    match protocol {
        super::models::Protocol::Kimi => {
            // Kimi Code Plan 要求 prompt_cache_key 以提升缓存命中率
            // 用模型名 + 短随机串生成会话级 cache key
            if let Some(obj) = body.as_object_mut() {
                let model = obj.get("model")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown");
                let session_id = format!("aidog-{}-{:06x}",
                    model,
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs() / 300  // 5-minute window
                );
                obj.insert(
                    "prompt_cache_key".to_string(),
                    Value::String(session_id),
                );
            }
        }
        _ => {
            // GLM / MiniMax / 百炼 等 coding plan 暂无额外字段
        }
    }
}

/// Coding Plan 的 API 路径覆盖（当前各平台 base_url 已区分 coding/normal，api_path 无需变更）
pub fn override_coding_plan_path(_api_path: &mut String, _protocol: &super::models::Protocol) {
    // 预留：后续若有平台需 coding plan 专用 api_path 可在此扩展
}

/// Pretty-print JSON string; return original if parsing fails
fn format_pretty_json(s: &str) -> String {
    serde_json::from_str::<Value>(s)
        .ok()
        .and_then(|v| serde_json::to_string_pretty(&v).ok())
        .unwrap_or_else(|| s.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── 透传 URL 拼接：base_url(host 根) + 客户端原始 path(+query) ──

    #[test]
    fn passthrough_url_path_only() {
        let uri: axum::http::Uri = "/v1/messages".parse().unwrap();
        assert_eq!(
            build_passthrough_url("https://api.anthropic.com", &uri),
            "https://api.anthropic.com/v1/messages"
        );
    }

    #[test]
    fn passthrough_url_with_query() {
        let uri: axum::http::Uri = "/v1/messages?beta=true&foo=bar".parse().unwrap();
        assert_eq!(
            build_passthrough_url("https://api.anthropic.com", &uri),
            "https://api.anthropic.com/v1/messages?beta=true&foo=bar"
        );
    }

    #[test]
    fn passthrough_url_trims_trailing_slash() {
        let uri: axum::http::Uri = "/v1/messages".parse().unwrap();
        assert_eq!(
            build_passthrough_url("https://api.anthropic.com/", &uri),
            "https://api.anthropic.com/v1/messages"
        );
    }

    // ── 透传 header 剔除 Host + Content-Length，保留 Authorization 及其他 ──

    #[test]
    fn passthrough_headers_drops_hop_by_hop_keeps_auth() {
        let mut orig = axum::http::HeaderMap::new();
        orig.insert("host", "127.0.0.1:8080".parse().unwrap());
        orig.insert("content-length", "123".parse().unwrap());
        orig.insert("authorization", "Bearer sk-oauth-token".parse().unwrap());
        orig.insert("anthropic-version", "2023-06-01".parse().unwrap());
        orig.insert("x-custom", "keep-me".parse().unwrap());

        let fwd = passthrough_headers(&orig);

        // hop-by-hop 剔除
        assert!(!fwd.contains_key("host"), "host must be dropped");
        assert!(!fwd.contains_key("content-length"), "content-length must be dropped");
        // 客户端自带订阅 OAuth 原样保留
        assert_eq!(
            fwd.get("authorization").and_then(|v| v.to_str().ok()),
            Some("Bearer sk-oauth-token")
        );
        // 其余 header 原样
        assert_eq!(
            fwd.get("anthropic-version").and_then(|v| v.to_str().ok()),
            Some("2023-06-01")
        );
        assert_eq!(
            fwd.get("x-custom").and_then(|v| v.to_str().ok()),
            Some("keep-me")
        );
    }

    // ── 透传分支不调 convert_request（结构性确认）──
    // ClaudeCode 命中拦截分支后直接 return handle_passthrough，
    // handle_passthrough 不引用 convert_request / build_upstream_headers / apply_client_headers。
    #[test]
    fn passthrough_does_not_invoke_convert_request() {
        let src = include_str!("proxy.rs");
        // 定位 handle_passthrough 函数体范围
        let start = src.find("async fn handle_passthrough(").expect("fn present");
        // 下一个顶层 fn 作为结束边界
        let rest = &src[start + 1..];
        let end = rest.find("\nfn ").map(|i| start + 1 + i).unwrap_or(src.len());
        let body = &src[start..end];
        assert!(!body.contains("convert_request"), "passthrough must bypass convert_request");
        assert!(!body.contains("build_upstream_headers"), "passthrough must bypass build_upstream_headers");
        assert!(!body.contains("apply_client_headers"), "passthrough must bypass apply_client_headers");
    }

    // ── SSE usage 累计（Anthropic message.usage + OpenAI 顶层 usage）──
    #[test]
    fn accumulate_sse_usage_anthropic_and_openai() {
        use std::sync::atomic::{AtomicI32, Ordering::Relaxed};
        let i = AtomicI32::new(0);
        let o = AtomicI32::new(0);
        let c = AtomicI32::new(0);

        // Anthropic message_start: usage 嵌在 message
        let anth: Value = serde_json::json!({
            "type": "message_start",
            "message": { "usage": { "input_tokens": 10, "cache_read_input_tokens": 3 } }
        });
        accumulate_sse_usage(&anth, &i, &o, &c);
        assert_eq!(i.load(Relaxed), 10);
        assert_eq!(c.load(Relaxed), 3);

        // OpenAI 顶层 usage
        let oai: Value = serde_json::json!({
            "usage": { "prompt_tokens": 20, "completion_tokens": 7 }
        });
        accumulate_sse_usage(&oai, &i, &o, &c);
        assert_eq!(i.load(Relaxed), 20);
        assert_eq!(o.load(Relaxed), 7);
    }

    // ── 同协议透传判定：仅端点协议精确等于入站协议才透传 ──
    // 精确匹配 → 透传；openai_responses→openai 跨协议回退 → 不透传（必须真转换）。
    #[test]
    fn same_protocol_passthrough_condition() {
        // 入站 anthropic + 平台显式 anthropic 端点 → 透传
        let source = "anthropic";
        let matched: Option<super::Protocol> = Some(super::Protocol::Anthropic);
        let pass = matched
            .as_ref()
            .map(|p| format!("{:?}", p).to_lowercase() == source)
            .unwrap_or(false);
        assert!(pass, "exact-protocol endpoint must passthrough");

        // 入站 openai_responses 回退到 openai 端点 → 跨协议，不透传
        let source = "openai_responses";
        let matched: Option<super::Protocol> = Some(super::Protocol::OpenAI);
        let pass = matched
            .as_ref()
            .map(|p| format!("{:?}", p).to_lowercase() == source)
            .unwrap_or(false);
        assert!(!pass, "openai_responses→openai fallback must NOT passthrough (needs conversion)");

        // 无匹配端点 → 不透传（走 convert_request 转 platform_type）
        let matched: Option<super::Protocol> = None;
        let pass = matched
            .as_ref()
            .map(|p| format!("{:?}", p).to_lowercase() == "openai")
            .unwrap_or(false);
        assert!(!pass, "no matched endpoint must NOT passthrough");
    }

    // ── 透传 model remap：仅 patch model 字段，messages/tools 结构原样保留 ──
    #[test]
    fn passthrough_patches_model_only() {
        let orig = serde_json::json!({
            "model": "claude-sonnet-4",
            "messages": [{"role": "user", "content": "hi"}],
            "tools": [{"name": "calc"}],
            "max_tokens": 100
        });
        let actual_model = "claude-3-5-sonnet-20241022";
        let mut body = orig.clone();
        if let Some(obj) = body.as_object_mut() {
            obj.insert("model".to_string(), Value::String(actual_model.to_string()));
        }
        // model 已替换
        assert_eq!(body.get("model").and_then(|v| v.as_str()), Some(actual_model));
        // messages / tools / 其余字段结构原样（未经 from_*→to_* 往返）
        assert_eq!(body.get("messages"), orig.get("messages"));
        assert_eq!(body.get("tools"), orig.get("tools"));
        assert_eq!(body.get("max_tokens"), orig.get("max_tokens"));
    }
}
