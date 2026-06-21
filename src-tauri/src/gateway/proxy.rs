use axum::{
    body::{Body, Bytes},
    extract::{Request, State as AxumState},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
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
use super::router::{select_candidates_ctx, RouteResult, ScheduleCtx};

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
    /// 调度器状态（per-platform 熔断 + 延迟 EMA + 在途计数，内存）。
    pub scheduler: Arc<super::scheduling::SchedulerState>,
    /// Sticky session 绑定表（内存 LRU + TTL）。
    pub sticky: Arc<super::scheduling::StickyTable>,
    /// 渐进式日志的 per-id 已落库列快照（in-flight 请求各 1 份）。
    /// 首节点 INSERT 后存快照；后续节点与快照 diff，仅 UPDATE 变化列；终态写入后移除。
    /// 用 Mutex<HashMap> 而非线程局部：流式 guard 在独立 task/Drop 路径写终态，
    /// 须与 handler 主链路共享同一 id 的快照才能正确 diff。
    pub log_snapshots: std::sync::Mutex<std::collections::HashMap<String, super::db::ProxyLogColumns>>,
}

/// 启动代理服务器，返回 shutdown handle
pub async fn start_proxy(
    db: Arc<Db>,
    port: u16,
    app: Option<tauri::AppHandle>,
    middleware: Arc<MiddlewareEngine>,
) -> Result<(tokio::task::JoinHandle<()>, u16), String> {
    let state = Arc::new(ProxyState {
        db,
        app,
        middleware,
        scheduler: Arc::new(super::scheduling::SchedulerState::new()),
        sticky: Arc::new(super::scheduling::StickyTable::new()),
        log_snapshots: std::sync::Mutex::new(std::collections::HashMap::new()),
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

/// 健康端点（`GET /` 与 `GET /proxy`）：客户端启动探测命中代理根 URL 时，
/// 既无 Authorization 也无上游请求语义 —— 直接返回 200 + 身份 JSON，
/// 不进 handle_proxy（否则 resolve_group None → 404）也不落 proxy_log（避免污染统计）。
async fn handle_root() -> Response {
    (
        StatusCode::OK,
        Json(serde_json::json!({
            "service": "aidog",
            "ok": true,
        })),
    )
        .into_response()
}

/// 分组信息端点 — 仅单平台分组返回本地预估值。
/// 鉴权：`Authorization: Bearer <group_key>`，localhost-only 端点。
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

    // 从 Authorization: Bearer <token> 提取 group_key
    let group_key = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .map(|s| s.trim().to_string());
    let group_key = match group_key {
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
    let group = match groups.iter().find(|g| g.group_key == group_key) {
        Some(g) => g,
        None => {
            tracing::debug!(group = %group_key, "group-info: group not found, not-applicable");
            return (StatusCode::OK, Json(empty())).into_response();
        }
    };

    // 关联平台 —— 恰好 1 个才适用
    let platforms = match super::db::get_group_platforms(&state.db, group.id).await {
        Ok(p) => p,
        Err(e) => {
            tracing::warn!(group = %group_key, error = %e, "group-info: get_group_platforms failed, not-applicable");
            return (StatusCode::OK, Json(empty())).into_response();
        }
    };
    if platforms.len() != 1 {
        return (StatusCode::OK, Json(empty())).into_response();
    }
    let platform = &platforms[0].platform;

    // usage 统计（复用现有 db 查询，只读）
    let stats = super::db::get_group_usage_stats(&state.db, &group.group_key).await.unwrap_or(
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
            today_tokens: 0,
            today_cost: 0.0,
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
        let rate_per_hour = super::db::get_group_hourly_rate(&state.db, &group.group_key).await.unwrap_or(None);
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

// ─── /api/notify（N1 — 系统通知端点）────────────────────────

/// 通知端点请求体：`{event?, type?, content?, vars?}`。
/// - `event`（N2）：CC hook 事件名（通用脚本 aidog-notify.py 发；后端按 per_event 解析 type+模板）。
/// - `type`（兼容旧路径 / Codex complete 脚本）：通知类型字面量，未知 → TaskComplete。
///   event 命中 per_event 时优先于 type。两者都缺省 → type 空串 → 兜底 TaskComplete。
#[derive(serde::Deserialize)]
struct NotifyReq {
    #[serde(default)]
    event: Option<String>,
    #[serde(rename = "type", default)]
    notif_type: String,
    #[serde(default)]
    content: Option<String>,
    #[serde(default)]
    vars: std::collections::HashMap<String, String>,
}

/// 通知端点 — localhost-only，鉴权 `Authorization: Bearer <group_key>`（仿 /api/group-info）。
/// hook 脚本调用此端点触发通知。body `{type, content?, vars?}`。
/// 鉴权用的 group_key 校验存在性，并作为 `{group}` 变量回填（脚本未显式带 group 时）。
async fn handle_notify(
    state: AxumState<Arc<ProxyState>>,
    headers: axum::http::HeaderMap,
    body: Bytes,
) -> Response {
    let span = tracing::info_span!("notify", trace_id = %crate::logging::new_trace_id());
    handle_notify_inner(state, headers, body).instrument(span).await
}

async fn handle_notify_inner(
    AxumState(state): AxumState<Arc<ProxyState>>,
    headers: axum::http::HeaderMap,
    body: Bytes,
) -> Response {
    // Bearer group_key 鉴权
    let group_key = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .map(|s| s.trim().to_string());
    let group_key = match group_key {
        Some(n) if !n.is_empty() => n,
        _ => return StatusCode::UNAUTHORIZED.into_response(),
    };
    // 校验分组存在（防任意 token 触发；不存在则拒绝）；同时取显示名供脚本 {group} 渲染。
    let group_name = match super::db::list_groups(&state.db).await {
        Ok(groups) => match groups.iter().find(|g| g.group_key == group_key) {
            Some(g) => g.name.clone(),
            None => {
                tracing::debug!(group = %group_key, "notify: group not found, reject");
                return StatusCode::UNAUTHORIZED.into_response();
            }
        },
        Err(e) => {
            tracing::warn!(error = %e, "notify: list_groups failed");
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    let req: NotifyReq = match serde_json::from_slice(&body) {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!(error = %e, "notify: invalid body");
            return (StatusCode::BAD_REQUEST, format!("invalid body: {e}")).into_response();
        }
    };

    // 注入内置变量：{group} 默认取鉴权分组的显示名（name，非 token group_key）；{time} 默认当前本地时间（脚本可覆盖）。
    let mut vars = req.vars;
    vars.entry("group".to_string()).or_insert_with(|| group_name.clone());
    vars.entry("time".to_string()).or_insert_with(|| {
        chrono::Local::now().format("%H:%M:%S").to_string()
    });

    let result = super::notification::dispatch(
        &state.db,
        state.app.as_ref(),
        req.event.as_deref(),
        &req.notif_type,
        req.content.as_deref(),
        &vars,
    )
    .await;

    tracing::debug!(
        event = ?req.event,
        notif_type = %req.notif_type,
        dispatched = result.dispatched,
        inbox = result.inbox,
        popup = result.popup,
        tts = result.tts,
        "notify dispatched"
    );

    (StatusCode::OK, Json(result)).into_response()
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
    // 按 settings 就地脱敏构造入库列快照（仅克隆受影响 String 字段，不再 clone 整 ProxyLog 结构）。
    let strip_user = !settings.log_user_request;
    let strip_upstream = !settings.log_upstream_request;
    let mut cols = super::db::ProxyLogColumns::from_log(log, strip_user, strip_upstream);

    // Calculate est_cost from model_price if tokens are present（语义同旧路径，作用于列快照）
    if cols.est_cost == 0.0 && (cols.input_tokens > 0 || cols.output_tokens > 0) {
        let model_name = if log.actual_model.is_empty() { &log.model } else { &log.actual_model };
        // best-effort 取平台主类型的 serde 裸名（如 "deepseek"）以启用 pricing[platform_type] override；
        // 拿不到则传 ""，calc_est_cost 的 fallback 回退链仍保证非 0。
        let platform_type = super::db::get_platform(&state.db, log.platform_id)
            .await
            .ok()
            .flatten()
            .map(|p| serde_json::to_string(&p.platform_type).unwrap_or_default().trim_matches('"').to_string())
            .unwrap_or_default();
        cols.est_cost = super::db::calc_est_cost(
            &state.db,
            model_name,
            &platform_type,
            cols.input_tokens,
            cols.output_tokens,
            cols.cache_tokens,
        )
        .await;
    }

    let id = cols.id.clone();
    let platform_id = log.platform_id;
    // 终态判定：有真实 HTTP 状态(status!=0)。唯一例外是流式占位写（response_body=="[stream]"，
    // 终态由 guard.flush 后显式 remove，不在此误删以免 guard 再 INSERT 撞主键）。
    // 覆盖流式请求在占位前就出错(如 502)的分支，避免快照泄漏。
    let is_terminal = cols.status_code != 0 && cols.response_body != "[stream]";

    // 取上一快照决定 INSERT(首节点) 还是 部分列 UPDATE(后续节点)。
    let prev = {
        let map = state.log_snapshots.lock().unwrap();
        map.get(&id).cloned()
    };
    let write_ok = match prev {
        None => {
            // 首节点：建行。成功后存快照供后续 diff。
            let ok = super::db::insert_proxy_log_columns(&state.db, cols.clone()).await.is_ok();
            if ok {
                state.log_snapshots.lock().unwrap().insert(id.clone(), cols);
            }
            ok
        }
        Some(prev) => {
            // 后续节点：仅 UPDATE 变化列；成功后刷新快照。
            let ok = super::db::update_proxy_log_columns(&state.db, cols.clone(), &prev).await.is_ok();
            if ok {
                state.log_snapshots.lock().unwrap().insert(id.clone(), cols);
            }
            ok
        }
    };

    // 终态写完移除快照，防 in-flight map 无限增长（流式占位写除外，由 guard 显式移除）。
    if is_terminal {
        remove_log_snapshot(state, &id);
    }

    if write_ok {
        // 日志写库成功后通知前端三页（Platforms/Groups/Stats）实时刷新统计。
        // 同时通知托盘刷新今日统计（请求数、Token、费用等）。
        // app handle 为 None（无 GUI 上下文）时安全跳过，不影响代理逻辑。
        if let Some(app) = &state.app {
            use tauri::Emitter;
            let _ = app.emit("proxy-log-updated", platform_id);
            let _ = app.emit("tray-refresh", ());
        }
    }
}

/// 移除某请求 id 的列快照（终态写入后调用，防止 in-flight 快照 map 无限增长）。
/// 流式 guard 终态 flush / 非流式终态返回前调用。重复调用安全（不存在即 no-op）。
fn remove_log_snapshot(state: &Arc<ProxyState>, id: &str) {
    state.log_snapshots.lock().unwrap().remove(id);
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
    extra: String,
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
            &extra,
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
        group_key: String::new(),
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
                if is_sensitive_auth_header(k.as_str()) {
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
    tracing::debug!(method = %orig_method, path = %path, body = %super::log_util::log_body_preview(&log.request_body), "inbound request body");

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
        match resolve_group(&state.db, auth_header.as_deref()).await {
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
    log.group_key = group.group_key.clone();
    // Auto-detect source_protocol from request path (group no longer restricts inbound protocol)
    let source_protocol = detect_source_protocol(&path);
    log.source_protocol = source_protocol.clone();
    tracing::info!(group = %group.name, source_protocol = %source_protocol, model = %log.model, "group resolved");
    upsert_log(&state, &log, &log_settings).await;

    // ── 模型列表端点分流（必须在 parse_incoming_request 之前）──
    // GET /v1/models | /models（openai/anthropic 同名）空 body，不能进 chat 解析（EOF 400）。
    // 命中 → 走 handle_models_passthrough：选分组首个启用平台，relay 上游模型列表。
    if orig_method == axum::http::Method::GET && is_models_endpoint(&path) {
        return handle_models_passthrough(&state, &mut log, &log_settings, &group, start, lang).await;
    }

    // ── Responses API 子端点分流（必须在 parse_incoming_request 之前）──
    // retrieve(GET /v1/responses/{id}) / cancel(POST .../{id}/cancel) / delete(DELETE .../{id})
    // / compact(POST /v1/responses/compact) / input_items(GET .../{id}/input_items)。
    // 这些是对某次 create 产生的上游 response 对象的操作，必须原样透传到上游 responses 平台
    // （body/path 不可经 chat 有损转换；GET/DELETE 空 body 进 chat parse 会 EOF 400）。
    // create（裸 /v1/responses，无尾段）不被拦，继续走下方 parse + same_protocol_passthrough（已 work）。
    if is_responses_subendpoint(&path) {
        return handle_responses_subendpoint(
            &state, &mut log, &log_settings, &group, &orig_method, &bytes, &path, start, lang,
        )
        .await;
    }

    // ── Anthropic count_tokens 子端点分流（必须在 parse_incoming_request 之前）──
    // claude-cli 发实际对话前会 POST /v1/messages/count_tokens 预估 token 数。
    // 该 path 前缀匹配 /v1/messages，若不前置分流会被当普通 messages 转发，且出站
    // passthrough_api_path 写死 /v1/messages 吞掉 count_tokens 尾段 → 上游按 messages
    // 处理 count_tokens 形态 body 而崩溃（GLM 实测 500）。命中 → 透传优先 + 本地估算兜底。
    if is_count_tokens_endpoint(&path) {
        return handle_count_tokens(&state, &mut log, &log_settings, &group, &bytes, start).await;
    }

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
            state.middleware.apply_inbound(&mw_settings, &mut chat_req, Some(&group.group_key))
        {
            return block_inbound(&state, log, &log_settings, lang, blocked_by, blocked_reason, start).await;
        }
    }

    // ── 路由选择有序候选平台列表（失败逐个重试）──
    // 调度上下文：scheduler(熔断+延迟+在途) / sticky(粘性绑定) / scheduling settings。
    let sched_settings = super::db::get_scheduling_settings(&state.db).await;
    // Sticky session 键：aidog 无 session_id 概念（见 design.md），用 group_key + 客户端稳定标识。
    // 稳定标识优先取 x-session-id / session_id header，缺省回退 user-agent；再缺省仅用 group_key。
    let sticky_key = {
        let client_id = orig_headers
            .get("x-session-id")
            .or_else(|| orig_headers.get("session_id"))
            .or_else(|| orig_headers.get("user-agent"))
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");
        Some(format!("{}|{}", group.group_key, client_id))
    };
    let sched_ctx = ScheduleCtx {
        scheduler: &state.scheduler,
        sticky: &state.sticky,
        settings: &sched_settings,
        sticky_key,
    };
    let candidate_set = match select_candidates_ctx(&state.db, &group, &chat_req.model, Some(&sched_ctx)).await {
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

    // OpenCode Zen：api_key 留空 → 注入匿名免费 key（$opencode）；用户填了用用户的。
    let eff_api_key = resolve_opencode_zen_key(&route.platform);

    // 尝试匹配端点：按 source_protocol 查找平台是否支持对应协议的端点。
    // 先精确匹配；openai_responses 源（Codex）若无 Responses 端点，回退到 openai 端点
    // （普通 chat/completions 平台），出站经 to_openai 转换。
    let ep_proto = |ep: &super::models::PlatformEndpoint| format!("{:?}", ep.protocol).to_lowercase();
    let matched_ep = select_endpoint_for_protocol(&route.platform.endpoints, &source_protocol);

    // ── UA 透传分支（[protocol-same-proto-passthrough] 扩展，PRD §5 级别 1）──
    // 仅当 path 推断的入站协议在平台无任何对应 endpoint（matched_ep == None，
    // 现状会落入 platform_type + ClientType::Default 有损兜底）时尝试：
    // 按入站 User-Agent 推断客户端原生协议（claude-cli→anthropic / codex→openai_responses），
    // 若平台确有该协议的 endpoint → matched_ep 改指向该 UA-endpoint，并以该协议为透传 wire 协议。
    // UA 不识别 / 平台无该协议 endpoint → matched_ep 保持 None，回退现有兜底（零行为变更）。
    // matched_ep 命中（path 已支持）时不介入。
    let (matched_ep, passthrough_proto) = if matched_ep.is_none() {
        let ua_proto = orig_headers
            .get("user-agent")
            .and_then(|v| v.to_str().ok())
            .and_then(infer_passthrough_protocol_from_ua);
        match ua_proto {
            Some(p) => match route.platform.endpoints.iter().find(|ep| ep_proto(ep) == p) {
                Some(ep) => {
                    tracing::info!(
                        platform = %route.platform.name, platform_id = route.platform.id,
                        source_protocol = %source_protocol, ua_protocol = %p,
                        "ua-passthrough: path protocol unsupported by platform, routing to UA-inferred endpoint"
                    );
                    (Some(ep), Some(p))
                }
                // UA 命中但平台无该协议 endpoint（级别 2）→ 回退现有兜底
                None => (matched_ep, None),
            },
            // UA 不识别（级别 3）→ 回退现有兜底
            None => (matched_ep, None),
        }
    } else {
        (matched_ep, None)
    };

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
    // 透传判定：
    // - 级别 0（现状）：端点协议精确等于 path 推断的 source_protocol。
    // - 级别 1（UA 透传）：passthrough_proto == Some(p) 且端点协议等于 UA 推断协议 p
    //   → 端点协议 == source_protocol 不成立（否则 matched_ep 在级别 0 已命中），故单独判定。
    let same_protocol_passthrough = match passthrough_proto {
        Some(p) => matched_ep.map(|ep| ep_proto(ep) == p).unwrap_or(false),
        None => matched_ep.map(|ep| ep_proto(ep) == source_protocol).unwrap_or(false),
    };

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

    // ── max_tokens 出站裁剪（convert_request 前）──
    // 客户端 max_tokens 超过选定模型上限时裁剪到上限；未传 / 模型无上限则不动（Q3 保守）。
    // 仅作用于 convert_request（读 chat_req）；同协议透传分支用原始 req_value 不受影响
    // （客户端原生协议，上游自纠；已知限制见 report）。
    {
        let model_max = super::db::get_model_max_output_tokens(&state.db, &actual_model)
            .await
            .ok()
            .flatten();
        let (capped, did_cap) = super::router::cap_max_tokens(chat_req.max_tokens, model_max);
        if did_cap {
            tracing::info!(
                model = %actual_model,
                requested = ?chat_req.max_tokens, capped_to = ?capped,
                "max_tokens exceeds model limit, capping"
            );
            chat_req.max_tokens = capped;
        }
    }

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
    // 流式响应 body 读取不计入总超时：reqwest .timeout 覆盖「连接→响应头→body 全部读完」，
    // 会砍断长 thinking/tool_use 流（body 读取 > request_timeout_secs）致无 message_stop → 客户端
    // JSON Parse error / 内容残缺。流式禁总超时（传 0），connect_timeout 仍保护连接期，客户端自有超时兜底。
    let req_timeout = if is_stream { 0 } else { req_timeout };
    let client = super::http_client::build_http_client(
        &state.db, req_timeout, conn_timeout,
        Some(&route.platform.extra), None,
    ).await;

    // ── 构建上游请求头 ──
    // convert 路径：先铺底透传入站头（anthropic-* / x-stainless-* / x-app / session-id 等，
    // 跨协议也带，上游忽略未知头不报错），再由 apply_client_headers 覆盖 UA + auth + CT。
    // passthrough_convert_headers 已剔 hop-by-hop + auth/UA/CT（由下方覆盖），无同名多值。
    let upstream_headers = build_upstream_headers(&client_type, target_protocol_enum, &eff_api_key, &orig_headers);

    let mut req_builder = client
        .post(&url)
        .header("Content-Type", "application/json")
        .headers(passthrough_convert_headers(&orig_headers))
        .body(req_body_str.clone());

    // ── 覆盖 UA + auth（平台 api_key）──
    req_builder = apply_client_headers(req_builder, &client_type, target_protocol_enum, &eff_api_key);

    // ── 记录上游实际请求 ──
    log.upstream_request_headers = serde_json::Value::Object(
        upstream_headers.into_iter().map(|(k, v)| (k, Value::String(v))).collect()
    ).to_string();
    log.upstream_request_body = format_pretty_json(&req_body_str);
    tracing::info!(method = "POST", url = %url, "upstream request");
    tracing::debug!(method = "POST", url = %url, body = %super::log_util::log_body_preview(&req_body_str), "upstream request body");

    // ── 熔断指标：本次 forward 尝试前在途 +1；解析本平台有效阈值 ──
    let breaker_th = {
        let (ft, os, hom) = sched_settings.effective_thresholds(&route.platform);
        super::scheduling::BreakerThresholds { failure_threshold: ft, open_secs: os, half_open_max: hom }
    };
    state.scheduler.inc_inflight(route.platform.id);

    let resp = match req_builder.send().await {
        Ok(r) => r,
        Err(e) => {
            // 连接失败 / 超时 → 可重试，换下个候选；候选耗尽则返回 502。
            // 熔断：连接失败 / 超时计一次失败（in-flight -1 + breaker fail 计数）。
            state.scheduler.record_failure(route.platform.id, &breaker_th, super::db::now());
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
    // clone 上游响应头，供回包前透传筛选用（resp 后续被 bytes()/bytes_stream() 消费）
    let upstream_resp_headers = resp.headers().clone();
    {
        let mut h = serde_json::Map::new();
        for (k, v) in resp.headers() {
            if let Ok(s) = v.to_str() {
                h.insert(k.to_string(), Value::String(s.to_string()));
            }
        }
        log.upstream_response_headers = Value::Object(h).to_string();
    }

    // ── 流式判定以实际上游响应为准：请求 body 的 stream 字段与上游响应 content-type 取并。
    //   中转站常对未声明 stream 的请求强制以 text/event-stream 响应；若仅凭请求字段会误判为
    //   非流式，进而用 JSON 解析 SSE 文本拿不到 usage → token/est_cost 全为 0。此处纠偏，
    //   使任何 SSE 响应都走流式 token 聚合路径。OR 语义保证既有正常流式路径不回归。──
    let upstream_ct = upstream_resp_headers
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    let is_stream = resolve_is_stream(is_stream, upstream_ct);
    log.is_stream = is_stream;

    if !status.is_success() {
        let body = resp.text().await.unwrap_or_default();
        let duration_ms = start.elapsed().as_millis() as i64;
        let code = status.as_u16();
        tracing::warn!(
            url = %url, platform = %route.platform.name, status = code,
            duration_ms, "upstream returned non-success status"
        );
        tracing::debug!(url = %url, status = code, body = %super::log_util::log_body_preview(&body), "upstream error response body");
        attempts.push(ProxyAttempt {
            platform_id: route.platform.id,
            platform_name: route.platform.name.clone(),
            status_code: code as i32,
            error: truncate_attempt_error(&body),
            duration_ms: attempt_start.elapsed().as_millis() as i64,
            ts: attempt_ts,
        });

        // ── 熔断计数：5xx 或 429 计一次失败；401/403/其他客户端 4xx 不计熔断（仅 inflight-1）。
        //   熔断与 auto_disabled 解耦：401/403 走下方 auto_disabled，不参与熔断。──
        if code >= 500 || code == 429 {
            state.scheduler.record_failure(route.platform.id, &breaker_th, super::db::now());
        } else {
            state.scheduler.record_ignored(route.platform.id);
        }

        // ── 401/403：上游鉴权失败（key 问题）→ 单次即自动禁用平台（指数退避），换下个候选 ──
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
        // ── 404/405：死端点信号（端点不存在 / 方法不允许，如 nginx "Not Allowed"）。
        //   与 401/403 共用 auto_disabled + 指数退避机制，但语义不同：404/405 可能是上游瞬时
        //   配置抖动，故连续累计达阈值（DEAD_ENDPOINT_STRIKE_THRESHOLD）才禁用，防偶发误伤。
        //   未达阈值仅计数、保持 enabled 继续参与调度；一次 2xx 即清零计数（见下方成功路径）。──
        else if code == 404 || code == 405 {
            match super::db::record_dead_endpoint_strike(
                &state.db, route.platform.id, super::db::DEAD_ENDPOINT_STRIKE_THRESHOLD,
            ).await {
                Ok((strikes, until)) if until > 0 => tracing::warn!(
                    platform = %route.platform.name, platform_id = route.platform.id, status = code,
                    strikes, auto_disabled_until = until,
                    "platform auto-disabled (404/405 dead-endpoint, strike threshold reached)"
                ),
                Ok((strikes, _)) if strikes > 0 => tracing::info!(
                    platform = %route.platform.name, platform_id = route.platform.id, status = code,
                    strikes, threshold = super::db::DEAD_ENDPOINT_STRIKE_THRESHOLD,
                    "platform dead-endpoint strike accumulating (404/405), not yet disabled"
                ),
                Ok(_) => {} // 用户手动 disabled，不动
                Err(e) => tracing::error!(platform_id = route.platform.id, error = %e, "record dead-endpoint strike failed"),
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
                Some(&group.group_key), Some(route.platform.id as i64),
            )
        };
        // ── 决策 A：状态码硬错圈定 ──
        //   400 / 422（请求体本身非法）→ 不重试，直接返客户端（换平台无用，避免无谓遍历）。
        //   其余非 2xx（401/403/404/405/429/5xx/未知）→ 默认可重试（换下个候选）。
        //   400/422 的硬停优先于中间件 error_rule 的 retryable 分类（status 硬错语义不可被覆盖回可重试）。
        let status_retryable = is_status_retryable(code);
        // 中间件 error_rule：仅在 status 本身可重试时，允许其将错误显式降级为 non-retryable（缩小重试面）；
        //   不允许把硬错（400/422）反向放大为可重试。
        let mw_non_retryable = err_class.as_ref().map(|c| !c.retryable).unwrap_or(false);
        let non_retryable = !status_retryable || mw_non_retryable;
        if let Some(ref c) = err_class {
            tracing::info!(
                matched_by = %c.matched_by, category = %c.category, retryable = c.retryable,
                status = code, "middleware error_rule classified upstream error"
            );
        }
        if !status_retryable {
            tracing::info!(
                status = code, platform = %route.platform.name,
                "decision-A: hard request error (400/422), not retrying next platform"
            );
        }

        // 可重试（非 400/422 硬错 且 中间件未标 non-retryable）→ 换下个候选；
        // 候选耗尽 / 超 max_retries 则返回最后一次错误。non-retryable → 立即返回（不换候选）。
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

    // ── 2xx：状态码成功，但「200 + 空/无效响应」按决策 B 仍当作失败重试。──
    // 成功记账（record_success / 恢复 auto_disabled / 清 strike / attempts.push 成功 / log.attempts）
    // 推迟到「确认非空有效响应」之后，由 commit_2xx_success! 宏统一执行（避免重复且保证仅真成功才记账）。
    let attempt_latency_ms = attempt_start.elapsed().as_millis() as i64;

    // 决策 B 失败（200 空响应）时记一次失败 attempt 并 failover；候选耗尽则返回 502。
    // 与连接错误/超时同语义：熔断计一次失败（record_failure），但不 auto_disable（非鉴权/死端点信号）。
    macro_rules! retry_on_empty_2xx {
        ($reason:expr) => {{
            state.scheduler.record_failure(route.platform.id, &breaker_th, super::db::now());
            tracing::warn!(
                platform = %route.platform.name, platform_id = route.platform.id,
                reason = $reason, "decision-B: upstream 200 but empty/invalid response, failover next platform"
            );
            attempts.push(ProxyAttempt {
                platform_id: route.platform.id,
                platform_name: route.platform.name.clone(),
                status_code: 200,
                error: $reason.to_string(),
                duration_ms: attempt_latency_ms,
                ts: attempt_ts,
            });
            if !is_last_candidate {
                continue;
            }
            // 候选耗尽：返回 502 + 已记录的 attempts（此时尚未向客户端发任何字节，安全）。
            log.platform_id = route.platform.id;
            log.status_code = 502;
            log.upstream_status_code = status.as_u16() as i32;
            let err_body = format!("{}: 200 but empty/invalid response", i18n::t(lang, ErrorKey::Upstream));
            log.response_body = $reason.to_string();
            log.user_response_body = err_body.clone();
            log.user_response_headers = r#"{"content-type":"text/plain"}"#.to_string();
            log.duration_ms = start.elapsed().as_millis() as i32;
            log.retry_count = (attempts.len() as i32 - 1).max(0);
            log.attempts = std::mem::take(&mut attempts);
            upsert_log(&state, &log, &log_settings).await;
            return (StatusCode::BAD_GATEWAY, err_body).into_response();
        }};
    }

    // 真成功记账：熔断成功 + 恢复 auto_disabled + 清 strike + attempts.push 成功 + 填 log.attempts。
    macro_rules! commit_2xx_success {
        () => {{
            // 熔断指标：成功 → 更新延迟 EMA + breaker Closed/HalfOpen→Closed + inflight-1。
            // 注意流式此处为「首个有效内容」延迟（peek 已收到内容）；作为延迟近似用于 LeastLatency。
            state.scheduler.record_success(route.platform.id, attempt_latency_ms);
            attempts.push(ProxyAttempt {
                platform_id: route.platform.id,
                platform_name: route.platform.name.clone(),
                status_code: status.as_u16() as i32,
                error: String::new(),
                duration_ms: attempt_latency_ms,
                ts: attempt_ts,
            });
            if route.platform.status == super::models::PlatformStatus::AutoDisabled {
                if let Err(e) = super::db::recover_platform_auto_disabled(&state.db, route.platform.id).await {
                    tracing::error!(platform_id = route.platform.id, error = %e, "recover auto-disabled platform failed");
                } else {
                    tracing::info!(platform = %route.platform.name, platform_id = route.platform.id, "platform recovered from auto-disabled (2xx)");
                }
            } else if let Err(e) =
                // 成功一次即证明端点非死端点 → 清零累计的 404/405 strikes（仅 enabled 平台有计数时生效）
                super::db::reset_dead_endpoint_strikes(&state.db, route.platform.id).await
            {
                tracing::error!(platform_id = route.platform.id, error = %e, "reset dead-endpoint strikes failed");
            }
            log.platform_id = route.platform.id;
            log.retry_count = (attempts.len() as i32 - 1).max(0);
            log.attempts = std::mem::take(&mut attempts);
        }};
    }

    // 非流式：直接透传 JSON
    if !is_stream {
        let body = resp.bytes().await.unwrap_or_default();
        let resp_str = String::from_utf8_lossy(&body).to_string();

        // ── 决策 B（非流式）：200 但空 body / error 结构 / 无有效 choices/content → 失败重试。──
        if !is_nonstream_body_valid(&resp_str) {
            retry_on_empty_2xx!("200 but empty/invalid body");
        }
        commit_2xx_success!();

        let (input_tokens, output_tokens, cache_tokens) = extract_usage(&resp_str);

        log.response_body = resp_str.clone();
        log.status_code = 200;
        log.duration_ms = start.elapsed().as_millis() as i32;
        log.input_tokens = input_tokens;
        log.output_tokens = output_tokens;
        log.cache_tokens = cache_tokens;

        // ── 非流式跨协议响应转换 ──
        // 流式路径靠 parse_sse→to_client_sse 转换响应格式，但非流式分支历史上**直接透传上游 body**，
        // 致 source≠target 且非同协议透传时（如 anthropic 客户端 ↔ openai 平台），CC 收到上游原生
        // openai chat completion JSON（含 tool_calls）而非 anthropic messages → "empty or malformed (200)"。
        // 这里补齐：同协议透传跳过；否则按 (wire=target, client=source) 转换。返回 None 表示无需转换，透传原文。
        let body = if !same_protocol_passthrough {
            let upstream_json: Value = serde_json::from_slice(&body).unwrap_or(Value::Null);
            match adapter::convert_response(
                &upstream_json,
                target_protocol_enum,
                &source_protocol,
                &requested_model,
            ) {
                Some(converted) => serde_json::to_vec(&converted).unwrap_or_else(|_| body.to_vec()),
                None => body.to_vec(),
            }
        } else {
            body.to_vec()
        };
        let body = Bytes::from(body);

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
                Some(&group.group_key), Some(route.platform.id as i64),
            );
            s.into_bytes()
        };
        log.user_response_body = String::from_utf8_lossy(&body).to_string();

        // ── 透传上游响应头（黑名单剔除 content-encoding/content-length/hop-by-hop）──
        let mut filtered = filter_upstream_resp_headers(&upstream_resp_headers, false);
        // 上游缺 content-type 时回退默认 application/json
        if !filtered
            .iter()
            .any(|(n, _)| n == axum::http::header::CONTENT_TYPE)
        {
            filtered.push((
                axum::http::header::CONTENT_TYPE,
                axum::http::HeaderValue::from_static("application/json"),
            ));
        }
        // 日志字段 = 实际发回客户端的头集合（不再写死 content-type）
        log.user_response_headers = resp_headers_to_log_json(&filtered);

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
            eff_api_key.clone(),
            actual_model.clone(),
            route.platform.extra.clone(),
            input_tokens,
            output_tokens,
            cache_tokens,
            coding_plan,
            tracing::Span::current(),
        );

        let mut response = (StatusCode::OK, body.to_vec()).into_response();
        // into_response 对 Vec<u8> 写死 content-type: application/octet-stream；
        // HeaderMap::extend 用 append 语义，直接 extend 会产生重复 content-type（octet-stream + 真实值）。
        // 故先 remove 默认 content-type，再 extend（filtered 已含真实 content-type 或回退 application/json）。
        response
            .headers_mut()
            .remove(axum::http::header::CONTENT_TYPE);
        response.headers_mut().extend(filtered);
        return response;
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

    // ── 决策 B（流式）：提交转发前缓冲(peek)上游首个「有效内容」chunk 再决定。──
    // 在向客户端发任何字节前，先从上游 SSE 流拉取若干 chunk，扫描累积原文：
    //   - Meaningful（真实内容事件）→ 提交：把已缓冲的 chunk 原样 prepend 回流，继续既有 relay。
    //   - EmptyOrError（立即 [DONE] / 立即 error / 流秒断无内容 / 空 body）→ 当作失败 failover（header 未发，安全）。
    // 仅 peek 到「判定够了」即停（收到首个有效内容立即提交），不缓冲整条流（接受首字节延迟）。
    // 缓冲上限兜底：累计字节 / chunk 数到上限仍未判定 → 视为已产出内容，提交（避免饿死长 keepalive 流）。
    const PEEK_MAX_BYTES: usize = 64 * 1024;
    const PEEK_MAX_CHUNKS: usize = 64;
    let mut upstream_stream = resp.bytes_stream();
    let mut peek_buf: Vec<Bytes> = Vec::new();
    let mut peek_text = String::new();
    let mut peek_bytes = 0usize;
    let peek_decision = loop {
        match upstream_stream.next().await {
            Some(Ok(chunk)) => {
                peek_bytes += chunk.len();
                peek_text.push_str(&String::from_utf8_lossy(&chunk));
                peek_buf.push(chunk);
                match classify_stream_first(&peek_text, false) {
                    StreamPeek::Meaningful => break StreamPeek::Meaningful,
                    StreamPeek::EmptyOrError => break StreamPeek::EmptyOrError,
                    StreamPeek::NeedMore => {
                        if peek_bytes >= PEEK_MAX_BYTES || peek_buf.len() >= PEEK_MAX_CHUNKS {
                            // 上限兜底：已收到字节但未见明确内容/错误标记 → 保守提交，避免误杀长流。
                            break StreamPeek::Meaningful;
                        }
                    }
                }
            }
            // 上游流秒断（peek 期间出错）→ 与连接错误同语义，failover。
            Some(Err(e)) => {
                tracing::warn!(error = %e, "decision-B: upstream stream error during first-chunk peek");
                break StreamPeek::EmptyOrError;
            }
            // 流结束：用 stream_ended=true 收敛判定（无内容 → EmptyOrError）。
            None => break classify_stream_first(&peek_text, true),
        }
    };

    if peek_decision == StreamPeek::EmptyOrError {
        retry_on_empty_2xx!("200 but empty/invalid stream");
    }
    // Meaningful：确认上游真实产出 → 提交成功记账（在构建 guard 前，使 guard 的 log 快照含正确 attempts）。
    commit_2xx_success!();

    // ── 中间件出站流式逐块改写上下文：在构建 stream 闭包前读取 settings（闭包在 req span 外轮询，
    //   不可再 await DB）。引擎 Arc clone 进闭包，每 chunk 文本应用 mask/override/sensitive。
    //   error 已由上游 HTTP 状态码在 forward 后判定（非 2xx 不会走到这里，故流式无需再判 error）。──
    let mw_engine = state.middleware.clone();
    let mw_settings = super::db::get_middleware_settings(&state.db).await;
    let mw_active = mw_settings.enabled;
    let mw_group = group.group_key.clone();
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
            api_key: eff_api_key.clone(),
            model: actual_model.clone(),
            extra: route.platform.extra.clone(),
            coding_plan,
        }),
    };

    // guard 被 move 进闭包，随 stream 生命周期存活；stream 被 Drop（含客户端断连）时 guard.drop 触发兜底 flush。
    // 决策 B：把 peek 阶段已缓冲的首批 chunk 原样 prepend 回流（不能吞首块），再接上游剩余流；
    // 下游闭包对缓冲块与后续块一视同仁（token 聚合 / 转换 / finalize 不受影响）。
    let buffered_head = futures::stream::iter(
        peek_buf.into_iter().map(Ok::<Bytes, reqwest::Error>),
    );
    let upstream_rest = buffered_head.chain(upstream_stream);
    let stream = upstream_rest.map(move |chunk_result| {
        let chunk = match chunk_result {
            Ok(c) => c,
            Err(e) => {
                // 上游流中途断裂（如 GLM ~60s 截断）：不向客户端报错，仅记日志 +
                // 按客户端协议合成干净的 Stop 终止事件收尾，已输出内容保留。
                // （不再注入 `event: error`，避免 CC 显示 "API Error: error decoding response body"。）
                tracing::warn!(error = %e, "SSE upstream stream chunk error; closing stream gracefully");
                let stop = adapter::to_client_sse(&ChatStreamEvent::Stop {
                    finish_reason: Some("end_turn".to_string()),
                }, &client_protocol, &model_for_sse).unwrap_or_default();
                return Ok::<_, std::io::Error>(Bytes::from(stop));
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
            // 跨 chunk 行重组后累计 usage（逐 chunk .lines() 会因 data: 行被切断而丢 usage）。
            guard.agg.feed_sse_usage(&text);
            chunk.clone()
        } else {
            // token 累计走跨 chunk 行重组（逐 chunk .lines() 会因 data: 行被切断丢 usage）。
            // 协议转换仍逐 chunk 处理（输出格式转换路径，行为不变）。
            guard.agg.feed_sse_usage(&text);
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
    // ── SSE 三自管头（content-type/cache-control/connection）+ 叠加筛选上游头（is_stream=true 额外剔这三者，防上游覆盖）──
    let sse_self_managed: [(axum::http::HeaderName, axum::http::HeaderValue); 3] = [
        (axum::http::header::CONTENT_TYPE, axum::http::HeaderValue::from_static("text/event-stream")),
        (axum::http::header::CACHE_CONTROL, axum::http::HeaderValue::from_static("no-cache")),
        (axum::http::header::CONNECTION, axum::http::HeaderValue::from_static("keep-alive")),
    ];
    let stream_filtered = filter_upstream_resp_headers(&upstream_resp_headers, true);
    // 日志字段 = 实发头 = SSE 三自管头 + 透传上游头
    let mut all_stream_headers: Vec<(axum::http::HeaderName, axum::http::HeaderValue)> =
        sse_self_managed.to_vec();
    all_stream_headers.extend(stream_filtered.iter().cloned());

    log.status_code = 200;
    log.response_body = "[stream]".to_string();
    log.user_response_body = "[stream]".to_string();
    log.user_response_headers = resp_headers_to_log_json(&all_stream_headers);
    log.duration_ms = start.elapsed().as_millis() as i32;
    upsert_log(&state, &log, &log_settings).await;

    let mut response = (StatusCode::OK, body).into_response();
    {
        let h = response.headers_mut();
        for (n, v) in sse_self_managed {
            h.insert(n, v);
        }
        h.extend(stream_filtered);
    }
    return response;
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

/// 上游响应头透传黑名单（必剔 + RFC 7230 §6.1 hop-by-hop）。
/// 全小写常量；HeaderName 本身即小写存储，用 as_str() 比对即可。
const RESP_HEADER_BLACKLIST: &[&str] = &[
    // §4.1 必剔（解压/长度/传输编码失真）
    "content-encoding",
    "content-length",
    "transfer-encoding",
    // §4.2 应剔（hop-by-hop, RFC 7230 §6.1）
    "connection",
    "keep-alive",
    "proxy-authenticate",
    "proxy-authorization",
    "te",
    "trailer",
    "upgrade",
];

/// 流式（SSE）额外剔除集：这三个头归 SSE 自管，禁用上游值覆盖 SSE 语义。
const SSE_EXTRA_BLACKLIST: &[&str] = &["content-type", "cache-control", "connection"];

/// 上游响应头 → 透传给客户端的头（黑名单剔除 + 非法 value 跳过 + 多值逐个保留）。
///
/// - `is_stream=false`：仅按 RESP_HEADER_BLACKLIST 剔除（非流式 2xx 路径）。
/// - `is_stream=true`：在 RESP_HEADER_BLACKLIST 基础上额外剔除 SSE_EXTRA_BLACKLIST，
///   叠加于调用方设置的 SSE 三自管头之上。
///
/// 返回 `Vec<(HeaderName, HeaderValue)>`，调用方用 `extend` 注入 axum Response。
/// 多值头（如多个 set-cookie）逐项保留（Vec append 语义，不覆盖）。
/// 无法转为 axum header 类型的非法名/值跳过（不 panic）。
fn filter_upstream_resp_headers(
    src: &reqwest::header::HeaderMap,
    is_stream: bool,
) -> Vec<(axum::http::HeaderName, axum::http::HeaderValue)> {
    let mut out = Vec::with_capacity(src.len());
    for (k, v) in src.iter() {
        let name = k.as_str(); // HeaderName 已小写
        if RESP_HEADER_BLACKLIST.iter().any(|b| name.eq_ignore_ascii_case(b)) {
            continue;
        }
        if is_stream && SSE_EXTRA_BLACKLIST.iter().any(|b| name.eq_ignore_ascii_case(b)) {
            continue;
        }
        // reqwest header 类型 → axum(http) header 类型；非法则跳过不 panic
        if let (Ok(hn), Ok(hv)) = (
            axum::http::HeaderName::from_bytes(name.as_bytes()),
            axum::http::HeaderValue::from_bytes(v.as_bytes()),
        ) {
            out.push((hn, hv));
        }
    }
    out
}

/// 把实发头集合（HeaderName, HeaderValue）序列化为日志 JSON 字符串，
/// 与 upstream_response_headers 同格式 `{name: value}`；多值同名头保留首值（与既有格式约定一致）。
fn resp_headers_to_log_json(headers: &[(axum::http::HeaderName, axum::http::HeaderValue)]) -> String {
    let mut h = serde_json::Map::new();
    for (k, v) in headers {
        if let Ok(s) = v.to_str() {
            h.entry(k.as_str().to_string())
                .or_insert_with(|| Value::String(s.to_string()));
        }
    }
    Value::Object(h).to_string()
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

/// 决策 A：非 2xx 上游状态码是否应 failover 重试下一候选平台。
///
/// - **不重试（硬错，换平台也没用）**：400 / 422 —— 请求体本身非法（协议转换产物上游拒收），
///   遍历其他平台同样会被拒，直接返客户端避免无谓遍历。
/// - **重试**：401 / 403（鉴权，配合 auto_disabled）、404 / 405（死端点，配合 strike）、
///   429（限流/配额，换平台可能成功）、所有 5xx（上游故障）、其余未知非 2xx（保守重试）。
///
/// 连接错误 / 超时不经此函数（在 send() Err 分支已按可重试处理）。
/// 注意：中间件 error_rule 的 non-retryable 分类是显式覆盖机制，独立于本函数（见调用点）。
fn is_status_retryable(code: u16) -> bool {
    !matches!(code, 400 | 422)
}

/// 决策 B：流式 200 首块缓冲判定结果。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StreamPeek {
    /// 已确认上游在产出有效内容（anthropic 真实事件 / openai choices delta / 通用 data 事件）→ 提交转发。
    Meaningful,
    /// 200 但空 / 无效（立即 [DONE] 无内容 / 立即 error 事件 / 流秒断无内容 / 空 body）→ 当作失败重试。
    EmptyOrError,
    /// 累积的字节尚不足以判定（仅注释/keepalive/不完整 SSE 帧）→ 继续缓冲下一块。
    NeedMore,
}

/// 决策 B：扫描已缓冲的上游 SSE 原文，判定首个「有效内容」是否到达。
///
/// 在**上游原始 wire 格式**上判定（转换前），覆盖 anthropic / openai / 同协议透传三类：
/// - **EmptyOrError**（重试）：首个有效事件是 `error`（`event: error` 或 JSON `{"type":"error"}` / 顶层 `error` 字段）；
///   或在任何内容事件前先出现 `[DONE]`。
/// - **Meaningful**（提交）：出现真实内容事件 —— anthropic `message_start`/`content_block_*`/`message_delta`；
///   openai `choices`（含 delta/role/content/tool_calls/finish_reason）；或任何非 error/非 [DONE] 的 `data:` JSON 事件。
/// - **NeedMore**：目前只见 SSE 注释行（`:` 开头 keepalive）/ 空行 / `event:` 名行但对应 `data:` 帧尚未到齐。
///
/// `stream_ended=true`（上游流已结束）时强制收敛：仍无内容事件 → EmptyOrError（流秒断无内容 / 空 body）。
fn classify_stream_first(text: &str, stream_ended: bool) -> StreamPeek {
    let mut saw_any_data = false;
    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with(':') {
            // SSE 注释 / keepalive / 空分隔行 → 不构成判定依据
            continue;
        }
        // `event: error` 行：下一帧 data 即错误；提前判 error（无需等 data）
        if let Some(ev) = line.strip_prefix("event:") {
            if ev.trim().eq_ignore_ascii_case("error") {
                return StreamPeek::EmptyOrError;
            }
            // 其他 event 名行（message_start/content_block_delta...）单独不足以判定，等 data 帧
            continue;
        }
        let Some(data) = line.strip_prefix("data:") else {
            // 非 SSE 字段行（不完整帧的中段）→ 等更多
            continue;
        };
        let data = data.trim();
        if data == "[DONE]" {
            // 任何内容前先 [DONE] → 空响应；内容后的 [DONE] 不会进入本函数（已 Meaningful 提前返回）
            return StreamPeek::EmptyOrError;
        }
        saw_any_data = true;
        let Ok(json) = serde_json::from_str::<Value>(data) else {
            // data 帧 JSON 尚不完整（跨 chunk 截断）→ 等更多
            continue;
        };
        // 顶层 error 结构（openai `{"error":{...}}` / anthropic `{"type":"error",...}`）→ 失败
        let ty = json.get("type").and_then(|v| v.as_str()).unwrap_or("");
        if ty == "error" || json.get("error").is_some() {
            return StreamPeek::EmptyOrError;
        }
        // 到此即确认上游产出了一个真实（非 error / 非 [DONE]）内容事件 → 提交
        return StreamPeek::Meaningful;
    }
    if stream_ended {
        // 流已结束仍无任何有效内容事件 → 空响应（哪怕收到过无法解析的残帧也判空）
        let _ = saw_any_data;
        StreamPeek::EmptyOrError
    } else {
        StreamPeek::NeedMore
    }
}

/// 决策 B：非流式 200 响应体是否「非空有效」。返回 false → 当作失败重试下一平台。
///
/// 空 body / 不含有效 choices/content / 是 error 结构 → false。
/// 在**上游原始 JSON**上判定（转换前 / 透传同理）。
fn is_nonstream_body_valid(body: &str) -> bool {
    let trimmed = body.trim();
    if trimmed.is_empty() {
        return false;
    }
    let Ok(json) = serde_json::from_str::<Value>(trimmed) else {
        // 非 JSON 200 body：保守视为有效（避免把上游非标准但实质有内容的响应误判为空）
        return true;
    };
    // error 结构（顶层 error 字段 / type==error）→ 无效
    if json.get("error").is_some()
        || json.get("type").and_then(|v| v.as_str()) == Some("error")
    {
        return false;
    }
    // openai 风格：choices 非空且含实质内容（message/content/text/delta/tool_calls）
    if let Some(choices) = json.get("choices").and_then(|v| v.as_array()) {
        return choices.iter().any(|c| {
            c.get("message").is_some()
                || c.get("text").is_some()
                || c.get("delta").is_some()
        });
    }
    // anthropic 风格：content 数组非空
    if let Some(content) = json.get("content").and_then(|v| v.as_array()) {
        return !content.is_empty();
    }
    // 其他形态（如 openai responses `output` 等）：非 error 且 JSON 有内容 → 视为有效
    json.as_object().map(|o| !o.is_empty()).unwrap_or(false)
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
    // passthrough 透明 relay：禁用总超时——reqwest .timeout 覆盖「连接→响应头→body 全部读完」，
    // 会砍断长 SSE 流（thinking/tool_use body 读取 > request_timeout_secs）致无 message_stop → 客户端
    // JSON Parse error / 内容残缺。透传语义上不替客户端施加任意 body 超时；connect_timeout 仍保护连接期，
    // 客户端自有超时兜底，上游真断由 stream-error-graceful-passthrough 合成 message_stop 兜底。
    let req_timeout = 0u64;
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
            if is_sensitive_auth_header(name) {
                h.insert(name.to_string(), Value::String("[REDACTED]".into()));
            } else if let Ok(s) = v.to_str() {
                h.insert(name.to_string(), Value::String(s.to_string()));
            }
        }
        Value::Object(h).to_string()
    };
    log.upstream_request_body = String::from_utf8_lossy(&bytes).to_string();
    tracing::info!(method = %orig_method, url = %url, "passthrough upstream request");
    tracing::debug!(method = %orig_method, url = %url, body = %super::log_util::log_body_preview(&log.upstream_request_body), "passthrough upstream request body");

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
    // 透传 user_response_body == upstream 原文：当 log_user_request 开启时（record_client_body=true）
    // 闭包把上游 chunk 同步 push 进 client_body，flush 即从 client_body 写 user_response_body。
    // 故 guard.record_client_body 必须 == record_client_body（曾误设 false，导致 flush 跳过
    // user_response_body 回写，透传日志的 user_response_body 永不落内容）。
    let passthrough_user_body = record_client_body;
    let guard = StreamLogGuard {
        agg: agg.clone(),
        est_fired: est_fired.clone(),
        log: log.clone(),
        state: state.clone(),
        settings: log_settings.clone(),
        start,
        record_upstream_body,
        record_client_body: passthrough_user_body,
        req_span: req_span.clone(),
        // 透传分支历史上不做请求驱动预估，保持现状
        est: None,
    };

    // guard 被 move 进闭包；stream 被 Drop（含客户端断连）时 guard.drop 触发兜底 flush。
    let stream = resp.bytes_stream().map(move |chunk_result| {
        let chunk = match chunk_result {
            Ok(c) => c,
            Err(e) => {
                // 上游流中途断裂：不向客户端报错（避免 CC "error decoding response body"），
                // 仅记日志 + 合成 anthropic message_stop 干净收尾（claude_code relay wire = anthropic）。
                tracing::warn!(error = %e, "passthrough upstream stream chunk error; closing stream gracefully");
                return Ok::<_, std::io::Error>(Bytes::from(
                    "event: message_stop\ndata: {\"type\":\"message_stop\"}\n\n",
                ));
            }
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
        // 尽力从 SSE data 累计 usage（Anthropic / OpenAI 兼容字段，含 message.usage 兜底），不改写 chunk。
        // 跨 chunk 行重组：data: 行被切到两个 chunk 时逐 chunk .lines() 会丢 usage。
        let text = String::from_utf8_lossy(&chunk);
        guard.agg.feed_sse_usage(&text);
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

/// 模型列表端点 relay：选分组首个启用平台，按平台协议拉上游 /models 并原样 relay status + body。
/// 不做 model mapping / 重试 / 转换（模型列表无此语义，取第一个可用平台即可）。
/// 鉴权注入平台凭证（非客户端 group token，上游不认）；URL 遵 url-construction-rule。
async fn handle_models_passthrough(
    state: &Arc<ProxyState>,
    log: &mut ProxyLog,
    log_settings: &ProxyLogSettings,
    group: &Group,
    start: std::time::Instant,
    lang: Lang,
) -> Response {
    // 选分组首个启用平台（endpoint 优先取首个端点协议/URL，否则平台主配置）。
    // Mock 平台无真实上游（base_url 空），不能 relay 模型列表 —— 跳过，否则
    // build_models_url 产无 scheme 的相对 URL，reqwest .send() → builder error → 502。
    let group_platforms = match super::db::get_group_platforms(&state.db, group.id).await {
        Ok(p) => p,
        Err(e) => {
            tracing::warn!(group = %group.name, error = %e, "models: get_group_platforms failed");
            log.response_body = format!("group platforms error: {e}");
            log.status_code = 503;
            log.duration_ms = start.elapsed().as_millis() as i32;
            upsert_log(state, log, log_settings).await;
            return (StatusCode::SERVICE_UNAVAILABLE, format!("{}: {e}", i18n::t(lang, ErrorKey::Route))).into_response();
        }
    };
    let platform = match group_platforms
        .iter()
        .find(|gp| gp.platform.enabled && !matches!(gp.platform.platform_type, Protocol::Mock))
    {
        Some(gp) => gp.platform.clone(),
        None => {
            tracing::warn!(group = %group.name, "models: no enabled upstream platform in group (mock skipped)");
            log.response_body = "no enabled upstream platform for models endpoint".to_string();
            log.status_code = 503;
            log.duration_ms = start.elapsed().as_millis() as i32;
            upsert_log(state, log, log_settings).await;
            return (StatusCode::SERVICE_UNAVAILABLE, i18n::t(lang, ErrorKey::Route)).into_response();
        }
    };

    // endpoint 优先（首个端点协议/URL），否则平台主配置。api_key 始终取平台凭证。
    let (protocol, base_url) = if let Some(ep) = platform.endpoints.first() {
        (ep.protocol.clone(), ep.base_url.clone())
    } else {
        (platform.platform_type.clone(), platform.base_url.clone())
    };
    let url = build_models_url(&protocol, &base_url);

    log.platform_id = platform.id;
    log.target_protocol = format!("{:?}", protocol).to_lowercase();
    log.upstream_request_url = url.clone();
    log.upstream_request_headers = r#"{"authorization":"[REDACTED]"}"#.to_string();

    let system_timeout = get_system_timeout(&state.db).await;
    let req_timeout = if system_timeout.request_timeout_secs > 0 { system_timeout.request_timeout_secs } else { 60 };
    let conn_timeout = if system_timeout.connect_timeout_secs > 0 { system_timeout.connect_timeout_secs } else { 10 };
    let client = super::http_client::build_http_client(&state.db, req_timeout, conn_timeout, None, None).await;

    // OpenCode Zen 同款兜底：/v1/models 无 auth 也能列，留空时注入 $opencode 与 chat 路径一致。
    let models_api_key = resolve_opencode_zen_key(&platform);
    let rb = apply_models_auth(client.get(&url), &protocol, &models_api_key);
    tracing::info!(group = %group.name, platform = %platform.name, url = %url, "models endpoint upstream request");

    let resp = match rb.send().await {
        Ok(r) => r,
        Err(e) => {
            tracing::error!(url = %url, error = %e, "models endpoint upstream request failed (502)");
            log.response_body = format!("upstream error: {e}");
            log.status_code = 502;
            log.upstream_status_code = 0;
            log.user_response_body = format!("{}: {e}", i18n::t(lang, ErrorKey::Upstream));
            log.duration_ms = start.elapsed().as_millis() as i32;
            upsert_log(state, log, log_settings).await;
            return (StatusCode::BAD_GATEWAY, format!("{}: {e}", i18n::t(lang, ErrorKey::Upstream))).into_response();
        }
    };

    let status = resp.status();
    log.upstream_status_code = status.as_u16() as i32;
    let content_type = resp
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("application/json")
        .to_string();
    let body = resp.bytes().await.unwrap_or_default();
    let body_str = String::from_utf8_lossy(&body).to_string();

    log.status_code = status.as_u16() as i32;
    log.response_body = body_str.clone();
    log.user_response_body = body_str;
    log.user_response_headers = format!(r#"{{"content-type":"{}"}}"#, content_type);
    log.duration_ms = start.elapsed().as_millis() as i32;
    tracing::info!(url = %url, status = status.as_u16(), "models endpoint upstream responded");
    upsert_log(state, log, log_settings).await;

    let resp_status = StatusCode::from_u16(status.as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);
    let mut response = (resp_status, body.to_vec()).into_response();
    if let Ok(hv) = axum::http::HeaderValue::from_str(&content_type) {
        response.headers_mut().insert(axum::http::header::CONTENT_TYPE, hv);
    }
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

/// 判定请求 path（已含 group/proxy 前缀）是否为模型列表端点。
/// strip 任意前缀后尾段为 `/v1/models` | `/models`（openai/anthropic 同名）→ true。
/// gemini `/v1beta/models` 本期不在代理 relay 范围（标 TODO，见 prd 失败处理）。
fn is_models_endpoint(path: &str) -> bool {
    let p = path.trim_end_matches('/');
    // gemini /v1beta/models 本期不在代理 relay 范围（鉴权/响应格式不同），显式排除。
    if p.contains("/v1beta/") {
        return false;
    }
    p.ends_with("/v1/models") || p.ends_with("/models")
}

/// 按平台协议构造上游模型列表端点 URL（遵 url-construction-rule：base_url 已含版本前缀，仅 trim 尾 `/` + 端点后缀，禁额外拼版本）。
/// 三类后缀：Anthropic → `/v1/models`（base_url 通常不含 /v1）；Bailian → `/compatible-mode/v1/models`；
/// 其余 OpenAI 兼容（含 glm `.../api/paas/v4`、openai `.../v1`）→ `/models`。
/// 与 lib.rs `platform_fetch_models` 单一事实源，避免按协议拉 /models 的 URL 构造重复腐化。
pub fn build_models_url(protocol: &Protocol, base_url: &str) -> String {
    let base = base_url.trim_end_matches('/');
    match protocol {
        Protocol::Anthropic => format!("{base}/v1/models"),
        Protocol::Bailian => format!("{base}/compatible-mode/v1/models"),
        _ => format!("{base}/models"),
    }
}

/// 按平台协议给上游模型列表请求注入鉴权头（平台凭证，非客户端 group token）。
/// Anthropic → `x-api-key` + `anthropic-version`；其余 OpenAI 兼容 → `Authorization: Bearer`。
/// 与 lib.rs `platform_fetch_models` 鉴权风格对齐。
pub fn apply_models_auth(
    rb: reqwest::RequestBuilder,
    protocol: &Protocol,
    api_key: &str,
) -> reqwest::RequestBuilder {
    match protocol {
        Protocol::Anthropic => rb
            .header("x-api-key", api_key)
            .header("anthropic-version", "2023-06-01"),
        // openai/兼容：Bearer 之外叠加 api-key 头（小米 token-plan openai 端点要求），其他上游忽略未知头。
        _ => rb
            .header("Authorization", format!("Bearer {api_key}"))
            .header("api-key", api_key),
    }
}

/// 判定请求 path（已含 group/proxy 前缀）是否为 Responses API **子端点**（非 create）。
/// strip `/proxy`+group 前缀后 api_path 以 `/v1/responses/`（**带尾斜杠 + 后续段**）开头 → true。
/// 精确放行 create：裸 `/v1/responses`（含末尾单斜杠 `/v1/responses/` 但无后续段）→ false，不拦。
/// 覆盖：/v1/responses/compact、/v1/responses/{id}、/v1/responses/{id}/cancel、/v1/responses/{id}/input_items。
/// 与 detect_source_protocol 同款 strip（path.find("/v1/")），无 /v1/ 前缀 → 非 responses 子端点。
fn is_responses_subendpoint(path: &str) -> bool {
    let api_path = match path.find("/v1/") {
        Some(idx) => &path[idx..],
        None => return false,
    };
    // strip 末尾斜杠后，必须严格长于裸 `/v1/responses`（即 `/v1/responses/<seg>...`）才算子端点。
    // 裸 `/v1/responses` 或 `/v1/responses/`（create，无后续段）→ false。
    let trimmed = api_path.trim_end_matches('/');
    trimmed.starts_with("/v1/responses/") && trimmed.len() > "/v1/responses".len()
}

/// Responses API 子端点透传：选分组首个支持 responses 的平台，原样转发 method/body 到上游 + 平台凭证。
/// 不做转换 / model mapping / 重试（子端点是对上游 response 对象的操作，无 chat 语义）。
/// 平台选择：分组首个 enabled 且 endpoint 协议含 OpenAIResponses 的平台；无则回退首个 enabled 平台。
/// 上游 URL：取该平台 responses 端点 base_url + 子路径（api_path 去 `/v1` 前缀，如 `/responses/{id}/cancel`），
///   镜像 create same_protocol_passthrough 的 `base_url.trim_end('/') + api_path` 构造，base_url 已含 /v1 禁重复拼。
/// 鉴权：平台凭证 `Authorization: Bearer <api_key>` + `OpenAI-Beta: responses=experimental`（不透传客户端 group token）。
/// 已知限制：response_id→platform 无持久映射，多 responses 平台分组下取首个平台，若 create 落到非首个 → 上游可能 404
///   （单 responses 平台分组安全，Codex 常见场景）。此限制在 prd 失败处理已标注，log 记录真实 status。
#[allow(clippy::too_many_arguments)]
async fn handle_responses_subendpoint(
    state: &Arc<ProxyState>,
    log: &mut ProxyLog,
    log_settings: &ProxyLogSettings,
    group: &Group,
    orig_method: &axum::http::Method,
    bytes: &[u8],
    path: &str,
    start: std::time::Instant,
    lang: Lang,
) -> Response {
    log.source_protocol = "openai_responses".to_string();
    log.target_protocol = "openai_responses".to_string();

    // 分组平台列表
    let group_platforms = match super::db::get_group_platforms(&state.db, group.id).await {
        Ok(p) => p,
        Err(e) => {
            tracing::warn!(group = %group.name, error = %e, "responses subendpoint: get_group_platforms failed");
            log.response_body = format!("group platforms error: {e}");
            log.status_code = 503;
            log.duration_ms = start.elapsed().as_millis() as i32;
            upsert_log(state, log, log_settings).await;
            return (StatusCode::SERVICE_UNAVAILABLE, format!("{}: {e}", i18n::t(lang, ErrorKey::Route))).into_response();
        }
    };

    // 平台选择：首个 enabled 且含 OpenAIResponses 端点的平台 → 取其 responses 端点 base_url。
    // 回退：首个 enabled 平台（取其首个端点或平台主配置 base_url）。
    let selected = group_platforms.iter().find_map(|gp| {
        if !gp.platform.enabled {
            return None;
        }
        gp.platform
            .endpoints
            .iter()
            .find(|ep| matches!(ep.protocol, Protocol::OpenAIResponses))
            .map(|ep| (gp.platform.clone(), ep.base_url.clone()))
    });
    let (platform, base_url) = match selected {
        Some(p) => p,
        None => {
            // 回退：首个 enabled 平台
            match group_platforms.iter().find(|gp| gp.platform.enabled) {
                Some(gp) => {
                    let base = gp
                        .platform
                        .endpoints
                        .first()
                        .map(|ep| ep.base_url.clone())
                        .unwrap_or_else(|| gp.platform.base_url.clone());
                    (gp.platform.clone(), base)
                }
                None => {
                    tracing::warn!(group = %group.name, "responses subendpoint: no enabled platform in group");
                    log.response_body = "no responses-capable or enabled platform for responses subendpoint".to_string();
                    log.status_code = 503;
                    log.duration_ms = start.elapsed().as_millis() as i32;
                    upsert_log(state, log, log_settings).await;
                    return (StatusCode::SERVICE_UNAVAILABLE, i18n::t(lang, ErrorKey::Route)).into_response();
                }
            }
        }
    };

    // 上游子路径：api_path（strip /proxy+group 前缀，同 detect_source_protocol）去 `/v1` 前缀。
    // base_url 已含版本前缀（如 .../v1）→ 子路径只保留 `/responses/...`，禁重复拼 /v1（url-construction-rule）。
    let api_path = match path.find("/v1/") {
        Some(idx) => &path[idx..],
        None => path,
    };
    let sub_path = api_path.strip_prefix("/v1").unwrap_or(api_path);
    let url = format!("{}{}", base_url.trim_end_matches('/'), sub_path);

    log.platform_id = platform.id;
    log.upstream_request_url = url.clone();
    log.upstream_request_headers = r#"{"authorization":"[REDACTED]","openai-beta":"responses=experimental"}"#.to_string();

    let system_timeout = get_system_timeout(&state.db).await;
    let req_timeout = if system_timeout.request_timeout_secs > 0 { system_timeout.request_timeout_secs } else { 60 };
    let conn_timeout = if system_timeout.connect_timeout_secs > 0 { system_timeout.connect_timeout_secs } else { 10 };
    let client = super::http_client::build_http_client(&state.db, req_timeout, conn_timeout, Some(&platform.extra), None).await;

    // 保留原始 method + 原样转发 body（GET/DELETE 无 body；POST cancel/compact 原样）。
    let mut rb = client
        .request(orig_method.clone(), &url)
        .header("Authorization", format!("Bearer {}", platform.api_key))
        .header("OpenAI-Beta", "responses=experimental");
    if !bytes.is_empty() {
        rb = rb.header("Content-Type", "application/json").body(bytes.to_vec());
        log.upstream_request_body = String::from_utf8_lossy(bytes).to_string();
    }
    tracing::info!(group = %group.name, platform = %platform.name, method = %orig_method, url = %url, "responses subendpoint upstream request");

    let resp = match rb.send().await {
        Ok(r) => r,
        Err(e) => {
            tracing::error!(url = %url, error = %e, "responses subendpoint upstream request failed (502)");
            log.response_body = format!("upstream error: {e}");
            log.status_code = 502;
            log.upstream_status_code = 0;
            log.user_response_body = format!("{}: {e}", i18n::t(lang, ErrorKey::Upstream));
            log.duration_ms = start.elapsed().as_millis() as i32;
            upsert_log(state, log, log_settings).await;
            return (StatusCode::BAD_GATEWAY, format!("{}: {e}", i18n::t(lang, ErrorKey::Upstream))).into_response();
        }
    };

    let status = resp.status();
    log.upstream_status_code = status.as_u16() as i32;
    let content_type = resp
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("application/json")
        .to_string();
    let body = resp.bytes().await.unwrap_or_default();
    let body_str = String::from_utf8_lossy(&body).to_string();

    log.status_code = status.as_u16() as i32;
    log.response_body = body_str.clone();
    log.user_response_body = body_str;
    log.user_response_headers = format!(r#"{{"content-type":"{}"}}"#, content_type);
    log.duration_ms = start.elapsed().as_millis() as i32;
    tracing::info!(url = %url, status = status.as_u16(), "responses subendpoint upstream responded");
    upsert_log(state, log, log_settings).await;

    let resp_status = StatusCode::from_u16(status.as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);
    let mut response = (resp_status, body.to_vec()).into_response();
    if let Ok(hv) = axum::http::HeaderValue::from_str(&content_type) {
        response.headers_mut().insert(axum::http::header::CONTENT_TYPE, hv);
    }
    response
}

/// 判定请求 path（已含 group/proxy 前缀）是否为 Anthropic count_tokens 端点。
/// strip 任意前缀后尾段为 `/v1/messages/count_tokens`（可带末尾斜杠）→ true。
/// 与 is_responses_subendpoint 同款 strip（path.find("/v1/")），无 /v1/ 前缀 → false。
fn is_count_tokens_endpoint(path: &str) -> bool {
    let api_path = match path.find("/v1/") {
        Some(idx) => &path[idx..],
        None => return false,
    };
    api_path
        .trim_end_matches('/')
        .ends_with("/v1/messages/count_tokens")
}

/// 本地近似估算 anthropic count_tokens body 的 input_tokens（透传失败兜底）。
/// 启发式：累计 system + 全部 messages 文本 + tools 定义的字符数，按 ~4 字符/token 折算
/// （英文经验值；中文偏低但 count_tokens 仅用于客户端预估，不参与计费，可接受偏差）。
/// 拿不到任何文本字段 → 返回保底 1（避免返回 0 误导客户端流程）。
fn estimate_input_tokens(body: &Value) -> i64 {
    fn collect_text(v: &Value, acc: &mut usize) {
        match v {
            Value::String(s) => *acc += s.len(),
            Value::Array(arr) => arr.iter().for_each(|e| collect_text(e, acc)),
            Value::Object(map) => map.values().for_each(|e| collect_text(e, acc)),
            _ => {}
        }
    }
    let mut chars = 0usize;
    if let Some(obj) = body.as_object() {
        for key in ["system", "messages", "tools"] {
            if let Some(v) = obj.get(key) {
                collect_text(v, &mut chars);
            }
        }
    }
    let tokens = chars.div_ceil(4) as i64;
    tokens.max(1)
}

/// Anthropic `/v1/messages/count_tokens` 子端点：透传优先 + 本地估算兜底（方案 X）。
/// 1. 复用 select_candidates_ctx 选首选平台 + 拿模型映射（claude-opus-4-8 → glm-5.1）。
/// 2. 取该平台 anthropic 端点 base_url（无则回退平台主 base_url），URL = base_url + `/v1/messages/count_tokens`
///    （遵 url-construction-rule：anthropic base_url 不含 /v1，仅拼 endpoint 后缀，与 build_models_url 同款）。
/// 3. 透传客户端原始 body（仅 patch model 字段为路由目标模型），x-api-key + anthropic-version 鉴权 POST。
/// 4. 上游 2xx → 原样回客户端（anthropic count_tokens 响应 schema）。
/// 5. 上游 4xx/5xx 或连接失败（平台不支持该端点）→ 本地估算 `{"input_tokens": N}` 返 200，
///    不返回错误，避免 claude-cli 预估流程被上游 500/404 阻断。
///
/// proxy_log：source/target protocol=anthropic，upstream_request_url 含尾段，status 记真实结果。
async fn handle_count_tokens(
    state: &Arc<ProxyState>,
    log: &mut ProxyLog,
    log_settings: &ProxyLogSettings,
    group: &Group,
    bytes: &[u8],
    start: std::time::Instant,
) -> Response {
    log.source_protocol = "anthropic".to_string();
    log.target_protocol = "anthropic".to_string();

    // 原始 body（用于透传 + 估算兜底）+ 入站 model
    let raw_body: Value = serde_json::from_slice(bytes).unwrap_or(Value::Null);
    let requested_model = raw_body
        .get("model")
        .and_then(|m| m.as_str())
        .unwrap_or_default()
        .to_string();
    log.model = requested_model.clone();

    // 本地估算值（透传失败时回客户端；提前算好，避免分支重复）
    let est_tokens = estimate_input_tokens(&raw_body);
    let est_body = serde_json::json!({ "input_tokens": est_tokens }).to_string();
    // 兜底响应：返回本地估算 `{"input_tokens":N}` 200，并把回客户端正文记入 log.user_response_body
    // （与 handle_responses_subendpoint 成功路径一致：客户端实际收到的正文落库）。
    let est_response = |body: &str| -> Response {
        (
            StatusCode::OK,
            [(axum::http::header::CONTENT_TYPE, "application/json")],
            body.to_string(),
        )
            .into_response()
    };
    // 在各兜底分支统一回写 log 的客户端响应正文/头（est_response 闭包不可借 &mut log，故在调用点写 log）。
    macro_rules! fallback_log {
        () => {{
            log.input_tokens = est_tokens as i32;
            log.user_response_body = est_body.clone();
            log.user_response_headers = r#"{"content-type":"application/json"}"#.to_string();
            log.duration_ms = start.elapsed().as_millis() as i32;
        }};
    }

    // 路由选平台（复用 group→platform 选择，拿模型映射目标）
    let sched_settings = super::db::get_scheduling_settings(&state.db).await;
    let sched_ctx = ScheduleCtx {
        scheduler: &state.scheduler,
        sticky: &state.sticky,
        settings: &sched_settings,
        sticky_key: Some(format!("{}|count_tokens", group.group_key)),
    };
    let candidate_set =
        match select_candidates_ctx(&state.db, group, &requested_model, Some(&sched_ctx)).await {
            Ok(c) => c,
            Err(e) => {
                // 路由失败 → 本地估算兜底（不阻断 claude-cli）
                tracing::warn!(group = %group.name, model = %requested_model, error = %e, "count_tokens: route failed, falling back to local estimate");
                log.status_code = 200;
                log.response_body = format!("route error (local estimate fallback): {e}");
                fallback_log!();
                upsert_log(state, log, log_settings).await;
                return est_response(&est_body);
            }
        };
    let route = match candidate_set.candidates.into_iter().next() {
        Some(r) => r,
        None => {
            tracing::warn!(group = %group.name, "count_tokens: no candidate platform, local estimate fallback");
            log.status_code = 200;
            log.response_body = "no candidate platform (local estimate fallback)".to_string();
            fallback_log!();
            upsert_log(state, log, log_settings).await;
            return est_response(&est_body);
        }
    };

    let actual_model = route.target_model.clone();
    log.platform_id = route.platform.id;
    log.actual_model = actual_model.clone();

    // 取 anthropic 端点 base_url（无则回退平台主 base_url）
    let base_url = route
        .platform
        .endpoints
        .iter()
        .find(|ep| matches!(ep.protocol, Protocol::Anthropic))
        .map(|ep| ep.base_url.clone())
        .unwrap_or_else(|| route.platform.base_url.clone());
    // URL：base_url + /v1/messages/count_tokens（anthropic base_url 不含 /v1，与 build_models_url 同款拼接）
    let url = format!(
        "{}/v1/messages/count_tokens",
        base_url.trim_end_matches('/')
    );
    log.upstream_request_url = url.clone();
    log.upstream_request_headers =
        r#"{"x-api-key":"[REDACTED]","anthropic-version":"2023-06-01"}"#.to_string();

    // 透传 body：仅 patch model 字段为路由目标模型
    let mut upstream_body = raw_body.clone();
    if let Some(obj) = upstream_body.as_object_mut() {
        obj.insert("model".to_string(), Value::String(actual_model.clone()));
    }
    let upstream_body_str = serde_json::to_string(&upstream_body).unwrap_or_default();
    log.upstream_request_body = format_pretty_json(&upstream_body_str);

    let system_timeout = get_system_timeout(&state.db).await;
    let req_timeout = if system_timeout.request_timeout_secs > 0 { system_timeout.request_timeout_secs } else { 60 };
    let conn_timeout = if system_timeout.connect_timeout_secs > 0 { system_timeout.connect_timeout_secs } else { 10 };
    let client = super::http_client::build_http_client(
        &state.db, req_timeout, conn_timeout, Some(&route.platform.extra), None,
    )
    .await;

    let rb = client
        .post(&url)
        .header("Content-Type", "application/json")
        .header("x-api-key", &route.platform.api_key)
        .header("anthropic-version", "2023-06-01")
        .body(upstream_body_str.clone());
    tracing::info!(group = %group.name, platform = %route.platform.name, model = %actual_model, url = %url, "count_tokens upstream request");

    let resp = match rb.send().await {
        Ok(r) => r,
        Err(e) => {
            // 连接失败 / 超时 → 本地估算兜底（不阻断 claude-cli）
            tracing::warn!(url = %url, error = %e, "count_tokens upstream request failed, local estimate fallback");
            log.upstream_status_code = 0;
            log.status_code = 200;
            log.response_body = format!("upstream error (local estimate fallback): {e}");
            fallback_log!();
            upsert_log(state, log, log_settings).await;
            return est_response(&est_body);
        }
    };

    let status = resp.status();
    log.upstream_status_code = status.as_u16() as i32;
    let body = resp.bytes().await.unwrap_or_default();
    let body_str = String::from_utf8_lossy(&body).to_string();

    if status.is_success() {
        // 上游支持 count_tokens → 原样回客户端真实值
        log.status_code = status.as_u16() as i32;
        log.response_body = body_str.clone();
        log.user_response_body = body_str;
        log.user_response_headers = r#"{"content-type":"application/json"}"#.to_string();
        log.input_tokens = serde_json::from_slice::<Value>(&body)
            .ok()
            .and_then(|v| v.get("input_tokens").and_then(|t| t.as_i64()))
            .unwrap_or(0) as i32;
        log.duration_ms = start.elapsed().as_millis() as i32;
        tracing::info!(url = %url, status = status.as_u16(), "count_tokens upstream responded (passthrough)");
        upsert_log(state, log, log_settings).await;
        let mut response = (StatusCode::OK, body.to_vec()).into_response();
        response.headers_mut().insert(
            axum::http::header::CONTENT_TYPE,
            axum::http::HeaderValue::from_static("application/json"),
        );
        return response;
    }

    // 上游不支持该端点（4xx/5xx）→ 本地估算兜底，返回 200 而非透传错误
    tracing::warn!(url = %url, upstream_status = status.as_u16(), "count_tokens upstream unsupported, local estimate fallback");
    log.status_code = 200;
    log.response_body = format!("upstream {} (local estimate fallback): {}", status.as_u16(), body_str);
    fallback_log!();
    upsert_log(state, log, log_settings).await;
    est_response(&est_body)
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

/// hop-by-hop + 强覆盖头名（convert 路径透传时剔除）。
/// host / content-length / 标准 hop-by-hop（RFC 7230 §6.1）交给 reqwest 按目标重设；
/// auth 三件套 / user-agent / content-type 由 apply_client_headers 用平台配置覆盖，
/// 故透传底座剔除，避免同名 append 造成多值。
const STRIPPED_ON_CONVERT_PASSTHROUGH: &[&str] = &[
    "host",
    "content-length",
    "connection",
    "keep-alive",
    "proxy-authenticate",
    "proxy-authorization",
    "te",
    "trailer",
    "trailers",
    "transfer-encoding",
    "upgrade",
    "authorization",
    "x-api-key",
    "x-goog-api-key",
    "user-agent",
    "content-type",
];

/// 鉴权凭证头名（proxy_log 脱敏判定，不区分大小写）。
/// `api-key` 系小米 token-plan openai 端点要求的鉴权头（与 Authorization 同发），属凭证须 redact。
const SENSITIVE_AUTH_HEADERS: &[&str] = &[
    "authorization",
    "api-key",
    "x-api-key",
    "x-goog-api-key",
];

/// 判定 header 是否为需脱敏的鉴权凭证头（不区分大小写）。
fn is_sensitive_auth_header(name: &str) -> bool {
    SENSITIVE_AUTH_HEADERS.iter().any(|h| name.eq_ignore_ascii_case(h))
}

/// convert 路径透传入站头底座：全量入站头，剔 hop-by-hop + auth/UA/CT（由 apply 覆盖）。
/// 其余（anthropic-* / x-stainless-* / x-app / session-id / originator / version / 未知自定义头）
/// 原样透传 —— 跨协议（如 CC 入站转 OpenAI）也带，上游忽略未知头不报错，保留利于诊断。
fn passthrough_convert_headers(orig: &axum::http::HeaderMap) -> reqwest::header::HeaderMap {
    let mut out = reqwest::header::HeaderMap::new();
    for (k, v) in orig {
        let name = k.as_str();
        if STRIPPED_ON_CONVERT_PASSTHROUGH.iter().any(|s| name.eq_ignore_ascii_case(s)) {
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
    // SSE 行重组缓冲：网络 chunk 边界与 SSE event 边界不对齐，单个 `data:` 行可能被
    // 切到两个 reqwest chunk。逐 chunk `.lines()` 解析会把尾部不完整行喂给 serde 解析失败
    // 静默丢弃 usage（尤其 anthropic 尾部 message_delta 携带最终 input/output_tokens 时）。
    // 此缓冲保留每个 chunk 末尾未以换行结束的残行，拼到下个 chunk 头部，保证 usage 解析始终见完整行。
    sse_line_buf: std::sync::Mutex<String>,
}

impl StreamAggregator {
    fn new() -> Self {
        Self {
            upstream_body: std::sync::Mutex::new(Vec::new()),
            client_body: std::sync::Mutex::new(Vec::new()),
            tokens_in: std::sync::atomic::AtomicI32::new(0),
            tokens_out: std::sync::atomic::AtomicI32::new(0),
            tokens_cache: std::sync::atomic::AtomicI32::new(0),
            sse_line_buf: std::sync::Mutex::new(String::new()),
        }
    }

    /// 从一个网络 chunk 的文本累计 SSE usage，跨 chunk 边界重组 `data:` 行。
    /// 仅用于 usage 提取，不影响向客户端 relay 的原始字节。
    /// 缓冲未以换行结束的尾部残行，拼到后续 chunk；遇 `[DONE]`/解析失败的行静默跳过。
    fn feed_sse_usage(&self, text: &str) {
        let mut buf = match self.sse_line_buf.lock() {
            Ok(b) => b,
            Err(_) => return,
        };
        buf.push_str(text);
        // 末尾若无换行，说明最后一行可能被切断 → 保留为残行，仅处理已完整行。
        let ends_complete = buf.ends_with('\n');
        let mut remainder = String::new();
        let mut lines: Vec<String> = buf.split('\n').map(|s| s.to_string()).collect();
        if !ends_complete {
            // 最后一段是不完整残行，留到下次。
            remainder = lines.pop().unwrap_or_default();
        }
        for line in &lines {
            let line = line.trim();
            if let Some(data) = line.strip_prefix("data: ") {
                let data = data.trim();
                if data == "[DONE]" {
                    continue;
                }
                if let Ok(json) = serde_json::from_str::<Value>(data) {
                    accumulate_sse_usage(&json, &self.tokens_in, &self.tokens_out, &self.tokens_cache);
                }
            }
        }
        *buf = remainder;
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
    extra: String,
    coding_plan: bool,
}

impl StreamLogGuard {
    /// 若 chunk 文本含 SSE 终止标记则触发 flush（确定性回写，不依赖 Drop 兜底）。
    /// 覆盖两类协议终止符：
    ///   - OpenAI / 兼容：`data: [DONE]`
    ///   - Anthropic：`event: message_stop`（含 `data: {"type":"message_stop"}`）—— 原生
    ///     Anthropic 流**不发 `[DONE]`**，仅以 message_stop 收尾。漏检此标记会使 anthropic→anthropic
    ///     透传流仅靠 Drop 兜底回写；Drop 内 `tokio::spawn` 在连接 abort 时序下偶发丢写，
    ///     导致 response_body 永久停在 `[stream]` 占位（见修复）。
    ///
    /// 正常结束走此路径回写（token 已累加完整）；仍未命中（如上游中途断裂无终止符）由 Drop 兜底。
    fn flush_if_done(&self, text: &str) {
        for line in text.lines() {
            let line = line.trim();
            if let Some(data) = line.strip_prefix("data: ") {
                let data = data.trim();
                if data == "[DONE]" {
                    self.flush();
                    return;
                }
                // Anthropic message_stop 也可能以 data 行携带 type 字段出现
                if data.contains("\"type\":\"message_stop\"")
                    || data.contains("\"type\": \"message_stop\"")
                {
                    self.flush();
                    return;
                }
            }
            // SSE event 行形式：`event: message_stop`
            if let Some(ev) = line.strip_prefix("event: ") {
                if ev.trim() == "message_stop" {
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
        let task = async move {
            let id = final_log.id.clone();
            upsert_log(&upsert_state, &final_log, &upsert_settings).await;
            // 流式终态：移除 in-flight 列快照，防 map 无限增长。
            remove_log_snapshot(&upsert_state, &id);
        }
        .instrument(span);
        // 经显式 runtime handle 落库：Drop（含客户端 abort / 连接 teardown）路径下
        // 裸 `tokio::spawn` 可能不在 runtime 上下文 → panic 被 Drop 吞掉、最终态丢写
        // （response_body 停在 `[stream]` 占位）。捕获 handle 后 spawn 始终落到 runtime，
        // 保证 flush 在所有收尾路径（[DONE] / message_stop / Drop 兜底）确定性回写。
        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            handle.spawn(task);
        } else {
            tracing::warn!(
                "stream flush: no tokio runtime in scope, final log write skipped (response_body may stay placeholder)"
            );
        }

        if let Some(est) = &self.est {
            spawn_estimate(
                &self.state,
                est.platform_id,
                &est.platform_type,
                est.base_url.clone(),
                est.api_key.clone(),
                est.model.clone(),
                est.extra.clone(),
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
///
/// 用 fetch_max（只增不减）而非 store（覆盖）：Anthropic 流式语义下 input/cache 在
/// `message_start` 起始即定值，但后续 `message_delta`（及中转站尾部汇总事件）常携带
/// `input_tokens: 0`，store 覆盖会把真实 input 清零。output 在 message_delta 里是累计值，
/// 取流中最大即终值。OpenAI 末尾一次性给全量，从 0 升上去同样安全。
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
        acc_in.fetch_max(i as i32, Relaxed);
    }
    if let Some(o) = usage
        .get("output_tokens")
        .or_else(|| usage.get("completion_tokens"))
        .and_then(|v| v.as_i64())
    {
        acc_out.fetch_max(o as i32, Relaxed);
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
        acc_cache.fetch_max(c as i32, Relaxed);
    }
}

/// Extract input/output/cache tokens from non-stream response JSON
/// 流式判定：请求 body 的 stream 字段与上游响应 content-type 取并。
/// 中转站常对未声明 stream 的请求强制以 `text/event-stream` 响应；仅凭请求字段会误判为非流式，
/// 进而用 JSON 解析 SSE 文本拿不到 usage → token/est_cost 全为 0。OR 语义保证既有流式路径不回归。
fn resolve_is_stream(req_stream: bool, upstream_content_type: &str) -> bool {
    req_stream || upstream_content_type.contains("text/event-stream")
}

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
    // 定位到 /v1/ 起始（跳过代理根前缀如 /proxy）；分组路由已纯按 apikey，无 group path 前缀
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

/// 按入站 User-Agent 推断客户端"原生" wire 协议（仅用于 UA 透传分支，见 [protocol-same-proto-passthrough] 扩展）。
///
/// 复用现有出站合成 UA 的子串特征规则（`claude_code_ua` / `codex_ua`）应用到入站匹配：
/// - 含 `claude-cli`（Claude Code CLI/VSCode/SDK/GhAction 全家族）→ `"anthropic"`
/// - 含 `codex`（codex_cli_rs / Codex/ / codex desktop / codex-vscode 全家族）→ `"openai_responses"`
/// - 其它（Cursor / Windsurf / gemini-cli / 未知 / 缺失）→ None（回退现有处理）
///
/// 大小写不敏感（Codex TUI UA 为 `Codex/...`，需匹配 `codex`）。返回的字面量与
/// `detect_source_protocol` / `ep_proto` 产出的协议名一致，便于直接比对 endpoint。
/// 按入站协议(`source_protocol`)从平台端点中选目标 endpoint。
///
/// 通用原则：**尽可能用原协议直发，避免有损转换**（[protocol-same-proto-passthrough]）。
/// 优先级链（从最优到兜底）：
///   1. coding_plan 端点中按入站协议精确匹配（同协议 coding，直发不转换）
///      —— 平台同时含多个 coding 端点（如 GLM/千帆/小米：openai coding + anthropic coding）时，
///      anthropic 入站选 anthropic coding 端点、openai 入站选 openai coding 端点，各走原协议。
///   2. coding_plan 端点中回退 openai coding（入站无对应同协议 coding 端点时，转换出站）
///      —— Kimi coding 仅有 openai coding 端点，anthropic 入站经此回退，`convert_request` 转 openai。
///   3. 非 coding 端点按入站协议精确匹配（普通双协议平台，同协议直发）。
///   4. `openai_responses` 源(Codex)无 Responses 端点时回退到 openai 端点（出站经 to_openai 转换）。
///
/// ── coding-plan 端点排他（防 401，务必保留）──
/// coding-plan 平台的 api_key **仅对 coding endpoint(`coding_plan:true`)有效**；其非 coding endpoint
/// (如 kimi 的 `api.moonshot.cn/anthropic`，指向常规 API host)需另一把常规 key，被 coding key 打成 401
/// → 连累整个平台 auto_disabled。故**平台含任一 coding 端点时，绝不落到非 coding 端点**：优先级链 1→2
/// 全部限定 `coding_plan==true`，仅当无任何 coding 端点(普通平台)才进入 3/4。
/// 这同时满足通用原则：coding 平台的同协议 coding 端点（步骤 1）优先于跨协议转换（步骤 2）。
/// 从 endpoint 的 `base_url` 提取 host（authority 主机名，小写、不含端口/路径）。
///
/// 规则：剥离 `scheme://` 前缀后，取到首个 `/`、`?`、`#` 或 `:`（端口分隔）之前的部分，
/// 并去掉可能的 `user@` 凭证段，最后小写化。解析失败（空 host）返回 None——
/// 调用方据此**保守处理**：host 解析不出 → 不视为同 host（宁可走转换也不误用 coding key）。
fn endpoint_host(base_url: &str) -> Option<String> {
    let after_scheme = base_url
        .split_once("://")
        .map(|(_, rest)| rest)
        .unwrap_or(base_url);
    // authority 段：截到首个路径/查询/锚点分隔符之前
    let authority = after_scheme
        .split(['/', '?', '#'])
        .next()
        .unwrap_or(after_scheme);
    // 去掉 userinfo（user:pass@host）
    let host_port = authority.rsplit_once('@').map(|(_, h)| h).unwrap_or(authority);
    // 去掉端口（注意 IPv6 字面量含 ':'，但 base_url 平台预设均为域名，简单截端口即可）
    let host = host_port.split(':').next().unwrap_or(host_port);
    let host = host.trim().to_lowercase();
    if host.is_empty() {
        None
    } else {
        Some(host)
    }
}

fn select_endpoint_for_protocol<'a>(
    endpoints: &'a [super::models::PlatformEndpoint],
    source_protocol: &str,
) -> Option<&'a super::models::PlatformEndpoint> {
    let ep_proto = |ep: &super::models::PlatformEndpoint| format!("{:?}", ep.protocol).to_lowercase();
    let has_coding_ep = endpoints.iter().any(|ep| ep.coding_plan);
    if has_coding_ep {
        // 步骤 1（加固）：同协议端点直发原协议。采纳条件放宽为 `coding_plan ||
        // 与某 coding 端点同 host`——后者覆盖 GLM 形态（anthropic 端点 base_url 与
        // openai coding 端点同 host `open.bigmodel.cn`，同一把 coding key 通用，DB 中
        // anthropic 端点 coding_plan=false 仍应原协议直发，无需 migration 改数据）。
        // 跨 host 的同协议端点（Kimi anthropic 端点 host=moonshot.cn ≠ coding host
        // kimi.com，需另一把常规 key，coding key 打过去 401）不采纳，落步骤 2 转换。
        // 步骤 2：openai coding 兜底（转换出站）。两步均不落「跨 host 非 coding」端点（防 401）。
        let key_usable = |ep: &super::models::PlatformEndpoint| {
            ep.coding_plan
                || endpoint_host(&ep.base_url).is_some_and(|h| {
                    endpoints
                        .iter()
                        .any(|c| c.coding_plan && endpoint_host(&c.base_url).as_deref() == Some(&h))
                })
        };
        endpoints
            .iter()
            .find(|ep| ep_proto(ep) == source_protocol && key_usable(ep))
            .or_else(|| endpoints.iter().find(|ep| ep.coding_plan && ep_proto(ep) == "openai"))
    } else {
        // 普通平台：步骤 3 同协议直发；步骤 4 openai_responses 回退 openai。
        endpoints
            .iter()
            .find(|ep| ep_proto(ep) == source_protocol)
            .or_else(|| {
                if source_protocol == "openai_responses" {
                    endpoints.iter().find(|ep| ep_proto(ep) == "openai")
                } else {
                    None
                }
            })
    }
}

fn infer_passthrough_protocol_from_ua(ua: &str) -> Option<&'static str> {
    let lower = ua.to_lowercase();
    if lower.contains("claude-cli") {
        Some("anthropic")
    } else if lower.contains("codex") {
        Some("openai_responses")
    } else {
        None
    }
}

/// 在已取出的分组列表中按 group_key（= Authorization Bearer apikey）精确匹配。
/// 分组路由纯按 apikey(group_key)，不再支持 URL path 前缀匹配。
async fn resolve_group(db: &Db, token: Option<&str>) -> Option<Group> {
    let groups = match super::db::list_groups(db).await {
        Ok(g) => g,
        Err(e) => {
            tracing::warn!(error = %e, "resolve_group: list_groups failed");
            return None;
        }
    };
    if let Some(token) = token {
        if let Some(idx) = groups.iter().position(|g| g.group_key == token) {
            return groups.into_iter().nth(idx);
        }
        tracing::warn!(token = %token, "resolve_group: token did not match any group_key");
    }
    tracing::warn!(group_count = groups.len(), "resolve_group: no group matched token");
    None
}

// ─── 客户端模拟 Header ────────────────────────────────────────

/// 根据客户端类型和目标协议，构建模拟的 HTTP 请求头。
/// 数据来源：GitHub 逆向分析 + claude-code-hub 参考实现
/// OpenCode Zen 平台 api_key 解析：用户填了用用户的；留空时注入匿名免费 key `$opencode`
/// （实测被服务端接受，与 `public` 等价走免费共享限频；裸随机串/$ 大写变体均 401）。
/// 对 `Protocol::OpenCodeZen` 平台或 base_url/endpoint 含 `opencode.ai/zen` 的平台生效，
/// 其余平台原样返回（空即空）。枚举判定与 lib.rs(fetch_models/model_test) 对齐，
/// 保证自定义 base_url 时 proxy 与 fetch_models 兜底一致（model-test-proxy parity）。
pub fn resolve_opencode_zen_key(platform: &super::models::Platform) -> String {
    let is_zen = matches!(platform.platform_type, Protocol::OpenCodeZen)
        || platform.base_url.to_lowercase().contains("opencode.ai/zen")
        || platform
            .endpoints
            .iter()
            .any(|ep| ep.base_url.to_lowercase().contains("opencode.ai/zen"));
    opencode_zen_fallback(&platform.api_key, is_zen)
}

/// `resolve_opencode_zen_key` 的纯决策核（便于单测，免构造 Platform）。
pub fn opencode_zen_fallback(api_key: &str, is_zen: bool) -> String {
    if !api_key.trim().is_empty() || !is_zen {
        api_key.to_string()
    } else {
        "$opencode".to_string()
    }
}

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
    // 仅设 auth（UA/Content-Type 由别处，其余入站头透传）。anthropic-version 走入站透传。
    match protocol {
        super::models::Protocol::Anthropic => {
            rb = rb.header("x-api-key", api_key);
        }
        super::models::Protocol::Gemini => {
            rb = rb.header("x-goog-api-key", api_key);
        }
        _ => {
            // openai/兼容：Bearer 之外叠加 api-key 头（小米 token-plan openai 端点要求）。
            rb = rb
                .header("Authorization", format!("Bearer {api_key}"))
                .header("api-key", api_key);
        }
    }
    rb
}

/// Claude Code 家族：仅设 User-Agent + auth（覆盖）。
/// Stainless SDK 头（x-stainless-* / anthropic-version / anthropic-beta /
/// anthropic-dangerous-direct-browser-access / x-app / x-claude-code-session-id）
/// 由 convert 路径从入站透传（passthrough_convert_headers），不再硬编码静态默认 ——
/// 上游可见客户端真实 SDK 版本/会话，跨协议（CC→OpenAI）也带（透明自定义头）。
/// 来源: @anthropic-ai/claude-code/cli.js — buildHeaders() + fV()
/// 参考: claude-code-hub client-detector.ts — confirmClaudeCodeSignals()
fn apply_claude_code_family_headers(
    mut rb: reqwest::RequestBuilder,
    client_type: &ClientType,
    protocol: &super::models::Protocol,
    api_key: &str,
) -> reqwest::RequestBuilder {
    rb = rb.header("User-Agent", claude_code_ua(client_type));

    match protocol {
        super::models::Protocol::Anthropic => {
            rb = rb.header("x-api-key", api_key);
        }
        super::models::Protocol::OpenAI => {
            // openai：Bearer 之外叠加 api-key 头（小米 token-plan openai 端点要求）。
            rb = rb
                .header("Authorization", format!("Bearer {api_key}"))
                .header("api-key", api_key);
        }
        super::models::Protocol::Gemini => {
            rb = rb.header("x-goog-api-key", api_key);
        }
        _ => {
            rb = rb
                .header("Authorization", format!("Bearer {api_key}"))
                .header("api-key", api_key);
        }
    }
    rb
}

/// Codex 家族：仅设 UA + auth + OpenAI 协议必需（OpenAI-Beta / session_id / conversation_id）。
/// originator/version/Accept 等由入站透传。session_id/conversation_id 入站无则生成。
/// 来源: codex-rs/core/src/default_client.rs + model_provider_info.rs + client.rs
/// 参考: claude-code-hub client-detector.ts — CODEX_FAMILY_RULES
fn apply_codex_family_headers(
    mut rb: reqwest::RequestBuilder,
    client_type: &ClientType,
    protocol: &super::models::Protocol,
    api_key: &str,
) -> reqwest::RequestBuilder {
    rb = rb.header("User-Agent", codex_ua(client_type));

    match protocol {
        super::models::Protocol::OpenAI => {
            // openai：Bearer 之外叠加 api-key 头（小米 token-plan openai 端点要求）。
            rb = rb
                .header("Authorization", format!("Bearer {api_key}"))
                .header("api-key", api_key)
                .header("OpenAI-Beta", "responses=experimental")
                .header("conversation_id", uuid_sim())
                .header("session_id", uuid_sim());
        }
        super::models::Protocol::Anthropic => {
            rb = rb.header("x-api-key", api_key);
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

/// 模拟 Cursor IDE：仅 UA + auth。x-app / anthropic-version 由入站透传。
/// 来源: GitHub 逆向 — 使用 Anthropic SDK 但有特定 header 组合
fn apply_cursor_headers(
    mut rb: reqwest::RequestBuilder,
    protocol: &super::models::Protocol,
    api_key: &str,
) -> reqwest::RequestBuilder {
    rb = rb.header("User-Agent", "Cursor/0.50.7");

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

/// 模拟 Windsurf IDE：仅 UA + auth。x-app / anthropic-version 由入站透传。
/// 来源: GitHub 逆向 — 类似 Cursor，使用 Anthropic SDK
fn apply_windsurf_headers(
    mut rb: reqwest::RequestBuilder,
    protocol: &super::models::Protocol,
    api_key: &str,
) -> reqwest::RequestBuilder {
    rb = rb.header("User-Agent", "Windsurf/1.5.0");

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

/// 构建上游请求头 KV 表（用于日志记录，反映实际发送：入站透传 + apply 覆盖）。
/// 透传头从 orig 取并脱敏（auth/cookie），覆盖头（UA/auth/CT + codex 协议必需）按 apply 逻辑。
pub fn build_upstream_headers(
    client_type: &ClientType,
    protocol: &super::models::Protocol,
    api_key: &str,
    orig: &axum::http::HeaderMap,
) -> Vec<(String, String)> {
    let mut h: Vec<(String, String)> = Vec::new();
    // ① 透传入站头（剔 stripped：hop-by-hop + auth/UA/CT）。脱敏敏感值。
    for (k, v) in orig {
        let name = k.as_str();
        if STRIPPED_ON_CONVERT_PASSTHROUGH.iter().any(|s| name.eq_ignore_ascii_case(s)) {
            continue;
        }
        let val = v.to_str().unwrap_or("");
        let val = if name.eq_ignore_ascii_case("cookie") || name.eq_ignore_ascii_case("set-cookie") {
            "[REDACTED]".to_string()
        } else {
            val.to_string()
        };
        h.push((name.to_string(), val));
    }
    // ② 覆盖：Content-Type + auth（redact_key 日志安全）+ UA + codex 协议必需。
    h.push(("Content-Type".into(), "application/json".into()));
    match protocol {
        super::models::Protocol::Anthropic => {
            h.push(("x-api-key".into(), redact_key(api_key)));
        }
        super::models::Protocol::Gemini => {
            h.push(("x-goog-api-key".into(), redact_key(api_key)));
        }
        _ => {
            h.push(("Authorization".into(), format!("Bearer {}", redact_key(api_key))));
        }
    }
    match client_type {
        ClientType::Default => {}
        ClientType::ClaudeCode
        | ClientType::ClaudeCodeVscode
        | ClientType::ClaudeCodeSdkTs
        | ClientType::ClaudeCodeSdkPy
        | ClientType::ClaudeCodeGhAction => {
            h.push(("User-Agent".into(), claude_code_ua(client_type).into()));
        }
        ClientType::CodexCli
        | ClientType::CodexTui
        | ClientType::CodexDesktop
        | ClientType::CodexVscode => {
            h.push(("User-Agent".into(), codex_ua(client_type).into()));
            if matches!(protocol, super::models::Protocol::OpenAI) {
                h.push(("OpenAI-Beta".into(), "responses=experimental".into()));
                h.push(("conversation_id".into(), uuid_sim()));
                h.push(("session_id".into(), uuid_sim()));
            }
        }
        ClientType::Cursor => {
            h.push(("User-Agent".into(), "Cursor/0.50.7".into()));
        }
        ClientType::Windsurf => {
            h.push(("User-Agent".into(), "Windsurf/1.5.0".into()));
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

    // ── 上游响应头透传筛选 filter_upstream_resp_headers ──

    /// 构造 reqwest HeaderMap（append 语义保留多值）
    fn rq_headers(pairs: &[(&str, &str)]) -> reqwest::header::HeaderMap {
        let mut m = reqwest::header::HeaderMap::new();
        for (k, v) in pairs {
            let name = reqwest::header::HeaderName::from_bytes(k.as_bytes()).unwrap();
            m.append(name, reqwest::header::HeaderValue::from_str(v).unwrap());
        }
        m
    }

    fn has(out: &[(axum::http::HeaderName, axum::http::HeaderValue)], name: &str) -> bool {
        out.iter().any(|(n, _)| n.as_str().eq_ignore_ascii_case(name))
    }
    fn val_of<'a>(
        out: &'a [(axum::http::HeaderName, axum::http::HeaderValue)],
        name: &str,
    ) -> Option<&'a str> {
        out.iter()
            .find(|(n, _)| n.as_str().eq_ignore_ascii_case(name))
            .and_then(|(_, v)| v.to_str().ok())
    }
    fn count_of(out: &[(axum::http::HeaderName, axum::http::HeaderValue)], name: &str) -> usize {
        out.iter().filter(|(n, _)| n.as_str().eq_ignore_ascii_case(name)).count()
    }

    #[test]
    fn is_stream_request_false_but_upstream_sse() {
        // 中转站对未声明 stream 的请求强制以 SSE 响应 → 必须判为流式（修复账目零 token bug）。
        assert!(resolve_is_stream(false, "text/event-stream"));
        assert!(resolve_is_stream(false, "text/event-stream; charset=utf-8"));
    }

    #[test]
    fn is_stream_request_true_kept() {
        // 既有正常流式路径不回归。
        assert!(resolve_is_stream(true, "application/json"));
        assert!(resolve_is_stream(true, "text/event-stream"));
    }

    #[test]
    fn is_stream_non_stream_json() {
        // 非流式 JSON 响应保持非流式（走 JSON usage 解析路径）。
        assert!(!resolve_is_stream(false, "application/json"));
        assert!(!resolve_is_stream(false, ""));
    }

    // ── 决策 A：failover 重试状态码圈定 is_status_retryable ──

    #[test]
    fn retry_hard_request_errors_not_retried() {
        // 400 / 422：请求体本身非法，换平台也没用 → 不重试，直接返客户端。
        assert!(!is_status_retryable(400));
        assert!(!is_status_retryable(422));
    }

    #[test]
    fn retry_auth_dead_endpoint_retried() {
        // 401/403（鉴权→auto_disabled）、404/405（死端点→strike）均重试下一平台。
        assert!(is_status_retryable(401));
        assert!(is_status_retryable(403));
        assert!(is_status_retryable(404));
        assert!(is_status_retryable(405));
    }

    #[test]
    fn retry_rate_limit_and_server_errors_retried() {
        // 429（限流/配额，换平台可能成功）+ 所有 5xx（上游故障）→ 重试。
        assert!(is_status_retryable(429));
        assert!(is_status_retryable(500));
        assert!(is_status_retryable(502));
        assert!(is_status_retryable(503));
        assert!(is_status_retryable(504));
        assert!(is_status_retryable(599));
    }

    #[test]
    fn retry_other_4xx_retried_conservatively() {
        // 其余未列举 4xx（非 400/422 硬错）保守重试，不误把上游临时拒绝当硬错。
        assert!(is_status_retryable(408)); // request timeout
        assert!(is_status_retryable(409)); // conflict
        assert!(is_status_retryable(425)); // too early
        assert!(is_status_retryable(402)); // 注：本路径不含 manual budget 402（那条在 forward 前短路返回）
    }

    // ── 决策 B：流式 200 首块判定 classify_stream_first ──

    #[test]
    fn peek_anthropic_message_start_is_meaningful() {
        let text = "event: message_start\ndata: {\"type\":\"message_start\",\"message\":{\"id\":\"m1\"}}\n\n";
        assert_eq!(classify_stream_first(text, false), StreamPeek::Meaningful);
    }

    #[test]
    fn peek_openai_choices_delta_is_meaningful() {
        let text = "data: {\"choices\":[{\"delta\":{\"content\":\"hi\"}}]}\n\n";
        assert_eq!(classify_stream_first(text, false), StreamPeek::Meaningful);
    }

    #[test]
    fn peek_immediate_done_is_empty() {
        // 任何内容前先 [DONE] → 空响应，应重试。
        assert_eq!(classify_stream_first("data: [DONE]\n\n", false), StreamPeek::EmptyOrError);
    }

    #[test]
    fn peek_event_error_is_empty() {
        // event: error 行即判错（无需等 data）。
        let text = "event: error\ndata: {\"type\":\"error\",\"error\":{\"message\":\"boom\"}}\n\n";
        assert_eq!(classify_stream_first(text, false), StreamPeek::EmptyOrError);
    }

    #[test]
    fn peek_error_json_is_empty() {
        // 无 event 行、直接 data 为 error 结构 → 失败。
        let text = "data: {\"error\":{\"message\":\"bad\"}}\n\n";
        assert_eq!(classify_stream_first(text, false), StreamPeek::EmptyOrError);
        let text2 = "data: {\"type\":\"error\",\"message\":\"bad\"}\n\n";
        assert_eq!(classify_stream_first(text2, false), StreamPeek::EmptyOrError);
    }

    #[test]
    fn peek_keepalive_only_needs_more() {
        // 仅 SSE 注释 / event 名行 / 空行 → 尚不足以判定，继续缓冲。
        assert_eq!(classify_stream_first(": ping\n\n", false), StreamPeek::NeedMore);
        assert_eq!(classify_stream_first("event: message_start\n", false), StreamPeek::NeedMore);
        assert_eq!(classify_stream_first("", false), StreamPeek::NeedMore);
    }

    #[test]
    fn peek_partial_json_frame_needs_more() {
        // data 帧 JSON 跨 chunk 截断（尚不可解析）→ 等更多，不误判。
        let text = "data: {\"choices\":[{\"delta\":{\"cont";
        assert_eq!(classify_stream_first(text, false), StreamPeek::NeedMore);
    }

    #[test]
    fn peek_stream_ended_no_content_is_empty() {
        // 流秒断 / 空 body（结束时仍无有效内容事件）→ 空响应，重试。
        assert_eq!(classify_stream_first("", true), StreamPeek::EmptyOrError);
        assert_eq!(classify_stream_first(": ping\n\n", true), StreamPeek::EmptyOrError);
    }

    // ── 决策 B：非流式 200 body 有效性 is_nonstream_body_valid ──

    #[test]
    fn nonstream_empty_body_invalid() {
        assert!(!is_nonstream_body_valid(""));
        assert!(!is_nonstream_body_valid("   "));
    }

    #[test]
    fn nonstream_error_body_invalid() {
        assert!(!is_nonstream_body_valid("{\"error\":{\"message\":\"bad\"}}"));
        assert!(!is_nonstream_body_valid("{\"type\":\"error\",\"message\":\"x\"}"));
    }

    #[test]
    fn nonstream_valid_openai_and_anthropic() {
        assert!(is_nonstream_body_valid(
            "{\"choices\":[{\"message\":{\"role\":\"assistant\",\"content\":\"hi\"}}]}"
        ));
        assert!(is_nonstream_body_valid(
            "{\"content\":[{\"type\":\"text\",\"text\":\"hi\"}],\"role\":\"assistant\"}"
        ));
    }

    #[test]
    fn nonstream_empty_choices_or_content_invalid() {
        // 200 但 choices/content 为空数组 → 无实质内容，重试。
        assert!(!is_nonstream_body_valid("{\"choices\":[]}"));
        assert!(!is_nonstream_body_valid("{\"content\":[]}"));
    }

    #[test]
    fn nonstream_non_json_treated_valid() {
        // 非 JSON 但有内容（上游非标准 200）→ 保守视为有效，不误杀。
        assert!(is_nonstream_body_valid("plain text response"));
    }

    #[test]
    fn filter_resp_drops_blacklist() {
        let src = rq_headers(&[
            ("content-encoding", "gzip"),
            ("content-length", "123"),
            ("transfer-encoding", "chunked"),
            ("connection", "keep-alive"),
            ("keep-alive", "timeout=5"),
            ("date", "Tue, 17 Jun 2026 00:00:00 GMT"),
        ]);
        let out = filter_upstream_resp_headers(&src, false);
        assert!(!has(&out, "content-encoding"));
        assert!(!has(&out, "content-length"));
        assert!(!has(&out, "transfer-encoding"));
        assert!(!has(&out, "connection"));
        assert!(!has(&out, "keep-alive"));
        // 非黑名单保留
        assert!(has(&out, "date"));
    }

    #[test]
    fn filter_resp_keeps_business_headers() {
        let src = rq_headers(&[
            ("date", "Tue, 17 Jun 2026 00:00:00 GMT"),
            ("x-log-id", "abc123"),
            ("x-process-time", "0.042"),
            ("vary", "Accept-Encoding"),
            ("set-cookie", "sid=1; Path=/"),
            ("content-type", "application/json; charset=utf-8"),
        ]);
        let out = filter_upstream_resp_headers(&src, false);
        assert_eq!(val_of(&out, "date"), Some("Tue, 17 Jun 2026 00:00:00 GMT"));
        assert_eq!(val_of(&out, "x-log-id"), Some("abc123"));
        assert_eq!(val_of(&out, "x-process-time"), Some("0.042"));
        assert_eq!(val_of(&out, "vary"), Some("Accept-Encoding"));
        assert_eq!(val_of(&out, "set-cookie"), Some("sid=1; Path=/"));
        assert_eq!(val_of(&out, "content-type"), Some("application/json; charset=utf-8"));
    }

    #[test]
    fn filter_resp_keeps_multiple_set_cookie() {
        let src = rq_headers(&[
            ("set-cookie", "a=1; Path=/"),
            ("set-cookie", "b=2; Path=/"),
            ("set-cookie", "c=3; Path=/"),
        ]);
        let out = filter_upstream_resp_headers(&src, false);
        assert_eq!(count_of(&out, "set-cookie"), 3, "多值 set-cookie 不得丢值");
    }

    #[test]
    fn filter_resp_blacklist_case_insensitive() {
        let src = rq_headers(&[
            ("Content-Encoding", "gzip"),
            ("Transfer-Encoding", "chunked"),
            ("X-Log-Id", "keep-me"),
        ]);
        let out = filter_upstream_resp_headers(&src, false);
        assert!(!has(&out, "content-encoding"), "大小写混合仍须剔除");
        assert!(!has(&out, "transfer-encoding"));
        assert!(has(&out, "x-log-id"));
    }

    #[test]
    fn filter_resp_stream_drops_sse_self_managed() {
        let src = rq_headers(&[
            ("content-type", "application/json"),
            ("cache-control", "max-age=60"),
            ("connection", "close"),
            ("x-log-id", "from-upstream"),
            ("date", "Tue, 17 Jun 2026 00:00:00 GMT"),
        ]);
        let out = filter_upstream_resp_headers(&src, true);
        // SSE 自管头不得来自上游
        assert!(!has(&out, "content-type"));
        assert!(!has(&out, "cache-control"));
        assert!(!has(&out, "connection"));
        // 透传价值头仍保留
        assert_eq!(val_of(&out, "x-log-id"), Some("from-upstream"));
        assert!(has(&out, "date"));
    }

    #[test]
    fn filter_resp_stream_sse_headers_take_self_managed_values() {
        // 模拟流式实发头组装：SSE 三自管头 + filter(is_stream=true)
        let src = rq_headers(&[
            ("content-type", "application/json"),  // 上游值，须被 SSE 自管覆盖
            ("cache-control", "max-age=60"),
            ("connection", "close"),
            ("x-log-id", "from-upstream"),
        ]);
        let sse: [(axum::http::HeaderName, axum::http::HeaderValue); 3] = [
            (axum::http::header::CONTENT_TYPE, axum::http::HeaderValue::from_static("text/event-stream")),
            (axum::http::header::CACHE_CONTROL, axum::http::HeaderValue::from_static("no-cache")),
            (axum::http::header::CONNECTION, axum::http::HeaderValue::from_static("keep-alive")),
        ];
        let mut all: Vec<(axum::http::HeaderName, axum::http::HeaderValue)> = sse.to_vec();
        all.extend(filter_upstream_resp_headers(&src, true));
        assert_eq!(val_of(&all, "content-type"), Some("text/event-stream"));
        assert_eq!(val_of(&all, "cache-control"), Some("no-cache"));
        assert_eq!(val_of(&all, "connection"), Some("keep-alive"));
        assert_eq!(val_of(&all, "x-log-id"), Some("from-upstream"));
    }

    #[test]
    fn resp_headers_log_json_first_value_and_format() {
        let headers = vec![
            (axum::http::header::CONTENT_TYPE, axum::http::HeaderValue::from_static("application/json")),
            (
                axum::http::HeaderName::from_static("x-log-id"),
                axum::http::HeaderValue::from_static("xyz"),
            ),
        ];
        let json = resp_headers_to_log_json(&headers);
        let v: Value = serde_json::from_str(&json).unwrap();
        assert_eq!(v.get("content-type").and_then(|x| x.as_str()), Some("application/json"));
        assert_eq!(v.get("x-log-id").and_then(|x| x.as_str()), Some("xyz"));
    }

    #[test]
    fn nonstream_response_has_single_content_type() {
        // 复现非流式 2xx 成功路径头组装：(StatusCode, Vec<u8>).into_response() 默认写死
        // content-type: application/octet-stream，须先 remove 再 extend，避免重复 content-type。
        use axum::response::IntoResponse;
        let src = rq_headers(&[
            ("content-type", "application/json; charset=utf-8"),
            ("x-log-id", "abc"),
        ]);
        let filtered = filter_upstream_resp_headers(&src, false);
        let mut response = (StatusCode::OK, b"{}".to_vec()).into_response();
        response
            .headers_mut()
            .remove(axum::http::header::CONTENT_TYPE);
        response.headers_mut().extend(filtered);
        let cts: Vec<_> = response
            .headers()
            .get_all(axum::http::header::CONTENT_TYPE)
            .iter()
            .map(|v| v.to_str().unwrap().to_string())
            .collect();
        assert_eq!(cts, vec!["application/json; charset=utf-8".to_string()], "须单一 content-type 取上游真实值");
        assert!(response.headers().contains_key("x-log-id"));
    }

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

    // ── convert 路径透传：剔 hop-by-hop + auth/UA/CT，保留客户端 SDK 头（跨协议也带）──
    #[test]
    fn passthrough_convert_strips_hop_and_override_keeps_sdk_headers() {
        let mut orig = axum::http::HeaderMap::new();
        // hop-by-hop / 强覆盖（应剔）
        orig.insert("host", "127.0.0.1:8080".parse().unwrap());
        orig.insert("content-length", "123".parse().unwrap());
        orig.insert("connection", "keep-alive".parse().unwrap());
        orig.insert("authorization", "Bearer sk-inbound".parse().unwrap());
        orig.insert("user-agent", "inbound-ua/1.0".parse().unwrap());
        orig.insert("content-type", "text/plain".parse().unwrap());
        // 客户端 SDK 头（应保留透传）
        orig.insert("anthropic-beta", "interleaved-thinking-2025-05-14".parse().unwrap());
        orig.insert("anthropic-version", "2023-06-01".parse().unwrap());
        orig.insert("x-stainless-package-version", "0.94.0".parse().unwrap());
        orig.insert("x-stainless-runtime-version", "v24.3.0".parse().unwrap());
        orig.insert("x-stainless-timeout", "3000".parse().unwrap());
        orig.insert("x-claude-code-session-id", "sess-abc".parse().unwrap());
        orig.insert("x-app", "cli".parse().unwrap());

        let fwd = passthrough_convert_headers(&orig);

        // 剔除项
        for stripped in ["host", "content-length", "connection", "authorization", "user-agent", "content-type"] {
            assert!(!fwd.contains_key(stripped), "{stripped} must be stripped for convert apply to override");
        }
        // 透传项（含跨协议透明的 SDK 头）
        assert_eq!(fwd.get("anthropic-beta").and_then(|v| v.to_str().ok()), Some("interleaved-thinking-2025-05-14"));
        assert_eq!(fwd.get("x-stainless-package-version").and_then(|v| v.to_str().ok()), Some("0.94.0"));
        assert_eq!(fwd.get("x-stainless-runtime-version").and_then(|v| v.to_str().ok()), Some("v24.3.0"));
        assert_eq!(fwd.get("x-stainless-timeout").and_then(|v| v.to_str().ok()), Some("3000"));
        assert_eq!(fwd.get("x-claude-code-session-id").and_then(|v| v.to_str().ok()), Some("sess-abc"));
    }

    // ── build_upstream_headers：透传入站（脱敏）+ 覆盖 UA/auth，日志反映真实上游头 ──
    #[test]
    fn build_upstream_headers_passes_through_and_overrides_auth() {
        let mut orig = axum::http::HeaderMap::new();
        orig.insert("anthropic-beta", "beta-x".parse().unwrap());
        orig.insert("x-stainless-package-version", "0.94.0".parse().unwrap());
        orig.insert("cookie", "secret-cookie".parse().unwrap());
        orig.insert("authorization", "Bearer sk-inbound".parse().unwrap());

        let h = build_upstream_headers(&ClientType::ClaudeCode, &crate::gateway::models::Protocol::Anthropic, "sk-realkey-1234567890", &orig);
        let m: std::collections::HashMap<&str, &str> = h.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();

        // 入站 SDK 头透传
        assert_eq!(m.get("anthropic-beta"), Some(&"beta-x"));
        assert_eq!(m.get("x-stainless-package-version"), Some(&"0.94.0"));
        // cookie 脱敏
        assert_eq!(m.get("cookie"), Some(&"[REDACTED]"));
        // auth 覆盖为平台 key（redact）+ UA 模拟
        assert!(m.get("x-api-key").unwrap().contains("****"), "x-api-key must be redacted platform key");
        assert!(m.get("User-Agent").unwrap().starts_with("claude-cli/"));
        assert_eq!(m.get("Content-Type"), Some(&"application/json"));
    }

    // ── OpenCode Zen api_key 兜底（决策核单测）──
    #[test]
    fn opencode_zen_fallback_user_key_wins() {
        assert_eq!(opencode_zen_fallback("$realkey", true), "$realkey");
        assert_eq!(opencode_zen_fallback("$realkey", false), "$realkey");
    }

    #[test]
    fn opencode_zen_fallback_empty_zen_to_literal() {
        assert_eq!(opencode_zen_fallback("", true), "$opencode");
        assert_eq!(opencode_zen_fallback("   ", true), "$opencode");
    }

    #[test]
    fn opencode_zen_fallback_non_zen_passthrough_empty() {
        assert_eq!(opencode_zen_fallback("", false), "");
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

    // ── 模型列表端点识别：strip 任意前缀后尾段 /v1/models | /models ──
    #[test]
    fn models_endpoint_detection() {
        assert!(is_models_endpoint("/proxy/v1/models"));
        assert!(is_models_endpoint("/glm-coding-plan-auto/v1/models"));
        assert!(is_models_endpoint("/v1/models"));
        assert!(is_models_endpoint("/models"));
        assert!(is_models_endpoint("/proxy/models"));
        assert!(is_models_endpoint("/v1/models/")); // 容尾斜杠
        // chat / messages / responses 不命中
        assert!(!is_models_endpoint("/v1/chat/completions"));
        assert!(!is_models_endpoint("/v1/messages"));
        assert!(!is_models_endpoint("/v1/responses"));
        // 子路径 /v1/models/<id> 不当模型列表（尾段非 models）
        assert!(!is_models_endpoint("/v1/models/gpt-4"));
        // gemini /v1beta/models 本期不命中（标 TODO）
        assert!(!is_models_endpoint("/v1beta/models"));
    }

    // ── 模型列表 URL 构造（遵 url-construction-rule：base_url 已含前缀，仅 trim + 后缀）──
    #[test]
    fn models_url_construction() {
        // glm openai 协议端点（base_url 含 /api/paas/v4）→ + /models
        assert_eq!(
            build_models_url(&super::Protocol::Glm, "https://open.bigmodel.cn/api/paas/v4"),
            "https://open.bigmodel.cn/api/paas/v4/models"
        );
        // openai（base_url 含 /v1）→ + /models（禁额外拼 /v1）
        assert_eq!(
            build_models_url(&super::Protocol::OpenAI, "https://api.openai.com/v1"),
            "https://api.openai.com/v1/models"
        );
        // 尾斜杠 trim
        assert_eq!(
            build_models_url(&super::Protocol::OpenAI, "https://api.openai.com/v1/"),
            "https://api.openai.com/v1/models"
        );
        // anthropic（base_url 为 host 根）→ /v1/models
        assert_eq!(
            build_models_url(&super::Protocol::Anthropic, "https://api.anthropic.com"),
            "https://api.anthropic.com/v1/models"
        );
        // bailian → /compatible-mode/v1/models
        assert_eq!(
            build_models_url(&super::Protocol::Bailian, "https://dashscope.aliyuncs.com"),
            "https://dashscope.aliyuncs.com/compatible-mode/v1/models"
        );
    }

    // ── 模型列表鉴权按协议分流：anthropic x-api-key vs openai Bearer ──
    #[test]
    fn models_auth_by_protocol() {
        let client = reqwest::Client::new();
        // anthropic → x-api-key + anthropic-version，无 authorization
        let req = apply_models_auth(client.get("http://x/v1/models"), &super::Protocol::Anthropic, "sk-ant")
            .build()
            .unwrap();
        assert_eq!(req.headers().get("x-api-key").and_then(|v| v.to_str().ok()), Some("sk-ant"));
        assert_eq!(req.headers().get("anthropic-version").and_then(|v| v.to_str().ok()), Some("2023-06-01"));
        assert!(req.headers().get("authorization").is_none());
        // openai 兼容 → Authorization Bearer，无 x-api-key
        let req = apply_models_auth(client.get("http://x/models"), &super::Protocol::Glm, "sk-glm")
            .build()
            .unwrap();
        assert_eq!(req.headers().get("authorization").and_then(|v| v.to_str().ok()), Some("Bearer sk-glm"));
        assert!(req.headers().get("x-api-key").is_none());
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

        // OpenAI 顶层 usage（新 atomics，避免与上面 max 语义相互干扰）
        let oi = AtomicI32::new(0);
        let oo = AtomicI32::new(0);
        let oc = AtomicI32::new(0);
        let oai: Value = serde_json::json!({
            "usage": { "prompt_tokens": 20, "completion_tokens": 7 }
        });
        accumulate_sse_usage(&oai, &oi, &oo, &oc);
        assert_eq!(oi.load(Relaxed), 20);
        assert_eq!(oo.load(Relaxed), 7);
    }

    // ── 回归：Anthropic 流式 message_start 的 input/cache 不被尾部 message_delta(input:0) 覆盖 ──
    // 根因：中转站/relay 的 message_delta 常带 input_tokens:0，store 覆盖会把真实 input 清零。
    // 期望：fetch_max 语义下 input=356、cache=50880 保留，output 取 delta 累计终值 29。
    #[test]
    fn accumulate_sse_usage_anthropic_stream_input_not_clobbered() {
        use std::sync::atomic::{AtomicI32, Ordering::Relaxed};
        let i = AtomicI32::new(0);
        let o = AtomicI32::new(0);
        let c = AtomicI32::new(0);

        // 1) message_start：input/cache 起始即定值
        let start: Value = serde_json::json!({
            "type": "message_start",
            "message": { "usage": {
                "input_tokens": 356,
                "cache_read_input_tokens": 50880,
                "output_tokens": 1
            }}
        });
        accumulate_sse_usage(&start, &i, &o, &c);
        assert_eq!(i.load(Relaxed), 356);
        assert_eq!(c.load(Relaxed), 50880);

        // 2) message_delta（中途）：output 累计上升，input 被中转站带成 0
        let delta1: Value = serde_json::json!({
            "type": "message_delta",
            "usage": { "input_tokens": 0, "output_tokens": 15 }
        });
        accumulate_sse_usage(&delta1, &i, &o, &c);
        assert_eq!(i.load(Relaxed), 356, "input 不可被 message_delta 的 0 清零");
        assert_eq!(o.load(Relaxed), 15);

        // 3) message_delta（终值）：output 累计终值 29，input 仍 0
        let delta2: Value = serde_json::json!({
            "type": "message_delta",
            "usage": { "input_tokens": 0, "output_tokens": 29 }
        });
        accumulate_sse_usage(&delta2, &i, &o, &c);
        assert_eq!(i.load(Relaxed), 356, "input 终态保留");
        assert_eq!(c.load(Relaxed), 50880, "cache 终态保留");
        assert_eq!(o.load(Relaxed), 29, "output 取累计终值");
    }

    // ── 回归：尾部 message_delta(usage) 行被切到两个网络 chunk 仍能解析 usage ──
    // 根因：逐 chunk `.lines()` 解析时，被切断的 `data:` 行喂给 serde 解析失败被静默丢弃，
    // usage(input/output) 永久丢失 → token=0 / est_cost=0（response_body 完整落库但 token 全 0）。
    // 期望：feed_sse_usage 跨 chunk 重组残行后，input=723 / output=2922 / cache=84480 正确累计。
    #[test]
    fn feed_sse_usage_reassembles_split_chunk_boundary() {
        use std::sync::atomic::Ordering::Relaxed;
        let agg = StreamAggregator::new();
        // 真实复现：长流尾部 message_delta usage 行在某字节处被切成两块。
        let full = "event: content_block_stop\ndata: {\"type\": \"content_block_stop\", \"index\": 3}\n\nevent: message_delta\ndata: {\"type\": \"message_delta\", \"delta\": {\"stop_reason\": \"tool_use\"}, \"usage\": {\"input_tokens\": 723, \"output_tokens\": 2922, \"cache_read_input_tokens\": 84480}}\n\nevent: message_stop\ndata: {\"type\": \"message_stop\"}\n\n";
        // 在 message_delta 的 data: 行中间切断（模拟 TCP chunk 边界）。
        let split_at = full.find("\"output_tokens\"").unwrap();
        let (head, tail) = full.split_at(split_at);
        agg.feed_sse_usage(head);
        // 第一块结束时 message_delta 的 data 行不完整，尚不能解析出 output。
        assert_eq!(agg.tokens_out.load(Relaxed), 0, "残行未完成前不应误解析");
        agg.feed_sse_usage(tail);
        assert_eq!(agg.tokens_in.load(Relaxed), 723, "跨 chunk 重组后 input 正确");
        assert_eq!(agg.tokens_out.load(Relaxed), 2922, "跨 chunk 重组后 output 正确");
        assert_eq!(agg.tokens_cache.load(Relaxed), 84480, "跨 chunk 重组后 cache 正确");
    }

    // ── 回归：OpenAI 流式末尾一次性 usage 不因 fetch_max 回退 ──
    // 中途 chunk 无 usage（None → 不触发），末尾一次性给全量，从 0 升上去。
    #[test]
    fn accumulate_sse_usage_openai_stream_final_usage() {
        use std::sync::atomic::{AtomicI32, Ordering::Relaxed};
        let i = AtomicI32::new(0);
        let o = AtomicI32::new(0);
        let c = AtomicI32::new(0);

        // 中途 chunk：无 usage 字段
        let mid: Value = serde_json::json!({
            "choices": [{ "delta": { "content": "hi" } }]
        });
        accumulate_sse_usage(&mid, &i, &o, &c);
        assert_eq!(i.load(Relaxed), 0);
        assert_eq!(o.load(Relaxed), 0);

        // 末尾 chunk：一次性全量 usage（含 cached_tokens）
        let last: Value = serde_json::json!({
            "usage": {
                "prompt_tokens": 1024,
                "completion_tokens": 200,
                "prompt_tokens_details": { "cached_tokens": 512 }
            }
        });
        accumulate_sse_usage(&last, &i, &o, &c);
        assert_eq!(i.load(Relaxed), 1024);
        assert_eq!(o.load(Relaxed), 200);
        assert_eq!(c.load(Relaxed), 512);
    }

    // ── Responses API 子端点识别：精确放行 create，拦所有子端点 ──
    #[test]
    fn responses_subendpoint_detection() {
        use super::is_responses_subendpoint;
        // create（裸 /v1/responses，无尾段）→ false（关键回归：不被新分流拦）
        assert!(!is_responses_subendpoint("/proxy/v1/responses"), "create bare path must NOT be subendpoint");
        assert!(!is_responses_subendpoint("/proxy/v1/responses/"), "create with trailing slash must NOT be subendpoint");
        assert!(!is_responses_subendpoint("/v1/responses"), "create (no proxy prefix) must NOT be subendpoint");
        // 子端点 → true
        assert!(is_responses_subendpoint("/proxy/v1/responses/resp_123"), "retrieve must be subendpoint");
        assert!(is_responses_subendpoint("/proxy/v1/responses/resp_123/cancel"), "cancel must be subendpoint");
        assert!(is_responses_subendpoint("/proxy/v1/responses/resp_123/input_items"), "input_items must be subendpoint");
        assert!(is_responses_subendpoint("/proxy/v1/responses/compact"), "compact must be subendpoint");
        assert!(is_responses_subendpoint("/v1/responses/resp_123"), "subendpoint without proxy prefix must be true");
        // 无 /v1/ 前缀 / 非 responses → false
        assert!(!is_responses_subendpoint("/proxy/v1/chat/completions"), "chat must NOT be subendpoint");
        assert!(!is_responses_subendpoint("/proxy/responses/resp_123"), "missing /v1/ prefix must NOT be subendpoint");
        assert!(!is_responses_subendpoint("/v1/messages"), "anthropic must NOT be subendpoint");
    }

    // ── 子端点上游 URL 构造：base_url(含 /v1) + 子路径(去 /v1)，禁重复拼版本 ──
    #[test]
    fn responses_subendpoint_url_construction() {
        // 镜像 handle_responses_subendpoint 的 URL 拼接逻辑
        let build = |base_url: &str, path: &str| -> String {
            let api_path = match path.find("/v1/") {
                Some(idx) => &path[idx..],
                None => path,
            };
            let sub_path = api_path.strip_prefix("/v1").unwrap_or(api_path);
            format!("{}{}", base_url.trim_end_matches('/'), sub_path)
        };
        // openai 标准 base_url 含 /v1 → 不重复拼
        assert_eq!(
            build("https://api.openai.com/v1", "/proxy/v1/responses/resp_abc/cancel"),
            "https://api.openai.com/v1/responses/resp_abc/cancel"
        );
        assert_eq!(
            build("https://api.openai.com/v1", "/proxy/v1/responses/resp_abc"),
            "https://api.openai.com/v1/responses/resp_abc"
        );
        assert_eq!(
            build("https://api.openai.com/v1", "/proxy/v1/responses/compact"),
            "https://api.openai.com/v1/responses/compact"
        );
        // base_url 末尾带斜杠 → trim 后正确
        assert_eq!(
            build("https://api.openai.com/v1/", "/proxy/v1/responses/resp_abc/input_items"),
            "https://api.openai.com/v1/responses/resp_abc/input_items"
        );
    }

    // ── count_tokens 端点识别：尾段 /v1/messages/count_tokens 才命中，普通 /v1/messages 不命中 ──
    #[test]
    fn count_tokens_endpoint_detection() {
        use super::is_count_tokens_endpoint;
        // 命中（关键修复点）
        assert!(is_count_tokens_endpoint("/proxy/v1/messages/count_tokens"));
        assert!(is_count_tokens_endpoint("/glm-coding-plan-auto/v1/messages/count_tokens"));
        assert!(is_count_tokens_endpoint("/v1/messages/count_tokens"));
        assert!(is_count_tokens_endpoint("/v1/messages/count_tokens/")); // 容尾斜杠
        // 普通 messages → 不命中（关键回归：普通对话路径不被新分流拦）
        assert!(!is_count_tokens_endpoint("/proxy/v1/messages"));
        assert!(!is_count_tokens_endpoint("/v1/messages"));
        assert!(!is_count_tokens_endpoint("/v1/messages/"));
        // 无 /v1/ 前缀 / 其他端点 → 不命中
        assert!(!is_count_tokens_endpoint("/proxy/messages/count_tokens"));
        assert!(!is_count_tokens_endpoint("/v1/chat/completions"));
        assert!(!is_count_tokens_endpoint("/v1/responses/resp_1"));
    }

    // ── count_tokens 上游 URL 构造：anthropic base_url(不含 /v1) + /v1/messages/count_tokens ──
    #[test]
    fn count_tokens_url_construction() {
        let build = |base_url: &str| format!("{}/v1/messages/count_tokens", base_url.trim_end_matches('/'));
        // GLM anthropic 端点（base_url 不含 /v1）→ 拼出含尾段的完整 URL
        assert_eq!(
            build("https://open.bigmodel.cn/api/anthropic"),
            "https://open.bigmodel.cn/api/anthropic/v1/messages/count_tokens"
        );
        // anthropic 官方 base_url → 同款
        assert_eq!(
            build("https://api.anthropic.com"),
            "https://api.anthropic.com/v1/messages/count_tokens"
        );
        // base_url 末尾带斜杠 → trim 后正确，不双斜杠
        assert_eq!(
            build("https://api.anthropic.com/"),
            "https://api.anthropic.com/v1/messages/count_tokens"
        );
    }

    // ── 本地估算兜底：累计文本字符数 ~4 字符/token，保底 1 ──
    #[test]
    fn count_tokens_local_estimate() {
        use super::estimate_input_tokens;
        // 空 body / 无文本字段 → 保底 1（不返回 0 误导客户端）
        assert_eq!(estimate_input_tokens(&serde_json::json!({})), 1);
        assert_eq!(estimate_input_tokens(&serde_json::Value::Null), 1);
        // messages 递归累计全部字符串值：role "user"(4) + content "abcdefgh"(8) = 12 → ceil(12/4)=3
        let body = serde_json::json!({
            "model": "claude-opus-4-8",
            "messages": [{"role": "user", "content": "abcdefgh"}]
        });
        assert_eq!(estimate_input_tokens(&body), 3);
        // system + messages + tools 全字符串值累计：
        // system "syst"(4) + role "user"(4) + content "msgs"(4) + tool name "x"(1) + desc "tdsc"(4) = 17 → ceil(17/4)=5
        let body = serde_json::json!({
            "system": "syst",
            "messages": [{"role": "user", "content": "msgs"}],
            "tools": [{"name": "x", "description": "tdsc"}]
        });
        assert_eq!(estimate_input_tokens(&body), 5);
        // model 字段不计入文本估算（仅 system/messages/tools）
        let with_model = serde_json::json!({ "model": "very-long-model-name-not-counted" });
        assert_eq!(estimate_input_tokens(&with_model), 1);
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

    // ── coding-plan 平台端点选择：anthropic 入站不得落到非 coding endpoint(coding key→401) ──
    #[test]
    fn select_endpoint_coding_plan_exclusivity() {
        use super::select_endpoint_for_protocol as sel;
        use super::super::models::{ClientType, PlatformEndpoint, Protocol};
        let ep = |proto: Protocol, url: &str, cp: bool| PlatformEndpoint {
            protocol: proto,
            base_url: url.to_string(),
            client_type: ClientType::ClaudeCode,
            coding_plan: cp,
        };

        // ── Kimi coding plan：唯一 openai coding endpoint，anthropic 入站须选 coding(转换) ──
        let kimi_cp = vec![ep(Protocol::OpenAI, "https://api.kimi.com/coding/v1", true)];
        let m = sel(&kimi_cp, "anthropic").expect("anthropic inbound must resolve to coding endpoint");
        assert_eq!(m.base_url, "https://api.kimi.com/coding/v1");
        assert!(m.coding_plan, "selected endpoint must be the coding endpoint");
        // openai 入站同样落 coding endpoint
        let m = sel(&kimi_cp, "openai").unwrap();
        assert!(m.coding_plan);

        // ── Kimi 跨 host 防 401（核心约束）：openai coding host=api.kimi.com，
        //    anthropic endpoint host=api.moonshot.cn（cp=false，需另一把常规 key）。
        //    两 host 不同 → 加固后**不采纳**该 anthropic 端点，anthropic 入站回退 openai coding 转换。
        //    coding key 绝不打到 moonshot.cn（否则 401 连累整个平台 auto_disabled）。 ──
        let kimi_cp_legacy = vec![
            ep(Protocol::OpenAI, "https://api.kimi.com/coding/v1", true),
            ep(Protocol::Anthropic, "https://api.moonshot.cn/anthropic", false),
        ];
        let m = sel(&kimi_cp_legacy, "anthropic").unwrap();
        assert_eq!(
            m.base_url, "https://api.kimi.com/coding/v1",
            "anthropic inbound on coding platform must NOT pick the cross-host non-coding anthropic endpoint"
        );
        assert!(m.coding_plan);

        // ── 非 coding-plan 平台(GLM 常规双端点)：anthropic 入站正常选 anthropic endpoint(行为不变) ──
        let glm = vec![
            ep(Protocol::OpenAI, "https://open.bigmodel.cn/api/paas/v4", false),
            ep(Protocol::Anthropic, "https://open.bigmodel.cn/api/anthropic", false),
        ];
        let m = sel(&glm, "anthropic").unwrap();
        assert_eq!(m.base_url, "https://open.bigmodel.cn/api/anthropic");
        assert!(!m.coding_plan);
        // openai 入站选 openai endpoint
        let m = sel(&glm, "openai").unwrap();
        assert_eq!(m.base_url, "https://open.bigmodel.cn/api/paas/v4");

        // ── GLM Coding Plan(openai coding cp=true + anthropic cp=true，同 host)：「同协议优先于转换」──
        // anthropic(Claude Code)入站 → 选 anthropic coding 端点原协议直发，不得回退 openai coding 转换。
        let glm_cp = vec![
            ep(Protocol::OpenAI, "https://open.bigmodel.cn/api/coding/paas/v4", true),
            ep(Protocol::Anthropic, "https://open.bigmodel.cn/api/anthropic", true),
        ];
        let m = sel(&glm_cp, "anthropic")
            .expect("anthropic inbound must resolve to the anthropic coding endpoint");
        assert_eq!(
            m.base_url, "https://open.bigmodel.cn/api/anthropic",
            "GLM coding plan: anthropic inbound must use anthropic coding endpoint (no openai conversion)"
        );
        assert!(m.coding_plan, "selected endpoint must be the coding endpoint");
        // openai 入站仍选 openai coding 端点
        let m = sel(&glm_cp, "openai").unwrap();
        assert_eq!(m.base_url, "https://open.bigmodel.cn/api/coding/paas/v4");
        assert!(m.coding_plan);

        // ── GLM 形态（真实 DB 数据）：openai coding cp=true + anthropic cp=FALSE，同 host ──
        // 加固后：anthropic 入站凭「与 openai coding 端点同 host(open.bigmodel.cn)、同一把 key 通用」
        // 采纳该 cp=false anthropic 端点原协议直发，**无需 migration 把它标 cp=true**。
        let glm_cp_real = vec![
            ep(Protocol::OpenAI, "https://open.bigmodel.cn/api/coding/paas/v4", true),
            ep(Protocol::Anthropic, "https://open.bigmodel.cn/api/anthropic", false),
        ];
        let m = sel(&glm_cp_real, "anthropic")
            .expect("anthropic inbound must resolve to the same-host anthropic endpoint");
        assert_eq!(
            m.base_url, "https://open.bigmodel.cn/api/anthropic",
            "GLM (anthropic ep cp=false, same host): anthropic inbound must use anthropic endpoint, no conversion"
        );
        // openai 入站仍走 openai coding 端点
        let m = sel(&glm_cp_real, "openai").unwrap();
        assert_eq!(m.base_url, "https://open.bigmodel.cn/api/coding/paas/v4");
        assert!(m.coding_plan);

        // ── 非 coding-plan：openai_responses 无 Responses endpoint → 回退 openai(行为不变) ──
        let openai_only = vec![ep(Protocol::OpenAI, "https://api.deepseek.com/v1", false)];
        let m = sel(&openai_only, "openai_responses").unwrap();
        assert_eq!(m.base_url, "https://api.deepseek.com/v1");
        // 无任何匹配且非 openai_responses → None
        assert!(sel(&openai_only, "gemini").is_none());
    }

    // ── endpoint_host：scheme/端口/路径/userinfo/大小写 边界 ──
    #[test]
    fn endpoint_host_extraction() {
        use super::endpoint_host as host;
        assert_eq!(host("https://open.bigmodel.cn/api/anthropic").as_deref(), Some("open.bigmodel.cn"));
        assert_eq!(host("https://open.bigmodel.cn/api/coding/paas/v4").as_deref(), Some("open.bigmodel.cn"));
        // 端口被剥离
        assert_eq!(host("http://localhost:8080/v1").as_deref(), Some("localhost"));
        // 大小写归一
        assert_eq!(host("https://API.Kimi.COM/coding/v1").as_deref(), Some("api.kimi.com"));
        // userinfo 被剥离
        assert_eq!(host("https://user:pass@api.moonshot.cn/anthropic").as_deref(), Some("api.moonshot.cn"));
        // 无 scheme 也能取 host
        assert_eq!(host("api.moonshot.cn/anthropic").as_deref(), Some("api.moonshot.cn"));
        // 跨 host 判定：GLM 同 host，Kimi 异 host
        assert_eq!(
            host("https://open.bigmodel.cn/api/coding/paas/v4"),
            host("https://open.bigmodel.cn/api/anthropic")
        );
        assert_ne!(
            host("https://api.kimi.com/coding/v1"),
            host("https://api.moonshot.cn/anthropic")
        );
        // 空 / 不可解析 → None（保守，不视为同 host）
        assert_eq!(host(""), None);
        assert_eq!(host("https://"), None);
    }

    // ── UA → 透传协议推断：claude-cli→anthropic / codex→openai_responses / 其它→None ──
    #[test]
    fn infer_passthrough_protocol_from_ua_mapping() {
        use super::infer_passthrough_protocol_from_ua as infer;
        // Claude Code 家族（全部含 claude-cli 前缀）
        assert_eq!(infer("claude-cli/1.0.117 (external, cli)"), Some("anthropic"));
        assert_eq!(infer("claude-cli/1.0.117 (external, claude-vscode, agent-sdk/0.1.30)"), Some("anthropic"));
        // Codex 家族（codex_cli_rs / Codex/ / codex desktop / codex-vscode；大小写不敏感）
        assert_eq!(infer("codex_cli_rs/0.38.0 (MacOS; arm64) Terminal"), Some("openai_responses"));
        assert_eq!(infer("Codex/0.38.0"), Some("openai_responses"));
        assert_eq!(infer("codex desktop/0.38.0"), Some("openai_responses"));
        assert_eq!(infer("codex-vscode/0.38.0"), Some("openai_responses"));
        // 不识别（Cursor / Windsurf / gemini-cli / 未知 / 空）→ None
        assert_eq!(infer("Cursor/0.50.7"), None);
        assert_eq!(infer("Windsurf/1.5.0"), None);
        assert_eq!(infer("gemini-cli/0.1.0"), None);
        assert_eq!(infer("curl/8.0"), None);
        assert_eq!(infer(""), None);
    }

    // ── UA 透传三级回退分支判定（镜像插入点逻辑：matched_ep==None 时按 UA 推断）──
    // 级别 1：UA 命中 + 平台有该协议 endpoint → 透传 wire = UA 协议。
    // 级别 2：UA 命中 + 平台无该协议 endpoint → 回退（不透传）。
    // 级别 3：UA 不识别 → 回退（不透传）。
    #[test]
    fn ua_passthrough_three_level_fallback() {
        use super::infer_passthrough_protocol_from_ua as infer;
        // 模拟平台端点协议集合（小写名）
        let try_passthrough = |ua: &str, platform_protos: &[&str], source_matched: bool| -> Option<&'static str> {
            // path 已被支持（source_matched=true）→ 不介入
            if source_matched {
                return None;
            }
            // matched_ep == None → 尝试 UA 推断
            let p = infer(ua)?;
            // 平台需确有该协议 endpoint
            if platform_protos.contains(&p) {
                Some(p)
            } else {
                None
            }
        };
        // 级别 1：codex UA + 平台有 openai_responses → 透传该协议
        assert_eq!(
            try_passthrough("codex_cli_rs/0.38.0", &["openai_responses", "anthropic"], false),
            Some("openai_responses")
        );
        // 级别 1：claude-cli + 平台有 anthropic → 透传 anthropic
        assert_eq!(
            try_passthrough("claude-cli/1.0.117 (external, cli)", &["anthropic"], false),
            Some("anthropic")
        );
        // 级别 2：codex UA 但平台只有 openai（无 openai_responses）→ 回退
        assert_eq!(
            try_passthrough("codex_cli_rs/0.38.0", &["openai", "anthropic"], false),
            None
        );
        // 级别 3：UA 不识别 → 回退
        assert_eq!(
            try_passthrough("Cursor/0.50.7", &["anthropic", "openai_responses"], false),
            None
        );
        // 级别 0：path 已被平台支持（source_matched）→ 不介入，UA 不参与
        assert_eq!(
            try_passthrough("codex_cli_rs/0.38.0", &["openai_responses"], true),
            None
        );
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

    // ── 上游 gzip 压缩响应解压回归（修复 token/成本全 0 + 日志乱码）──
    // 背景: 上游 GLM anthropic 端点回 content-encoding: gzip。reqwest 启用 gzip feature 后
    // 由响应头 Content-Encoding 驱动自动解压，resp.bytes() 得明文。本 test 用 flate2 构造
    // 一段 gzip 压缩的 anthropic usage JSON，解压后喂 extract_usage，断言 token > 0，
    // 证明「解压后 JSON → extract_usage → token>0」链路成立（reqwest 解压本身为黑盒，
    // 由 Cargo feature gzip/brotli/deflate/zstd 保证，行为有 docs.rs 官方背书）。
    #[test]
    fn gzip_decompressed_anthropic_usage_extracts_tokens() {
        use flate2::read::GzDecoder;
        use flate2::write::GzEncoder;
        use flate2::Compression;
        use std::io::{Read, Write};

        // anthropic 非流式响应体（含 usage.input_tokens / output_tokens / cache_read_input_tokens）
        let json = r#"{
            "id": "msg_01abc",
            "type": "message",
            "role": "assistant",
            "model": "glm-5.1",
            "content": [{"type": "text", "text": "hello"}],
            "stop_reason": "end_turn",
            "usage": {
                "input_tokens": 1234,
                "output_tokens": 567,
                "cache_read_input_tokens": 89
            }
        }"#;

        // 模拟上游：gzip 压缩明文 JSON（等价上游回 content-encoding: gzip）
        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(json.as_bytes()).unwrap();
        let gzipped = encoder.finish().unwrap();
        // 压缩字节非 UTF-8 可读 → 直接喂 extract_usage 解析失败返回 (0,0,0)（复现旧 bug）
        let lossy = String::from_utf8_lossy(&gzipped);
        assert_eq!(
            extract_usage(&lossy),
            (0, 0, 0),
            "压缩字节当文本解析应失败（复现旧 bug）"
        );

        // 模拟 reqwest 启用 feature 后的解压结果：解压回明文
        let mut decoder = GzDecoder::new(&gzipped[..]);
        let mut decompressed = String::new();
        decoder.read_to_string(&mut decompressed).unwrap();

        // 解压后 JSON → extract_usage → token > 0（修复后语义）
        let (input, output, cache) = extract_usage(&decompressed);
        assert_eq!(input, 1234);
        assert_eq!(output, 567);
        assert_eq!(cache, 89);
        assert!(input > 0 && output > 0, "解压后 token 必须 > 0");
    }

    // ── StreamLogGuard flush / 终态回写 response_body 回归 ──
    //   根因：anthropic→anthropic 透传流不发 `[DONE]`（仅 message_stop 收尾），
    //   旧 flush_if_done 只认 [DONE] → 这类流仅靠 Drop 兜底，Drop 内 tokio::spawn
    //   在连接 abort 时序下偶发丢写，response_body 永久停在 `[stream]` 占位。

    use std::sync::atomic::AtomicBool;

    /// 构造一个最小可用、初始化好表的临时文件 DB（避免 :memory: 全局缓存跨 test 串味）。
    async fn flush_test_db() -> (Arc<super::super::db::Db>, std::path::PathBuf) {
        use std::sync::atomic::AtomicU64;
        static SEQ: AtomicU64 = AtomicU64::new(0);
        let mut path = std::env::temp_dir();
        let uniq = format!(
            "aidog_flush_test_{}_{}_{}.db",
            std::process::id(),
            super::super::db::now(),
            SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        );
        path.push(uniq);
        let db = super::super::db::Db::new(path.to_str().unwrap())
            .await
            .expect("open temp db");
        db.init_tables().await.expect("init tables");
        (Arc::new(db), path)
    }

    fn flush_test_state(db: Arc<super::super::db::Db>) -> Arc<ProxyState> {
        Arc::new(ProxyState {
            db,
            app: None,
            middleware: Arc::new(MiddlewareEngine::new()),
            scheduler: Arc::new(super::super::scheduling::SchedulerState::new()),
            sticky: Arc::new(super::super::scheduling::StickyTable::new()),
            log_snapshots: std::sync::Mutex::new(std::collections::HashMap::new()),
        })
    }

    fn placeholder_stream_log(id: &str) -> ProxyLog {
        let ts = super::super::db::now();
        ProxyLog {
            id: id.to_string(),
            group_key: "gk_test".to_string(),
            model: "claude".to_string(),
            actual_model: "glm-5".to_string(),
            source_protocol: "anthropic".to_string(),
            target_protocol: "anthropic".to_string(),
            platform_id: 0,
            request_headers: String::new(),
            request_body: String::new(),
            upstream_request_headers: String::new(),
            upstream_request_body: String::new(),
            response_body: "[stream]".to_string(),
            request_url: String::new(),
            upstream_request_url: String::new(),
            upstream_response_headers: String::new(),
            upstream_status_code: 200,
            user_response_headers: String::new(),
            user_response_body: "[stream]".to_string(),
            status_code: 200,
            duration_ms: 0,
            input_tokens: 0,
            output_tokens: 0,
            cache_tokens: 0,
            est_cost: 0.0,
            is_stream: true,
            attempts: Vec::new(),
            retry_count: 0,
            blocked_by: String::new(),
            blocked_reason: String::new(),
            created_at: ts,
            updated_at: ts,
            deleted_at: 0,
        }
    }

    /// 建一个 StreamLogGuard，settings = 默认（enabled=true, log_user_request=false）。
    /// upstream_chunks 预先 push 进 agg.upstream_body（模拟流式逐 chunk 累积）。
    fn make_guard(
        state: &Arc<ProxyState>,
        log: ProxyLog,
        upstream_chunks: &[&str],
        out_tokens: i32,
    ) -> StreamLogGuard {
        let agg = Arc::new(StreamAggregator::new());
        {
            let mut up = agg.upstream_body.lock().unwrap();
            for c in upstream_chunks {
                up.push(Bytes::from(c.to_string()));
            }
        }
        if out_tokens > 0 {
            agg.tokens_out
                .store(out_tokens, std::sync::atomic::Ordering::Relaxed);
        }
        StreamLogGuard {
            agg,
            est_fired: Arc::new(AtomicBool::new(false)),
            log,
            state: state.clone(),
            settings: ProxyLogSettings::default(),
            start: std::time::Instant::now(),
            record_upstream_body: true, // = log_settings.enabled
            record_client_body: false,  // log_user_request=false
            req_span: tracing::Span::current(),
            est: None,
        }
    }

    async fn read_response_body(db: &super::super::db::Db, id: &str) -> String {
        super::super::db::get_proxy_log(db, id)
            .await
            .expect("get log")
            .expect("row exists")
            .response_body
    }

    /// 等待 flush 内 tokio::spawn 的落库任务完成（短轮询，最多 ~2s）。
    async fn await_flush_write(db: &super::super::db::Db, id: &str) -> String {
        for _ in 0..200 {
            let body = read_response_body(db, id).await;
            if body != "[stream]" {
                return body;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
        read_response_body(db, id).await
    }

    // 1) 正常 [DONE] 收尾（OpenAI 风格）：flush 把聚合上游内容写回 response_body。
    #[tokio::test]
    async fn flush_done_writes_aggregated_body() {
        let (db, path) = flush_test_db().await;
        let state = flush_test_state(db.clone());
        let id = "flush_done_0001";
        let log = placeholder_stream_log(id);
        super::super::db::insert_proxy_log_columns(
            &state.db,
            super::super::db::ProxyLogColumns::from_log(&log, false, false),
        )
        .await
        .unwrap();
        state
            .log_snapshots
            .lock()
            .unwrap()
            .insert(id.to_string(), super::super::db::ProxyLogColumns::from_log(&log, false, false));

        let chunks = [
            "data: {\"choices\":[{\"delta\":{\"content\":\"hi\"}}]}\n\n",
            "data: [DONE]\n\n",
        ];
        let guard = make_guard(&state, log, &chunks, 7);
        // 模拟闭包逐 chunk：末 chunk 命中 [DONE] → flush_if_done 触发 flush。
        guard.flush_if_done(chunks[1]);
        let body = await_flush_write(&state.db, id).await;
        assert_ne!(body, "[stream]", "[DONE] 收尾后 response_body 不应停在占位");
        assert!(body.contains("hi"), "应写回聚合上游内容: {body}");

        drop(guard);
        let _ = std::fs::remove_file(path);
    }

    // 2) Anthropic message_stop 收尾（不发 [DONE]）：旧 bug 核心场景。
    #[tokio::test]
    async fn flush_message_stop_writes_aggregated_body() {
        let (db, path) = flush_test_db().await;
        let state = flush_test_state(db.clone());
        let id = "flush_mstop_0001";
        let log = placeholder_stream_log(id);
        super::super::db::insert_proxy_log_columns(
            &state.db,
            super::super::db::ProxyLogColumns::from_log(&log, false, false),
        )
        .await
        .unwrap();
        state
            .log_snapshots
            .lock()
            .unwrap()
            .insert(id.to_string(), super::super::db::ProxyLogColumns::from_log(&log, false, false));

        // 典型 anthropic 透传尾块：message_delta + message_stop，无 [DONE]
        let tail = "event: message_delta\ndata: {\"type\":\"message_delta\"}\n\nevent: message_stop\ndata: {\"type\":\"message_stop\"}\n\n";
        let chunks = ["event: message_start\ndata: {\"type\":\"message_start\"}\n\n", tail];
        let guard = make_guard(&state, log, &chunks, 11);
        // 旧实现 flush_if_done 只认 [DONE] → 此处不触发，response_body 卡占位（bug）。
        // 修复后认 message_stop → 触发 flush 确定性回写。
        guard.flush_if_done(tail);
        let body = await_flush_write(&state.db, id).await;
        assert_ne!(body, "[stream]", "message_stop 收尾后 response_body 不应停在占位（核心 bug）");
        assert!(body.contains("message_stop"), "应写回聚合上游内容: {body}");

        drop(guard);
        let _ = std::fs::remove_file(path);
    }

    // 3) 客户端断连 / 上游无终止符：Drop 兜底仍写 response_body（已聚合内容）。
    #[tokio::test]
    async fn flush_drop_writes_partial_body() {
        let (db, path) = flush_test_db().await;
        let state = flush_test_state(db.clone());
        let id = "flush_drop_0001";
        let log = placeholder_stream_log(id);
        super::super::db::insert_proxy_log_columns(
            &state.db,
            super::super::db::ProxyLogColumns::from_log(&log, false, false),
        )
        .await
        .unwrap();
        state
            .log_snapshots
            .lock()
            .unwrap()
            .insert(id.to_string(), super::super::db::ProxyLogColumns::from_log(&log, false, false));

        // 仅有部分内容，无 [DONE]/message_stop（模拟中途断裂 / 客户端断连）。
        let chunks = ["event: message_start\ndata: {\"type\":\"message_start\"}\n\n", "data: {\"delta\":{\"text\":\"partial\"}}\n\n"];
        let guard = make_guard(&state, log, &chunks, 3);
        // 不调用 flush_if_done（无终止符）；直接 Drop 触发兜底 flush。
        drop(guard);
        let body = await_flush_write(&state.db, id).await;
        assert_ne!(body, "[stream]", "Drop 兜底后 response_body 不应停在占位");
        assert!(body.contains("partial"), "Drop 应写回已聚合的部分内容: {body}");

        let _ = std::fs::remove_file(path);
    }

    // 4) 空流（上游回 200 头后秒断 / 仅心跳，零内容）：finalize 成空串，绝不留 [stream]。
    #[tokio::test]
    async fn flush_empty_stream_finalizes_to_empty_not_placeholder() {
        let (db, path) = flush_test_db().await;
        let state = flush_test_state(db.clone());
        let id = "flush_empty_0001";
        let log = placeholder_stream_log(id);
        super::super::db::insert_proxy_log_columns(
            &state.db,
            super::super::db::ProxyLogColumns::from_log(&log, false, false),
        )
        .await
        .unwrap();
        state
            .log_snapshots
            .lock()
            .unwrap()
            .insert(id.to_string(), super::super::db::ProxyLogColumns::from_log(&log, false, false));

        let guard = make_guard(&state, log, &[], 0); // 零 upstream chunk
        drop(guard); // Drop 兜底 flush
        // 空流：join_stream_body([]) == "" → response_body 应被改写成空串而非占位。
        for _ in 0..200 {
            let body = read_response_body(&state.db, id).await;
            if body != "[stream]" {
                assert_eq!(body, "", "空流 finalize 应为空串");
                let _ = std::fs::remove_file(&path);
                return;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
        let _ = std::fs::remove_file(path);
        panic!("空流 response_body 仍停在 [stream] 占位（finalize 未执行）");
    }
}
