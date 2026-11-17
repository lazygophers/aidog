use super::*;

// 票 09：出站整形纯函数已迁至 outbound_shape（机械迁移，零行为改动）。
use super::outbound_shape::{
    build_field_trace, dropped_media_fields, dropped_server_tools,
    fold_openai_max_completion_tokens, hoist_mid_messages_system, rename_openai_max_tokens_key,
    strip_default_image_detail, strip_redacted_thinking_blocks, strip_thinking_if_unmatched,
};

/// 阶段函数的退出契约。阶段只表达“换候选”或“终态响应”，
/// 不把外层 forward_attempt 的结果类型泄漏进阶段。
enum StageExit {
    Retry { connect_failed: bool },
    Respond(axum::response::Response),
}

impl From<StageExit> for AttemptOutcome {
    fn from(exit: StageExit) -> Self {
        match exit {
            StageExit::Retry { connect_failed } => Self::Next { connect_failed },
            StageExit::Respond(response) => Self::Respond(response),
        }
    }
}

/// 单次候选 forward 尝试的控制结果：Respond=已确定响应直接返回客户端；Next=换下个候选重试。
/// Next.connect_failed：本次尝试的终态错误是否为连接建立失败（reqwest is_connect）——
/// handler 用它做同 base_url 兄弟条目联动跳过（routing-health-optim R3，2026-09-28）。
pub(crate) enum AttemptOutcome {
    Respond(axum::response::Response),
    Next { connect_failed: bool },
}

/// 单次上游调用（一个「平台 + 模型」组合，请求尝试-逐次重算）：构建上游请求、发送、按状态码分类处理。
/// 返回 AttemptOutcome::Respond 直接回客户端，Next 表示失败、由 handler 的编排按
/// `attempts.last().exclusion` 排除后重排再试。
///
/// 一次调用只发 `route.target_model`（`fallback_models` 恒空）；同平台换模型不再在这里循环，
/// 而是由编排把失败的组合排除后重排选下一个。`rate_slot` 跨调用保存当前平台的限频占位：
/// 同平台连续调用复用同一占位（平台级只占一次），换平台时释放上一个。
#[allow(clippy::too_many_arguments)]
pub(crate) async fn forward_attempt(
    state: &Arc<ProxyState>,
    log: &mut ProxyLog,
    attempts: &mut Vec<ProxyAttempt>,
    route: RouteResult,
    model_alt: bool,
    candidate_total: usize,
    is_last_candidate: bool,
    budget: &mut CallBudget,
    rate_slot: &mut RateSlot,
    log_settings: &ProxyLogSettings,
    lang: Lang,
    group: &Group,
    chat_req: &mut ChatRequest,
    req_value: &Value,
    source_protocol: &Protocol,
    requested_model: &str,
    is_stream: bool,
    orig_headers: &axum::http::HeaderMap,
    sched_settings: &super::models::SchedulingBreakerSettings,
    start: std::time::Instant,
    group_header_injects: &[(String, String)],
) -> AttemptOutcome {
    let attempt_start = std::time::Instant::now();
    let attempt_ts = aidog_db::now();
    // 平台限频（extra.rate_limits）：一次访问平台级只占一次；候选筛选后仍可能被并发抢占，
    // 占位失败按「该平台不可用」排除。模型级在每个模型开始前重判。
    let access = match rate_slot.enter(
        state,
        log,
        attempts,
        RateSlotCtx {
            route: &route,
            requested_model,
            req_value,
            model_alt,
            label: "chat",
        },
    ) {
        Ok(access) => access,
        Err(()) => return AttemptOutcome::Next { connect_failed: false },
    };
    // 额度窗口闸门（extra.quota_windows 执行）：超限不发上游请求（不计调用次数），排除整个平台。
    if !crate::gateway::quota_window::gate_attempt(
        &state.db,
        log,
        attempts,
        &route.platform,
        &route.target_model,
        model_alt,
    )
    .await
    {
        return AttemptOutcome::Next { connect_failed: false };
    }
    let tried: Vec<u64> = attempts.iter().map(|a| a.platform_id).chain([route.platform.id]).collect();
    let capped = budget.begin_call_untried(log, &tried);
    let mut switched = false;
    let out = forward_attempt_model(
        state,
        log,
        attempts,
        route,
        candidate_total,
        is_last_candidate || capped,
        attempt_start,
        attempt_ts,
        log_settings,
        lang,
        group,
        chat_req,
        req_value,
        source_protocol,
        requested_model,
        is_stream,
        orig_headers,
        sched_settings,
        start,
        group_header_injects,
        model_alt && !capped,
        &mut switched,
        &access,
    )
    .await;
    match (out, access) {
        (AttemptOutcome::Respond(r), Some(a)) => {
            AttemptOutcome::Respond(crate::gateway::rate_limit::hold_until_body_done(r, a))
        }
        (out, _) => out,
    }
}

/// 候选耗尽终态状态码：被限频挡掉（`mark_rate_limited` 已标）或额度窗口耗尽
/// （extra.quota_windows 执行）→ 429（客户端应退避重试），其余 503。
pub(crate) fn exhausted_status(log: &ProxyLog) -> StatusCode {
    if log.blocked_reason == "rate_limit"
        || log.blocked_reason == crate::gateway::quota_window::QUOTA_BLOCKED_REASON
    {
        StatusCode::TOO_MANY_REQUESTS
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    }
}

/// 429 回复带 `Retry-After`（整数秒，向上取整，最小 1）：取路由快照里记的最早放行时刻；无已知时刻不带。
pub(crate) fn add_retry_after(r: &mut axum::response::Response, log: &ProxyLog) {
    if r.status() != StatusCode::TOO_MANY_REQUESTS {
        return;
    }
    let Some(at) = log.routing.as_ref().and_then(|x| x.retry_at_ms()) else { return };
    let secs = ((at - aidog_db::now()).max(0) + 999) / 1000;
    r.headers_mut().insert(axum::http::header::RETRY_AFTER, secs.max(1).into());
}

/// 记一次限频放行时刻（取最早）进路由快照。
pub(crate) fn note_retry_at(log: &mut ProxyLog, at_ms: Option<i64>) {
    if let (Some(at), Some(r)) = (at_ms, log.routing.as_mut()) {
        r.note_retry_at_ms(at);
    }
}

/// 一次请求的上游调用配额（分组「最大尝试次数」，请求尝试-逐次重算 票 01）：
/// 换平台与同平台换模型各调用一次都计 1，用尽即停；四类请求经 `AttemptLoop` 共用。
pub(crate) struct CallBudget {
    max: usize,
    used: usize,
}

impl CallBudget {
    /// DB 里不合法的 0 按 1 处理（至少调用一次）。
    pub(crate) fn new(max_attempts: u32) -> Self {
        Self { max: (max_attempts as usize).max(1), used: 0 }
    }
    pub(crate) fn max(&self) -> usize {
        self.max
    }
    pub(crate) fn exhausted(&self) -> bool {
        self.used >= self.max
    }
    /// 下一次调用是否就是配额内最后一次（外层循环据此给候选定 `is_last_candidate`）。
    pub(crate) fn next_is_last(&self) -> bool {
        self.used + 1 >= self.max
    }
    /// 只计数：返回 true = 这是配额内最后一次。
    pub(crate) fn count_call(&mut self) -> bool {
        self.used += 1;
        self.exhausted()
    }
    /// 逐次重算版（聊天）：最后一次时把从未尝试（`tried` 之外）的入选候选补记为 retry-limit。
    pub(crate) fn begin_call_untried(&mut self, log: &mut ProxyLog, tried: &[u64]) -> bool {
        let last = self.count_call();
        if last && let Some(r) = log.routing.as_mut() {
            r.mark_retry_limit_untried(self.max as u32, tried);
        }
        last
    }
}

/// 单个模型的一次上游尝试。`has_next_model`：链上还有备选；`switched`：置 true 表示
/// 本次失败属模型类、已留痕未记账，调用方应换下一个模型重发（返回值此时无意义）。
#[allow(clippy::too_many_arguments)]
async fn forward_attempt_model(
    state: &Arc<ProxyState>,
    log: &mut ProxyLog,
    attempts: &mut Vec<ProxyAttempt>,
    route: RouteResult,
    candidate_total: usize,
    is_last_candidate: bool,
    attempt_start: std::time::Instant,
    attempt_ts: i64,
    log_settings: &ProxyLogSettings,
    lang: Lang,
    group: &Group,
    chat_req: &mut ChatRequest,
    req_value: &Value,
    source_protocol: &Protocol,
    requested_model: &str,
    is_stream: bool,
    orig_headers: &axum::http::HeaderMap,
    sched_settings: &super::models::SchedulingBreakerSettings,
    start: std::time::Instant,
    group_header_injects: &[(String, String)],
    has_next_model: bool,
    switched: &mut bool,
    rate: &Option<Arc<crate::gateway::rate_limit::Access>>,
) -> AttemptOutcome {
    // group 层（路由前）收集到的 header 注入 + 下方 platform 层新增的，一起写进上游请求头。
    // 每个候选各建一份，换平台重试时不残留上一候选的注入。
    let mut header_injects: Vec<(String, String)> = group_header_injects.to_vec();
    let actual_model = route.target_model.clone();

    // OpenCode Zen：api_key 留空 → 注入匿名免费 key（$opencode）；用户填了用用户的。
    let eff_api_key = resolve_opencode_zen_key(&route.platform);

    let target = match select_endpoint_target(
            state,
            &route,
            source_protocol,
            orig_headers,
            attempts,
            attempt_start,
            attempt_ts,
            is_last_candidate,
            lang,
            start,
            log,
            log_settings,
        )
        .await
        {
            Ok(v) => v,
            Err(o) => return o.into(),
        };
    let target_protocol = target.target_protocol_enum.wire_str();
    let needs_model_remap = actual_model != requested_model;
    let disable_thinking = req_value.get("disable_thinking").and_then(|v| v.as_bool()).unwrap_or(false);

    // Upsert #3: route resolved
    log.actual_model = actual_model.clone();
    log.target_protocol = target_protocol.clone();
    log.platform_id = route.platform.id;
    tracing::info!(
        platform = %route.platform.name, platform_id = route.platform.id,
        requested_model = %requested_model, actual_model = %actual_model,
        source_protocol = ?source_protocol, target_protocol = %target_protocol,
        coding_plan = target.coding_plan, stream = is_stream, remap = needs_model_remap,
        "request routed to upstream"
    );
    upsert_log(state, log, log_settings).await;

    // 替换模型名
    chat_req.model = actual_model.clone();

    // 票 10：预取两处出站整形读库进 OutboundCtx（M4「单入口 shape(body)」落选形状的前置：
    // 读库进 ctx 后整形链可再收成纯函数单入口）。上限按 (平台协议裸名, 实际请求模型) 查
    // model_entry —— 同一模型在不同平台上限可不同。
    let model_max = aidog_db::model_max_output_tokens(
        &state.db,
        &route.platform.platform_type.wire_str(),
        &actual_model,
    )
    .await
    .ok()
    .flatten();
    let model_forces_thinking = model_forces_thinking(
        &state.db,
        &route.platform.platform_type.wire_str(),
        &actual_model,
    )
    .await;

    let Prepared { mut req_body, url } = match prepare_outbound(OutboundCtx {
        state,
        group,
        route: &route,
        log,
        chat_req,
        req_value,
        header_injects: &mut header_injects,
        target: &target,
        actual_model: actual_model.clone(),
        model_max,
        model_forces_thinking,
        is_stream,
        lang,
        log_settings,
        start,
        attempts,
        attempt_start,
        attempt_ts,
        disable_thinking,
    })
        .await
        {
            Ok(v) => v,
            Err(o) => return o.into(),
        };

    // ── 中间件可还原 mask（Reversible Mask）：在最终出站 body 上执行（协议转换之后），
    // 命中片段换成占位符；映射表只在本 attempt 内存，响应侧还原。应用失败 → fail-closed 503。
    // 用户请求侧日志同步改成占位符形态（日志库任何列不留原文）。──
    let mw_restore = {
        let mw_settings = state.settings_cache.read().await.middleware_settings.clone();
        match middleware_body::apply_reversible_mask_body(
            &state.middleware,
            &mw_settings,
            &mut req_body,
            target.target_protocol_enum,
            &actual_model,
            Some(&group.group_key),
            route.platform.id as i64,
            Some(&log.request_headers),
        ) {
            Ok(map) if map.is_empty() => None,
            Ok(map) => {
                log.request_body = map.mask_log_body(&log.request_body);
                Some(Arc::new(map))
            }
            Err(e) => {
                tracing::warn!(error = %e, "reversible mask failed: fail-closed 503");
                return AttemptOutcome::Respond(
                    block_privacy_fail_closed(state, log.clone(), log_settings, lang, start).await,
                );
            }
        }
    };

    let req_body_str = serde_json::to_string(&req_body).unwrap_or_default();


    let (resp, breaker_th) = match send_upstream(
        state,
        &route,
        group,
        log,
        attempts,
        OutboundRequest {
            req_body_str,
            header_injects,
            client_type: &target.client_type,
            eff_api_key: eff_api_key.clone(),
            orig_headers,
            is_stream,
            target_protocol_enum: target.target_protocol_enum,
            url: url.clone(),
            model: &actual_model,
        },
        lang,
        candidate_total,
        is_last_candidate,
        attempt_start,
        attempt_ts,
        start,
        log_settings,
        sched_settings,
        has_next_model,
    )
        .await
        {
            Ok(v) => v,
            Err(o) => return o.into(),
        };


    // 首字节耗时：send_upstream 返回即响应头已到（流式 / 非流式同口径）。
    let ttfb_ms = attempt_start.elapsed().as_millis() as i64;

    // ── 捕获上游响应 headers + status ──
    let status = resp.status();
    log.upstream_status_code = status.as_u16() as i32;
    // builtin-tool-compat 运行时审计：4xx + 出站含 tools → blocked_by=upstream/tools_4xx
    builtin_tools::mark_tools_4xx(log, &req_body, status.as_u16());
    // clone 上游响应头，供回包前透传筛选用（resp 后续被 bytes()/bytes_stream() 消费）
    let upstream_resp_headers = resp.headers().clone();
    log.upstream_response_headers =
        upstream_headers_json_for_log(&upstream_resp_headers, log_settings.log_upstream_request);

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
        return handle_non_success(
            resp,
            status,
            state,
            log,
            attempts,
            &route,
            group,
            &breaker_th,
            NonSuccessCtx {
                url: &url,
                start,
                attempt_start,
                attempt_ts,
                is_last_candidate,
                log_settings,
                requested_model,
                has_next_model,
                switched,
                ttfb_ms: Some(ttfb_ms),
                candidate_total,
            },
        )
        .await;
    }

    // ── 2xx：状态码成功，但「200 + 空/无效响应」按决策 B 仍当作失败重试。──
    // 成功记账（record_success / 恢复 auto_disabled / 清 strike / attempts.push 成功 / log.attempts）
    // 推迟到「确认非空有效响应」之后，由 commit_2xx_success! 宏统一执行（避免重复且保证仅真成功才记账）。
    let attempt_latency_ms = attempt_start.elapsed().as_millis() as i64;

    // 决策 B 失败（200 空响应）时记一次失败 attempt 并 failover；候选耗尽则返回 502。
    // 不降权（2026-09-15 用户裁决）：不计熔断、不 auto_disable，仅 inflight-1。
    macro_rules! retry_on_empty_2xx {
        ($reason:expr, $upstream_text:expr) => {{
            state.scheduler.record_ignored(route.platform.id);
            // 失败降权（health-aware，2026-09-29）：决策 B 空响应（非流式空 body / 流式 peek
            // 秒断无内容）计入服务端/网络档降权。peek 在 commit_2xx_success 之前，
            // 流式中途断连（已提交成功后）不经过本宏，不计降权。
            state.scheduler.record_penalty(
                route.platform.id,
                super::scheduling::PenaltyTier::Server,
                aidog_db::now(),
            );
            tracing::warn!(
                platform = %route.platform.name, platform_id = route.platform.id,
                reason = $reason, "decision-B: upstream 200 but empty/invalid response, failover next platform"
            );
            attempts.push(ProxyAttempt {
                // 空 200 属服务类，直接换平台（或 502），链上剩余模型不再尝试
                chain_stop: has_next_model.then(|| "service".to_string()),
                kind: Some("empty_200".to_string()),
                error_class: Some("service".to_string()),
                effects: vec!["penalty_server".to_string()],
                ttfb_ms: Some(ttfb_ms),
                platform_id: route.platform.id,
                platform_name: route.platform.name.clone(),
                status_code: 200,
                error: $reason.to_string(),
                duration_ms: attempt_latency_ms,
                ts: attempt_ts,
                model: Some(route.target_model.clone()),
                ..Default::default()
            });
            let _ = aidog_db::set_platform_last_error(
                &state.db, route.platform.id, Some(format!("HTTP 200: {}", $reason)),
            ).await;
            if !is_last_candidate {
                return AttemptOutcome::Next { connect_failed: false };
            }
            // 候选耗尽：返回 502 + 已记录的 attempts（此时尚未向客户端发任何字节，安全）。
            log.platform_id = route.platform.id;
            log.status_code = 502;
            log.done = true;
            log.upstream_status_code = status.as_u16() as i32;
            let err_body = format!("{}: 200 but empty/invalid response", i18n::t(lang, ErrorKey::Upstream));
            // 取证：把上游真实首块原文截断（≤4KB + truncated 标记）落 response_body，替代占位文案；
            // upstream_text 为空时回退占位兜底。下次 GLM 间歇空流复现自动留 DB 证据。
            let captured = truncate_peek_text($upstream_text);
            log.response_body = if captured.is_empty() { $reason.to_string() } else { captured };
            log.user_response_body = err_body.clone();
            log.user_response_headers = r#"{"content-type":"text/plain"}"#.to_string();
            log.duration_ms = start.elapsed().as_millis() as i32;
            log.retry_count = aidog_db::models::retry_count_of(&attempts);
            log.attempts = std::mem::take(attempts);
            upsert_log(state, log, log_settings).await;
            return AttemptOutcome::Respond({
                let mut r = (StatusCode::BAD_GATEWAY, err_body).into_response();
                inject_trace_header(&mut r);
                r
            });
        }};
    }

    // 真成功记账：熔断成功 + 恢复 auto_disabled + attempts.push 成功 + 填 log.attempts。
    macro_rules! commit_2xx_success {
        () => {{
            // 熔断指标：成功 → 更新延迟 EMA + breaker Closed/HalfOpen→Closed + inflight-1。
            // 注意流式此处为「首个有效内容」延迟（peek 已收到内容）；作为延迟近似记入 EMA。
            super::call_scope::commit_success(state, &route, attempts, status.as_u16(), Some(ttfb_ms), attempt_latency_ms, attempt_ts).await;
            // 成功后把 attempts 收进 log（guard 快照与 finish 终态写依赖此前填充；原宏同语义）。
            log.platform_id = route.platform.id;
            log.retry_count = aidog_db::models::retry_count_of(&attempts);
            log.attempts = std::mem::take(attempts);
        }};
    }

    // forward_attempt 本次路由决策的协议/模型元数据，finish_nonstream / finish_stream 共用（见 finish.rs::AttemptCtx）。
    let attempt_ctx = AttemptCtx {
        source_protocol: source_protocol.clone(),
        target_protocol: target.target_protocol_enum.clone(),
        same_protocol_passthrough: target.same_protocol_passthrough,
        coding_plan: target.coding_plan,
        requested_model: requested_model.to_string(),
        actual_model: actual_model.clone(),
        eff_api_key: eff_api_key.clone(),
        quota_base_url: target.target_base_url.clone(),
        disable_thinking,
        // issue 71：OpenAI Chat / Completions 客户端的 usage 块门控（stream_options.include_usage）
        include_usage: req_value
            .get("stream_options")
            .and_then(|v| v.get("include_usage"))
            .and_then(|v| v.as_bool())
            .unwrap_or(false),
        rate: rate.clone(),
        cache_key: sched_settings
            .cache_affinity
            .then(|| crate::gateway::scheduling::sticky_session_key(&group.group_key, orig_headers)),
    };

    // 非流式：直接透传 JSON
    if !is_stream {
        let body = resp.bytes().await.unwrap_or_default();
        // usage 借用：lossy 不经 to_string 中转（extract_usage 在 finish_nonstream 内）
        let lossy = String::from_utf8_lossy(&body);
        let resp_str: &str = &lossy;

        // ── 决策 B（非流式）：200 但空 body / error 结构 / 无有效 choices/content → 失败重试。──
        if !is_nonstream_body_valid(resp_str) {
            retry_on_empty_2xx!("200 but empty/invalid body", resp_str);
        }
        commit_2xx_success!();

        return AttemptOutcome::Respond(
            finish_nonstream(
                state,
                log,
                log_settings,
                group,
                &route,
                &attempt_ctx,
                &upstream_resp_headers,
                start,
                mw_restore,
                body,
            )
            .await,
        );
    }

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
        retry_on_empty_2xx!("200 but empty/invalid stream", &peek_text);
    }
    // Meaningful：确认上游真实产出 → 提交成功记账（在构建 guard 前，使 guard 的 log 快照含正确 attempts）。
    commit_2xx_success!();

    // 决策 B：把 peek 阶段已缓冲的首批 chunk 原样 prepend 回流（不能吞首块），再接上游剩余流；
    // finish_stream 内对缓冲块与后续块一视同仁（token 聚合 / 转换 / finalize 不受影响）。
    let buffered_head =
        futures::stream::iter(peek_buf.into_iter().map(Ok::<Bytes, reqwest::Error>));
    let full_stream = buffered_head.chain(upstream_stream);

    AttemptOutcome::Respond(
        finish_stream(
            full_stream,
            state,
            log,
            log_settings,
            group,
            &route,
            &attempt_ctx,
            &upstream_resp_headers,
            start,
            mw_restore,
        )
        .await,
    )
}

/// 候选耗尽时统一终态：填 log 502 字段 + upsert 落库 + 构造 502 Response（含 trace 头）。
///
/// 抽自 3 处同构 502 终态（invalid protocol / base_url missing / upstream send error），
/// 共享 8 个 log 字段 + upsert + (StatusCode::BAD_GATEWAY, msg) 响应。
/// 签名预留 `response_body` 与 `user_response_body` 分离：前者落库审计取证（上游原文/原因），
/// 后者返回客户端（i18n 友好文案）。`platform_id` 显式传入便于后续 devin/handler/passthrough 复用。
///
/// ponytail: 禁动内嵌宏 retry_on_empty_2xx! —— 其 502 分支含额外 upstream_status_code +
/// truncate_peek_text 取证逻辑，非完全同构；后续若需统一，扩参 `Option<(i32, String)>` 取证元组即可。
#[allow(clippy::too_many_arguments)]
pub(crate) async fn finalize_proxy_502(
    state: &Arc<ProxyState>,
    log: &mut ProxyLog,
    attempts: &mut Vec<ProxyAttempt>,
    platform_id: u64,
    response_body: String,
    user_response_body: String,
    start: std::time::Instant,
    log_settings: &ProxyLogSettings,
) -> axum::response::Response {
    log.platform_id = platform_id;
    log.response_body = response_body;
    log.status_code = 502;
    log.done = true;
    log.user_response_body = user_response_body.clone();
    log.user_response_headers = r#"{"content-type":"text/plain"}"#.to_string();
    log.duration_ms = start.elapsed().as_millis() as i32;
    log.retry_count = aidog_db::models::retry_count_of(attempts);
    log.attempts = std::mem::take(attempts);
    upsert_log(state, log, log_settings).await;
    let mut r = (StatusCode::BAD_GATEWAY, user_response_body).into_response();
    inject_trace_header(&mut r);
    r
}

/// registry 条目是否强制思考（price_data 里 `thinking_toggleable == false`；
/// 未标注 / 条目缺失 / 查询失败 → false，行为同现状）。
async fn model_forces_thinking(db: &aidog_db::Db, platform_code: &str, model_id: &str) -> bool {
    aidog_db::get_model_entry(db, platform_code, model_id)
        .await
        .ok()
        .flatten()
        .and_then(|e| serde_json::from_str::<Value>(&e.price_data).ok())
        .and_then(|v| v.get("thinking_toggleable").and_then(Value::as_bool))
        .is_some_and(|t| !t)
}

// ── 阶段函数：select → prepare → send（forward_attempt 只留「选 → 备 → 发 → 收」骨架）──
// 阶段早退使用 StageExit，由 forward_attempt 转成外层 AttemptOutcome；保留原早退副作用。

/// 阶段 1：端点选择（含 UA 透传分支）+ target_protocol 合法性 / base_url guard。
struct TargetSel<'a> {
    /// 端点协议（引用借用自 route.platform / matched endpoint）
    target_protocol_enum: &'a Protocol,
    target_base_url: String,
    client_type: String,
    coding_plan: bool,
    same_protocol_passthrough: bool,
}

// ponytail: 仅检测 5 协议，未来扩展协议需同步更新此列表。
fn is_valid_wire_protocol(p: &Protocol) -> bool {
    matches!(
        p,
        Protocol::Anthropic
            | Protocol::OpenAI
            | Protocol::OpenAIResponses
            | Protocol::OpenAICompletions
            | Protocol::Gemini
    )
}

#[allow(clippy::too_many_arguments, clippy::result_large_err)]
async fn select_endpoint_target<'a>(
    state: &Arc<ProxyState>,
    route: &'a RouteResult,
    source_protocol: &Protocol,
    orig_headers: &axum::http::HeaderMap,
    attempts: &mut Vec<ProxyAttempt>,
    attempt_start: std::time::Instant,
    attempt_ts: i64,
    is_last_candidate: bool,
    lang: Lang,
    start: std::time::Instant,
    log: &mut ProxyLog,
    log_settings: &ProxyLogSettings,
) -> Result<TargetSel<'a>, StageExit> {
    // 尝试匹配端点：按 source_protocol 查找平台是否支持对应协议的端点。
    // 先精确匹配；openai_responses 源（Codex）若无 Responses 端点，回退到 openai 端点
    // （普通 chat/completions 平台），出站经 to_openai 转换。
    let matched_ep = select_endpoint_for_protocol(&route.platform.endpoints, source_protocol);

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
            Some(p) => match route.platform.endpoints.iter().find(|ep| ep.protocol == p) {
                Some(ep) => {
                    tracing::info!(
                        platform = %route.platform.name, platform_id = route.platform.id,
                        source_protocol = ?source_protocol, ua_protocol = ?p,
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
        .map(|ep| {
            (
                &ep.protocol,
                ep.base_url.clone(),
                ep.client_type.clone(),
                ep.coding_plan,
            )
        })
        .unwrap_or((
            &route.platform.platform_type,
            route.platform.base_url.clone(),
            "default".to_string(),
            false,
        ));

    // ── target_protocol 合法性 guard（bugfix: s2-bug1-target-protocol）──
    // matched_ep=None 时 fallback 到 platform_type，但 platform_type 可能是平台别名(sensenova/glm等)
    // 而非 5 个有效协议之一(anthropic/openai/openai_responses/openai_completions/gemini)。
    // 这种情况下 target_protocol 会落库为平台名，导致后续统计/审计出错。
    //
    // 验收：endpoint 匹配失败时 target_protocol 必须落 5 协议之一；否则 route fail。
    if !is_valid_wire_protocol(target_protocol_enum) {
        tracing::error!(
            platform = %route.platform.name, platform_id = route.platform.id,
            source_protocol = ?source_protocol, target_protocol = ?target_protocol_enum,
            endpoints_len = route.platform.endpoints.len(),
            "target_protocol is not a valid wire protocol, endpoint selection failed"
        );
        // endpoint 选择失败且无有效兜底 → 记录 error 并 route fail
        if !is_last_candidate {
            // 非 last candidate：记录 attempt 并换下一个候选
            attempts.push(ProxyAttempt {
                chain_stop: None,
                platform_id: route.platform.id,
                platform_name: route.platform.name.clone(),
                status_code: 0,
                error: format!("invalid target protocol: {:?}", target_protocol_enum),
                duration_ms: attempt_start.elapsed().as_millis() as i64,
                ts: attempt_ts,
                model: Some(route.target_model.clone()),
                ..Default::default()
            });
            let _ = aidog_db::set_platform_last_error(
                &state.db,
                route.platform.id,
                Some(format!(
                    "invalid target protocol: {:?}",
                    target_protocol_enum
                )),
            )
            .await;
            return Err(StageExit::Retry {
                connect_failed: false,
            });
        }
        // last candidate：返回 502 + 审计落库
        let msg = format!(
            "{}: endpoint selection failed (no valid wire protocol)",
            i18n::t(lang, ErrorKey::Upstream)
        );
        return Err(StageExit::Respond(
            finalize_proxy_502(
                state,
                log,
                attempts,
                route.platform.id,
                format!("invalid target protocol: {:?}", target_protocol_enum),
                msg,
                start,
                log_settings,
            )
            .await,
        ));
    }

    // ── base_url 缺失 guard ──
    // endpoints/base_url 均空（OAuth 未回填 / 用户手建平台漏配）→ 友好错误替代 reqwest builder error。
    // 空 base_url 拼 api_path 得无 host 相对 URL，reqwest builder 直接 error → 502「upstream error」无诊断价值。
    // 不发上游：记录平台 last_error + attempts；非末位候选 → Next 换下个；末位候选 → 502 + 审计落库。
    // ponytail: 对称防护——forward.rs 单一 URL 构造点覆盖流式 + 非流式两分支（URL 在分支前已定）。
    if target_base_url.trim().is_empty() {
        tracing::warn!(
            platform = %route.platform.name, platform_id = route.platform.id,
            "upstream base_url empty, skipping platform"
        );
        attempts.push(ProxyAttempt {
            chain_stop: None,
            platform_id: route.platform.id,
            platform_name: route.platform.name.clone(),
            status_code: 0,
            error: "base_url missing".to_string(),
            duration_ms: attempt_start.elapsed().as_millis() as i64,
            ts: attempt_ts,
            model: Some(route.target_model.clone()),
            ..Default::default()
        });
        let _ = aidog_db::set_platform_last_error(
            &state.db,
            route.platform.id,
            Some("base_url missing".to_string()),
        )
        .await;
        if !is_last_candidate {
            return Err(StageExit::Retry {
                connect_failed: false,
            });
        }
        let msg = format!("{}: base_url 缺失", i18n::t(lang, ErrorKey::Upstream));
        return Err(StageExit::Respond(
            finalize_proxy_502(
                state,
                log,
                attempts,
                route.platform.id,
                "base_url missing".to_string(),
                msg,
                start,
                log_settings,
            )
            .await,
        ));
    }

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
        Some(p) => matched_ep.map(|ep| ep.protocol == p).unwrap_or(false),
        None => matched_ep
            .map(|ep| ep.protocol == *source_protocol)
            .unwrap_or(false),
    };
    Ok(TargetSel {
        target_protocol_enum,
        target_base_url,
        client_type,
        coding_plan,
        same_protocol_passthrough,
    })
}

/// 阶段 2：出站 body 预备——chat_req 裁剪、中间件入站/预算、协议转换或透传分叉、
/// 字段级出站处理与留痕；产出最终上游 body 与 URL。
struct Prepared {
    req_body: Value,
    url: String,
}

/// 阶段 2（prepare_outbound）的入参聚合（票 10，参照 NonSuccessCtx 先例）：
/// 17 个散参收成一个结构。`model_max` / `model_forces_thinking` 由调用方预取
/// （spec M4「单入口 shape(body)」落选形状的前置：读库进 ctx 后，整形链可再收纯函数单入口）。
struct OutboundCtx<'a> {
    state: &'a Arc<ProxyState>,
    group: &'a Group,
    route: &'a RouteResult,
    log: &'a mut ProxyLog,
    chat_req: &'a mut ChatRequest,
    req_value: &'a Value,
    header_injects: &'a mut Vec<(String, String)>,
    target: &'a TargetSel<'a>,
    actual_model: String,
    model_max: Option<i64>,
    model_forces_thinking: bool,
    is_stream: bool,
    lang: Lang,
    log_settings: &'a ProxyLogSettings,
    start: std::time::Instant,
    attempts: &'a mut Vec<ProxyAttempt>,
    attempt_start: std::time::Instant,
    attempt_ts: i64,
    disable_thinking: bool,
}

#[allow(clippy::result_large_err)]
async fn prepare_outbound(ctx: OutboundCtx<'_>) -> Result<Prepared, StageExit> {
    let OutboundCtx {
        state,
        group,
        route,
        log,
        chat_req,
        req_value,
        header_injects,
        target,
        actual_model,
        model_max,
        model_forces_thinking,
        is_stream,
        lang,
        log_settings,
        start,
        attempts,
        attempt_start,
        attempt_ts,
        disable_thinking,
    } = ctx;
    // ── max_tokens 出站裁剪（convert_request 前）──
    // 客户端 max_tokens 超过选定模型上限时裁剪到上限；未传 / 模型无上限则不动（Q3 保守）。
    // 此处裁 chat_req（转换分支的入参，同时是 token 估算口径）；出站 body 上的同上限复裁
    // 见下方 `cap_body_max_tokens` 调用点（透传分支唯一生效处）。
    // 票 11：转换分支在这里就把 chat_req 裁到位，下方 body 层的 `cap_body_max_tokens` 因此
    // 是幂等复裁、不再命中 → 留痕会漏掉整条转换路径。故在此处记住「裁过」，与 body 层的留痕合并。
    let mut capped_on_chat_req = false;
    {
        let (capped, did_cap) = super::router::cap_max_tokens(chat_req.max_tokens, model_max);
        if did_cap {
            tracing::info!(
                model = %actual_model,
                requested = ?chat_req.max_tokens, capped_to = ?capped,
                "max_tokens exceeds model limit, capping"
            );
            chat_req.max_tokens = capped;
            capped_on_chat_req = true;
        }
    }

    // ── 中间件入站规则（platform 层，候选选定后、convert_request 前）──
    // 仅应用 platform 作用域规则（global/group 已在路由前应用，避免重复）。
    // block 在 forward 前返回，对透传/转换分支均生效；mask/inject 改写 chat_req，
    // 转换分支(convert_request 读 chat_req)由此生效；同协议透传分支用 req_value 原体，
    // 由下方 `apply_middleware_body` 在出站 body 上补齐（票 02）。
    // inject 的 header_set 只收集进 header_injects，两条分支共用下方同一个上游请求头构造点。
    {
        let mw_settings = state
            .settings_cache
            .read()
            .await
            .middleware_settings
            .clone();
        // request_headers 条件同 group 层：用已脱敏的 log.request_headers。
        let outcome = state.middleware.apply_inbound_platform(
            &mw_settings,
            chat_req,
            // group_key 必须透传：同时限定 groups + platforms 的规则只在本挂载点判定，
            // 少了它 g_ok 恒 false → 规则永不命中（评审 F2）。
            Some(&group.group_key),
            route.platform.id as i64,
            Some(&log.request_headers),
            header_injects,
        );
        match outcome {
            InboundOutcome::Blocked {
                blocked_by,
                blocked_reason,
            } => {
                log.platform_id = route.platform.id;
                return Err(StageExit::Respond(
                    block_inbound(
                        state,
                        log.clone(),
                        log_settings,
                        lang,
                        blocked_by,
                        blocked_reason,
                        start,
                    )
                    .await,
                ));
            }
            // 观察模式命中（票 04）：请求照常转发、照常计费，只标审计列。
            InboundOutcome::Observed { blocked_by } => {
                super::record_observed(log, blocked_by);
            }
            InboundOutcome::Continue => {}
        }
        // 成本预算闸门（票 06）：限定了平台的规则在此才拿得到 platform_id（未限定平台的
        // 已在路由前的 group 挂载点判过，不会重复查库）。无 budget_gate 规则时零查询。
        // model 传 actual_model：与花费聚合侧 stats_agg_hourly.model 同口径（评审 F5）。
        if let InboundOutcome::Blocked {
            blocked_by,
            blocked_reason,
        } = state
            .middleware
            .check_budget(
                &mw_settings,
                &state.db,
                chat_req,
                &actual_model,
                Some(&group.group_key),
                Some(route.platform.id as i64),
                Some(&log.request_headers),
            )
            .await
        {
            log.platform_id = route.platform.id;
            return Err(StageExit::Respond(
                block_inbound(
                    state,
                    log.clone(),
                    log_settings,
                    lang,
                    blocked_by,
                    blocked_reason,
                    start,
                )
                .await,
            ));
        }
    }

    // ── 手动预算耗尽阻断（mock / 上游平台均适用，转发前惰性只读判定，不写库）──
    // 任一 enabled 限额剩余 ≤ 0（含窗口惰性重置后）→ 不发上游/不出 mock，返回 402。
    // 平台保持启用，窗口/次日恢复后自动放行。无 manual_budgets（含透传）→ 跳过。
    if let Some(info) =
        super::manual_budget::evaluate_depletion(&route.platform.manual_budgets, aidog_db::now())
    {
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
        log.done = true;
        log.platform_id = route.platform.id;
        log.response_body = body.clone();
        log.user_response_body = body.clone();
        log.user_response_headers = r#"{"content-type":"application/json"}"#.to_string();
        log.duration_ms = start.elapsed().as_millis() as i32;
        attempts.push(ProxyAttempt {
            chain_stop: None,
            platform_id: route.platform.id,
            platform_name: route.platform.name.clone(),
            status_code: 402,
            error: "manual budget exhausted".to_string(),
            duration_ms: attempt_start.elapsed().as_millis() as i64,
            ts: attempt_ts,
            model: Some(route.target_model.clone()),
            ..Default::default()
        });
        log.retry_count = aidog_db::models::retry_count_of(attempts);
        log.attempts = std::mem::take(attempts);
        upsert_log(state, log, log_settings).await;
        return Err(StageExit::Respond({
            let mut r = (
                StatusCode::PAYMENT_REQUIRED,
                [(axum::http::header::CONTENT_TYPE, "application/json")],
                body,
            )
                .into_response();
            inject_trace_header(&mut r);
            r
        }));
    }

    // 协议转换 / 同协议透传：
    // - 透传分支（同协议）：用客户端原始请求体，仅 patch model 字段，跳过 messages/tools 结构转换；
    //   path 由 wire 协议决定（passthrough_api_path，与 convert_request 一致但不转 body）。
    // - 转换分支：wire format 由 endpoint 协议决定，API path 由平台类型决定。
    let platform_protocol = &route.platform.platform_type;
    // issue 68：按平台 code 取出站适配器（registry code = platform_type 的 wire 名），
    // 请求构造 / 响应解析 / 差异钩子全部经它分派，不再直接按 wire 协议调通用转换。
    let pa = aidog_adapter::platform::adapter_for(&route.platform.platform_type.wire_str());
    let (mut req_body, api_path) = if target.same_protocol_passthrough {
        // O3 判定：保留 clone。model remap 对几乎每个透传请求都要改写 body（&mut 必需），
        // 下方 cap/strip/hoist 链也全按 &mut Value 写；Cow 只在「零改写」请求上省这一次拷贝，
        // 覆盖面太窄，不值当把整条链改成 to_mut。真正的全量拷贝已在 request.rs 从 raw 解析处省掉。
        let mut body = req_value.clone();
        super::tool_search::emulate_value(&mut body, target.target_base_url.as_str());
        // model remap：透传下仍必须替换路由模型名（请求体 model 字段）
        if let Some(obj) = body.as_object_mut() {
            match target.target_protocol_enum {
                // Gemini generateContent 的 body 规范里没有顶层 model（模型在 URL path，
                // 由下方 passthrough_api_path 用改写后的 actual_model 构造）。客户端若带了这个键，
                // 上游按未知字段判 400，故剔除而非改写（票 09）。
                Protocol::Gemini => {
                    obj.remove("model");
                }
                _ => {
                    obj.insert("model".to_string(), Value::String(actual_model.clone()));
                }
            }
        }
        // 无签名 thinking 块清洗（仅 Anthropic wire 上游）：客户端回灌的历史里可能带 aidog 从
        // 非 Anthropic 上游合成的 thinking 块（reasoning_content / 行内 `<thinking>` 标签转换而来），
        // 这类块没有 Anthropic 签名，原样上送官方端点判 400。转换分支早有同语义丢弃，此处补齐透传分支。
        if target.target_protocol_enum.same_wire_family(&Protocol::Anthropic) {
            let removed = pa.strip_unsigned_thinking_blocks(&mut body);
            if removed > 0 {
                tracing::info!(
                    removed,
                    "passthrough: stripped unsigned thinking blocks before anthropic upstream"
                );
            }
        }
        let path =
            pa.passthrough_api_path(target.target_protocol_enum, &actual_model, platform_protocol, is_stream);
        tracing::debug!(protocol = %target.target_protocol_enum.wire_str(), "same-protocol passthrough: skip request format conversion");
        (body, path)
    } else {
        super::tool_search::emulate_chat_req(chat_req, target.target_base_url.as_str());
        pa.build_request(chat_req, target.target_protocol_enum, platform_protocol)
    };

    // Coding Plan 特殊处理：注入平台特有字段（issue 68：经适配器钩子分派；
    // 原 override_coding_plan_path 空 hook 已删——api_path 无需覆盖）。
    if target.coding_plan {
        pa.inject_coding_plan(&mut req_body, platform_protocol, target.target_protocol_enum);
    }

    // disable_thinking：aidog 本地扩展字段（客户端请求禁用思考）。非标字段任何上游不认 →
    // 识别后必剥除。语义 = 剔掉开启型思考参数后，按目标 wire 协议写入显式禁用参数
    // （用户决策 2026-08-26；只剔不写会让上游按自身默认开启思考，见 apply_disable_thinking 注释）。
    // MiniMax-M2 等内置思考模型上游仍无法真正禁用，响应不剥离。
    //
    // 强制思考模型（registry `thinking_toggleable: false`，MiniMax-M3.1-Flash 实证 400 code 2013
    // "requires adaptive thinking"）：显式禁用参数一个都不写出站。票 10 起该 registry 读由
    // 调用方预取进 OutboundCtx（M4 单入口前置），不再按需读。
    let explicit_disable = disable_thinking
        || req_body
            .get("thinking")
            .and_then(|t| t.get("type"))
            .and_then(Value::as_str)
            == Some("disabled")
        || req_body.get("reasoning_effort").and_then(Value::as_str) == Some("none")
        || req_body
            .get("reasoning")
            .and_then(|r| r.get("effort"))
            .and_then(Value::as_str)
            == Some("none");
    let forced_thinking = explicit_disable && model_forces_thinking;
    pa.apply_disable_thinking(
        &mut req_body,
        disable_thinking,
        target.target_protocol_enum,
        target.target_base_url.as_str(),
        forced_thinking,
    );

    // tool_choice 平台差异调整（issue 76 跟进：Kimi 家族 kimi-k2.6 / kimi-k2.7-code 系
    // 上游对 `tool_choice:"required"` 报错 → 降级为 "auto"；透传与转换两分支共用本 seam）。
    pa.adjust_tool_choice(&mut req_body, target.target_protocol_enum, &actual_model);

    // builtin-tool-compat：内置工具兼容（全局总开关，settings scope "proxy" / key
    // "builtin_tool_compat"，默认关闭零进入）。开启 = 所有平台所有模型剔除内置工具定义。
    // 透传与转换两分支共用本 seam（见 builtin_tools.rs 模块注释）。
    let btc_global = state
        .settings_cache
        .read()
        .await
        .builtin_tool_compat
        .enabled;
    builtin_tools::apply_builtin_tool_compat(&mut req_body, &actual_model, btc_global);

    // ── max_completion_tokens 归一（必须排在下方裁剪之前）──
    // 透传分支的 body 是客户端原体：新版 OpenAI SDK 只发 `max_completion_tokens`，
    // 不折成 `max_tokens` 的话下方 cap 找不到槽位 → 超模型上限的值原样上送（票 05）。
    fold_openai_max_completion_tokens(&mut req_body, target.target_protocol_enum);

    // ── 图片块默认 detail 剔除（透传与转换两分支共用本 seam）──
    // OpenAI SDK 会自己填 `image_url.detail:"auto"`，部分兼容上游只认 low/high 见 auto 即 400。
    let stripped_detail = strip_default_image_detail(&mut req_body, target.target_protocol_enum);
    if stripped_detail > 0 {
        tracing::info!(
            count = stripped_detail, model = %actual_model,
            "stripped default image_url.detail=auto before upstream"
        );
    }

    // ── max_tokens 出站裁剪（body 层，透传与转换两分支共用本 seam）──
    // 上限口径与上方 chat_req 侧同源（同一个 `model_max`，不会出现「裁两次不同上限」）：
    // 转换分支此处为幂等复裁（chat_req 已裁到同值，不再命中）；透传分支此处是唯一生效点，
    // 修掉「客户端 max_tokens 超模型上限 → 上游 400」（票 02）。
    // 留痕：proxy_log 同时存客户端原始 body（request_body）与上游实际 body
    // （upstream_request_body），两者一比即知裁到多少。
    // 票 10：本次出站 body 被丢弃 / 被改写的字段名 token（只记名不记值），落 proxy_log.field_trace。
    let capped_on_body = pa.cap_max_tokens(&mut req_body, model_max, target.target_protocol_enum);
    if let Some((requested, capped)) = capped_on_body {
        tracing::info!(
            model = %actual_model, requested, capped_to = capped,
            passthrough = target.same_protocol_passthrough,
            "max_tokens exceeds model limit, capping outbound body"
        );
    }

    // ── 中间件入站改写（Value 层，仅透传分支）──
    // 转换分支的 mask/override/inject 已在分叉前作用于 chat_req，两处都跑会把 system_append
    // 注入两遍，故此处只补透传分支（脱敏规则不再因为「恰好同协议」被绕过，票 02）。
    let mut middleware_changed = false;
    if target.same_protocol_passthrough {
        let mw_settings = state
            .settings_cache
            .read()
            .await
            .middleware_settings
            .clone();
        middleware_changed = middleware_body::apply_middleware_body(
            &state.middleware,
            &mw_settings,
            &mut req_body,
            target.target_protocol_enum,
            &actual_model,
            Some(&group.group_key),
            route.platform.id as i64,
            Some(&log.request_headers),
        );
        if middleware_changed {
            tracing::info!(
                platform_id = route.platform.id, model = %actual_model,
                "middleware inbound rules rewrote passthrough body"
            );
        }
    }

    // ── 未建模顶层字段兜底透传（票 01，透传与转换两分支共用本 seam）──
    // 客户端设的采样参数（stop / top_k / seed / response_format / …）在 ChatRequest 强类型模型里
    // 没有对应字段，转换分支经 wire struct 序列化后静默消失。本 seam 从客户端原体按**目标 wire
    // 协议的允许集合**补齐（键名按协议换名），允许集合外的字段一律不写出。
    let dropped = pa.passthrough_unmodeled_fields(
        &mut req_body,
        req_value,
        target.target_protocol_enum,
        target.target_base_url.as_str(),
    );

    // 票 10：留痕落 log（无丢弃无改写时写空串，不产生噪音记录；本值随后续 upsert 入库，
    // 受 log_upstream_request 开关与 upstream_request_retention_days 清理约束）。
    // 每个候选平台重跑本段 → 无条件赋值，换平台重试时不残留上一候选的留痕。
    let mut dropped_fields = dropped;
    if !target.same_protocol_passthrough {
        dropped_fields.extend(dropped_media_fields(chat_req, target.target_protocol_enum));
        dropped_fields.extend(dropped_server_tools(chat_req, target.target_protocol_enum));
    }
    log.field_trace = build_field_trace(
        capped_on_chat_req,
        capped_on_body.is_some(),
        middleware_changed,
        &dropped_fields,
    );

    // ── 官方 OpenAI 输出长度键改写（票 05）──
    // 排在裁剪、中间件与兜底透传之后：前几步都按 `max_tokens` 认字段，改名放最后才不会漏。
    if rename_openai_max_tokens_key(&mut req_body, target.target_protocol_enum, target.target_base_url.as_str()) {
        tracing::debug!(model = %actual_model, "official OpenAI host: max_tokens → max_completion_tokens");
    }

    // 构建目标 URL
    let base_url = target.target_base_url.as_str().trim_end_matches('/');
    // api_path 版本段与 base_url 末段重复时去重（anthropic 系 api_path 自带 /v1，聚合站 base_url 也带）。
    let mut url = super::passthrough::join_upstream_path(base_url, &api_path);
    // Gemini streamGenerateContent 不带 alt=sse 时上游返回单个 JSON 数组（非 SSE），流式解析全部落空。
    if matches!(target.target_protocol_enum, Protocol::Gemini) && is_stream {
        url.push_str("?alt=sse");
    }
    log.upstream_request_url = url.clone();

    // ── 第三方 anthropic 端点不支持字段剔除 / 非标结构规整 ──
    // host-gated（仅 !is_official_anthropic_host）：
    //   - context_management：thinking 开启即无条件剔（第三方不认该协商字段；首轮 GLM 1210 + 有历史 DeepSeek 400）
    //   - thinking：仅历史 assistant 轮缺 thinking block（必 400 的不匹配）才剔，齐全直传
    //   - messages 内 role=system 非标位置规整：非流式多轮（有 assistant 历史）+ messages 内含 role=system
    //     时，GLM/DeepSeek 等 anthropic-compat 端点拒绝 → 400 code 1210 "API 调用参数有误"
    //     （DB 全样本交叉验证：9/9 失败均为 no_stream+assistant+messages 内 role=system；
    //      官方 Anthropic 接受该 CC 注入的非标位置，第三方严格）。规整=将 messages 内 role=system
    //     合并到顶层 system 数组（语义等价、Anthropic 规范形式），messages 数组移除该消息。
    //     仅非流式触发：流式 + 同结构当前工作正常（9279 PASS），不动避免回归。
    if matches!(target.target_protocol_enum, Protocol::Anthropic) && !is_official_anthropic_host(&url) {
        strip_thinking_if_unmatched(&mut req_body);
        // 无条件剥离 redacted_thinking content block：第三方 anthropic 端点（火山 doubao coding、
        // deepseek 等）不认该 Claude 4.x extended thinking 加密块 → 400 InvalidParameter
        // "invalid value: `redacted_thinking`"。同协议 passthrough 不走 to_anthropic 转换
        // （后者已 filter Unknown 含 redacted_thinking），content 原样透传即触发。redacted 内容
        // 加密 opaque 不可回放，剥离安全。trace 81dc4466 / 87e3c500 实证。
        strip_redacted_thinking_blocks(&mut req_body);
        if !is_stream {
            hoist_mid_messages_system(&mut req_body);
        }
    }
    Ok(Prepared { req_body, url })
}

/// 阶段 3：建上游请求（超时/HTTP client/请求头/日志镜像/beta 备用 builder）→
/// 熔断在途 +1 → 发送（同平台瞬时重试）→ anthropic-beta 400 剔头重试。返回上游响应。
struct OutboundRequest<'a> {
    req_body_str: String,
    header_injects: Vec<(String, String)>,
    client_type: &'a String,
    eff_api_key: String,
    orig_headers: &'a axum::http::HeaderMap,
    is_stream: bool,
    target_protocol_enum: &'a Protocol,
    url: String,
    model: &'a str,
}

#[allow(clippy::too_many_arguments, clippy::result_large_err)]
async fn send_upstream(
    state: &Arc<ProxyState>,
    route: &RouteResult,
    group: &Group,
    log: &mut ProxyLog,
    attempts: &mut Vec<ProxyAttempt>,
    outbound: OutboundRequest<'_>,
    lang: Lang,
    candidate_total: usize,
    is_last_candidate: bool,
    attempt_start: std::time::Instant,
    attempt_ts: i64,
    start: std::time::Instant,
    log_settings: &ProxyLogSettings,
    sched_settings: &super::models::SchedulingBreakerSettings,
    has_next_model: bool,
) -> Result<(reqwest::Response, super::scheduling::BreakerThresholds), StageExit> {
    let OutboundRequest {
        req_body_str,
        header_injects,
        client_type,
        eff_api_key,
        orig_headers,
        is_stream,
        target_protocol_enum,
        url,
        model,
    } = outbound;

    // ── 解析超时：模型 > 分组 > 系统 ──（system_timeout + proxy_client 一次缓存借齐）
    let (system_timeout, proxy_client) = {
        let c = state.settings_cache.read().await;
        (c.system_timeout.clone(), c.proxy_client.clone())
    };
    let (req_timeout, conn_timeout) = resolve_timeout(&route.mapping, group, &system_timeout);
    // 流式响应 body 读取不计入总超时：reqwest .timeout 覆盖「连接→响应头→body 全部读完」，
    // 会砍断长 thinking/tool_use 流（body 读取 > request_timeout_secs）致无 message_stop → 客户端
    // JSON Parse error / 内容残缺。流式禁总超时（传 0），connect_timeout 仍保护连接期，客户端自有超时兜底。
    let req_timeout = if is_stream { 0 } else { req_timeout };
    let client = super::http_client::build_http_client(
        &proxy_client,
        req_timeout,
        conn_timeout,
        super::http_client::platform_should_use_proxy(&proxy_client, &route.platform.extra),
    )
    .await;

    // ── 构建上游请求头 ──
    // convert 路径：先铺底透传入站头（anthropic-* / x-stainless-* / x-app / session-id 等，
    // 跨协议也带，上游忽略未知头不报错），再由 apply_client_headers 覆盖 UA + auth + CT。
    // passthrough_convert_headers 已剔 hop-by-hop + auth/UA/CT（由下方覆盖），无同名多值。
    let mut upstream_headers = build_upstream_headers_for_model(
        client_type,
        target_protocol_enum,
        &eff_api_key,
        model,
        orig_headers,
    );

    // ── 中间件 header 注入（inject / header_set，转换与透传两分支共用本 seam）──
    // 认证头 / 代理自有头拒绝覆盖、非法头名值跳过，均记 warn 不阻断（见 sanitize_header_injects）。
    // 写进透传底座（insert = 替换同名客户端头），底座里的名与 apply_client_headers 覆盖的
    // UA/auth/CT 不相交（后者全在拒绝名单里），故不会产生同名多值。
    let injected = sanitize_header_injects(&header_injects);
    let mut passthrough_base = passthrough_convert_headers(orig_headers);
    for (n, v) in &injected {
        passthrough_base.insert(n.clone(), v.clone());
        // 日志镜像实发：同名替换后追加（受 log_upstream_request 开关控制，值照常按敏感头脱敏）。
        upstream_headers.retain(|(k, _)| !k.eq_ignore_ascii_case(n.as_str()));
        let shown = if n.as_str() == "cookie" {
            "[REDACTED]".to_string()
        } else {
            v.to_str().unwrap_or("").to_string()
        };
        upstream_headers.push((n.to_string(), shown));
    }

    // ── anthropic-beta 自动降级的备用请求（与主请求同构，只少这一个头）──
    // 背景：`anthropic-beta` 现在一律 verbatim 转发（官方明令禁按值 allowlist），但第三方兼容
    // 端点不保证认识每个新 beta 值。实测 GLM / CometAPI 对四种值全 200，其余平台没 key 测不了，
    // 故留这条兜底：第三方回 400 就剔头原地重试一次，成功即继续，客户端无感。
    // 判据只用「400 + 本次带了这个头 + 上游非官方」，**不匹配报文措辞**——当初促成剔除规则的
    // GLM 400 code 1210「API 调用参数有误」根本没点名这个头，按措辞匹配会漏掉它这类不透明错误。
    // 代价是第三方的真·参数错也会多发一次请求（已经失败的路径上多一次，且每次 attempt 至多一次）。
    let beta_retry_builder = (passthrough_base.contains_key("anthropic-beta")
        && !is_official_anthropic_host(&url))
    .then(|| {
        let mut without_beta = passthrough_base.clone();
        without_beta.remove("anthropic-beta");
        apply_client_headers_for_model(
            client
                .post(&url)
                .header("Content-Type", "application/json")
                .headers(without_beta)
                .body(req_body_str.clone()),
            client_type,
            target_protocol_enum,
            &eff_api_key,
            model,
        )
    });

    let mut req_builder = client
        .post(&url)
        .header("Content-Type", "application/json")
        .headers(passthrough_base);

    // ── 覆盖 UA + auth（平台 api_key）──
    req_builder = apply_client_headers_for_model(
        req_builder,
        client_type,
        target_protocol_enum,
        &eff_api_key,
        model,
    );

    // ── 记录上游实际请求 ──
    log.upstream_request_headers =
        super::retry::upstream_header_pairs_json_for_log(upstream_headers, log_settings.log_upstream_request);
    // O2（perf-backend spec §2）：日志存原文（紧凑 JSON），不再写库前 pretty（省 CPU + 写盘）。
    // 展示侧（Logs 详情 safeParseJson + JSON.stringify(,2) / 复制路径 fj）格式化，对存量
    // 已 pretty 的旧行同样成立（parse→stringify 幂等）。
    log.upstream_request_body = if log_settings.log_upstream_request {
        req_body_str.clone()
    } else {
        String::new()
    };
    tracing::info!(method = "POST", url = %url, "upstream request");
    tracing::debug!(method = "POST", url = %url, body = %super::log_util::log_body_preview(&req_body_str), "upstream request body");
    // O3（perf-backend spec §2）：主请求最后 move 所有权（上面日志/备用 builder 已各取所需，
    // 省掉此前无条件的一次全量 body String clone）。
    let req_builder = req_builder.body(req_body_str);

    // ── 熔断指标：本次 forward 尝试前在途 +1；解析本平台有效阈值 ──
    let breaker_th = super::scheduling::BreakerThresholds::resolve(sched_settings, &route.platform, group.routing_mode);
    state.scheduler.inc_inflight(route.platform.id);

    // ── 发上游 + 同平台瞬时重试 ──
    // transport 错误（上游中途掐连接 / 连不上）先在同一平台原地重试 TRANSPORT_RETRY_MAX 次再
    // 换候选：单平台组没有 failover 候选，不原地重试则任何瞬断都直接 502 到客户端。
    // 重试期间不记账（in-flight 仍 +1，本次尝试尚未定终态）；最终失败仅 inflight-1（record_ignored，
    // 网络类失败不降权——见下方 Err 分支注释）。
    // try_clone 对本路径恒 Some（body 是 String，非 stream body）；None 时退化为不重试。
    let mut pending = Some(req_builder);
    let mut transport_retried = 0u32;
    let resp = loop {
        let builder = pending
            .take()
            .expect("pending builder always set at loop head");
        let next_builder = if transport_retried < TRANSPORT_RETRY_MAX {
            builder.try_clone()
        } else {
            None
        };
        // 本轮尝试自身的耗时（非累计）：慢失败不重试的判据，见 is_transport_retryable。
        let send_start = std::time::Instant::now();
        match builder.send().await {
            Ok(r) => break r,
            Err(e)
                if is_transport_retryable(&e, send_start.elapsed()) && next_builder.is_some() =>
            {
                let backoff = transport_retry_backoff(transport_retried);
                tracing::warn!(
                    url = %url, platform = %route.platform.name, error = %err_chain(&e),
                    retry = transport_retried + 1, backoff_ms = backoff.as_millis() as u64,
                    "upstream transport error, retrying same platform"
                );
                attempts.push(ProxyAttempt {
                    kind: Some("retry".to_string()),
                    chain_stop: None,
                    platform_id: route.platform.id,
                    platform_name: route.platform.name.clone(),
                    status_code: 0,
                    error: format!("upstream error (retrying): {}", err_chain(&e)),
                    duration_ms: attempt_start.elapsed().as_millis() as i64,
                    ts: attempt_ts,
                    model: Some(route.target_model.clone()),
                    ..Default::default()
                });
                transport_retried += 1;
                tokio::time::sleep(backoff).await;
                pending = next_builder;
                continue;
            }
            Err(e) => {
                // 同平台重试已用尽 / 错误不宜重试 → 换下个候选；候选耗尽则返回 502。
                // R1（2026-09-28，推翻 2026-09-15「网络类故障不降权」）：connect 失败计入熔断
                // （死站滞留候选的根治），带本地网络保护（60s 滑窗失败平台数达候选半数不计数）；
                // 其余 transport 错误（读超时/中途掐线）仍不降权仅 inflight-1。
                // 票 04：judge(Transport) + apply 记账收口到 CallScope::transport_failed。
                let scope = super::call_scope::CallScope {
                    state,
                    group,
                    route,
                    log_settings,
                    lang,
                    start,
                    attempt_start,
                    attempt_ts,
                    is_last_candidate,
                    candidate_total,
                    has_next_model,
                    breaker_th,
                    proxy_client: &proxy_client,
                    req_timeout,
                    conn_timeout,
                };
                match scope.transport_failed(log, attempts, &e, &url).await {
                    AttemptOutcome::Respond(r) => return Err(StageExit::Respond(r)),
                    AttemptOutcome::Next { connect_failed } => return Err(StageExit::Retry { connect_failed }),
                }
            }
        }
    };

    // ── anthropic-beta 自动降级：第三方回 400 → 剔头原地重试一次 ──
    // 重试发不出去就保留原始 400 走正常失败路径（降级是兜底，不该把失败换成另一种失败）。
    let resp = match (resp.status().as_u16(), beta_retry_builder) {
        (400, Some(retry)) => {
            tracing::warn!(
                url = %url, platform = %route.platform.name,
                "third-party upstream rejected request with 400 while anthropic-beta was set, retrying once without it"
            );
            attempts.push(ProxyAttempt {
                kind: Some("beta_downgrade".to_string()),
                chain_stop: None,
                platform_id: route.platform.id,
                platform_name: route.platform.name.clone(),
                status_code: 400,
                error: "400 with anthropic-beta (retrying without it)".to_string(),
                duration_ms: attempt_start.elapsed().as_millis() as i64,
                ts: attempt_ts,
                model: Some(route.target_model.clone()),
                ..Default::default()
            });
            retry.send().await.unwrap_or(resp)
        }
        _ => resp,
    };
    Ok((resp, breaker_th))
}


#[cfg(test)]
mod stage_contract_tests {
    use super::*;

    #[test]
    fn stage_exit_maps_only_at_forward_boundary() {
        let retry: AttemptOutcome = StageExit::Retry { connect_failed: true }.into();
        assert!(matches!(retry, AttemptOutcome::Next { connect_failed: true }));
        let response: AttemptOutcome = StageExit::Respond(
            (StatusCode::OK, "stage").into_response(),
        )
        .into();
        assert!(matches!(response, AttemptOutcome::Respond(_)));
    }

    #[test]
    fn endpoint_guard_accepts_wire_protocols_only() {
        assert!(is_valid_wire_protocol(&Protocol::Anthropic));
        assert!(is_valid_wire_protocol(&Protocol::OpenAIResponses));
        assert!(!is_valid_wire_protocol(&Protocol::Glm));
        assert!(!is_valid_wire_protocol(&Protocol::Unknown));
    }
}
