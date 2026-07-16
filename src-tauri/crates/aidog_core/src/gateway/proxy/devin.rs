use super::*;

/// Devin（Cognition）平台请求处理：chat ↔ session 协议转换。
///
/// Devin 是 stateful session API（create → poll → fetch output），非标准 chat completions wire。
/// 接入走 handler.rs Protocol::Devin 分支直接调本 handler，不经 forward_attempt/adapter/converter。
///
/// 生命周期（详见 research/devin-api-lifecycle.md）：
///   1. create_session: POST /sessions {prompt, devin_mode, ...} → session_id
///   2. poll_session: GET /sessions/{id} 轮询到终态（exit | error | suspended）
///   3. get_messages: GET /sessions/{id}/messages → 取最后 source==devin 的 message 作输出
///
/// **s2 仅骨架**：HTTP 框架 + 占位响应，真实转换逻辑（messages→prompt 拼接 / model→mode 映射 /
/// 响应包装 / 伪流式 / 超时 504 / stateful 字段映射）由 s3 起填充，TODO(s3) 标记。
#[allow(clippy::too_many_arguments)]
pub(crate) async fn handle_devin(
    state: Arc<ProxyState>,
    mut log: ProxyLog,
    log_settings: ProxyLogSettings,
    platform: &models::Platform,
    chat_req: &ChatRequest,
    _req_value: &Value,
    source_protocol: &str,
    requested_model: &str,
    is_stream: bool,
    start: std::time::Instant,
    lang: Lang,
) -> Response {
    log.target_protocol = "devin".to_string();
    log.platform_id = platform.id;
    log.actual_model = requested_model.to_string();

    // org_id 必填（path 段 + Bearer realm），api_key = Devin cog_ key。
    let extra_v: Value = serde_json::from_str(&platform.extra).unwrap_or_else(|_| Value::Object(Default::default()));
    let org_id = match extra_v.get("org_id").and_then(|v| v.as_str()) {
        Some(id) if !id.is_empty() => id.to_string(),
        _ => {
            return devin_error(
                &state, &mut log, &log_settings, lang, start,
                StatusCode::BAD_REQUEST,
                "devin platform missing extra.org_id",
            ).await;
        }
    };
    let api_key = platform.api_key.trim();
    if api_key.is_empty() {
        return devin_error(
            &state, &mut log, &log_settings, lang, start,
            StatusCode::BAD_REQUEST,
            "devin platform missing api_key",
        ).await;
    }
    // base = base_url（platform.base_url 已含 /v3/organizations/{org_id}；缺失则用官方 host 拼）。
    // ponytail: base_url 真值源单一——preset / 用户已含完整 path，仅空兜底官方 host。
    let base_url = if platform.base_url.trim().is_empty() {
        format!("https://api.devin.ai/v3/organizations/{org_id}")
    } else {
        platform.base_url.trim().trim_end_matches('/').to_string()
    };

    // 超时 + proxy_client 一次缓存借齐（与 passthrough 同款）。
    let (system_timeout, proxy_client) = {
        let c = state.settings_cache.read().await;
        (c.system_timeout.clone(), c.proxy_client.clone())
    };
    let conn_timeout = if system_timeout.connect_timeout_secs > 0 { system_timeout.connect_timeout_secs } else { 10 };
    // TODO(s4): Devin session 轮询可达分钟级；这里先用系统 request_timeout，s4 再加 session 专用超时 + 504。
    let req_timeout = if system_timeout.request_timeout_secs > 0 { system_timeout.request_timeout_secs } else { 300 };
    let client = http_client::build_http_client(
        &proxy_client, req_timeout, conn_timeout,
        Some(&platform.extra), None,
    ).await;

    // ── 1. create session ──
    let session_id = match create_session(&client, &base_url, api_key, chat_req, requested_model).await {
        Ok(id) => id,
        Err(e) => {
            tracing::warn!(platform_id = platform.id, error = %e, "devin create_session failed");
            return devin_error(
                &state, &mut log, &log_settings, lang, start,
                StatusCode::BAD_GATEWAY,
                &format!("devin create_session error: {e}"),
            ).await;
        }
    };
    tracing::info!(platform_id = platform.id, session_id = %session_id, "devin session created");

    // ── 2. poll until terminal ──
    // TODO(s4): 轮询间隔 / 最大时长 / 伪流式进度推送。
    let final_state = match poll_session(&client, &base_url, api_key, &session_id).await {
        Ok(s) => s,
        Err(e) => {
            tracing::warn!(platform_id = platform.id, session_id = %session_id, error = %e, "devin poll_session failed");
            return devin_error(
                &state, &mut log, &log_settings, lang, start,
                StatusCode::BAD_GATEWAY,
                &format!("devin poll_session error: {e}"),
            ).await;
        }
    };
    tracing::info!(platform_id = platform.id, session_id = %session_id, final_status = ?final_state.status, "devin session terminal");

    // ── 3. fetch output messages ──
    let output = match get_messages(&client, &base_url, api_key, &session_id).await {
        Ok(text) => text,
        Err(e) => {
            tracing::warn!(platform_id = platform.id, session_id = %session_id, error = %e, "devin get_messages failed");
            return devin_error(
                &state, &mut log, &log_settings, lang, start,
                StatusCode::BAD_GATEWAY,
                &format!("devin get_messages error: {e}"),
            ).await;
        }
    };

    // TODO(s3): 真实 chat response 包装（按 source_protocol openai/anthropic 格式化 content + usage）。
    // TODO(s3): token / ACU 计费落 log（input/output_tokens + calc_est_cost）。
    // TODO(s5): 伪流式（is_stream=true 时把 output 切片成 SSE）。
    let _ = (is_stream, source_protocol);
    let body = serde_json::json!({
        "id": session_id,
        "model": requested_model,
        "choices": [{
            "message": { "role": "assistant", "content": output },
            "finish_reason": "stop",
        }],
        "usage": { "prompt_tokens": 0, "completion_tokens": 0, "total_tokens": 0 },
    });
    let body_str = body.to_string();

    log.status_code = 200;
    log.input_tokens = 0;
    log.output_tokens = 0;
    log.duration_ms = start.elapsed().as_millis() as i32;
    log.response_body = body_str.clone();
    log.user_response_body = body_str.clone();
    log.user_response_headers = r#"{"content-type":"application/json"}"#.to_string();
    upsert_log(&state, &log, &log_settings).await;

    let mut r = (StatusCode::OK, [(axum::http::header::CONTENT_TYPE, "application/json")], body_str).into_response();
    inject_trace_header(&mut r);
    r
}

// ── Devin session HTTP 客户端骨架 ──
// 三个 async fn 发真实 HTTP（Bearer cog_ key），解析用 serde_json::Value 占位；
// 真实字段映射 / 错误模型 / 轮询策略由 s3+ 填充。

/// POST /sessions → session_id（devin- 前缀）。
async fn create_session(
    client: &reqwest::Client,
    base_url: &str,
    api_key: &str,
    chat_req: &ChatRequest,
    requested_model: &str,
) -> Result<String, String> {
    let url = format!("{base_url}/sessions");
    // TODO(s3): chat messages → 单 prompt 拼接（Devin 不收 messages 数组，需把 messages + system 折叠成单条 prompt）。
    let prompt = chat_req.messages.first()
        .map(|m| match &m.content {
            adapter::types::MessageContent::Text(s) => s.clone(),
            adapter::types::MessageContent::Blocks(blocks) => {
                // 占位：取首个 text block；s3 补多 block 拼接。
                blocks.iter().find_map(|b| match b {
                    adapter::types::ContentBlock::Text { text } => Some(text.clone()),
                    _ => None,
                }).unwrap_or_default()
            }
        })
        .unwrap_or_default();
    let _ = &chat_req.system; // TODO(s3): system prompt 折叠进 Devin prompt。
    // TODO(s3): model → devin_mode 映射（normal/fast/lite/ultra/fusion → devin-normal/...）。
    let devin_mode = map_devin_mode(requested_model);
    let body = serde_json::json!({
        "prompt": prompt,
        "devin_mode": devin_mode,
    });
    let resp = client.post(&url)
        .bearer_auth(api_key)
        .json(&body)
        .send().await
        .map_err(|e| format!("http: {e}"))?;
    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(format!("status {}: {}", status.as_u16(), text));
    }
    let v: Value = resp.json().await.map_err(|e| format!("decode: {e}"))?;
    // TODO(s3): 确认字段名（session_id / sessionID / id），按真实 schema 取。
    v.get("session_id").and_then(|x| x.as_str()).map(String::from)
        .ok_or_else(|| format!("missing session_id in response: {v}"))
}

/// GET /sessions/{id} → 终态 status（exit | error | suspended）。
#[derive(Debug)]
pub(crate) struct DevinSessionState {
    pub status: String,
}

async fn poll_session(
    client: &reqwest::Client,
    base_url: &str,
    api_key: &str,
    session_id: &str,
) -> Result<DevinSessionState, String> {
    let url = format!("{base_url}/sessions/{session_id}");
    // TODO(s4): 真实轮询循环（间隔 / 最大次数 / 超时 504）。s2 仅单次探测占位。
    let resp = client.get(&url)
        .bearer_auth(api_key)
        .send().await
        .map_err(|e| format!("http: {e}"))?;
    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(format!("status {}: {}", status.as_u16(), text));
    }
    let v: Value = resp.json().await.map_err(|e| format!("decode: {e}"))?;
    let status = v.get("status").and_then(|x| x.as_str()).unwrap_or("running").to_string();
    Ok(DevinSessionState { status })
}

/// GET /sessions/{id}/messages → 最后一条 source==devin 的 message。
async fn get_messages(
    client: &reqwest::Client,
    base_url: &str,
    api_key: &str,
    session_id: &str,
) -> Result<String, String> {
    let url = format!("{base_url}/sessions/{session_id}/messages");
    let resp = client.get(&url)
        .bearer_auth(api_key)
        .send().await
        .map_err(|e| format!("http: {e}"))?;
    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(format!("status {}: {}", status.as_u16(), text));
    }
    let v: Value = resp.json().await.map_err(|e| format!("decode: {e}"))?;
    // TODO(s3): messages 数组取最后 source==devin 的 message 字段。
    if let Some(arr) = v.as_array() {
        for m in arr.iter().rev() {
            let is_devin = m.get("source").and_then(|s| s.as_str()).map(|s| s == "devin").unwrap_or(false);
            if is_devin
                && let Some(text) = m.get("message").and_then(|x| x.as_str())
            {
                return Ok(text.to_string());
            }
        }
    }
    Ok(String::new())
}

/// model → Devin devin_mode 占位映射。TODO(s3): 按 5 档虚拟模型补全。
fn map_devin_mode(model: &str) -> &'static str {
    match model {
        m if m.contains("fast") => "fast",
        m if m.contains("lite") => "lite",
        m if m.contains("ultra") => "ultra",
        m if m.contains("fusion") => "fusion",
        _ => "normal",
    }
}

/// Devin 错误统一落库 + 响应。
async fn devin_error(
    state: &Arc<ProxyState>,
    log: &mut ProxyLog,
    log_settings: &ProxyLogSettings,
    lang: Lang,
    start: std::time::Instant,
    status: StatusCode,
    msg: &str,
) -> Response {
    let _ = lang;
    log.status_code = status.as_u16() as i32;
    log.response_body = msg.to_string();
    log.user_response_body = msg.to_string();
    log.user_response_headers = r#"{"content-type":"text/plain"}"#.to_string();
    log.duration_ms = start.elapsed().as_millis() as i32;
    upsert_log(state, log, log_settings).await;
    let mut r = (status, msg.to_string()).into_response();
    inject_trace_header(&mut r);
    r
}
