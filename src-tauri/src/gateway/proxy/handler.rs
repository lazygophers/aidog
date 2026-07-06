use super::*;

/// 主代理处理函数 — 渐进式日志：每个阶段即时 upsert，用 request_id 串联
pub async fn handle_proxy(
    state: AxumState<Arc<ProxyState>>,
    req: Request,
) -> Response {
    // 每请求生成 trace id（复用为 ProxyLog 主键）, 建 span → 该请求生命周期内所有日志
    // 自动携带 req{id=xxxxxxxx} 前缀（含 mock/passthrough 子调用, fmt 默认渲染当前 span）。
    let request_id = uuid::Uuid::new_v4().simple().to_string();
    // 同时挂 6-[0-9a-z] trace_id（base36(request_id 前 8 hex 截 31 bit)，可 grep 直达日志行）
    // 与完整 32-hex request_id（= proxy_log.id 主键，可从任一子日志行串回 proxy_log）。
    // 该请求生命周期内所有子 tracing 行自动携带两者。
    let trace_id = crate::logging::trace_id_from_request_id(&request_id);
    let span = tracing::info_span!("req", trace_id = %trace_id, request_id = %request_id);
    handle_proxy_inner(state, req, request_id).instrument(span).await
}


/// 请求级中断兜底 guard：客户端断连 / 请求 future 被 axum drop（任一 .await 未完成返回）时，
/// 把仍卡在 status_code=0 的 proxy_log 行补写为终态 499（client closed request）。
/// 正常返回任意 Response（成功 / 各类错误 / 流式 200 占位）即视为「已交接」→ `disarm()` 解除，
/// 不触发兜底（流式终态由 StreamLogGuard 接管，非流式终态已在 DB 内非 0）。
/// Drop 内不可 await → tokio::spawn fire-and-forget 落库（与 StreamLogGuard 同款）。
struct RequestLogGuard {
    state: Arc<ProxyState>,
    id: String,
    start: std::time::Instant,
    armed: bool,
}

impl RequestLogGuard {
    fn disarm(&mut self) {
        self.armed = false;
    }
}

impl Drop for RequestLogGuard {
    fn drop(&mut self) {
        if !self.armed {
            return;
        }
        // 499 = client closed request（nginx 约定语义）：请求未达任何服务端终态即被中断。
        // finalize_incomplete_proxy_log 的 WHERE status_code=0 谓词保证仅翻卡死行，幂等安全。
        let state = self.state.clone();
        let id = self.id.clone();
        let duration_ms = self.start.elapsed().as_millis() as i32;
        // Drop 发生在 req span 仍 enter 状态（future drop 时 span 未 exit）→ spawn_traced
        // 在父线程读栈拿到 parent trace_id → 包 info_span 让子任务内 thread-local 栈正确。
        // 保留 Handle::try_current 守卫：Drop 路径可能在 runtime teardown 后触发（罕见），
        // 守卫避免无 runtime 时 spawn 静默丢失；spawn_traced 内 tokio::spawn 与 handle.spawn
        // 在 runtime 上下文内等价（tokio::spawn 即 Handle::current().spawn）。
        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            use tracing::Instrument;
            let parent = crate::logging::current_trace_id().unwrap_or_else(crate::logging::gen_trace_id);
            let child = crate::logging::gen_child_id(&parent);
            let span = tracing::info_span!("spawn", name = %"reqlog_guard", trace_id = %child);
            handle.spawn(async move {
                if let Err(e) =
                    super::db::finalize_incomplete_proxy_log(&state.db, &id, 499, duration_ms).await
                {
                    tracing::warn!(error = %e, id = %id, "finalize incomplete proxy log failed");
                }
            }.instrument(span));
        }
    }
}

async fn handle_proxy_inner(
    state: AxumState<Arc<ProxyState>>,
    req: Request,
    request_id: String,
) -> Response {
    // P1 CONNECT 隧道早期分流：authority-form URI（`host:port`）走 CONNECT handler，
    // 不破现有 /proxy AI 协议 path 路由。fallback 命中 CONNECT 时 request_id 已生成，
    // 直接复用；CONNECT handler 内部自管日志（upsert_connect_log，独立路径）。
    //
    // ponytail: CONNECT 分流前置于 guard + handle_proxy_core —— CONNECT 走独立日志路径
    // （upsert_connect_log），不经 RequestLogGuard 499 兜底（CONNECT 自管 spawn 内终态）。
    // 分流从 handle_proxy_core 顶部移到此处的关键原因：打破 handle_proxy_core ↔ handle_connect
    // 互递归（ST5 明文路径在 connect.rs::handle_connect spawn 内调 handle_proxy_core，若分流
    // 仍在 core 内则递归类型无法证 Send，tokio::spawn 拒绝）。
    if req.method() == axum::http::Method::CONNECT {
        // P2-D：复用 handle_proxy 已生成的 request_id（已挂 req span），传入 handle_connect
        // 使 CONNECT 子日志行串回 proxy_log.id（与 AI 路径同款 span 对齐）。
        return super::connect::handle_connect(state, req, request_id).await;
    }

    // 中断兜底 guard：core 未正常返回（客户端断连致 future drop）时 Drop 补写终态 499。
    let mut guard = RequestLogGuard {
        state: state.0.clone(),
        id: request_id.clone(),
        start: std::time::Instant::now(),
        armed: true,
    };
    let resp = handle_proxy_core(state, req, request_id).await;
    // 已正常返回 Response（含流式占位）→ 解除兜底：非流式终态已在 DB 非 0，流式由 StreamLogGuard 接管。
    guard.disarm();
    resp
}

pub(crate) async fn handle_proxy_core(
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
    // group token 来源: Authorization: Bearer <key> (OpenAI/通用) 或 x-api-key (Anthropic SDK/claude-cli)。
    // 二者皆承载 group_key —— 只读前者会让原生 Anthropic 客户端 (仅发 x-api-key) 在 resolve_group 落空 → 404 no matching group。
    let auth_header = req.headers()
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .map(|s| s.trim().to_string())
        .or_else(|| {
            req.headers()
                .get("x-api-key")
                .and_then(|v| v.to_str().ok())
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
        });
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
            let mut r = (StatusCode::BAD_REQUEST, format!("{}: {e}", i18n::t(lang, ErrorKey::ReadBody))).into_response();
            inject_trace_header(&mut r);
            return r;
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

    // ── 模型列表端点分流（必须在 resolve_group 之前）──
    // GET /v1/models | /models 总是返回静态默认模型列表，**不依赖 group / token**：
    // tokenless / 错 token 的模型发现探测此前在 resolve_group 阶段就 404（晚于旧分流位置），
    // 故分流前置于 group 解析之前彻底消除 404 根因。不 relay 上游，按 path 协议静态格式化。
    if orig_method == axum::http::Method::GET && is_models_endpoint(&path) {
        return handle_models_static(&state, &mut log, &log_settings, &path, start).await;
    }

    // ── 查找分组 ──
    let group = {
        match resolve_group(&state.db, auth_header.as_deref()).await {
            Some(g) => g,
            None => {
                // fallback 直通判定：MITM 解密的非 API 流量（Host ≠ 代理自身监听 host）
                // 直通原 host 透明转发，落虚拟「未匹配」桶统计（不计费）。
                // API 流量（错 token / 无 token 直连代理自身）仍 404，不旁路。
                let host_header = orig_headers
                    .get(axum::http::header::HOST)
                    .and_then(|v| v.to_str().ok())
                    .unwrap_or("");
                if should_fallback_passthrough(host_header, &path, state.listen_addr.get().copied()) {
                    tracing::info!(host = %host_header, path = %path, "no matching group → fallback passthrough to orig host");
                    return forward_passthrough_to_orig_host(
                        &state, &mut log, &log_settings,
                        orig_method, orig_uri, orig_headers, bytes,
                        start, lang,
                    ).await;
                }
                if let Some(ref token) = auth_header {
                    log.response_body = format!("no matching group for token '{}' or path '{}'", token, path);
                    log.status_code = 404;
                    log.duration_ms = start.elapsed().as_millis() as i32;
                    upsert_log(&state, &log, &log_settings).await;
                    let mut r = (StatusCode::NOT_FOUND, log.response_body.clone()).into_response();
                    inject_trace_header(&mut r);
                    return r;
                } else {
                    log.response_body = "no matching group".to_string();
                    log.status_code = 404;
                    log.duration_ms = start.elapsed().as_millis() as i32;
                    upsert_log(&state, &log, &log_settings).await;
                    let mut r = (StatusCode::NOT_FOUND, i18n::t(lang, ErrorKey::NoMatchingGroup)).into_response();
                    inject_trace_header(&mut r);
                    return r;
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
            let mut r = (StatusCode::BAD_REQUEST, format!("{}: {e}", i18n::t(lang, ErrorKey::ParseJson))).into_response();
            inject_trace_header(&mut r);
            return r;
        }
    };
    let mut chat_req: ChatRequest = match adapter::parse_incoming_request(&log.source_protocol, &req_value) {
        Ok(r) => r,
        Err(e) => {
            log.response_body = format!("failed to parse request for protocol ({}): {e}", log.source_protocol);
            log.status_code = 400;
            log.duration_ms = start.elapsed().as_millis() as i32;
            upsert_log(&state, &log, &log_settings).await;
            let mut r = (StatusCode::BAD_REQUEST, i18n::t(lang, ErrorKey::ParseRequest)).into_response();
            inject_trace_header(&mut r);
            return r;
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
            let mut r = (StatusCode::BAD_REQUEST, format!("{}: {e}", i18n::t(lang, ErrorKey::Route))).into_response();
            inject_trace_header(&mut r);
            return r;
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

        match forward_attempt(
            &state,
            &mut log,
            &mut attempts,
            route,
            is_last_candidate,
            attempt_start,
            attempt_ts,
            &log_settings,
            lang,
            &group,
            &mut chat_req,
            &req_value,
            &source_protocol,
            &requested_model,
            is_stream,
            &orig_headers,
            &sched_settings,
            start,
        )
        .await
        {
            AttemptOutcome::Respond(r) => return r,
            AttemptOutcome::Next => continue,
        }
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
    let mut r = (StatusCode::SERVICE_UNAVAILABLE, err_body).into_response();
    inject_trace_header(&mut r);
    r
}
