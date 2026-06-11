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
    pub db: std::sync::Mutex<Db>,
}

/// 启动代理服务器，返回 shutdown handle
pub async fn start_proxy(
    db: std::sync::Mutex<Db>,
    port: u16,
) -> Result<(tokio::task::JoinHandle<()>, u16), String> {
    let state = Arc::new(ProxyState { db });

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
    let db = match state.db.lock() {
        Ok(d) => d,
        Err(_) => return,
    };
    let _ = super::db::upsert_proxy_log(&db, &log);
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
    let log_settings = {
        let db = match state.db.lock() {
            Ok(d) => d,
            Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, "db lock").into_response(),
        };
        get_log_settings(&db)
    };

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
        let db_result = state.db.lock().map_err(|e| e.to_string());
        match db_result {
            Ok(db) => {
                match resolve_group(&db, auth_header.as_deref(), &path) {
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
            }
            Err(e) => {
                log.response_body = format!("db error: {e}");
                log.status_code = 500;
                log.duration_ms = start.elapsed().as_millis() as i32;
                upsert_log(&state, &log, &log_settings);
                return (StatusCode::INTERNAL_SERVER_ERROR, e).into_response();
            }
        }
    };

    // Upsert #2: group resolved
    log.group_name = group.name.clone();
    // Auto-detect source_protocol from request path if group default doesn't match
    let source_protocol = detect_source_protocol(&path, &group.source_protocol);
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
    let route = {
        let db_result = state.db.lock().map_err(|e| e.to_string());
        match db_result {
            Ok(db) => select_platform(&db, &group, &chat_req.model),
            Err(e) => {
                log.response_body = format!("db lock error: {e}");
                log.status_code = 500;
                log.duration_ms = start.elapsed().as_millis() as i32;
                upsert_log(&state, &log, &log_settings);
                return (StatusCode::INTERNAL_SERVER_ERROR, e).into_response();
            }
        }
    };
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
    let system_timeout = match state.db.lock() {
        Ok(db) => get_system_timeout(&db),
        Err(_) => ProxyTimeoutSettings::default(),
    };
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
///   回退到分组默认的 source_protocol
fn detect_source_protocol(path: &str, default: &str) -> String {
    // Strip group path prefix (e.g. /proxy/v1/chat/completions → /v1/chat/completions)
    let api_path = if let Some(idx) = path.find("/v1/") {
        &path[idx..]
    } else if path.contains("/v1beta/") {
        return "gemini".to_string();
    } else {
        return default.to_string();
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
        default.to_string()
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
