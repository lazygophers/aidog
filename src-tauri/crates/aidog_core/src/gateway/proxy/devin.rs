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
/// **s3 已填充真实转换逻辑**（messages→prompt 拼接 / model→mode 映射 / 轮询循环 /
/// chat response 包装 / usage+est_cost 落库）。**未实现**：伪流式（s4，is_stream=true
/// 走非流式占位）/ stateful X-Devin-Session-Id 映射（s5，每次新建 session）/ 超时 504 body
/// 完善 + extra 可配（s6，超时先简单 502 占位）。
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

    // ── 转换 1: messages → prompt（system 折叠 + role 标注 + multi-block text 提取）──
    let prompt = match build_prompt(chat_req) {
        Ok(p) => p,
        Err(e) => {
            return devin_error(
                &state, &mut log, &log_settings, lang, start,
                StatusCode::BAD_REQUEST,
                &format!("devin messages→prompt: {e}"),
            ).await;
        }
    };

    // ── 转换 3: tools 丢弃 + warn（契约 8：禁崩，仅记日志）──
    if let Some(tools) = &chat_req.tools
        && !tools.is_empty()
    {
        tracing::warn!(
            platform_id = platform.id,
            tool_count = tools.len(),
            "devin: chat_req.tools 非空但 Devin 不支持工具，丢弃（契约 8）"
        );
    }

    // 超时 + proxy_client 一次缓存借齐（与 passthrough 同款）。
    let (system_timeout, proxy_client) = {
        let c = state.settings_cache.read().await;
        (c.system_timeout.clone(), c.proxy_client.clone())
    };
    let conn_timeout = if system_timeout.connect_timeout_secs > 0 { system_timeout.connect_timeout_secs } else { 10 };
    // TODO(s6): Devin session 轮询可达分钟级；这里先用系统 request_timeout，s6 再加 session 专用超时 + 504 body。
    let req_timeout = if system_timeout.request_timeout_secs > 0 { system_timeout.request_timeout_secs } else { 300 };
    let client = http_client::build_http_client(
        &proxy_client, req_timeout, conn_timeout,
        Some(&platform.extra), None,
    ).await;

    // ── 1. create session ──
    let session_id = match create_session(&client, &base_url, api_key, &prompt, requested_model).await {
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

    // ── 2. poll until terminal (10s 间隔 + 300s 上限) ──
    // TODO(s6): 超时返 504 + 结构化 body；s3 先简单 Err → 502 占位。
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
    tracing::info!(
        platform_id = platform.id,
        session_id = %session_id,
        final_status = %final_state.status,
        status_detail = ?final_state.status_detail,
        acus_consumed = final_state.acus_consumed,
        "devin session terminal"
    );

    // ── 终态分流：exit 正常 / error 报错 / suspended 报欠费 ──
    let acus_consumed = final_state.acus_consumed;
    let content = match final_state.status.as_str() {
        "exit" => {
            // 正常完成 → 取 messages
            match get_messages(&client, &base_url, api_key, &session_id).await {
                Ok(text) => text,
                Err(e) => {
                    tracing::warn!(platform_id = platform.id, session_id = %session_id, error = %e, "devin get_messages failed");
                    return devin_error(
                        &state, &mut log, &log_settings, lang, start,
                        StatusCode::BAD_GATEWAY,
                        &format!("devin get_messages error: {e}"),
                    ).await;
                }
            }
        }
        "error" => {
            // 会话异常 → 客户端可见错误
            log.status_code = StatusCode::BAD_GATEWAY.as_u16() as i32;
            let body = format_chat_error_body(source_protocol, &session_id, "Devin session error");
            log.response_body = body.clone();
            log.user_response_body = body.clone();
            log.input_tokens = 0;
            log.output_tokens = 0;
            log.est_cost = acus_consumed; // 契约 9: 即使失败也记 ACU 消耗
            log.duration_ms = start.elapsed().as_millis() as i32;
            log.user_response_headers = r#"{"content-type":"application/json"}"#.to_string();
            upsert_log(&state, &log, &log_settings).await;
            let mut r = (StatusCode::BAD_GATEWAY, [(axum::http::header::CONTENT_TYPE, "application/json")], body).into_response();
            inject_trace_header(&mut r);
            return r;
        }
        "suspended" => {
            // 欠费 / 限额 / 停滞 → 可读消息
            let detail = final_state.status_detail.as_deref().unwrap_or("unknown");
            let human = suspended_human_message(detail);
            tracing::warn!(platform_id = platform.id, session_id = %session_id, status_detail = %detail, "devin session suspended");
            let body = format_chat_error_body(source_protocol, &session_id, &human);
            log.status_code = StatusCode::PAYMENT_REQUIRED.as_u16() as i32;
            log.response_body = body.clone();
            log.user_response_body = body.clone();
            log.input_tokens = 0;
            log.output_tokens = 0;
            log.est_cost = acus_consumed;
            log.duration_ms = start.elapsed().as_millis() as i32;
            log.user_response_headers = r#"{"content-type":"application/json"}"#.to_string();
            upsert_log(&state, &log, &log_settings).await;
            let mut r = (StatusCode::PAYMENT_REQUIRED, [(axum::http::header::CONTENT_TYPE, "application/json")], body).into_response();
            inject_trace_header(&mut r);
            return r;
        }
        other => {
            // 兜底：未预期非终态 status（理论上 poll_session 只返终态）
            tracing::error!(platform_id = platform.id, session_id = %session_id, status = %other, "devin unexpected non-terminal status reached handler");
            return devin_error(
                &state, &mut log, &log_settings, lang, start,
                StatusCode::BAD_GATEWAY,
                &format!("devin unexpected status: {other}"),
            ).await;
        }
    };

    // ── 4. 包 chat response（按 source_protocol openai/anthropic 格式化）──
    // TODO(s4): is_stream=true 时把 content 切成 SSE 块（伪流式），s3 先走非流式占位。
    let _ = is_stream;
    let body = format_chat_response(source_protocol, &session_id, requested_model, &content, acus_consumed);
    let body_str = body.to_string();

    // ── 5. usage + est_cost 落 log（契约 9: est_cost=acus_consumed 禁 $ 折算）──
    log.status_code = 200;
    log.input_tokens = 0;
    log.output_tokens = 0;
    log.est_cost = acus_consumed;
    log.duration_ms = start.elapsed().as_millis() as i32;
    log.response_body = body_str.clone();
    log.user_response_body = body_str.clone();
    log.user_response_headers = r#"{"content-type":"application/json"}"#.to_string();
    upsert_log(&state, &log, &log_settings).await;

    let mut r = (StatusCode::OK, [(axum::http::header::CONTENT_TYPE, "application/json")], body_str).into_response();
    inject_trace_header(&mut r);
    r
}

// ── 转换 / 格式化纯函数（便于单测）──

/// chat messages → Devin prompt 单字符串。
///
/// 规则：
///   - system（chat_req.system 或 messages 中 role=system）折叠为开头 `[system] ...` 前缀
///   - user/assistant/tool 按 `[<role>] content` 顺序拼接，`\n\n` 分隔
///   - content 为 array（multi-block）时取 text block 拼接，非 text block 丢弃 + warn
///   - 空消息（无任何 text） → Err，调用方返 400
pub(crate) fn build_prompt(chat_req: &ChatRequest) -> Result<String, String> {
    use crate::gateway::adapter::types::{Role, SystemContent};
    let mut parts: Vec<String> = Vec::new();

    // 顶层 system（Anthropic-style 独立字段）
    if let Some(sys) = &chat_req.system {
        let sys_text = match sys {
            SystemContent::Text(s) => Some(s.clone()),
            SystemContent::Blocks(blocks) => {
                let mut buf = String::new();
                for b in blocks {
                    if let Some(t) = b.get("text").and_then(|x| x.as_str()) {
                        buf.push_str(t);
                        buf.push('\n');
                    } else {
                        tracing::warn!(?b, "devin build_prompt: 非文本 system block 丢弃");
                    }
                }
                if buf.is_empty() { None } else { Some(buf.trim_end().to_string()) }
            }
        };
        if let Some(t) = sys_text
            && !t.trim().is_empty()
        {
            parts.push(format!("[system] {t}"));
        }
    }

    for m in &chat_req.messages {
        let role_str = match m.role {
            Role::User => "user",
            Role::Assistant => "assistant",
            Role::System => "system",
            Role::Tool => "tool",
        };
        let content = extract_message_text(&m.content);
        if content.trim().is_empty() {
            continue;
        }
        parts.push(format!("[{role_str}] {content}"));
    }

    if parts.is_empty() {
        return Err("empty messages (no text content)".into());
    }
    Ok(parts.join("\n\n"))
}

/// 从 MessageContent 取文本：Text 原样；Blocks 拼 text block，非 text 丢弃 + warn。
pub(crate) fn extract_message_text(content: &crate::gateway::adapter::types::MessageContent) -> String {
    use crate::gateway::adapter::types::{ContentBlock, MessageContent};
    match content {
        MessageContent::Text(s) => s.clone(),
        MessageContent::Blocks(blocks) => {
            let mut buf = String::new();
            for b in blocks {
                match b {
                    ContentBlock::Text { text } => {
                        buf.push_str(text);
                        buf.push('\n');
                    }
                    other => {
                        tracing::warn!(block = ?other, "devin extract_message_text: 非文本 block 丢弃（契约 8）");
                    }
                }
            }
            buf.trim_end().to_string()
        }
    }
}

/// model → devin_mode 5 档映射。
/// preset devin 模型 id: devin-normal/fast/lite/ultra/fusion（s1）。
/// 未知 model → normal（可被 extra.devin_mode 覆盖，由调用方实现；本 fn 只负责映射）。
pub(crate) fn map_devin_mode(model: &str) -> &'static str {
    match model {
        m if m.contains("fast") => "fast",
        m if m.contains("lite") => "lite",
        m if m.contains("ultra") => "ultra",
        m if m.contains("fusion") => "fusion",
        _ => "normal",
    }
}

/// Devin 终态判定：exit | error | suspended。
/// 其他（new/claimed/running/resuming 等）= 可继续，轮询继续。
pub(crate) fn is_terminal_status(status: &str) -> bool {
    matches!(status, "exit" | "error" | "suspended")
}

/// suspended status_detail → 用户可读消息（欠费 / 限额 / 停滞）。
/// 未知 detail → 通用 fallback。
pub(crate) fn suspended_human_message(detail: &str) -> String {
    let lower = detail.to_lowercase();
    if lower.contains("out_of_credits") || lower.contains("usage_limit_exceeded") || lower.contains("out_of_quota") {
        "Devin session suspended: out of credits / quota exceeded".to_string()
    } else if lower.contains("inactivity") {
        "Devin session suspended due to inactivity".to_string()
    } else {
        format!("Devin session suspended: {detail}")
    }
}

/// 按入站 source_protocol 格式化非流式 chat response。
///
/// - openai (/chat/completions) → OpenAI ChatCompletion 形态
/// - anthropic (/v1/messages) → Anthropic Message 形态
/// - 其他（gemini/openai_responses/...）→ 暂用 openai 形态兜底（Devin 接入主流是 openai/anthropic 两协议）
///
/// usage tokens 全 0（Devin 按 ACU 计费非 token，详见契约 9），acus_consumed 落 est_cost 不落 usage。
pub(crate) fn format_chat_response(
    source_protocol: &str,
    session_id: &str,
    requested_model: &str,
    content: &str,
    _acus_consumed: f64,
) -> Value {
    match source_protocol {
        "anthropic" => serde_json::json!({
            "id": format!("msg_{session_id}"),
            "type": "message",
            "role": "assistant",
            "content": [{ "type": "text", "text": content }],
            "model": requested_model,
            "stop_reason": "end_turn",
            "usage": { "input_tokens": 0, "output_tokens": 0 }
        }),
        _ => {
            // openai / 兜底
            serde_json::json!({
                "id": format!("chatcmpl-{session_id}"),
                "object": "chat.completion",
                "model": requested_model,
                "choices": [{
                    "index": 0,
                    "message": { "role": "assistant", "content": content },
                    "finish_reason": "stop"
                }],
                "usage": { "prompt_tokens": 0, "completion_tokens": 0, "total_tokens": 0 }
            })
        }
    }
}

/// 按入站 source_protocol 格式化错误 body（error / suspended 终态用）。
pub(crate) fn format_chat_error_body(source_protocol: &str, session_id: &str, msg: &str) -> String {
    match source_protocol {
        "anthropic" => serde_json::json!({
            "type": "error",
            "error": { "type": "api_error", "message": msg, "session_id": session_id }
        }).to_string(),
        _ => serde_json::json!({
            "error": { "message": msg, "type": "devin_session_error", "session_id": session_id }
        }).to_string(),
    }
}

// ── Devin session HTTP 客户端 ──

/// POST /sessions → session_id（devin- 前缀）。
async fn create_session(
    client: &reqwest::Client,
    base_url: &str,
    api_key: &str,
    prompt: &str,
    requested_model: &str,
) -> Result<String, String> {
    let url = format!("{base_url}/sessions");
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
    v.get("session_id").and_then(|x| x.as_str()).map(String::from)
        .ok_or_else(|| format!("missing session_id in response: {v}"))
}

/// GET /sessions/{id} 终态信息。
#[derive(Debug, Clone)]
pub(crate) struct DevinSessionState {
    pub status: String,
    pub status_detail: Option<String>,
    pub acus_consumed: f64,
}

/// 轮询 GET /sessions/{id} 直到终态（exit/error/suspended）或超时（300s）。
///
/// 间隔 10s（tokio::time::sleep）。超时上限硬编 300s（TODO(s6) extra 可配）。
/// 每次轮询解析 status / status_detail / acus_consumed；非终态则 sleep 后重试。
async fn poll_session(
    client: &reqwest::Client,
    base_url: &str,
    api_key: &str,
    session_id: &str,
) -> Result<DevinSessionState, String> {
    let url = format!("{base_url}/sessions/{session_id}");
    // TODO(s6): 上限改 extra.devin_timeout，超时返 504 body。
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(300);
    loop {
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
        let status_detail = v.get("status_detail").and_then(|x| x.as_str()).map(String::from);
        let acus_consumed = v.get("acus_consumed").and_then(|x| x.as_f64()).unwrap_or(0.0);
        if is_terminal_status(&status) {
            return Ok(DevinSessionState { status, status_detail, acus_consumed });
        }
        if std::time::Instant::now() >= deadline {
            return Err(format!("poll timeout (300s), last status: {status}"));
        }
        tokio::time::sleep(std::time::Duration::from_secs(10)).await;
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gateway::adapter::types::{
        ChatRequest, ContentBlock, Message, MessageContent, Role, SystemContent,
    };
    use serde_json::json;

    fn msg(role: Role, text: &str) -> Message {
        Message { role, content: MessageContent::Text(text.into()) }
    }

    // ── build_prompt ──

    #[test]
    fn build_prompt_single_user() {
        let req = ChatRequest {
            model: "devin-normal".into(),
            messages: vec![msg(Role::User, "hello")],
            system: None,
            max_tokens: None, temperature: None, top_p: None,
            stream: None, tools: None, tool_choice: None, extra: None,
        };
        let p = build_prompt(&req).unwrap();
        assert_eq!(p, "[user] hello");
    }

    #[test]
    fn build_prompt_multi_role_tagged_and_system_folded() {
        let req = ChatRequest {
            model: "devin-normal".into(),
            messages: vec![
                msg(Role::User, "hi"),
                msg(Role::Assistant, "hello"),
                msg(Role::User, "how are you"),
            ],
            system: Some(SystemContent::Text("be concise".into())),
            max_tokens: None, temperature: None, top_p: None,
            stream: None, tools: None, tool_choice: None, extra: None,
        };
        let p = build_prompt(&req).unwrap();
        assert_eq!(p, "[system] be concise\n\n[user] hi\n\n[assistant] hello\n\n[user] how are you");
    }

    #[test]
    fn build_prompt_multiblock_extracts_text_drops_non_text() {
        // openai-style multi-block: text + image_url + text
        let blocks_json = json!([
            { "type": "text", "text": "first" },
            { "type": "image_url", "image_url": { "url": "http://x" } },
            { "type": "text", "text": "second" }
        ]);
        let blocks: Vec<ContentBlock> = serde_json::from_value(blocks_json).unwrap();
        // image_url 未覆盖 → Unknown；文本保留
        let req = ChatRequest {
            model: "devin-normal".into(),
            messages: vec![Message {
                role: Role::User,
                content: MessageContent::Blocks(blocks),
            }],
            system: None,
            max_tokens: None, temperature: None, top_p: None,
            stream: None, tools: None, tool_choice: None, extra: None,
        };
        let p = build_prompt(&req).unwrap();
        assert_eq!(p, "[user] first\nsecond");
    }

    #[test]
    fn build_prompt_empty_returns_err() {
        let req = ChatRequest {
            model: "devin-normal".into(),
            messages: vec![],
            system: None,
            max_tokens: None, temperature: None, top_p: None,
            stream: None, tools: None, tool_choice: None, extra: None,
        };
        assert!(build_prompt(&req).is_err());
    }

    #[test]
    fn build_prompt_skips_empty_messages() {
        let req = ChatRequest {
            model: "devin-normal".into(),
            messages: vec![
                msg(Role::User, ""),
                msg(Role::Assistant, "real"),
            ],
            system: None,
            max_tokens: None, temperature: None, top_p: None,
            stream: None, tools: None, tool_choice: None, extra: None,
        };
        let p = build_prompt(&req).unwrap();
        assert_eq!(p, "[assistant] real");
    }

    // ── map_devin_mode ──

    #[test]
    fn map_devin_mode_5_tiers() {
        assert_eq!(map_devin_mode("devin-normal"), "normal");
        assert_eq!(map_devin_mode("devin-fast"), "fast");
        assert_eq!(map_devin_mode("devin-lite"), "lite");
        assert_eq!(map_devin_mode("devin-ultra"), "ultra");
        assert_eq!(map_devin_mode("devin-fusion"), "fusion");
    }

    #[test]
    fn map_devin_mode_unknown_defaults_normal() {
        assert_eq!(map_devin_mode("gpt-4"), "normal");
        assert_eq!(map_devin_mode(""), "normal");
        assert_eq!(map_devin_mode("claude-3-opus"), "normal");
    }

    // ── is_terminal_status ──

    #[test]
    fn terminal_status_set() {
        for s in &["exit", "error", "suspended"] {
            assert!(is_terminal_status(s), "{s} should be terminal");
        }
        for s in &["new", "claimed", "running", "resuming", "working", "waiting_for_user"] {
            assert!(!is_terminal_status(s), "{s} should NOT be terminal");
        }
    }

    // ── suspended_human_message ──

    #[test]
    fn suspended_msg_credit_variants() {
        assert!(suspended_human_message("out_of_credits").contains("out of credits"));
        assert!(suspended_human_message("usage_limit_exceeded").contains("quota"));
        assert!(suspended_human_message("out_of_quota").contains("quota"));
        assert!(suspended_human_message("inactivity").contains("inactivity"));
        assert!(suspended_human_message("other_reason").contains("other_reason"));
    }

    // ── format_chat_response ──

    #[test]
    fn chat_response_openai_shape() {
        let v = format_chat_response("openai", "devin-1", "devin-normal", "hi", 1.5);
        assert_eq!(v["object"], "chat.completion");
        assert_eq!(v["id"], "chatcmpl-devin-1");
        assert_eq!(v["choices"][0]["message"]["content"], "hi");
        assert_eq!(v["choices"][0]["message"]["role"], "assistant");
        assert_eq!(v["choices"][0]["finish_reason"], "stop");
        assert_eq!(v["usage"]["prompt_tokens"], 0);
        assert_eq!(v["usage"]["completion_tokens"], 0);
        assert_eq!(v["usage"]["total_tokens"], 0);
    }

    #[test]
    fn chat_response_anthropic_shape() {
        let v = format_chat_response("anthropic", "devin-2", "devin-fast", "hello", 2.0);
        assert_eq!(v["type"], "message");
        assert_eq!(v["role"], "assistant");
        assert_eq!(v["id"], "msg_devin-2");
        assert_eq!(v["content"][0]["type"], "text");
        assert_eq!(v["content"][0]["text"], "hello");
        assert_eq!(v["model"], "devin-fast");
        assert_eq!(v["stop_reason"], "end_turn");
        assert_eq!(v["usage"]["input_tokens"], 0);
        assert_eq!(v["usage"]["output_tokens"], 0);
    }

    #[test]
    fn chat_response_unknown_protocol_falls_back_openai() {
        let v = format_chat_response("gemini", "devin-3", "devin-lite", "hi", 0.5);
        assert_eq!(v["object"], "chat.completion");
    }

    #[test]
    fn chat_error_body_openai_and_anthropic() {
        let o = format_chat_error_body("openai", "devin-x", "err");
        let ov: Value = serde_json::from_str(&o).unwrap();
        assert_eq!(ov["error"]["message"], "err");
        assert_eq!(ov["error"]["session_id"], "devin-x");
        let a = format_chat_error_body("anthropic", "devin-y", "err");
        let av: Value = serde_json::from_str(&a).unwrap();
        assert_eq!(av["type"], "error");
        assert_eq!(av["error"]["message"], "err");
    }
}
