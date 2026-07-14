use super::*;

/// 单次候选 forward 尝试的控制结果：Respond=已确定响应直接返回客户端；Next=换下个候选重试。
pub(crate) enum AttemptOutcome {
    Respond(axum::response::Response),
    Next,
}

/// 单次候选 forward 尝试：构建上游请求、发送、按状态码分类处理。
/// 返回 AttemptOutcome::Respond 直接回客户端，Next 表示换下个候选重试。
#[allow(clippy::too_many_arguments)]
pub(crate) async fn forward_attempt(
    state: &Arc<ProxyState>,
    log: &mut ProxyLog,
    attempts: &mut Vec<ProxyAttempt>,
    route: RouteResult,
    is_last_candidate: bool,
    attempt_start: std::time::Instant,
    attempt_ts: i64,
    log_settings: &ProxyLogSettings,
    lang: Lang,
    group: &Group,
    chat_req: &mut ChatRequest,
    req_value: &Value,
    source_protocol: &str,
    requested_model: &str,
    is_stream: bool,
    orig_headers: &axum::http::HeaderMap,
    sched_settings: &super::models::SchedulingBreakerSettings,
    start: std::time::Instant,
) -> AttemptOutcome {
    let actual_model = route.target_model.clone();

    // OpenCode Zen：api_key 留空 → 注入匿名免费 key（$opencode）；用户填了用用户的。
    let eff_api_key = resolve_opencode_zen_key(&route.platform);

    // 尝试匹配端点：按 source_protocol 查找平台是否支持对应协议的端点。
    // 先精确匹配；openai_responses 源（Codex）若无 Responses 端点，回退到 openai 端点
    // （普通 chat/completions 平台），出站经 to_openai 转换。
    let ep_proto = |ep: &super::models::PlatformEndpoint| format!("{:?}", ep.protocol).to_lowercase();
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
        .unwrap_or((&route.platform.platform_type, route.platform.base_url.clone(), "default".to_string(), false));

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
    upsert_log(state, log, log_settings).await;

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
        let mw_settings = state.settings_cache.read().await.middleware_settings.clone();
        if let InboundOutcome::Blocked { blocked_by, blocked_reason } =
            state.middleware.apply_inbound_platform(&mw_settings, chat_req, route.platform.id as i64)
        {
            log.platform_id = route.platform.id;
            return AttemptOutcome::Respond(
                block_inbound(state, log.clone(), log_settings, lang, blocked_by, blocked_reason, start).await,
            );
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
        log.attempts = std::mem::take(attempts);
        upsert_log(state, log, log_settings).await;
        return AttemptOutcome::Respond({
            let mut r = (
                StatusCode::PAYMENT_REQUIRED,
                [(axum::http::header::CONTENT_TYPE, "application/json")],
                body,
            )
                .into_response();
            inject_trace_header(&mut r);
            r
        });
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
        adapter::convert_request(chat_req, target_protocol_enum, platform_protocol)
    };

    // Coding Plan 特殊处理：注入平台特有字段 + 覆盖 API 路径
    if coding_plan {
        inject_coding_plan_fields(&mut req_body, platform_protocol);
        override_coding_plan_path(&mut api_path, platform_protocol);
    }

    // 构建目标 URL
    let base_url = target_base_url.trim_end_matches('/');
    let url = format!("{}{}", base_url, api_path);
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
    if matches!(target_protocol_enum, Protocol::Anthropic) && !is_official_anthropic_host(&url) {
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

    let req_body_str = serde_json::to_string(&req_body).unwrap_or_default();

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
        &proxy_client, req_timeout, conn_timeout,
        Some(&route.platform.extra), None,
    ).await;

    // ── 构建上游请求头 ──
    // convert 路径：先铺底透传入站头（anthropic-* / x-stainless-* / x-app / session-id 等，
    // 跨协议也带，上游忽略未知头不报错），再由 apply_client_headers 覆盖 UA + auth + CT。
    // passthrough_convert_headers 已剔 hop-by-hop + auth/UA/CT（由下方覆盖），无同名多值。
    let upstream_headers = build_upstream_headers(&client_type, target_protocol_enum, &eff_api_key, orig_headers, &url);

    let mut req_builder = client
        .post(&url)
        .header("Content-Type", "application/json")
        .headers(passthrough_convert_headers(orig_headers, &url))
        .body(req_body_str.clone());

    // ── 覆盖 UA + auth（平台 api_key）──
    req_builder = apply_client_headers(req_builder, &client_type, target_protocol_enum, &eff_api_key);

    // ── 记录上游实际请求 ──
    log.upstream_request_headers = serde_json::Value::Object(
        upstream_headers.into_iter().map(|(k, v)| (k, Value::String(v))).collect()
    ).to_string();
    // ponytail: pretty 序列化仅当 log_upstream_request 开启时执行，关日志零开销
    log.upstream_request_body = if log_settings.log_upstream_request {
        format_pretty_json(&req_body_str)
    } else {
        String::new()
    };
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
            let _ = super::db::set_platform_last_error(
                &state.db, route.platform.id, Some(format!("upstream error: {e}")),
            ).await;
            if !is_last_candidate {
                return AttemptOutcome::Next;
            }
            log.platform_id = route.platform.id;
            log.response_body = format!("upstream error: {e}");
            log.status_code = 502;
            log.user_response_body = format!("{}: {e}", i18n::t(lang, ErrorKey::Upstream));
            log.user_response_headers = r#"{"content-type":"text/plain"}"#.to_string();
            log.duration_ms = start.elapsed().as_millis() as i32;
            log.retry_count = (attempts.len() as i32 - 1).max(0);
            log.attempts = std::mem::take(attempts);
            upsert_log(state, log, log_settings).await;
            return AttemptOutcome::Respond({
                let mut r = (StatusCode::BAD_GATEWAY, format!("{}: {e}", i18n::t(lang, ErrorKey::Upstream))).into_response();
                inject_trace_header(&mut r);
                r
            });
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
        return handle_non_success(
            resp, status, state, log, attempts, &route, group, &breaker_th, &url, start,
            attempt_start, attempt_ts, is_last_candidate, log_settings,
        )
        .await;
    }

    // ── 2xx：状态码成功，但「200 + 空/无效响应」按决策 B 仍当作失败重试。──
    // 成功记账（record_success / 恢复 auto_disabled / 清 strike / attempts.push 成功 / log.attempts）
    // 推迟到「确认非空有效响应」之后，由 commit_2xx_success! 宏统一执行（避免重复且保证仅真成功才记账）。
    let attempt_latency_ms = attempt_start.elapsed().as_millis() as i64;

    // 决策 B 失败（200 空响应）时记一次失败 attempt 并 failover；候选耗尽则返回 502。
    // 与连接错误/超时同语义：熔断计一次失败（record_failure），但不 auto_disable（非鉴权/死端点信号）。
    macro_rules! retry_on_empty_2xx {
        ($reason:expr, $upstream_text:expr) => {{
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
            let _ = super::db::set_platform_last_error(
                &state.db, route.platform.id, Some(format!("HTTP 200: {}", $reason)),
            ).await;
            if !is_last_candidate {
                return AttemptOutcome::Next;
            }
            // 候选耗尽：返回 502 + 已记录的 attempts（此时尚未向客户端发任何字节，安全）。
            log.platform_id = route.platform.id;
            log.status_code = 502;
            log.upstream_status_code = status.as_u16() as i32;
            let err_body = format!("{}: 200 but empty/invalid response", i18n::t(lang, ErrorKey::Upstream));
            // 取证：把上游真实首块原文截断（≤4KB + truncated 标记）落 response_body，替代占位文案；
            // upstream_text 为空时回退占位兜底。下次 GLM 间歇空流复现自动留 DB 证据。
            let captured = truncate_peek_text($upstream_text);
            log.response_body = if captured.is_empty() { $reason.to_string() } else { captured };
            log.user_response_body = err_body.clone();
            log.user_response_headers = r#"{"content-type":"text/plain"}"#.to_string();
            log.duration_ms = start.elapsed().as_millis() as i32;
            log.retry_count = (attempts.len() as i32 - 1).max(0);
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
            // 注意流式此处为「首个有效内容」延迟（peek 已收到内容）；作为延迟近似用于 LeastLatency。
            state.scheduler.record_success(route.platform.id, attempt_latency_ms);
            // 最近一次成功 → 清本平台 last_error。仅在原有 last_error 非空时写，避免成功热路径空写。
            if !route.platform.last_error.is_empty() {
                let _ = super::db::set_platform_last_error(&state.db, route.platform.id, None).await;
            }
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
            }
            log.platform_id = route.platform.id;
            log.retry_count = (attempts.len() as i32 - 1).max(0);
            log.attempts = std::mem::take(attempts);
        }};
    }

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
                state, log, log_settings, group, &route, source_protocol, requested_model,
                &actual_model, &eff_api_key, target_protocol_enum, same_protocol_passthrough,
                needs_model_remap, coding_plan, &upstream_resp_headers, start, body,
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

    AttemptOutcome::Respond(
        finish_stream(
            upstream_stream, peek_buf, state, log, log_settings, group, &route, source_protocol,
            requested_model, &actual_model, &eff_api_key, target_protocol_enum,
            same_protocol_passthrough, needs_model_remap, coding_plan, &upstream_resp_headers, start,
        )
        .await,
    )
}

/// 第三方 anthropic 端点不支持字段剔除（仅在已判定为非官方 anthropic 端点时调用）。
///
/// **`context_management`（无条件剔）**：thinking 开启（`thinking.type != "disabled"`）即剔，
/// 独立于 assistant 历史是否齐全。`context_management` 是官方 Anthropic 服务端协商特性
/// （Claude Code adaptive/summarized 模式 `clear_thinking_20251015`，让官方服务端自动清历史 thinking），
/// 第三方 anthropic-compat 端点普遍不实现该协商，保留该字段对第三方无益仅风险。两类复现：
/// 首轮请求（messages 仅 user，无 assistant 历史）GLM 直拒字段 → 400 code 1210 "API 调用参数有误"
/// （旧逻辑 `has_unmatched_assistant`=false 漏剔 → 本次修复根因）；有 assistant 历史时 DeepSeek 认字段
/// 判 thinking mode → 400 "thinking must be passed back"。函数名沿用 `strip_thinking_if_unmatched`
/// （单调用点 forward.rs，最小 diff；context_management 已超越 thinking unmatched 语义，注释说明）。
///
/// **`thinking`（仅 unmatched 时剔）**：thinking 开启且历史任一 assistant 轮缺 thinking block 时剔。
/// 第三方端点严格要求 thinking 模式下每 assistant 轮回传 thinking block，缺失即 400
/// `content[].thinking must be passed back`；官方 Anthropic 的 summarized/adaptive 模式无此约束，
/// Claude Code 故不回传，跨路由到第三方即触发该 400。thinking block 齐全（正常情况）保留直传，
/// 第三方能正常处理。
fn strip_thinking_if_unmatched(body: &mut Value) {
    let Some(obj) = body.as_object_mut() else { return };
    let thinking_on = obj
        .get("thinking")
        .map(|t| t.get("type").and_then(|v| v.as_str()) != Some("disabled"))
        .unwrap_or(false);
    if !thinking_on {
        return;
    }
    // context_management 无条件剔：第三方端点不认该协商字段（首轮 GLM 1210 + 有历史 DeepSeek 400）
    obj.remove("context_management");
    let has_unmatched_assistant = obj
        .get("messages")
        .and_then(|m| m.as_array())
        .map(|msgs| {
            msgs.iter().any(|m| {
                if m.get("role").and_then(|r| r.as_str()) != Some("assistant") {
                    return false;
                }
                match m.get("content") {
                    // 块数组：非空且无 thinking block → 不匹配
                    Some(Value::Array(blocks)) => {
                        !blocks.is_empty()
                            && !blocks
                                .iter()
                                .any(|b| b.get("type").and_then(|t| t.as_str()) == Some("thinking"))
                    }
                    // 纯文本 assistant 轮：thinking 模式下也应携带 thinking，缺失即不匹配
                    Some(Value::String(s)) => !s.is_empty(),
                    _ => false,
                }
            })
        })
        .unwrap_or(false);
    if has_unmatched_assistant {
        obj.remove("thinking");
    }
}

/// 第三方 anthropic 端点：无条件剥离 messages[].content 内 `redacted_thinking` block。
///
/// **根因（DB 响应体实证）**：Claude 4.x extended thinking 多轮请求含 `redacted_thinking`
/// content block（Claude Code 回传上轮 protected thinking 的加密关联块）。同协议 passthrough
/// （anthropic→anthropic, remap=true）不经 `to_anthropic` 转换（adapter/anthropic.rs 已 filter
/// Unknown 含 redacted_thinking），content 原样透传 → 第三方端点不认该 type → 400 InvalidParameter
/// `"invalid value: 'redacted_thinking', supported values: 'text','thinking','image','tool_use','tool_result'"`。
///
/// **trace 实证**：81dc4466（火山 doubao coding endpoint）+ 87e3c500（deepseek-v4-pro-260425）
/// 同根因 400。所有第三方 anthropic-compat 端点共性。
///
/// **剥离语义**：redacted_thinking 内容为客户端不可解读的加密 opaque blob（仅官方 Anthropic
/// 能关联上轮 protected thinking），第三方必无法处理，无条件剥离安全。仅遍历数组形态 content
/// （字符串形态无 block 可剔）。content 变空数组时保留 message 结构（剥离顺序敏感，下游规整
/// 依赖 message 序列完整）。
fn strip_redacted_thinking_blocks(body: &mut Value) {
    let Some(msgs) = body.get_mut("messages").and_then(|m| m.as_array_mut()) else {
        return;
    };
    for m in msgs.iter_mut() {
        let Some(blocks) = m.get_mut("content").and_then(|c| c.as_array_mut()) else {
            continue;
        };
        blocks.retain(|b| b.get("type").and_then(|t| t.as_str()) != Some("redacted_thinking"));
    }
}

/// 第三方 anthropic 端点：messages 内非首位的 role=system 规整到顶层 system 数组。
///
/// **根因（DB 全样本取证）**：Claude Code 把 SessionStart/UserPromptSubmit hook 注入的上下文
/// 以 `role=system` 消息插入 messages 数组中段/末尾（官方 Anthropic 接受该非标位置作为客户端
/// 约定）。GLM / DeepSeek 等第三方 anthropic-compat 端点严格执行规范（role=system 仅顶层 system
/// 字段或 messages[0]），多轮 + 非流式场景下拒绝 → 400 code 1210 "API 调用参数有误"。
///
/// **DB 交叉验证**（GLM `open.bigmodel.cn/api/anthropic`，10552 条 200 + 9 条 400 全样本）：
/// 失败全集 = `{no_stream, has_assistant, messages 含 role=system（含中段+末段）}` —— 9/9 命中；
/// 同结构流式 PASS=1166，非流式 PASS=3（GLM 间歇性接受，3 异常样本均为 14-112 msgs 长上下文）。
/// 故仅非流式触发规整：流式同结构当前工作正常（host-gated 但 is_stream=true 不动），避免回归。
///
/// **规整方式**：messages 内 role=system 消息按出现顺序，content 合并到顶层 `system` 数组
/// （顶层 system 已是数组则追加 text block；字符串则升级为数组；缺失则新建）。
/// messages 数组移除该消息，剩余 user/assistant 交替保持原序。仅多轮（含 assistant）才触发：
/// 首轮无 assistant 时 messages 内 role=system 多为客户端约首约定（DeepSeek/GLM 首轮接受），不动。
fn hoist_mid_messages_system(body: &mut Value) {
    let Some(obj) = body.as_object_mut() else { return };
    let Some(msgs) = obj.get_mut("messages").and_then(|m| m.as_array_mut()) else { return };
    // 仅多轮（有 assistant 历史）触发：首轮无 assistant 不动（首轮 role=system 第三方接受）。
    let has_assistant = msgs.iter().any(|m| m.get("role").and_then(|r| r.as_str()) == Some("assistant"));
    if !has_assistant {
        return;
    }
    // 收集 messages 内 role=system 的 content（保持出现顺序），同时保留非 system 消息原序。
    let mut hoisted_blocks: Vec<Value> = Vec::new();
    let mut kept: Vec<Value> = Vec::with_capacity(msgs.len());
    for m in msgs.drain(..) {
        if m.get("role").and_then(|r| r.as_str()) == Some("system") {
            // system message content：字符串 → text block；数组（blocks） → 逐项取
            match m.get("content") {
                Some(Value::String(s)) => {
                    hoisted_blocks.push(serde_json::json!({"type": "text", "text": s}));
                }
                Some(Value::Array(blocks)) => {
                    for b in blocks {
                        if b.is_object() {
                            hoisted_blocks.push(b.clone());
                        }
                    }
                }
                _ => {}
            }
        } else {
            kept.push(m);
        }
    }
    if hoisted_blocks.is_empty() {
        // 无 system 可规整：还原原 msgs（drain 清空了）
        *msgs = kept;
        return;
    }
    *msgs = kept;
    // 合并到顶层 system 数组：现有数组追加；字符串升级；缺失新建。
    match obj.get_mut("system") {
        Some(Value::Array(arr)) => arr.extend(hoisted_blocks),
        Some(Value::String(s)) => {
            let mut arr = vec![serde_json::json!({"type": "text", "text": s})];
            arr.extend(hoisted_blocks);
            obj.insert("system".to_string(), Value::Array(arr));
        }
        _ => {
            obj.insert("system".to_string(), Value::Array(hoisted_blocks));
        }
    }
}

#[cfg(test)]
mod test_strip_thinking {
    use super::{strip_redacted_thinking_blocks, strip_thinking_if_unmatched};
    use serde_json::json;

    #[test]
    fn strips_when_assistant_turn_lacks_thinking_block() {
        // 复现 0cea9d32 真因：thinking 开启 + assistant 轮仅 tool_use 无 thinking → 第三方 400
        let mut body = json!({
            "thinking": {"type": "adaptive", "display": "summarized"},
            "context_management": {"edits": [{"type": "clear_thinking_20251015", "keep": "all"}]},
            "messages": [
                {"role": "user", "content": [{"type": "text", "text": "hi"}]},
                {"role": "assistant", "content": [{"type": "tool_use", "id": "t1", "name": "x", "input": {}}]},
            ],
        });
        strip_thinking_if_unmatched(&mut body);
        assert!(body.get("thinking").is_none(), "应剔除 thinking");
        assert!(body.get("context_management").is_none(), "应剔除 context_management");
    }

    #[test]
    fn strips_context_management_with_adaptive_thinking_no_assistant_block() {
        // 复现 request_id=1658bb4b 真因：Claude Code adaptive/summarized 模式
        // (thinking adaptive + context_management clear_thinking_20251015) → assistant 轮不回传 thinking block
        // → 跨路由到第三方 anthropic 端点(DeepSeek)。单剔 thinking 不够，context_management 保留仍判 thinking mode → 400。
        // 修复：两字段皆剔。
        let mut body = json!({
            "thinking": {"type": "adaptive", "display": "summarized"},
            "context_management": {"edits": [{"type": "clear_thinking_20251015", "keep": "all"}]},
            "messages": [
                {"role": "user", "content": [{"type": "text", "text": "q1"}]},
                {"role": "assistant", "content": [{"type": "text", "text": "a1"}]},
                {"role": "user", "content": [{"type": "text", "text": "q2"}]},
                {"role": "assistant", "content": [{"type": "text", "text": "a2"}]},
            ],
        });
        strip_thinking_if_unmatched(&mut body);
        assert!(body.get("thinking").is_none(), "应剔除 thinking");
        assert!(body.get("context_management").is_none(), "应剔除 context_management");
    }

    #[test]
    fn keeps_thinking_when_blocks_present() {
        let mut body = json!({
            "thinking": {"type": "adaptive"},
            "context_management": {"edits": [{"type": "clear_thinking_20251015", "keep": "all"}]},
            "messages": [
                {"role": "assistant", "content": [
                    {"type": "thinking", "thinking": "...", "signature": "s"},
                    {"type": "tool_use", "id": "t1", "name": "x", "input": {}},
                ]},
            ],
        });
        strip_thinking_if_unmatched(&mut body);
        assert!(body.get("thinking").is_some(), "thinking 齐全应保留");
        // context_management 无条件剔（第三方不认该协商字段）
        assert!(body.get("context_management").is_none(), "thinking 开启即无条件剔 context_management（即使 thinking 齐全）");
    }

    #[test]
    fn noop_when_thinking_off() {
        let mut body = json!({
            "context_management": {"edits": [{"type": "clear_thinking_20251015", "keep": "all"}]},
            "messages": [{"role": "assistant", "content": [{"type": "tool_use", "id": "t1", "name": "x", "input": {}}]}],
        });
        strip_thinking_if_unmatched(&mut body);
        assert!(body.get("messages").is_some());
        assert!(body.get("context_management").is_some(), "thinking off 不命中 unmatched，context_management 保留");
    }

    #[test]
    fn keeps_thinking_first_turn_no_assistant_but_strips_context_management() {
        // 复现 request_id=3a76c297 真因（GLM 1210）：首轮请求 messages 仅 user，无 assistant 历史，
        // thinking adaptive + context_management clear_thinking_20251015。
        // 旧逻辑 has_unmatched_assistant=false → 两字段皆保留 → GLM 不认 context_management 报 1210。
        // 修复：context_management 无条件剔（thinking_on 即剔，独立于 has_unmatched）；thinking 无 unmatched 故保留。
        let mut body = json!({
            "thinking": {"type": "adaptive", "display": "summarized"},
            "context_management": {"edits": [{"type": "clear_thinking_20251015", "keep": "all"}]},
            "messages": [{"role": "user", "content": [{"type": "text", "text": "hi"}]}],
        });
        strip_thinking_if_unmatched(&mut body);
        assert!(body.get("thinking").is_some(), "首轮无 assistant → has_unmatched=false，thinking 保留");
        assert!(body.get("context_management").is_none(), "thinking 开启即无条件剔 context_management（首轮 GLM 1210 根因）");
    }

    #[test]
    fn test_strip_redacted_thinking_blocks_filters_only_redacted() {
        // 复现 trace 81dc4466（火山 doubao coding endpoint 400 InvalidParameter）
        // + 87e3c500（deepseek-v4-pro-260425 同根因）：redacted_thinking block 透传致第三方端点 400。
        let mut body = json!({
            "messages": [
                {"role": "user", "content": [{"type": "text", "text": "q"}]},
                {"role": "assistant", "content": [
                    {"type": "thinking", "thinking": "sig-data", "signature": "s"},
                    {"type": "redacted_thinking", "data": "encrypted-opaque-blob"},
                    {"type": "text", "text": "a"},
                ]},
                {"role": "user", "content": [
                    {"type": "tool_result", "tool_use_id": "t1", "content": "r"},
                    {"type": "redacted_thinking", "data": "encrypted-opaque-blob-2"},
                ]},
            ],
        });
        strip_redacted_thinking_blocks(&mut body);
        let msgs = body.get("messages").and_then(|m| m.as_array()).unwrap();
        // assistant 轮：仅剩 thinking + text
        let asst = msgs[1].get("content").and_then(|c| c.as_array()).unwrap();
        assert_eq!(asst.len(), 2, "应仅剥离 redacted_thinking，保留 thinking + text");
        assert!(
            asst.iter().all(|b| b.get("type").and_then(|t| t.as_str()) != Some("redacted_thinking")),
            "无残留 redacted_thinking"
        );
        // user 轮：仅剩 tool_result
        let u2 = msgs[2].get("content").and_then(|c| c.as_array()).unwrap();
        assert_eq!(u2.len(), 1, "tool_result 保留，仅剔 redacted_thinking");
        assert_eq!(u2[0].get("type").and_then(|t| t.as_str()), Some("tool_result"));
    }

    #[test]
    fn test_strip_redacted_thinking_blocks_all_redacted_keeps_empty_message() {
        // 全 redacted_thinking 的 message → 剥离后 content 为空数组，但 message 结构保留
        // （剥离顺序敏感，禁删整条 message）。
        let mut body = json!({
            "messages": [
                {"role": "user", "content": [{"type": "text", "text": "q"}]},
                {"role": "assistant", "content": [
                    {"type": "redacted_thinking", "data": "blob-1"},
                    {"type": "redacted_thinking", "data": "blob-2"},
                ]},
            ],
        });
        strip_redacted_thinking_blocks(&mut body);
        let msgs = body.get("messages").and_then(|m| m.as_array()).unwrap();
        assert_eq!(msgs.len(), 2, "message 数量不变（结构保留）");
        let asst = msgs[1].get("content").and_then(|c| c.as_array()).unwrap();
        assert!(asst.is_empty(), "全 redacted_thinking 剥离后 content 为空数组");
        // user 轮 text block 保留
        let u = msgs[0].get("content").and_then(|c| c.as_array()).unwrap();
        assert_eq!(u.len(), 1);
    }

    #[test]
    fn test_strip_redacted_thinking_blocks_noop_on_string_content() {
        // 字符串形态 content 无 block 可剔，不动。
        let mut body = json!({
            "messages": [
                {"role": "user", "content": "plain text"},
                {"role": "assistant", "content": [{"type": "redacted_thinking", "data": "x"}]},
            ],
        });
        strip_redacted_thinking_blocks(&mut body);
        let msgs = body.get("messages").and_then(|m| m.as_array()).unwrap();
        // 字符串 content 原样
        assert_eq!(msgs[0].get("content").and_then(|c| c.as_str()), Some("plain text"));
        // assistant 数组内 redacted_thinking 已剔
        let asst = msgs[1].get("content").and_then(|c| c.as_array()).unwrap();
        assert!(asst.is_empty());
    }
}

#[cfg(test)]
mod test_hoist_mid_messages_system {
    use super::hoist_mid_messages_system;
    use serde_json::json;

    #[test]
    fn hoists_mid_system_to_top_level_when_multiturn() {
        // 复现 GLM 1210 真因（request_id=7c8629eadb074648a71858ae388ea550 等 9 例）：
        // CC 注入 role=system 进 messages 中段+末段，多轮 + 非流式下 GLM 拒绝 → 400 code 1210。
        // 规整：messages 内 role=system 合并到顶层 system 数组，messages 仅留 user/assistant。
        let mut body = json!({
            "system": [{"type": "text", "text": "base system"}],
            "messages": [
                {"role": "user", "content": [{"type": "text", "text": "q1"}]},
                {"role": "system", "content": "mid reminder 1"},
                {"role": "assistant", "content": [{"type": "text", "text": "a1"}]},
                {"role": "user", "content": "q2"},
                {"role": "system", "content": "mid reminder 2"},
                {"role": "assistant", "content": [{"type": "text", "text": "a2"}]},
                {"role": "user", "content": "q3"},
                {"role": "system", "content": "trailing reminder"},
            ],
        });
        hoist_mid_messages_system(&mut body);
        let msgs = body["messages"].as_array().unwrap();
        // messages 内不再有 role=system
        assert!(!msgs.iter().any(|m| m["role"] == "system"), "messages 内不应再有 role=system");
        // user/assistant 交替保留
        let roles: Vec<&str> = msgs.iter().map(|m| m["role"].as_str().unwrap()).collect();
        assert_eq!(roles, vec!["user", "assistant", "user", "assistant", "user"]);
        // 顶层 system 数组追加 3 个 text block（原 1 + 合并 3 = 4）
        let sys = body["system"].as_array().unwrap();
        assert_eq!(sys.len(), 4, "顶层 system 数组应含原 1 + 合并 3 = 4 块");
        assert_eq!(sys[0]["text"], "base system");
        assert_eq!(sys[1]["text"], "mid reminder 1");
        assert_eq!(sys[2]["text"], "mid reminder 2");
        assert_eq!(sys[3]["text"], "trailing reminder");
    }

    #[test]
    fn noop_when_no_assistant_first_turn() {
        // 首轮无 assistant 历史：messages 内 role=system 多为客户端首约定，第三方接受，不动。
        let mut body = json!({
            "system": [{"type": "text", "text": "base"}],
            "messages": [
                {"role": "user", "content": "hi"},
                {"role": "system", "content": "ctx"},
            ],
        });
        hoist_mid_messages_system(&mut body);
        // messages 保持原样（含 role=system）
        let msgs = body["messages"].as_array().unwrap();
        assert_eq!(msgs.len(), 2, "首轮无 assistant 不应规整");
        assert_eq!(msgs[1]["role"], "system");
        assert_eq!(body["system"].as_array().unwrap().len(), 1, "顶层 system 不变");
    }

    #[test]
    fn noop_when_no_mid_system() {
        // 多轮但 messages 内无 role=system：无需规整
        let mut body = json!({
            "system": [{"type": "text", "text": "base"}],
            "messages": [
                {"role": "user", "content": "q1"},
                {"role": "assistant", "content": "a1"},
                {"role": "user", "content": "q2"},
            ],
        });
        hoist_mid_messages_system(&mut body);
        assert_eq!(body["messages"].as_array().unwrap().len(), 3, "无 mid-system 不动");
        assert_eq!(body["system"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn upgrades_top_system_str_to_array() {
        // 顶层 system 是字符串时：升级为数组并追加 mid-system
        let mut body = json!({
            "system": "base string",
            "messages": [
                {"role": "user", "content": "q1"},
                {"role": "assistant", "content": "a1"},
                {"role": "user", "content": "q2"},
                {"role": "system", "content": "injected"},
            ],
        });
        hoist_mid_messages_system(&mut body);
        let sys = body["system"].as_array().expect("顶层 system 应升级为数组");
        assert_eq!(sys.len(), 2);
        assert_eq!(sys[0]["text"], "base string");
        assert_eq!(sys[1]["text"], "injected");
    }

    #[test]
    fn creates_top_system_when_absent() {
        // 顶层无 system 字段：mid-system 合并新建
        let mut body = json!({
            "messages": [
                {"role": "user", "content": "q1"},
                {"role": "assistant", "content": "a1"},
                {"role": "user", "content": "q2"},
                {"role": "system", "content": [{"type": "text", "text": "block1"}]},
            ],
        });
        hoist_mid_messages_system(&mut body);
        let sys = body["system"].as_array().expect("应新建顶层 system 数组");
        assert_eq!(sys.len(), 1);
        assert_eq!(sys[0]["text"], "block1");
    }

    #[test]
    fn preserves_array_block_content_from_mid_system() {
        // mid-system content 是数组（blocks）时：逐项合并到顶层 system 数组
        let mut body = json!({
            "system": [{"type": "text", "text": "base"}],
            "messages": [
                {"role": "user", "content": "q1"},
                {"role": "assistant", "content": "a1"},
                {"role": "user", "content": "q2"},
                {"role": "system", "content": [
                    {"type": "text", "text": "block a"},
                    {"type": "text", "text": "block b"},
                ]},
            ],
        });
        hoist_mid_messages_system(&mut body);
        let sys = body["system"].as_array().unwrap();
        assert_eq!(sys.len(), 3, "原 1 + mid-system 2 block = 3");
        assert_eq!(sys[1]["text"], "block a");
        assert_eq!(sys[2]["text"], "block b");
    }
}
