use axum::{
    body::Body,
    extract::{Request, State as AxumState},
    http::StatusCode,
    response::{IntoResponse, Response},
    Router,
};
use futures::StreamExt;
use reqwest::Client;
use serde_json::Value;
use std::sync::Arc;

use super::adapter::{self, ChatRequest, ChatStreamEvent};
use super::db::Db;
use super::models::{ClientType, Group, Protocol, ProxyLog, ProxyLogSettings, ProxyTimeoutSettings};
use super::router::select_platform;

/// 代理服务器共享状态
pub struct ProxyState {
    /// 用 Arc<Db> 而非 Mutex<Db>：Db 内部已自带 Mutex<Connection>，
    /// Arc 便于克隆进后台预估 spawn（每次操作锁内自治，禁持锁跨 await）。
    pub db: Arc<Db>,
    /// 可选 AppHandle：预估更新后 emit "tray-refresh" 事件让主线程刷新托盘。
    /// 后台 spawn 不直接操作 tray（线程安全），改 emit 事件由主线程 setup 监听刷新。
    pub app: Option<tauri::AppHandle>,
}

/// 启动代理服务器，返回 shutdown handle
pub async fn start_proxy(
    db: Arc<Db>,
    port: u16,
    app: Option<tauri::AppHandle>,
) -> Result<(tokio::task::JoinHandle<()>, u16), String> {
    let state = Arc::new(ProxyState { db, app });

    let app = Router::new()
        .fallback(handle_proxy)
        .with_state(state);

    // Try binding from port upward; if occupied, try port+1..port+100
    let mut actual_port = port;
    let listener = loop {
        let addr = std::net::SocketAddr::from(([127, 0, 0, 1], actual_port));
        match tokio::net::TcpListener::bind(addr).await {
            Ok(l) => break l,
            Err(e) if e.kind() == std::io::ErrorKind::AddrInUse => {
                actual_port += 1;
                if actual_port > port + 100 {
                    return Err(format!("no available port in range {}..{}", port, port + 101));
                }
                continue;
            }
            Err(e) => return Err(format!("bind failed: {e}")),
        }
    };

    let handle = tokio::spawn(async move {
        axum::serve(listener, app).await.ok();
    });

    Ok((handle, actual_port))
}

/// Read proxy log settings from DB
fn get_log_settings(db: &Db) -> ProxyLogSettings {
    super::db::get_setting(db, "proxy", "logging")
        .ok()
        .flatten()
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or_default()
}

/// Upsert a proxy log entry; silently ignore errors.
/// Respects ProxyLogSettings: if logging disabled, does nothing;
/// if user/upstream recording disabled, clears those fields before writing.
fn upsert_log(state: &Arc<ProxyState>, log: &ProxyLog, settings: &ProxyLogSettings) {
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
        );
    }
    if super::db::upsert_proxy_log(&state.db, &log).is_ok() {
        // 日志写库成功后通知前端三页（Platforms/Groups/Stats）实时刷新统计。
        // app handle 为 None（无 GUI 上下文）时安全跳过，不影响代理逻辑。
        if let Some(app) = &state.app {
            use tauri::Emitter;
            let _ = app.emit("proxy-log-updated", log.platform_id);
        }
    }
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
    });
}

/// Read system-level timeout settings from DB
fn get_system_timeout(db: &Db) -> ProxyTimeoutSettings {
    super::db::get_setting(db, "proxy", "timeout")
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
    AxumState(state): AxumState<Arc<ProxyState>>,
    req: Request,
) -> Response {
    let start = std::time::Instant::now();
    let request_id = uuid::Uuid::new_v4().simple().to_string();
    let created_at = super::db::now();

    // Load log settings once per request
    let log_settings = get_log_settings(&state.db);

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
        created_at,
        updated_at: created_at,
        deleted_at: 0,
    };

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
    tracing::info!(path = %path, "http request");

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
            upsert_log(&state, &log, &log_settings);
            return (StatusCode::BAD_REQUEST, format!("read body: {e}")).into_response();
        }
    };
    log.request_body = String::from_utf8_lossy(&bytes).to_string();

    // Best-effort model extraction
    let raw_model = serde_json::from_slice::<Value>(&bytes)
        .ok()
        .and_then(|v| v.get("model").and_then(|m| m.as_str()).map(String::from))
        .unwrap_or_default();
    log.model = raw_model.clone();

    // Upsert #1: request received
    upsert_log(&state, &log, &log_settings);

    // ── 查找分组 ──
    let group = {
        match resolve_group(&state.db, auth_header.as_deref(), &path) {
            Some(g) => g,
            None => {
                if let Some(ref token) = auth_header {
                    log.response_body = format!("no matching group for token '{}' or path '{}'", token, path);
                    log.status_code = 404;
                    log.duration_ms = start.elapsed().as_millis() as i32;
                    upsert_log(&state, &log, &log_settings);
                    return (StatusCode::NOT_FOUND, log.response_body.clone()).into_response();
                } else {
                    log.response_body = "no matching group".to_string();
                    log.status_code = 404;
                    log.duration_ms = start.elapsed().as_millis() as i32;
                    upsert_log(&state, &log, &log_settings);
                    return (StatusCode::NOT_FOUND, "no matching group").into_response();
                }
            }
        }
    };

    // Upsert #2: group resolved
    log.group_name = group.name.clone();
    // Auto-detect source_protocol from request path (group no longer restricts inbound protocol)
    let source_protocol = detect_source_protocol(&path);
    log.source_protocol = source_protocol.clone();
    upsert_log(&state, &log, &log_settings);

    // ── 解析 ChatRequest（按入站协议解析） ──
    let req_value: serde_json::Value = match serde_json::from_slice(&bytes) {
        Ok(v) => v,
        Err(e) => {
            log.response_body = format!("parse request json error: {e}");
            log.status_code = 400;
            log.duration_ms = start.elapsed().as_millis() as i32;
            upsert_log(&state, &log, &log_settings);
            return (StatusCode::BAD_REQUEST, format!("parse json: {e}")).into_response();
        }
    };
    let mut chat_req: ChatRequest = match adapter::parse_incoming_request(&log.source_protocol, &req_value) {
        Some(r) => r,
        None => {
            log.response_body = "failed to parse request for protocol".to_string();
            log.status_code = 400;
            log.duration_ms = start.elapsed().as_millis() as i32;
            upsert_log(&state, &log, &log_settings);
            return (StatusCode::BAD_REQUEST, "failed to parse request").into_response();
        }
    };

    let is_stream = chat_req.stream.unwrap_or(false);
    let requested_model = if chat_req.model.is_empty() { raw_model } else { chat_req.model.clone() };
    log.model = requested_model.clone();

    // ── 路由选择平台 + 模型映射 ──
    let route = select_platform(&state.db, &group, &chat_req.model);
    let route = match route {
        Ok(r) => r,
        Err(e) => {
            log.response_body = format!("route error: {e}");
            log.status_code = 400;
            log.duration_ms = start.elapsed().as_millis() as i32;
            upsert_log(&state, &log, &log_settings);
            return (StatusCode::BAD_REQUEST, format!("route: {e}")).into_response();
        }
    };

    let actual_model = route.target_model;

    // 尝试匹配端点：按 source_protocol 查找平台是否支持对应协议的端点
    let (target_protocol_enum, target_base_url, client_type, coding_plan) = route.platform.endpoints
        .iter()
        .find(|ep| {
            let ep_str = format!("{:?}", ep.protocol).to_lowercase();
            ep_str == source_protocol
        })
        .map(|ep| (&ep.protocol, ep.base_url.clone(), ep.client_type.clone(), ep.coding_plan))
        .unwrap_or((&route.platform.platform_type, route.platform.base_url.clone(), ClientType::Default, false));

    let target_protocol = format!("{:?}", target_protocol_enum).to_lowercase();
    let needs_model_remap = actual_model != requested_model;

    // Upsert #3: route resolved
    log.actual_model = actual_model.clone();
    log.target_protocol = target_protocol.clone();
    log.platform_id = route.platform.id;
    upsert_log(&state, &log, &log_settings);

    // 替换模型名
    chat_req.model = actual_model.clone();

    // ── Mock 平台拦截：不发真实上游，本地生成可控假响应 ──
    if matches!(route.platform.platform_type, Protocol::Mock) {
        return handle_mock(
            state,
            log,
            log_settings,
            &route.platform.extra,
            &chat_req,
            &req_value,
            &source_protocol,
            &requested_model,
            is_stream,
            start,
        )
        .await;
    }

    // ── Claude Code 纯透传拦截：bypass 所有转换，1:1 relay 客户端原始请求到 base_url ──
    if matches!(route.platform.platform_type, Protocol::ClaudeCode) {
        return handle_passthrough(
            &state,
            &mut log,
            &log_settings,
            orig_method,
            orig_uri,
            orig_headers,
            bytes,
            &route.platform.base_url,
            start,
        )
        .await;
    }

    // 协议转换：wire format 由 endpoint 协议决定，API path 由平台类型决定
    let platform_protocol = &route.platform.platform_type;
    let (mut req_body, mut api_path) = adapter::convert_request(&chat_req, target_protocol_enum, platform_protocol);

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
    let system_timeout = get_system_timeout(&state.db);
    let (req_timeout, conn_timeout) = resolve_timeout(&route.mapping, &group, &system_timeout);
    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(req_timeout))
        .connect_timeout(std::time::Duration::from_secs(conn_timeout))
        .build()
        .unwrap_or_else(|_| Client::new());

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

    let resp = match req_builder.send().await {
        Ok(r) => r,
        Err(e) => {
            log.response_body = format!("upstream error: {e}");
            log.status_code = 502;
            log.user_response_body = format!("upstream: {e}");
            log.user_response_headers = r#"{"content-type":"text/plain"}"#.to_string();
            log.duration_ms = start.elapsed().as_millis() as i32;
            upsert_log(&state, &log, &log_settings);
            return (StatusCode::BAD_GATEWAY, format!("upstream: {e}")).into_response();
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
        log.response_body = body.clone();
        log.status_code = status.as_u16() as i32;
        status.as_u16() as i32;
        log.user_response_body = body.clone();
        log.user_response_headers = log.upstream_response_headers.clone();
        log.duration_ms = start.elapsed().as_millis() as i32;
        upsert_log(&state, &log, &log_settings);
        return (StatusCode::from_u16(status.as_u16()).unwrap_or(StatusCode::BAD_GATEWAY), body)
            .into_response();
    }

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
        log.user_response_body = String::from_utf8_lossy(&body).to_string();
        log.user_response_headers = r#"{"content-type":"application/json"}"#.to_string();

        upsert_log(&state, &log, &log_settings);

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
        );

        return (
            StatusCode::OK,
            [(axum::http::header::CONTENT_TYPE, "application/json")],
            body.to_vec(),
        )
            .into_response();
    }

    // 流式：转换 SSE 格式为 Anthropic 格式返回
    let protocol = target_protocol_enum.clone();
    let tokens_acc = Arc::new(std::sync::atomic::AtomicI32::new(0));
    let tokens_out = Arc::new(std::sync::atomic::AtomicI32::new(0));
    let tokens_cache = Arc::new(std::sync::atomic::AtomicI32::new(0));
    let client_protocol = source_protocol.clone();
    let model_for_sse = requested_model.clone();
    let model_for_response = if needs_model_remap {
        requested_model.clone()
    } else {
        String::new()
    };
    let acc_in = tokens_acc.clone();
    let acc_out = tokens_out.clone();
    let acc_cache = tokens_cache.clone();

    // ── 流式预估：token 仅在流被消费完（[DONE]）才确定，故在闭包内 [DONE] 处触发 ──
    let est_state = state.clone();
    let est_platform_id = route.platform.id;
    let est_platform_type = route.platform.platform_type.clone();
    let est_base_url = route.platform.base_url.clone();
    let est_api_key = route.platform.api_key.clone();
    let est_model = actual_model.clone();
    let est_coding_plan = coding_plan;
    let est_in = tokens_acc.clone();
    let est_out = tokens_out.clone();
    let est_cache = tokens_cache.clone();
    let est_fired = Arc::new(std::sync::atomic::AtomicBool::new(false));

    // ── 流式日志最终 token 回写：upsert 在返回 stream 前发生（:641），此时 token 仍为 0；
    // token 仅在流被消费完（[DONE]）才累加完整，故 clone log/state/settings 进闭包，
    // 在 [DONE] 处用最终 token 再次 upsert（INSERT OR REPLACE，同 log.id 覆盖）。──
    let done_log = log.clone();
    let done_state = state.clone();
    let done_settings = log_settings.clone();
    let done_start = start;
    let done_in = tokens_acc.clone();
    let done_out = tokens_out.clone();
    let done_cache = tokens_cache.clone();

    let stream = resp.bytes_stream().map(move |chunk_result| {
        let chunk = match chunk_result {
            Ok(c) => c,
            Err(e) => return Ok::<_, std::io::Error>(format!("event: error\ndata: {{\"error\":\"{e}\"}}\n\n")),
        };

        let text = String::from_utf8_lossy(&chunk);
        let mut output = String::new();

        for line in text.lines() {
            if let Some(data) = line.strip_prefix("data: ") {
                if data.trim() == "[DONE]" {
                    output.push_str(&adapter::to_client_sse(&ChatStreamEvent::Stop {
                        finish_reason: Some("end_turn".to_string()),
                    }, &client_protocol, &model_for_sse).unwrap_or_default());
                    // 流终止：token 已累加完整 → 回写日志最终 token + 后台预估（仅触发一次）
                    if !est_fired.swap(true, std::sync::atomic::Ordering::Relaxed) {
                        // 用最终 token 覆盖日志（提前 :641 upsert 时 token 仍为 0）
                        let mut final_log = done_log.clone();
                        final_log.input_tokens = done_in.load(std::sync::atomic::Ordering::Relaxed);
                        final_log.output_tokens = done_out.load(std::sync::atomic::Ordering::Relaxed);
                        final_log.cache_tokens = done_cache.load(std::sync::atomic::Ordering::Relaxed);
                        final_log.status_code = 200;
                        final_log.duration_ms = done_start.elapsed().as_millis() as i32;
                        upsert_log(&done_state, &final_log, &done_settings);

                        spawn_estimate(
                            &est_state,
                            est_platform_id,
                            &est_platform_type,
                            est_base_url.clone(),
                            est_api_key.clone(),
                            est_model.clone(),
                            est_in.load(std::sync::atomic::Ordering::Relaxed),
                            est_out.load(std::sync::atomic::Ordering::Relaxed),
                            est_cache.load(std::sync::atomic::Ordering::Relaxed),
                            est_coding_plan,
                        );
                    }
                    continue;
                }

                if let Ok(json) = serde_json::from_str::<Value>(data) {
                    if let Some(usage) = json.get("usage") {
                        if let Some(i) = usage.get("prompt_tokens").and_then(|v| v.as_i64()) {
                            acc_in.store(i as i32, std::sync::atomic::Ordering::Relaxed);
                        }
                        if let Some(o) = usage.get("completion_tokens").and_then(|v| v.as_i64()).or_else(|| usage.get("output_tokens").and_then(|v| v.as_i64())) {
                            acc_out.store(o as i32, std::sync::atomic::Ordering::Relaxed);
                        }
                        if let Some(c) = usage.get("cache_read_input_tokens").and_then(|v| v.as_i64())
                            .or_else(|| usage.get("prompt_tokens_details").and_then(|d| d.get("cached_tokens")).and_then(|v| v.as_i64()))
                            .or_else(|| usage.get("cache_tokens").and_then(|v| v.as_i64()))
                        {
                            acc_cache.store(c as i32, std::sync::atomic::Ordering::Relaxed);
                        }
                    }

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

        Ok(output)
    });

    let body = Body::from_stream(stream);

    // Upsert final: stream complete
    log.status_code = 200;
    log.response_body = "[stream]".to_string();
    log.user_response_body = "[stream]".to_string();
    log.user_response_headers = r#"{"content-type":"text/event-stream","cache-control":"no-cache","connection":"keep-alive"}"#.to_string();
    log.duration_ms = start.elapsed().as_millis() as i32;
    log.input_tokens = tokens_acc.load(std::sync::atomic::Ordering::Relaxed);
    log.output_tokens = tokens_out.load(std::sync::atomic::Ordering::Relaxed);
    log.cache_tokens = tokens_cache.load(std::sync::atomic::Ordering::Relaxed);
    upsert_log(&state, &log, &log_settings);

    (
        StatusCode::OK,
        [
            (axum::http::header::CONTENT_TYPE, "text/event-stream"),
            (axum::http::header::CACHE_CONTROL, "no-cache"),
            (axum::http::header::CONNECTION, "keep-alive"),
        ],
        body,
    )
        .into_response()
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
            let body = mock::build_error_body(source_protocol, cfg.status_code, "mock http_error");
            let body_str = serde_json::to_string(&body).unwrap_or_default();
            let status = StatusCode::from_u16(cfg.status_code).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
            log.status_code = cfg.status_code as i32;
            log.duration_ms = start.elapsed().as_millis() as i32;
            log.response_body = body_str.clone();
            log.user_response_body = body_str.clone();
            log.user_response_headers = r#"{"content-type":"application/json"}"#.to_string();
            upsert_log(&state, &log, &log_settings);
            return (status, [(axum::http::header::CONTENT_TYPE, "application/json")], body_str).into_response();
        }
        "rate_limit_429" => {
            let body = mock::build_error_body(source_protocol, 429, "mock rate limit");
            let body_str = serde_json::to_string(&body).unwrap_or_default();
            log.status_code = 429;
            log.duration_ms = start.elapsed().as_millis() as i32;
            log.response_body = body_str.clone();
            log.user_response_body = body_str.clone();
            log.user_response_headers = r#"{"content-type":"application/json","retry-after":"5"}"#.to_string();
            upsert_log(&state, &log, &log_settings);
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
            // sleep 上限保护，不真 hang 连接
            tokio::time::sleep(std::time::Duration::from_secs(600)).await;
            let body = mock::build_error_body(source_protocol, 504, "mock timeout");
            let body_str = serde_json::to_string(&body).unwrap_or_default();
            log.status_code = 504;
            log.duration_ms = start.elapsed().as_millis() as i32;
            log.response_body = body_str.clone();
            log.user_response_body = body_str.clone();
            log.user_response_headers = r#"{"content-type":"application/json"}"#.to_string();
            upsert_log(&state, &log, &log_settings);
            return (StatusCode::GATEWAY_TIMEOUT, [(axum::http::header::CONTENT_TYPE, "application/json")], body_str)
                .into_response();
        }
        _ => {}
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
        upsert_log(&state, &log, &log_settings);

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
    upsert_log(&state, &log, &log_settings);

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
) -> Response {
    // 透传不转换协议，source/target 都标 claude_code
    log.source_protocol = "claude_code".to_string();
    log.target_protocol = "claude_code".to_string();

    // 目标 URL = base_url(host 根) + 客户端原始 path(+query)
    let url = build_passthrough_url(base_url, &orig_uri);
    log.upstream_request_url = url.clone();

    // 解析超时（系统级；透传无 group/model mapping 覆盖）
    let system_timeout = get_system_timeout(&state.db);
    let req_timeout = if system_timeout.request_timeout_secs > 0 { system_timeout.request_timeout_secs } else { 300 };
    let conn_timeout = if system_timeout.connect_timeout_secs > 0 { system_timeout.connect_timeout_secs } else { 10 };
    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(req_timeout))
        .connect_timeout(std::time::Duration::from_secs(conn_timeout))
        .build()
        .unwrap_or_else(|_| Client::new());

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

    let method = match reqwest::Method::from_bytes(orig_method.as_str().as_bytes()) {
        Ok(m) => m,
        Err(_) => reqwest::Method::POST,
    };
    let mut req_builder = client.request(method, &url).body(bytes.to_vec());
    req_builder = req_builder.headers(fwd_headers);

    let resp = match req_builder.send().await {
        Ok(r) => r,
        Err(e) => {
            log.response_body = format!("upstream error: {e}");
            log.status_code = 502;
            log.user_response_body = format!("upstream: {e}");
            log.user_response_headers = r#"{"content-type":"text/plain"}"#.to_string();
            log.duration_ms = start.elapsed().as_millis() as i32;
            upsert_log(state, log, log_settings);
            return (StatusCode::BAD_GATEWAY, format!("upstream: {e}")).into_response();
        }
    };

    let status = resp.status();
    log.upstream_status_code = status.as_u16() as i32;

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
        upsert_log(state, log, log_settings);

        let mut response = (resp_status, body.to_vec()).into_response();
        *response.headers_mut() = resp_header_map;
        return response;
    }

    // ── 流式：原样透传 SSE bytes，不解析不转换；尽力累计 token ──
    let tokens_in = Arc::new(std::sync::atomic::AtomicI32::new(0));
    let tokens_out = Arc::new(std::sync::atomic::AtomicI32::new(0));
    let tokens_cache = Arc::new(std::sync::atomic::AtomicI32::new(0));
    let acc_in = tokens_in.clone();
    let acc_out = tokens_out.clone();
    let acc_cache = tokens_cache.clone();

    let stream = resp.bytes_stream().map(move |chunk_result| {
        let chunk = match chunk_result {
            Ok(c) => c,
            Err(e) => return Err(std::io::Error::other(e.to_string())),
        };
        // 尽力从 SSE data 累计 usage（Anthropic / OpenAI 兼容字段），不改写 chunk
        let text = String::from_utf8_lossy(&chunk);
        for line in text.lines() {
            if let Some(data) = line.strip_prefix("data: ") {
                if data.trim() == "[DONE]" {
                    continue;
                }
                if let Ok(json) = serde_json::from_str::<Value>(data) {
                    accumulate_sse_usage(&json, &acc_in, &acc_out, &acc_cache);
                }
            }
        }
        Ok::<_, std::io::Error>(chunk)
    });

    let body = Body::from_stream(stream);

    log.status_code = status.as_u16() as i32;
    log.duration_ms = start.elapsed().as_millis() as i32;
    log.response_body = "[stream]".to_string();
    log.user_response_body = "[stream]".to_string();
    log.user_response_headers = log.upstream_response_headers.clone();
    log.input_tokens = tokens_in.load(std::sync::atomic::Ordering::Relaxed);
    log.output_tokens = tokens_out.load(std::sync::atomic::Ordering::Relaxed);
    log.cache_tokens = tokens_cache.load(std::sync::atomic::Ordering::Relaxed);
    upsert_log(state, log, log_settings);

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
/// - /v1/chat/completions, /v1/completions, /v1/responses, /models, /images, /audio → openai
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
    } else if api_path.starts_with("/v1/chat/completions")
        || api_path.starts_with("/v1/completions")
        || api_path.starts_with("/v1/responses")
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
fn resolve_group(db: &Db, name: Option<&str>, request_path: &str) -> Option<Group> {
    let groups = super::db::list_groups(db).ok()?;
    if let Some(name) = name {
        if let Some(idx) = groups.iter().position(|g| g.name == name) {
            return groups.into_iter().nth(idx);
        }
    }
    groups.into_iter().find(|g| request_path.starts_with(&g.path))
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
fn inject_coding_plan_fields(body: &mut Value, protocol: &super::models::Protocol) {
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
fn override_coding_plan_path(_api_path: &mut String, _protocol: &super::models::Protocol) {
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
}
