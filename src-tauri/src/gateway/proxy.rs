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
use super::models::{ClientType, Group, ProxyLog, ProxyTimeoutSettings};
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

/// Upsert a proxy log entry; silently ignore errors
fn upsert_log(state: &Arc<ProxyState>, log: &ProxyLog) {
    let db = match state.db.lock() {
        Ok(d) => d,
        Err(_) => return,
    };
    let _ = super::db::upsert_proxy_log(&db, log);
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
    let request_id = uuid::Uuid::new_v4().to_string();
    let created_at = chrono::Utc::now().to_rfc3339();

    // ── 初始化日志条目 ──
    let mut log = ProxyLog {
        id: request_id,
        group_name: String::new(),
        model: String::new(),
        actual_model: String::new(),
        source_protocol: String::new(),  // will be set from group
        target_protocol: String::new(),
        request_headers: String::new(),
        request_body: String::new(),
        upstream_request_headers: String::new(),
        upstream_request_body: String::new(),
        response_body: String::new(),
        status_code: 0,
        duration_ms: 0,
        input_tokens: 0,
        output_tokens: 0,
        cache_tokens: 0,
        created_at,
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

    // ── 读取请求体 ──
    let (_parts, body) = req.into_parts();
    let bytes = match axum::body::to_bytes(body, 10 * 1024 * 1024).await {
        Ok(b) => b,
        Err(e) => {
            log.response_body = format!("read body error: {e}");
            log.status_code = 400;
            log.duration_ms = start.elapsed().as_millis() as i32;
            upsert_log(&state, &log);
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
    upsert_log(&state, &log);

    // ── 查找分组 ──
    let group = {
        let db_result = state.db.lock().map_err(|e| e.to_string());
        match db_result {
            Ok(db) => {
                if let Some(ref token) = auth_header {
                    if let Some(g) = find_group_by_name(&db, token) {
                        g
                    } else {
                        match find_group_by_path(&db, &path) {
                            Some(g) => g,
                            None => {
                                log.response_body = format!("no matching group for token '{}' or path '{}'", token, path);
                                log.status_code = 404;
                                log.duration_ms = start.elapsed().as_millis() as i32;
                                upsert_log(&state, &log);
                                return (StatusCode::NOT_FOUND, log.response_body.clone()).into_response();
                            }
                        }
                    }
                } else {
                    match find_group_by_path(&db, &path) {
                        Some(g) => g,
                        None => {
                            log.response_body = "no matching group".to_string();
                            log.status_code = 404;
                            log.duration_ms = start.elapsed().as_millis() as i32;
                            upsert_log(&state, &log);
                            return (StatusCode::NOT_FOUND, "no matching group").into_response();
                        }
                    }
                }
            }
            Err(e) => {
                log.response_body = format!("db error: {e}");
                log.status_code = 500;
                log.duration_ms = start.elapsed().as_millis() as i32;
                upsert_log(&state, &log);
                return (StatusCode::INTERNAL_SERVER_ERROR, e).into_response();
            }
        }
    };

    // Upsert #2: group resolved
    log.group_name = group.name.clone();
    // Auto-detect source_protocol from request path if group default doesn't match
    let source_protocol = detect_source_protocol(&path, &group.source_protocol);
    log.source_protocol = source_protocol.clone();
    upsert_log(&state, &log);

    // ── 解析 ChatRequest（按入站协议解析） ──
    let req_value: serde_json::Value = match serde_json::from_slice(&bytes) {
        Ok(v) => v,
        Err(e) => {
            log.response_body = format!("parse request json error: {e}");
            log.status_code = 400;
            log.duration_ms = start.elapsed().as_millis() as i32;
            upsert_log(&state, &log);
            return (StatusCode::BAD_REQUEST, format!("parse json: {e}")).into_response();
        }
    };
    let mut chat_req: ChatRequest = match adapter::parse_incoming_request(&log.source_protocol, &req_value) {
        Some(r) => r,
        None => {
            log.response_body = "failed to parse request for protocol".to_string();
            log.status_code = 400;
            log.duration_ms = start.elapsed().as_millis() as i32;
            upsert_log(&state, &log);
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
                upsert_log(&state, &log);
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
            upsert_log(&state, &log);
            return (StatusCode::BAD_REQUEST, format!("route: {e}")).into_response();
        }
    };

    let actual_model = route.target_model;

    // 尝试匹配端点：按 source_protocol 查找平台是否支持对应协议的端点
    let (target_protocol_enum, target_base_url, client_type) = route.platform.endpoints
        .iter()
        .find(|ep| {
            let ep_str = format!("{:?}", ep.protocol).to_lowercase();
            ep_str == source_protocol
        })
        .map(|ep| (&ep.protocol, ep.base_url.clone(), ep.client_type.clone()))
        .unwrap_or((&route.platform.protocol, route.platform.base_url.clone(), ClientType::Default));

    let target_protocol = format!("{:?}", target_protocol_enum).to_lowercase();
    let needs_model_remap = actual_model != requested_model;

    // Upsert #3: route resolved
    log.actual_model = actual_model.clone();
    log.target_protocol = target_protocol.clone();
    upsert_log(&state, &log);

    // 替换模型名
    chat_req.model = actual_model.clone();

    // 协议转换
    let (req_body, api_path) = adapter::convert_request(&chat_req, target_protocol_enum);
    let req_body_str = serde_json::to_string(&req_body).unwrap_or_default();

    // 构建目标 URL
    let base_url = target_base_url.trim_end_matches('/');
    let url = format!("{}{}", base_url, api_path);

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
            log.duration_ms = start.elapsed().as_millis() as i32;
            upsert_log(&state, &log);
            return (StatusCode::BAD_GATEWAY, format!("upstream: {e}")).into_response();
        }
    };

    let status = resp.status();
    if !status.is_success() {
        let body = resp.text().await.unwrap_or_default();
        log.status_code = status.as_u16() as i32;
        log.response_body = body.clone();
        log.duration_ms = start.elapsed().as_millis() as i32;
        upsert_log(&state, &log);
        return (StatusCode::from_u16(status.as_u16()).unwrap_or(StatusCode::BAD_GATEWAY), body)
            .into_response();
    }

    // 非流式：直接透传 JSON
    if !is_stream {
        let body = resp.bytes().await.unwrap_or_default();
        let resp_str = String::from_utf8_lossy(&body).to_string();
        let (input_tokens, output_tokens, cache_tokens) = extract_usage(&resp_str);

        log.status_code = 200;
        log.response_body = resp_str;
        log.duration_ms = start.elapsed().as_millis() as i32;
        log.input_tokens = input_tokens;
        log.output_tokens = output_tokens;
        log.cache_tokens = cache_tokens;
        upsert_log(&state, &log);

        // Replace model in response back to original if remapped
        let body = if needs_model_remap {
            replace_model_in_json(&body, &requested_model)
        } else {
            body.to_vec()
        };

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
                    output.push_str(&match adapter::to_client_sse(&ChatStreamEvent::Stop {
                        finish_reason: Some("end_turn".to_string()),
                    }, &client_protocol, &model_for_sse) {
                        Some(s) => s,
                        None => String::new(),
                    });
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
    log.duration_ms = start.elapsed().as_millis() as i32;
    log.input_tokens = tokens_acc.load(std::sync::atomic::Ordering::Relaxed);
    log.output_tokens = tokens_out.load(std::sync::atomic::Ordering::Relaxed);
    log.cache_tokens = tokens_cache.load(std::sync::atomic::Ordering::Relaxed);
    upsert_log(&state, &log);

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
/// 回退到分组默认的 source_protocol
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

/// 根据 path 前缀匹配分组
fn find_group_by_path(db: &Db, request_path: &str) -> Option<Group> {
    let groups = super::db::list_groups(db).ok()?;
    groups.into_iter().find(|g| request_path.starts_with(&g.path))
}

/// 根据 group name 精确匹配分组
fn find_group_by_name(db: &Db, name: &str) -> Option<Group> {
    let groups = super::db::list_groups(db).ok()?;
    groups.into_iter().find(|g| g.name == name)
}

// ─── 客户端模拟 Header ────────────────────────────────────────

/// 根据客户端类型和目标协议，构建模拟的 HTTP 请求头。
/// 数据来源：GitHub 逆向分析（Claude Code CLI、Codex CLI、Cursor、Windsurf）
fn apply_client_headers(
    req_builder: reqwest::RequestBuilder,
    client_type: &ClientType,
    protocol: &super::models::Protocol,
    api_key: &str,
) -> reqwest::RequestBuilder {
    match client_type {
        ClientType::Default => apply_default_headers(req_builder, protocol, api_key),
        ClientType::ClaudeCode => apply_claude_code_headers(req_builder, protocol, api_key),
        ClientType::CodexCli => apply_codex_cli_headers(req_builder, protocol, api_key),
        ClientType::Cursor => apply_cursor_headers(req_builder, protocol, api_key),
        ClientType::Windsurf => apply_windsurf_headers(req_builder, protocol, api_key),
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

/// 模拟 Claude Code CLI 请求头
/// 来源: @anthropic-ai/claude-code/cli.js — buildHeaders() + fV()
fn apply_claude_code_headers(
    mut rb: reqwest::RequestBuilder,
    protocol: &super::models::Protocol,
    api_key: &str,
) -> reqwest::RequestBuilder {
    // Anthropic 协议完整模拟
    rb = rb
        .header("User-Agent", "claude-cli/1.0.117 (external, undefined)")
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

/// 模拟 Codex CLI 请求头
/// 来源: codex-rs/core/src/default_client.rs + model_provider_info.rs + client.rs
fn apply_codex_cli_headers(
    mut rb: reqwest::RequestBuilder,
    protocol: &super::models::Protocol,
    api_key: &str,
) -> reqwest::RequestBuilder {
    rb = rb
        .header("User-Agent", "codex_cli_rs/0.38.0 (MacOS; arm64) Terminal")
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
        ((ts >> 16) as u16) & 0xffff,
        ((ts >> 32) as u16) & 0x0fff,
        ((ts >> 48) as u16) | 0x8000,
        ((ts >> 60) as u64) & 0xffffffffffff,
    )
}

/// 构建上游请求头 KV 表（用于日志记录，与 apply_client_headers 保持一致）
fn build_upstream_headers(client_type: &ClientType, protocol: &super::models::Protocol, api_key: &str) -> Vec<(String, String)> {
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
        ClientType::ClaudeCode => {
            h.push(("User-Agent".into(), "claude-cli/1.0.117 (external, undefined)".into()));
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
        ClientType::CodexCli => {
            h.push(("User-Agent".into(), "codex_cli_rs/0.38.0 (MacOS; arm64) Terminal".into()));
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
fn redact_key(key: &str) -> String {
    if key.len() <= 12 {
        "[REDACTED]".into()
    } else {
        format!("{}****{}", &key[..4], &key[key.len()-4..])
    }
}

/// Pretty-print JSON string; return original if parsing fails
fn format_pretty_json(s: &str) -> String {
    serde_json::from_str::<Value>(s)
        .ok()
        .and_then(|v| serde_json::to_string_pretty(&v).ok())
        .unwrap_or_else(|| s.to_string())
}
