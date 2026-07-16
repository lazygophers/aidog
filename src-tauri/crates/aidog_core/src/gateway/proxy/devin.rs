use super::*;

// ── s5: X-Devin-Session-Id → devin_id 内存映射（LRU + TTL 30min）──
//
// 设计：客户端传 `X-Devin-Session-Id` 请求头 → 命中且 session 非终态 → 复用（POST messages
// 续聊，省 ACU + 保上下文）；未传 / 未命中 / 已终态 / 已过期 → 新建 session + 响应头回传新
// session id（客户端下次续）。内存即可（Devin session 闲置 sleep，映射过期=新建可接受；
// 重启丢=下次新建不致命，非契约）。ponytail: 模块级 OnceLock<DashMap>，分片锁自带
// Send+Sync，无需 Arc<Mutex> 包裹（见 memory DashMap 分片锁）。

const DEVIN_SESSION_TTL_SECS: u64 = 30 * 60;

#[derive(Clone)]
struct DevinSessionMapping {
    devin_id: String,
    created_at: std::time::Instant,
}

static DEVIN_SESSION_MAP: std::sync::OnceLock<dashmap::DashMap<String, DevinSessionMapping>> =
    std::sync::OnceLock::new();

fn session_map() -> &'static dashmap::DashMap<String, DevinSessionMapping> {
    DEVIN_SESSION_MAP.get_or_init(dashmap::DashMap::new)
}

/// 查映射 + TTL 判定 + 命中滑动续期。返回 `Some(devin_id)` 当且仅当 key 存在且未过期。
///
/// ponytail: 滑动 TTL（命中时刷新 created_at）——活跃会话不会被 30min 硬上限误杀；
/// 全局 DashMap 查询 + 潜在 get_mut 二次写，O(1) 不持锁跨 await（memory DashMap 分片锁）。
fn lookup_session_at(client_id: &str, now: std::time::Instant) -> Option<String> {
    let map = session_map();
    let devin_id = {
        let entry = map.get(client_id)?;
        if now.duration_since(entry.created_at).as_secs() > DEVIN_SESSION_TTL_SECS {
            return None;
        }
        entry.devin_id.clone()
    };
    // 滑动续期（单独 get_mut 避免持读锁跨写）
    if let Some(mut entry) = map.get_mut(client_id) {
        entry.created_at = now;
    }
    Some(devin_id)
}

/// 存映射 + 惰性 sweep（size > 128 时清过期项，避免长跑泄漏）。
fn store_session_at(client_id: String, devin_id: String, now: std::time::Instant) {
    let map = session_map();
    if map.len() > 128 {
        map.retain(|_, v| now.duration_since(v.created_at).as_secs() <= DEVIN_SESSION_TTL_SECS);
    }
    map.insert(client_id, DevinSessionMapping { devin_id, created_at: now });
}

/// 生产入口：用当前时刻查映射。
fn lookup_session(client_id: &str) -> Option<String> {
    lookup_session_at(client_id, std::time::Instant::now())
}

/// 生产入口：用当前时刻存映射。
fn store_session(client_id: String, devin_id: String) {
    store_session_at(client_id, devin_id, std::time::Instant::now());
}

/// 纯函数决策（便于单测，不触网络 / 不触 map）：给定 header / mapped / status 推导复用 vs 新建。
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum SessionDecision {
    Reuse(String),
    CreateNew,
}

pub(crate) fn decide_session_reuse(
    header: Option<&str>,
    mapped: Option<&str>,
    current_status: Option<&str>,
) -> SessionDecision {
    match (header, mapped, current_status) {
        (Some(_), Some(devin_id), Some(status)) if !is_terminal_status(status) => {
            SessionDecision::Reuse(devin_id.to_string())
        }
        _ => SessionDecision::CreateNew,
    }
}

/// 给 AirDog 构造的响应附 `x-devin-session-id` 头（client_id 空则跳过）。
fn attach_session_header(resp: &mut axum::response::Response, client_id: &str) {
    if client_id.is_empty() { return; }
    let Ok(hv) = axum::http::HeaderValue::from_str(client_id) else { return };
    resp.headers_mut().insert(
        axum::http::HeaderName::from_static("x-devin-session-id"),
        hv,
    );
}

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
/// chat response 包装 / usage+est_cost 落库）。**s4** 伪流式 SSE / **s5** stateful
/// X-Devin-Session-Id 映射 / **s6** 轮询超时 504 body + `extra.devin.dev_timeout` 可配。
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
    req_headers: &axum::http::HeaderMap,
) -> Response {
    log.target_protocol = "devin".to_string();
    log.platform_id = platform.id;
    log.actual_model = requested_model.to_string();

    // org_id 必填（path 段 + Bearer realm），api_key = Devin cog_ key。
    // s9: nested `extra.devin.org_id`（对齐前端 serializeDevinConfig + quota/devin.rs::parse_devin_extra
    // + s6 read_dev_timeout_secs 同层级）。禁 flat extra.org_id（s2/s3 旧代码 bug，前端从未写此形态）。
    // ponytail: 复用 quota::parse_devin_extra 作单一真值源，避免 proxy/quota 两份解析逻辑漂移。
    let extra_v: Value = serde_json::from_str(&platform.extra).unwrap_or_else(|_| Value::Object(Default::default()));
    let org_id = match crate::gateway::quota::parse_devin_extra(&platform.extra) {
        Some(id) => id,
        None => {
            return devin_error(
                &state, &mut log, &log_settings, lang, start,
                StatusCode::BAD_REQUEST,
                r#"devin platform missing extra.devin.org_id (expected {"devin":{"org_id":"..."}})"#,
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
    // s6: devin 轮询超时（extra.devin.dev_timeout 秒，缺省 300）。nested 读取，禁 flat。
    let dev_timeout_secs = read_dev_timeout_secs(&extra_v);
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
    // ponytail: req_timeout 是单次 HTTP 调用上限（poll 循环每次 GET 各自计时）；
    // session 级轮询总上限 = dev_timeout_secs（poll_session 内 deadline），两者独立。
    let (system_timeout, proxy_client) = {
        let c = state.settings_cache.read().await;
        (c.system_timeout.clone(), c.proxy_client.clone())
    };
    let conn_timeout = if system_timeout.connect_timeout_secs > 0 { system_timeout.connect_timeout_secs } else { 10 };
    let req_timeout = if system_timeout.request_timeout_secs > 0 { system_timeout.request_timeout_secs } else { 300 };
    let client = http_client::build_http_client(
        &proxy_client, req_timeout, conn_timeout,
        Some(&platform.extra), None,
    ).await;

    // ── 1. resolve session id（s5: X-Devin-Session-Id 复用 / 否则新建）──
    //   有 header + 映射命中 + session 非终态 → POST messages 复用（省 ACU + 保上下文）
    //   有 header 但未命中/过期/终态/probe 失败 → 新建 + 存映射（覆盖旧条目）
    //   无 header → 新建 + 存映射（client_id = devin_id 本身，省 UUID）
    let header_id = req_headers
        .get("x-devin-session-id")
        .and_then(|v| v.to_str().ok())
        .filter(|s| !s.is_empty())
        .map(String::from);
    let mut client_id: String = header_id.clone().unwrap_or_default();

    // 命中映射 + probe 当前 status（任一缺失 → CreateNew）
    let mapped: Option<String> = header_id.as_ref().and_then(|hid| lookup_session(hid));
    let probed_status: Option<String> = match &mapped {
        Some(id) => fetch_session_state(&client, &base_url, api_key, id).await.ok().map(|s| s.status),
        None => None,
    };
    let session_id: String = match decide_session_reuse(
        header_id.as_deref(),
        mapped.as_deref(),
        probed_status.as_deref(),
    ) {
        SessionDecision::Reuse(mapped_id) => {
            client_id = header_id.clone().unwrap_or_default();
            tracing::info!(
                platform_id = platform.id,
                client_session_id = %client_id,
                devin_id = %mapped_id,
                status = probed_status.as_deref().unwrap_or("?"),
                "devin reuse session via X-Devin-Session-Id"
            );
            if let Err(e) = send_message_to_session(&client, &base_url, api_key, &mapped_id, &prompt).await {
                tracing::warn!(
                    platform_id = platform.id,
                    devin_id = %mapped_id,
                    error = %e,
                    "devin POST messages failed"
                );
                let mut r = devin_error(
                    &state, &mut log, &log_settings, lang, start,
                    StatusCode::BAD_GATEWAY,
                    &format!("devin send_message error: {e}"),
                ).await;
                attach_session_header(&mut r, &client_id);
                return r;
            }
            mapped_id
        }
        SessionDecision::CreateNew => {
            let new_id = match create_session(&client, &base_url, api_key, &prompt, requested_model).await {
                Ok(id) => id,
                Err(e) => {
                    tracing::warn!(platform_id = platform.id, error = %e, "devin create_session failed");
                    let mut r = devin_error(
                        &state, &mut log, &log_settings, lang, start,
                        StatusCode::BAD_GATEWAY,
                        &format!("devin create_session error: {e}"),
                    ).await;
                    attach_session_header(&mut r, &client_id);
                    return r;
                }
            };
            let cid = header_id.clone().unwrap_or_else(|| new_id.clone());
            tracing::info!(
                platform_id = platform.id,
                devin_id = %new_id,
                client_session_id = %cid,
                "devin session created (new)"
            );
            store_session(cid.clone(), new_id.clone());
            client_id = cid;
            new_id
        }
    };

    // ── 2. poll until terminal (10s 间隔 + dev_timeout_secs 上限) ──
    //   s6: 超时 → 504 + 结构化 body（禁 200 假回复，spec design.md line 72-76）。
    let final_state = match poll_session(&client, &base_url, api_key, &session_id, dev_timeout_secs).await {
        Ok(s) => s,
        Err(PollError::Other(e)) => {
            tracing::warn!(platform_id = platform.id, session_id = %session_id, error = %e, "devin poll_session failed");
            let mut r = devin_error(
                &state, &mut log, &log_settings, lang, start,
                StatusCode::BAD_GATEWAY,
                &format!("devin poll_session error: {e}"),
            ).await;
            attach_session_header(&mut r, &client_id);
            return r;
        }
        Err(PollError::Timeout { last_state }) => {
            // 超时：session 仍非终态 → 504 + devin_timeout body（含 session_id+url+message）。
            // est_cost = 轮询期间累计的 acus_consumed（契约 9）；blocked_reason="devin_timeout" 落库审计。
            let timeout_body = format_devin_timeout_body(&session_id);
            let acus_consumed = last_state.acus_consumed;
            tracing::warn!(
                platform_id = platform.id,
                session_id = %session_id,
                timeout_secs = dev_timeout_secs,
                last_status = %last_state.status,
                acus_consumed,
                "devin poll_session timeout → 504"
            );
            if is_stream {
                // 流式：emit Start + error SSE chunk（复用 sse_error_chunk）+ http 504。
                // ponytail: 直接复用 stream_terminal_response 的 "timeout" 分支，最小改。
                log.blocked_reason = "devin_timeout".into();
                return stream_terminal_response(
                    state, log, log_settings, source_protocol, requested_model,
                    &session_id, "", acus_consumed, "timeout",
                    None, start, &client_id,
                ).await;
            }
            log.status_code = StatusCode::GATEWAY_TIMEOUT.as_u16() as i32;
            log.blocked_reason = "devin_timeout".into();
            log.input_tokens = 0;
            log.output_tokens = 0;
            log.est_cost = acus_consumed; // 契约 9: 超时仍记 ACU 已消费部分
            log.duration_ms = start.elapsed().as_millis() as i32;
            log.response_body = timeout_body.clone();
            log.user_response_body = timeout_body.clone();
            log.user_response_headers = r#"{"content-type":"application/json"}"#.to_string();
            upsert_log(&state, &log, &log_settings).await;
            let mut r = (
                StatusCode::GATEWAY_TIMEOUT,
                [(axum::http::header::CONTENT_TYPE, "application/json")],
                timeout_body,
            ).into_response();
            inject_trace_header(&mut r);
            attach_session_header(&mut r, &client_id);
            return r;
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
                    let mut r = devin_error(
                        &state, &mut log, &log_settings, lang, start,
                        StatusCode::BAD_GATEWAY,
                        &format!("devin get_messages error: {e}"),
                    ).await;
                    attach_session_header(&mut r, &client_id);
                    return r;
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
            attach_session_header(&mut r, &client_id);
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
            attach_session_header(&mut r, &client_id);
            return r;
        }
        other => {
            // 兜底：未预期非终态 status（理论上 poll_session 只返终态）
            tracing::error!(platform_id = platform.id, session_id = %session_id, status = %other, "devin unexpected non-terminal status reached handler");
            let mut r = devin_error(
                &state, &mut log, &log_settings, lang, start,
                StatusCode::BAD_GATEWAY,
                &format!("devin unexpected status: {other}"),
            ).await;
            attach_session_header(&mut r, &client_id);
            return r;
        }
    };

    // ── 4. 包 chat response（按 source_protocol openai/anthropic 格式化）──
    if is_stream {
        // 契约 5: 伪流式（轮询中 diff 新 devin message → SSE delta），终态 [DONE]/message_stop。
        // 注意：Devin 无原生 SSE，chunk 粒度 = 轮询周期（10s）非 token，客户端看到的是离散 message burst。
        // 已轮询到终态（上方 poll_session）→ 直接取 messages（exit 路径已 fetch）+ 把 content 切块发 SSE。
        // ponytail: s3 已 poll 到终态，流式分支在终态后再 emit chunks（不再二次 poll）；
        // 真正的边 poll 边 emit 留待 s5（stateful session 复用）——此处单请求内 poll 完再 chunk 也满足契约 5。
        return stream_terminal_response(
            state, log, log_settings, source_protocol, requested_model,
            &session_id, &content, acus_consumed, final_state.status.as_str(),
            final_state.status_detail.as_deref(), start, &client_id,
        ).await;
    }
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
    attach_session_header(&mut r, &client_id);
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

/// 默认 Devin 轮询超时（秒）。spec design.md line 72-76。
pub(crate) const DEVIN_DEFAULT_TIMEOUT_SECS: u64 = 300;

/// 从 platform.extra JSON 读 `devin.dev_timeout`（秒），缺省 DEVIN_DEFAULT_TIMEOUT_SECS。
/// ponytail: nested 读取（与 quota/devin.rs extra.devin.org_id 同层级），禁 flat extra.dev_timeout。
/// ≤ 0 或非 u64 → 默认（安全侧兜底，禁 0 触发立即超时）。
pub(crate) fn read_dev_timeout_secs(extra_v: &Value) -> u64 {
    extra_v
        .get("devin")
        .and_then(|d| d.get("dev_timeout"))
        .and_then(|v| v.as_u64())
        .filter(|s| *s > 0)
        .unwrap_or(DEVIN_DEFAULT_TIMEOUT_SECS)
}

/// Devin session 前台 URL（app.devin.ai，非 API base_url）。
/// ponytail: 不读 create_session 响应的 url 字段（调用方已丢）——格式固定，拼即可。
pub(crate) fn devin_session_url(session_id: &str) -> String {
    format!("https://app.devin.ai/sessions/{session_id}")
}

/// 轮询超时 body（spec design.md line 72-76）：
/// `{"error":{"type":"devin_timeout","session_id":"...","url":"...","message":"Devin task still running, check url"}}`
/// 禁 200 假回复（chat 语义混淆），超时必 504 + 本 body。
pub(crate) fn format_devin_timeout_body(session_id: &str) -> String {
    serde_json::json!({
        "error": {
            "type": "devin_timeout",
            "session_id": session_id,
            "url": devin_session_url(session_id),
            "message": "Devin task still running, check url"
        }
    }).to_string()
}

/// poll_session 错误：区分超时（带最后状态 → 504）与网络/解码错误（→ 502）。
#[derive(Debug)]
pub(crate) enum PollError {
    /// 到 deadline 仍未达终态，携带最后一次 poll 的 state（acus_consumed 部分累计）。
    Timeout { last_state: DevinSessionState },
    /// 网络 / 解析 / 非 2xx 错误。
    Other(String),
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

/// 单次 GET /sessions/{id}（非轮询）：复用路径的状态探测。
///
/// ponytail: 与 poll_session 共享响应解析形态，但不轮询 —— 复用前一次性查 status，
/// 终态即 fallback 新建。返回的 `acus_consumed` 复用路径不消费（仅 create/poll 累计）。
async fn fetch_session_state(
    client: &reqwest::Client,
    base_url: &str,
    api_key: &str,
    session_id: &str,
) -> Result<DevinSessionState, String> {
    let url = format!("{base_url}/sessions/{session_id}");
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
    Ok(DevinSessionState {
        status: v.get("status").and_then(|x| x.as_str()).unwrap_or("running").to_string(),
        status_detail: v.get("status_detail").and_then(|x| x.as_str()).map(String::from),
        acus_consumed: v.get("acus_consumed").and_then(|x| x.as_f64()).unwrap_or(0.0),
    })
}

/// POST /sessions/{id}/messages body=`{"message": "..."}`（s5：续聊复用路径）。
/// 须 session running/claimed/new/resuming 态（调用方保证：先 fetch_session_state 探测）。
async fn send_message_to_session(
    client: &reqwest::Client,
    base_url: &str,
    api_key: &str,
    session_id: &str,
    message: &str,
) -> Result<(), String> {
    let url = format!("{base_url}/sessions/{session_id}/messages");
    let body = serde_json::json!({ "message": message });
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
    Ok(())
}

/// 轮询 GET /sessions/{id} 直到终态（exit/error/suspended）或超时。
///
/// 间隔 10s（tokio::time::sleep）。超时上限由 `timeout_secs` 传入（s6：`extra.devin.dev_timeout`，
/// 默认 DEVIN_DEFAULT_TIMEOUT_SECS=300）。超时返 `PollError::Timeout` 携带最后状态（含部分 acus），
/// 调用方构造 504 body（禁 200 假回复，spec design.md line 72-76）。
async fn poll_session(
    client: &reqwest::Client,
    base_url: &str,
    api_key: &str,
    session_id: &str,
    timeout_secs: u64,
) -> Result<DevinSessionState, PollError> {
    let url = format!("{base_url}/sessions/{session_id}");
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(timeout_secs);
    loop {
        let resp = client.get(&url)
            .bearer_auth(api_key)
            .send().await
            .map_err(|e| PollError::Other(format!("http: {e}")))?;
        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(PollError::Other(format!("status {}: {}", status.as_u16(), text)));
        }
        let v: Value = resp.json().await.map_err(|e| PollError::Other(format!("decode: {e}")))?;
        let status = v.get("status").and_then(|x| x.as_str()).unwrap_or("running").to_string();
        let status_detail = v.get("status_detail").and_then(|x| x.as_str()).map(String::from);
        let acus_consumed = v.get("acus_consumed").and_then(|x| x.as_f64()).unwrap_or(0.0);
        // ponytail: 当前迭代拿到的 state 即「最后一次 poll」——终态直接 return Ok，
        // 超时（下方 deadline check）时 state 仍是本轮最新，作 last_state 透传给 504 body。
        let state = DevinSessionState { status: status.clone(), status_detail, acus_consumed };
        if is_terminal_status(&status) {
            return Ok(state);
        }
        if std::time::Instant::now() >= deadline {
            return Err(PollError::Timeout { last_state: state });
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

// ── 伪流式 SSE 构造（契约 5）──

/// 终态后 emit SSE 流：按 source_protocol 把 content 切块发 delta，终态 [DONE]/message_stop。
///
/// s4 设计：s3 的 poll_session 已轮询到终态（exit/error/suspended）拿到 final_status + accus_consumed，
/// 流式分支不再二次 poll，直接把已 fetch 的 content 切块发 SSE delta（单请求内 poll→chunk 两阶段）。
/// error/suspended 终态发 error SSE + close 流；正常 exit 发完整 delta 序列 + stop。
///
/// ponytail: 不做 progressive poll-during-stream（那需要 s5 stateful session 映射或长任务 spawn）。
/// 客户端看到的仍是 SSE 流（content 逐块到达），满足契约 5 的「伪流式 + [DONE] 终态」；
/// 块粒度 = chunk_size 字节（默认整条 message 一块），非 token 粒度——Devin 本就是离散 message 产出。
#[allow(clippy::too_many_arguments)]
async fn stream_terminal_response(
    state: Arc<ProxyState>,
    mut log: ProxyLog,
    log_settings: ProxyLogSettings,
    source_protocol: &str,
    requested_model: &str,
    session_id: &str,
    content: &str,
    acus_consumed: f64,
    final_status: &str,
    final_status_detail: Option<&str>,
    start: std::time::Instant,
    client_session_id: &str,
) -> Response {
    use crate::gateway::adapter::converter::to_client_sse;

    let sse_id = format!("chatcmpl-{session_id}");
    // ── 构造 SSE chunk 序列（Start → N×Delta → Stop/Error）──
    let chunks: Vec<String> = match final_status {
        "exit" => build_devin_sse_seq(source_protocol, &sse_id, requested_model, &[content.to_string()], "stop"),
        "error" => {
            let mut v = vec![to_client_sse(&ChatStreamEvent::Start { id: sse_id.clone(), model: requested_model.to_string() }, source_protocol, requested_model).unwrap_or_default()];
            v.push(sse_error_chunk(source_protocol, "Devin session error"));
            v
        }
        "suspended" => {
            let detail = final_status_detail.unwrap_or("unknown");
            let human = suspended_human_message(detail);
            let mut v = vec![to_client_sse(&ChatStreamEvent::Start { id: sse_id.clone(), model: requested_model.to_string() }, source_protocol, requested_model).unwrap_or_default()];
            v.push(sse_error_chunk(source_protocol, &human));
            v
        }
        "timeout" => {
            // s6: 轮询超时 → Start + error SSE chunk（spec design.md line 72-76，禁 200）
            let mut v = vec![to_client_sse(&ChatStreamEvent::Start { id: sse_id.clone(), model: requested_model.to_string() }, source_protocol, requested_model).unwrap_or_default()];
            v.push(sse_error_chunk(source_protocol, "Devin task still running, check url"));
            v
        }
        _ => build_devin_sse_seq(source_protocol, &sse_id, requested_model, &[content.to_string()], "stop"),
    };

    // ── upsert log（终态后落库，est_cost = acus_consumed）──
    let (http_status, body_marker) = match final_status {
        "error" => (StatusCode::BAD_GATEWAY, "[devin stream error]".to_string()),
        "suspended" => (StatusCode::PAYMENT_REQUIRED, "[devin stream suspended]".to_string()),
        "timeout" => (StatusCode::GATEWAY_TIMEOUT, "[devin stream timeout]".to_string()),
        _ => (StatusCode::OK, content.to_string()),
    };
    log.status_code = http_status.as_u16() as i32;
    log.input_tokens = 0;
    log.output_tokens = 0;
    log.est_cost = acus_consumed;
    log.duration_ms = start.elapsed().as_millis() as i32;
    log.response_body = body_marker.clone();
    log.user_response_body = body_marker;
    log.user_response_headers = r#"{"content-type":"text/event-stream","cache-control":"no-cache","connection":"keep-alive"}"#.to_string();
    upsert_log(&state, &log, &log_settings).await;

    // ── 构造 SSE Body（参考 mock.rs:112 + finish.rs:332）──
    let body_stream = futures::stream::iter(chunks.into_iter().map(Ok::<_, std::io::Error>));
    let body = Body::from_stream(body_stream);
    let mut r = (
        http_status,
        [
            (axum::http::header::CONTENT_TYPE, "text/event-stream"),
            (axum::http::header::CACHE_CONTROL, "no-cache"),
            (axum::http::header::CONNECTION, "keep-alive"),
        ],
        body,
    ).into_response();
    inject_trace_header(&mut r);
    attach_session_header(&mut r, client_session_id);
    r
}

/// 把 content 切块发 SSE delta：Start → N×Delta(text) → Stop(finish_reason)。
/// 复用 `to_client_sse` 按 source_protocol 转 openai/anthropic 格式（含 gemini 兜底）。
pub(crate) fn build_devin_sse_seq(
    source_protocol: &str,
    sse_id: &str,
    model: &str,
    messages: &[String],
    finish_reason: &str,
) -> Vec<String> {
    use crate::gateway::adapter::converter::to_client_sse;
    let mut out: Vec<String> = Vec::new();
    if let Some(s) = to_client_sse(
        &ChatStreamEvent::Start { id: sse_id.to_string(), model: model.to_string() },
        source_protocol, model,
    ) {
        out.push(s);
    }
    for m in messages {
        // ponytail: Devin message 是离散产出，整条一个 delta chunk（非 token 切分）
        if !m.is_empty()
            && let Some(s) = to_client_sse(&ChatStreamEvent::Delta { text: m.clone() }, source_protocol, model)
        {
            out.push(s);
        }
    }
    if let Some(s) = to_client_sse(
        &ChatStreamEvent::Stop { finish_reason: Some(finish_reason.to_string()) },
        source_protocol, model,
    ) {
        out.push(s);
    }
    out
}

/// 错误 SSE chunk：openai → `data: {"error":{...}}\n\n`；anthropic → `event: error\ndata: {...}\n\n`。
/// 后接 [DONE]/message_stop 让客户端 clean close（openai 的 [DONE] 由 Stop 提供，这里仅 error）。
pub(crate) fn sse_error_chunk(source_protocol: &str, msg: &str) -> String {
    match source_protocol {
        "anthropic" => format!(
            "event: error\ndata: {}\n\n",
            serde_json::json!({ "type": "error", "error": { "type": "api_error", "message": msg } })
        ),
        _ => format!(
            "data: {}\n\n",
            serde_json::json!({ "error": { "message": msg, "type": "devin_session_error" } })
        ),
    }
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

    // ── 伪流式 SSE 构造（s4）──

    #[test]
    fn sse_seq_openai_has_delta_and_done() {
        let chunks = build_devin_sse_seq("openai", "chatcmpl-s1", "devin-normal", &["hello".into()], "stop");
        // Start + Delta + Stop = 3 chunks
        assert_eq!(chunks.len(), 3, "openai seq: {:?}", chunks);
        // Start: data: {"object":"chat.completion.chunk","choices":[{"delta":{"role":"assistant"...}}]}
        assert!(chunks[0].contains("chat.completion.chunk"), "start: {}", chunks[0]);
        assert!(chunks[0].contains("assistant"), "start role: {}", chunks[0]);
        // Delta: data: {"choices":[{"delta":{"content":"hello"}}]}
        assert!(chunks[1].contains("hello"), "delta: {}", chunks[1]);
        assert!(chunks[1].contains("\"content\":\"hello\""), "delta content: {}", chunks[1]);
        // Stop: 终态 finish_reason=stop + [DONE]
        assert!(chunks[2].contains("\"finish_reason\":\"stop\""), "stop reason: {}", chunks[2]);
        assert!(chunks[2].contains("[DONE]"), "stop [DONE]: {}", chunks[2]);
    }

    #[test]
    fn sse_seq_anthropic_has_message_start_stop() {
        let chunks = build_devin_sse_seq("anthropic", "msg_s2", "devin-fast", &["world".into()], "end_turn");
        assert_eq!(chunks.len(), 3, "anthropic seq: {:?}", chunks);
        // Start: event: message_start
        assert!(chunks[0].contains("event: message_start"), "anthropic start: {}", chunks[0]);
        assert!(chunks[0].contains("assistant"), "anthropic role: {}", chunks[0]);
        // Delta: event: content_block_delta + text_delta
        assert!(chunks[1].contains("event: content_block_delta"), "anthropic delta: {}", chunks[1]);
        assert!(chunks[1].contains("text_delta"), "anthropic delta type: {}", chunks[1]);
        assert!(chunks[1].contains("world"), "anthropic delta text: {}", chunks[1]);
        // Stop: message_delta + message_stop, stop_reason=end_turn
        assert!(chunks[2].contains("event: message_delta"), "anthropic stop delta: {}", chunks[2]);
        assert!(chunks[2].contains("event: message_stop"), "anthropic message_stop: {}", chunks[2]);
        assert!(chunks[2].contains("end_turn"), "anthropic stop_reason: {}", chunks[2]);
    }

    #[test]
    fn sse_seq_skips_empty_messages() {
        let chunks = build_devin_sse_seq("openai", "c", "m", &["".into(), "real".into(), "".into()], "stop");
        // Start + 1 Delta（空跳过）+ Stop
        assert_eq!(chunks.len(), 3);
        assert!(chunks[1].contains("real"));
    }

    #[test]
    fn sse_seq_gemini_falls_back_openai_like() {
        // gemini: Start→None（跳过），Delta + Stop = 2 chunks（gemini sse 无 data:/event: 前缀，纯 JSON 行）
        let chunks = build_devin_sse_seq("gemini", "c", "m", &["x".into()], "stop");
        assert_eq!(chunks.len(), 2, "gemini seq: {:?}", chunks);
        assert!(chunks[0].contains("\"text\":\"x\""), "gemini delta: {}", chunks[0]);
        // 末块是 Stop（finishReason=STOP）
        let last = chunks.last().unwrap();
        assert!(last.contains("finishReason"), "gemini stop finishReason: {last}");
        assert!(last.contains("STOP"), "gemini STOP: {last}");
    }

    #[test]
    fn sse_error_chunk_openai_and_anthropic() {
        let o = sse_error_chunk("openai", "boom");
        assert!(o.starts_with("data: "));
        let ov: Value = serde_json::from_str(o.trim_start_matches("data: ").trim()).unwrap();
        assert_eq!(ov["error"]["message"], "boom");
        let a = sse_error_chunk("anthropic", "kaboom");
        assert!(a.starts_with("event: error\ndata: "));
        let av: Value = serde_json::from_str(a.split("data: ").nth(1).unwrap().trim()).unwrap();
        assert_eq!(av["error"]["message"], "kaboom");
    }

    // ── s5: X-Devin-Session-Id 映射（LRU + TTL 30min）──

    /// 唯一 key 工厂：避免共享 DashMap 跨测试污染（memory singleton 警示）。
    fn fresh_key(tag: &str) -> String {
        static COUNTER: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let n = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        format!("test-key-{tag}-{n}-{}", std::time::Instant::now().elapsed().as_nanos())
    }

    #[test]
    fn session_map_store_and_lookup_hit() {
        let now = std::time::Instant::now();
        let k = fresh_key("hit");
        let id = format!("devin-{k}");
        store_session_at(k.clone(), id.clone(), now);
        assert_eq!(lookup_session_at(&k, now), Some(id));
    }

    #[test]
    fn session_map_miss_returns_none() {
        let now = std::time::Instant::now();
        let k = fresh_key("miss");
        assert_eq!(lookup_session_at(&k, now), None);
    }

    #[test]
    fn session_map_expired_returns_none() {
        // ponytail: 用 Instant 减法模拟过期（30min TTL，回拨 31min）
        let now = std::time::Instant::now();
        let past = now - std::time::Duration::from_secs(31 * 60);
        let k = fresh_key("expired");
        let id = format!("devin-{k}");
        store_session_at(k.clone(), id, past);
        assert_eq!(lookup_session_at(&k, now), None);
    }

    #[test]
    fn session_map_lookup_slides_ttl() {
        // 命中刷新 created_at，再过 20min（< 30min）仍命中
        let t0 = std::time::Instant::now();
        let k = fresh_key("slide");
        store_session_at(k.clone(), "devin-slide".into(), t0);
        // 29min 后查 → 命中且续期
        let t1 = t0 + std::time::Duration::from_secs(29 * 60);
        assert_eq!(lookup_session_at(&k, t1).as_deref(), Some("devin-slide"));
        // 再过 20min（从 t1 起，总 49min > 30 但滑动后续期）→ 仍命中
        let t2 = t1 + std::time::Duration::from_secs(20 * 60);
        assert_eq!(lookup_session_at(&k, t2).as_deref(), Some("devin-slide"));
    }

    // ── decide_session_reuse（纯函数决策，便于单测）──

    #[test]
    fn decide_reuse_when_header_mapped_non_terminal() {
        // 有 header + 映射命中 + 非终态 → Reuse
        assert_eq!(
            decide_session_reuse(Some("c1"), Some("devin-1"), Some("running")),
            SessionDecision::Reuse("devin-1".into())
        );
        assert_eq!(
            decide_session_reuse(Some("c1"), Some("devin-1"), Some("claimed")),
            SessionDecision::Reuse("devin-1".into())
        );
        assert_eq!(
            decide_session_reuse(Some("c1"), Some("devin-1"), Some("resuming")),
            SessionDecision::Reuse("devin-1".into())
        );
        assert_eq!(
            decide_session_reuse(Some("c1"), Some("devin-1"), Some("new")),
            SessionDecision::Reuse("devin-1".into())
        );
    }

    #[test]
    fn decide_new_when_terminal_status() {
        // 终态（exit/error/suspended）→ 不复用，新建
        for s in &["exit", "error", "suspended"] {
            assert_eq!(
                decide_session_reuse(Some("c1"), Some("devin-1"), Some(s)),
                SessionDecision::CreateNew,
                "status {s} should force new"
            );
        }
    }

    #[test]
    fn decide_new_when_mapping_miss_or_no_header() {
        // 映射未命中（None）→ 新建
        assert_eq!(
            decide_session_reuse(Some("c1"), None, None),
            SessionDecision::CreateNew
        );
        // 无 header → 即使有映射也走新建（client_id 后续从 devin_id 派生）
        assert_eq!(
            decide_session_reuse(None, Some("devin-1"), Some("running")),
            SessionDecision::CreateNew
        );
        // 完全空白 → 新建
        assert_eq!(
            decide_session_reuse(None, None, None),
            SessionDecision::CreateNew
        );
    }

    #[test]
    fn decide_new_when_status_probe_failed() {
        // probe 失败（current_status = None）→ 安全侧 fallback 新建
        assert_eq!(
            decide_session_reuse(Some("c1"), Some("devin-1"), None),
            SessionDecision::CreateNew
        );
    }

    // ── s6: devin_timeout 配置 + 504 body ──

    #[test]
    fn read_dev_timeout_secs_default_when_absent() {
        // 无 devin key / 无 dev_timeout → 默认 300
        assert_eq!(read_dev_timeout_secs(&json!({})), DEVIN_DEFAULT_TIMEOUT_SECS);
        assert_eq!(read_dev_timeout_secs(&json!({"devin": {}})), DEVIN_DEFAULT_TIMEOUT_SECS);
        assert_eq!(
            read_dev_timeout_secs(&json!({"devin": {"org_id": "o-1"}})),
            DEVIN_DEFAULT_TIMEOUT_SECS,
            "仅有 org_id 无 dev_timeout → 默认"
        );
    }

    #[test]
    fn read_dev_timeout_secs_reads_nested_devin_dev_timeout() {
        // nested 路径 extra.devin.dev_timeout 命中
        assert_eq!(
            read_dev_timeout_secs(&json!({"devin": {"dev_timeout": 120}})),
            120
        );
        assert_eq!(
            read_dev_timeout_secs(&json!({"devin": {"org_id": "o-1", "dev_timeout": 600}})),
            600
        );
    }

    #[test]
    fn read_dev_timeout_secs_rejects_flat_and_invalid() {
        // 禁 flat extra.dev_timeout（非 nested）→ 默认
        assert_eq!(
            read_dev_timeout_secs(&json!({"dev_timeout": 60})),
            DEVIN_DEFAULT_TIMEOUT_SECS,
            "flat extra.dev_timeout 禁读取，强制 nested extra.devin.dev_timeout"
        );
        // ≤ 0 → 默认（禁立即超时）
        assert_eq!(
            read_dev_timeout_secs(&json!({"devin": {"dev_timeout": 0}})),
            DEVIN_DEFAULT_TIMEOUT_SECS
        );
        // 非 u64（字符串 / float）→ 默认
        assert_eq!(
            read_dev_timeout_secs(&json!({"devin": {"dev_timeout": "300"}})),
            DEVIN_DEFAULT_TIMEOUT_SECS
        );
        assert_eq!(
            read_dev_timeout_secs(&json!({"devin": {"dev_timeout": 300.5}})),
            DEVIN_DEFAULT_TIMEOUT_SECS
        );
    }

    #[test]
    fn format_devin_timeout_body_has_all_required_fields() {
        // spec design.md line 72-76: type/session_id/url/message 全部就位
        let body = format_devin_timeout_body("devin-abc123");
        let v: Value = serde_json::from_str(&body).unwrap();
        let err = &v["error"];
        assert_eq!(err["type"], "devin_timeout", "type 必须 devin_timeout");
        assert_eq!(err["session_id"], "devin-abc123", "session_id 必须透传");
        assert_eq!(
            err["url"],
            "https://app.devin.ai/sessions/devin-abc123",
            "url 必须是 app.devin.ai 前台 session URL"
        );
        assert_eq!(
            err["message"],
            "Devin task still running, check url",
            "message 文案对齐 spec"
        );
        // 禁 200 假回复：body 内不含 fake content / choices（chat 语义污染）
        assert!(v.get("choices").is_none(), "禁choices假回复");
        assert!(v.get("content").is_none(), "禁content假回复");
    }

    #[test]
    fn devin_session_url_format() {
        assert_eq!(
            devin_session_url("devin-x"),
            "https://app.devin.ai/sessions/devin-x"
        );
    }

    // ── s9: org_id nested 读取回归（对齐前端 serializeDevinConfig + quota::parse_devin_extra）──

    /// 回归 guard：handle_devin 读 org_id 必须走 nested `extra.devin.org_id`。
    /// flat `extra.org_id`（s2/s3 旧 bug 形态）必须返 None，否则 Devin session 请求 BAD_REQUEST。
    /// ponytail: 直接调 quota::parse_devin_extra（proxy 复用的真值源），禁再抄一份 nested 解析到 proxy。
    #[test]
    fn org_id_nested_read_hits_and_flat_misses() {
        // nested 形态（前端 serializeDevinConfig 实际写入）→ 命中
        assert_eq!(
            crate::gateway::quota::parse_devin_extra(r#"{"devin":{"org_id":"org-abc"}}"#),
            Some("org-abc".to_string())
        );
        // 同时含 api_key + dev_timeout 的完整 nested 形态 → 仍命中
        assert_eq!(
            crate::gateway::quota::parse_devin_extra(
                r#"{"devin":{"org_id":"org-xyz","api_key":"cog_xxx","dev_timeout":120}}"#
            ),
            Some("org-xyz".to_string())
        );
        // flat 形态（s2/s3 旧 bug 读法，前端从未写）→ 必须返 None
        assert!(
            crate::gateway::quota::parse_devin_extra(r#"{"org_id":"org-flat"}"#).is_none(),
            "flat extra.org_id 必须不命中（nested-only，防 s2/s3 bug 回归）"
        );
        // 空 / 非 JSON / 缺 devin → None
        assert!(crate::gateway::quota::parse_devin_extra("").is_none());
        assert!(crate::gateway::quota::parse_devin_extra(r#"{"devin":{}}"#).is_none());
    }
}
