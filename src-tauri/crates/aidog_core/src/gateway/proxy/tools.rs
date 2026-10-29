//! 独立搜索/tools 请求转发（spec-独立搜索转发，票 19）：
//! 入站 path 命中平台 `tools_endpoints` 声明（handler 分流，语义 2）→ 复用 router 过滤 /
//! 冷却 / 熔断（`RequestKind::Tools`，语义 3/9，共享聊天同一套）→ body 原样透传到声明的
//! 绝对 URL（不 join base_url）→ strict 成功判定（语义 7 + 逐平台表）→ 按次费
//! `tools_cost`（语义 8，无 tools_pricing 声明记 0 + warn，转发照常可用）→
//! proxy_log 归类落库（`model = tools:<键>`，语义 10，不新增列）。
//! 出站鉴权：通用 `Authorization: Bearer <platform.api_key>`；qianfan 例外
//! `X-Appbuilder-Authorization: Bearer <platform.extra.appbuilder_key>`（语义 6，
//! AppBuilder key 是独立凭据体系；未配置不预过滤，401 走既有 auth 冷却链）。
//!
//! 结构全文仿 decision.rs：route_fail 复用其 `route_fail_response`（`no_tools_platform`
//! 400 分支在 `kind_route_error_message`）→ 候选选取 → 重试循环 → 单尝试 `tools_attempt`。

use super::*;

/// tools 请求主入口（handler.rs 在 decision 分流后调用，必须在 parse_incoming_request 之前）。
#[allow(clippy::too_many_arguments)]
pub(crate) async fn handle_tools(
    state: &Arc<ProxyState>,
    log: &mut ProxyLog,
    log_settings: &ProxyLogSettings,
    group: &Group,
    bytes: &[u8],
    path: &str,
    start: std::time::Instant,
    lang: Lang,
) -> Response {
    log.source_protocol = "tools".to_string();
    log.target_protocol = "tools".to_string();
    log.is_stream = false;
    // 语义 3：tools 请求体无路由用 model 字段，source_model 传空串（映射链与槽位匹配天然不命中）。

    // ── 路由选候选（复用过滤 / 冷却 / 熔断；Tools 类型过滤见 candidates.rs）──
    let sched_settings = aidog_db::get_scheduling_settings(&state.db).await;
    let sched_ctx = ScheduleCtx {
        scheduler: &state.scheduler,
        sticky: &state.sticky,
        settings: &sched_settings,
        sticky_key: Some(format!("{}|tools", group.group_key)),
    };
    let candidate_set = match select_candidates_ctx(
        &state.db,
        group,
        "",
        Some(&sched_ctx),
        RequestKind::Tools(path),
    )
    .await
    {
        Ok(c) => {
            log.routing = Some(c.routing.clone());
            c
        }
        Err(e) => {
            log.routing = Some(e.routing.clone());
            tracing::warn!(group = %group.name, error = %e, "tools route failed");
            return route_fail_response(state, log, log_settings, &e, start, lang).await;
        }
    };
    let candidates = candidate_set.candidates;

    // ── 重试编排：同 handler.rs 聊天 / decision.rs 循环 ──
    let max_retries = group.max_retries as usize;
    let mut attempts: Vec<ProxyAttempt> = Vec::new();
    let candidate_total = candidates.len();

    let (system_timeout, proxy_client) = {
        let c = state.settings_cache.read().await;
        (c.system_timeout.clone(), c.proxy_client.clone())
    };
    let req_timeout = if system_timeout.request_timeout_secs > 0 {
        system_timeout.request_timeout_secs
    } else {
        60
    };
    let conn_timeout = if system_timeout.connect_timeout_secs > 0 {
        system_timeout.connect_timeout_secs
    } else {
        10
    };

    for (attempt_idx, route) in candidates.into_iter().enumerate() {
        if attempt_idx > max_retries {
            break;
        }
        let attempt_start = std::time::Instant::now();
        let attempt_ts = aidog_db::now();
        let is_last_candidate = attempt_idx + 1 >= candidate_total || attempt_idx >= max_retries;

        match tools_attempt(
            state, log, &mut attempts, log_settings, group, &route, bytes, path,
            &proxy_client, req_timeout, conn_timeout, &sched_settings, start, lang,
            attempt_start, attempt_ts, is_last_candidate, candidate_total,
        )
        .await
        {
            AttemptOutcome::Respond(r) => return r,
            AttemptOutcome::Next { .. } => continue,
        }
    }

    // 候选耗尽且循环内未 return（同 decision.rs 兜底，理论不可达）。
    log.status_code = 503;
    log.done = true;
    let err_body = format!(
        "{}: all candidates exhausted",
        i18n::t(lang, ErrorKey::Upstream)
    );
    log.response_body = "all candidates exhausted".to_string();
    log.user_response_body = err_body.clone();
    log.user_response_headers = r#"{"content-type":"text/plain"}"#.to_string();
    log.duration_ms = start.elapsed().as_millis() as i32;
    log.retry_count = (attempts.len() as i32 - 1).max(0);
    log.attempts = std::mem::take(&mut attempts);
    upsert_log(state, log, log_settings).await;
    let mut r = (StatusCode::SERVICE_UNAVAILABLE, err_body).into_response();
    inject_trace_header(&mut r);
    r
}

/// strict 成功判定（语义 7 + 逐平台表）：HTTP 200 **且**响应体含结果字段才算成功、计 1 次；
/// 非 200 / 断连 / 200 无结果不计费。kimi/glm/stepfun/qianfan 的判定字段为 spec 推断口径
/// （同「200 且结果数组非空」同构；spec 开放问题 3/4，取到官方明文后改判据）。
fn tools_success_ok(platform_type: &Protocol, tool_key: &str, body: &[u8]) -> bool {
    let Ok(v) = serde_json::from_slice::<Value>(body) else {
        return false;
    };
    let nonempty_arr =
        |k: &str| v.get(k).and_then(|x| x.as_array()).is_some_and(|a| !a.is_empty());
    let nonblank_str = |k: &str| {
        v.get(k)
            .and_then(|x| x.as_str())
            .is_some_and(|s| !s.trim().is_empty())
    };
    match platform_type {
        // deepseek（官方明文）：search 空 results 不计费；fetch 必须返回网页正文。
        Protocol::DeepSeek => {
            if tool_key == "fetch" {
                nonblank_str("content")
            } else {
                nonempty_arr("results")
            }
        }
        // kimi / kimi_en（官方定价页明文）：search/search_pro 看 search_results；
        // fetch 看 markdown 非空白（404 markdown_not_found 不计）。
        Protocol::Kimi | Protocol::KimiEn => {
            if tool_key == "fetch" {
                nonblank_str("markdown")
            } else {
                nonempty_arr("search_results")
            }
        }
        // glm / glm_en：search_result 非空（推断，开放问题 4）。
        Protocol::Glm | Protocol::GlmEn => nonempty_arr("search_result"),
        // stepfun：results 非空（推断，开放问题 4）。
        Protocol::StepFun => nonempty_arr("results"),
        // qianfan：choices 非空（推断，开放问题 3：计费形态存疑，首版按次记 0 + warn 兜底）。
        Protocol::QianFan => nonempty_arr("choices"),
        _ => false,
    }
}

/// 阶段 fn：单候选 tools 尝试（URL/键解析 → 建请求 → 发送 → 非 2xx 沿用 handle_non_success →
/// 2xx strict 判定 + 按次计费 + 成功记账终态）。Next=换候选 continue；Respond=结束循环返回客户端。
#[allow(clippy::too_many_arguments)]
async fn tools_attempt(
    state: &Arc<ProxyState>,
    log: &mut ProxyLog,
    attempts: &mut Vec<ProxyAttempt>,
    log_settings: &ProxyLogSettings,
    group: &Group,
    route: &RouteResult,
    bytes: &[u8],
    path: &str,
    proxy_client: &super::models::ProxyClientSettings,
    req_timeout: u64,
    conn_timeout: u64,
    sched_settings: &aidog_db::models::SchedulingBreakerSettings,
    start: std::time::Instant,
    lang: Lang,
    attempt_start: std::time::Instant,
    attempt_ts: i64,
    is_last_candidate: bool,
    candidate_total: usize,
) -> AttemptOutcome {
    // 语义 6：URL = tools_endpoints 声明的绝对 URL（不再 join base_url）；键按该平台数据。
    let endpoints =
        crate::gateway::billing::default_tools_endpoints(&route.platform.platform_type.wire_str());
    let Some((tool_key, url)) = endpoints.and_then(|m| {
        m.into_iter()
            .find(|(_, u)| crate::gateway::billing::tools_url_matches(u, path))
    }) else {
        // platform_supports_kind(Tools) 已保证平台声明该 path；声明数据竞变（DB 同步覆盖）
        // 防御：跳下一候选，不 panic。
        tracing::warn!(platform = %route.platform.name, path, "tools endpoint declaration vanished mid-request");
        return AttemptOutcome::Next { connect_failed: false };
    };
    // 语义 10：日志 model 列 = tools:<tools_endpoints 键名>（键名按平台数据，不跨平台归一）。
    log.model = format!("tools:{tool_key}");

    // 语义 6：出站鉴权按平台选 header。qianfan 走 AppBuilder key（extra.appbuilder_key，
    // 未配置不预过滤——照常发送，401 走既有 auth 冷却链）。
    let appbuilder_key = if route.platform.platform_type == Protocol::QianFan {
        crate::gateway::models::PlatformExtra::parse(&route.platform.extra)
            .rest
            .get("appbuilder_key")
            .and_then(|v| v.as_str())
            .map(String::from)
    } else {
        None
    };

    log.platform_id = route.platform.id;
    log.actual_model = String::new();
    log.upstream_request_url = url.clone();
    log.upstream_request_headers = if appbuilder_key.is_some() {
        r#"{"x-appbuilder-authorization":"[REDACTED]","content-type":"application/json"}"#.to_string()
    } else {
        r#"{"authorization":"[REDACTED]","content-type":"application/json"}"#.to_string()
    };
    log.upstream_request_body = if log_settings.log_upstream_request {
        String::from_utf8_lossy(bytes).to_string()
    } else {
        String::new()
    };

    let breaker_th = {
        let (ft, os, hom) = sched_settings.effective_thresholds(&route.platform);
        super::scheduling::BreakerThresholds {
            failure_threshold: ft,
            open_secs: os,
            half_open_max: hom,
        }
    };

    let client = super::http_client::build_http_client(
        proxy_client,
        req_timeout,
        conn_timeout,
        Some(&route.platform.extra),
        None,
    )
    .await;
    let rb = client.post(&url).header("Content-Type", "application/json");
    let rb = if let Some(k) = &appbuilder_key {
        rb.header("X-Appbuilder-Authorization", format!("Bearer {k}"))
    } else {
        rb.header("Authorization", format!("Bearer {}", route.platform.api_key))
    };
    // 语义 6：body 原样透传不改写（含 qianfan body 自带 model 字段）。
    let rb = rb.body(bytes.to_vec());
    tracing::info!(group = %group.name, platform = %route.platform.name, tool = %tool_key, url = %url, "tools upstream request");

    let resp = match rb.send().await {
        Ok(r) => r,
        Err(e) => {
            // transport 错误：对齐 decision.rs / forward.rs —— connect 失败计熔断 + 降权，其余仅降权。
            if e.is_connect() {
                state.scheduler.record_connect_failure(
                    route.platform.id,
                    candidate_total,
                    &breaker_th,
                    aidog_db::now(),
                );
            } else {
                state.scheduler.record_ignored(route.platform.id);
            }
            state.scheduler.record_penalty(
                route.platform.id,
                super::scheduling::PenaltyTier::Server,
                aidog_db::now(),
            );
            let detail = err_chain(&e);
            tracing::error!(url = %url, platform = %route.platform.name, error = %detail, "tools upstream request failed (502)");
            let upstream_err = format!("upstream error: {detail}");
            attempts.push(ProxyAttempt {
                platform_id: route.platform.id,
                platform_name: route.platform.name.clone(),
                status_code: 0,
                error: upstream_err.clone(),
                duration_ms: attempt_start.elapsed().as_millis() as i64,
                ts: attempt_ts,
            });
            let _ = aidog_db::set_platform_last_error(
                &state.db,
                route.platform.id,
                Some(upstream_err.clone()),
            )
            .await;
            if !is_last_candidate {
                return AttemptOutcome::Next { connect_failed: false };
            }
            let msg = format!("{}: {detail}", i18n::t(lang, ErrorKey::Upstream));
            return AttemptOutcome::Respond(finalize_proxy_502(
                state,
                log,
                attempts,
                route.platform.id,
                upstream_err,
                msg,
                start,
                log_settings,
            )
            .await);
        }
    };

    let status = resp.status();
    log.upstream_status_code = status.as_u16() as i32;
    log.upstream_response_headers = upstream_headers_to_json(resp.headers());

    // 非 2xx 沿用 handle_non_success（429 冷却 / 5xx 换平台计熔断 / 401 auth 冷却 / 400 直返）。
    if !status.is_success() {
        match handle_non_success(
            resp,
            status,
            state,
            log,
            attempts,
            route,
            group,
            &breaker_th,
            NonSuccessCtx {
                url: &url,
                start,
                attempt_start,
                attempt_ts,
                is_last_candidate,
                log_settings,
                requested_model: "",
            },
        )
        .await
        {
            AttemptOutcome::Respond(r) => return AttemptOutcome::Respond(r),
            AttemptOutcome::Next { .. } => return AttemptOutcome::Next { connect_failed: false },
        }
    }

    // ── 2xx：非流式。strict 判定（语义 7）→ 按次计费（语义 8）→ 成功记账 → 终态。──
    let body = resp.bytes().await.unwrap_or_default();
    let body_str = String::from_utf8_lossy(&body).to_string();
    let billed = tools_success_ok(&route.platform.platform_type, &tool_key, &body);

    // 成功记账（同 decision.rs：延迟 EMA + auto_disabled 恢复）。200 无结果（strict 不计费）
    // 仍记 success：空结果是正常业务态非故障，不计熔断惩罚（语义 7 只约定不计费）。
    let attempt_latency_ms = attempt_start.elapsed().as_millis() as i64;
    state
        .scheduler
        .record_success(route.platform.id, attempt_latency_ms);
    if !route.platform.last_error.is_empty() {
        let _ =
            aidog_db::set_platform_last_error(&state.db, route.platform.id, None).await;
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
        if let Err(e) =
            aidog_db::recover_platform_auto_disabled(&state.db, route.platform.id).await
        {
            tracing::error!(platform_id = route.platform.id, error = %e, "recover auto-disabled platform failed");
        } else {
            aidog_ctx::emit("proxy-log-updated", route.platform.id.into());
        }
    }

    // 语义 8：按次费 = tools_pricing[键].per_1000 ÷ 1000 × CNY_PER_USD（CNY 条目换算 USD），
    // 不走 peak 倍率链、不走 resolve_price。无声明 / strict 未过 → 0（tools_cost 内部
    // warn 每请求至多一条）。est_cost != 0 时 log.rs 的 registry 价回落自动跳过（R14 同机制）；
    // token 列全 0（不提取 usage——tools 响应 usage 与 token 计价无关，写了反而会触发
    // est_cost==0 的回落重算），回落条件不触发，0 原样落库。
    log.est_cost = if billed {
        crate::gateway::billing::tools_cost(
            crate::gateway::billing::default_tools_pricing(&route.platform.platform_type.wire_str())
                .as_ref(),
            &[(tool_key.as_str(), 1)],
        )
    } else {
        0.0
    };

    // 按次费入余额预估：独立搜索 token 全 0，spawn_estimate 凭按次费绕过「无 token 跳过」。
    // log.est_cost 此刻即 tools_cost（USD，不含 peak 倍率），直接作 tool_cost_usd。
    spawn_estimate(
        state,
        route.platform.id,
        &route.platform.platform_type,
        route.platform.base_url.clone(),
        route.platform.api_key.clone(),
        log.model.clone(),
        route.platform.extra.clone(),
        0,
        0,
        0,
        0,
        route.platform.endpoints.iter().any(|ep| ep.coding_plan),
        log.created_at,
        log.est_cost,
        tracing::Span::current(),
    );

    log.status_code = status.as_u16() as i32;
    log.done = true;
    log.response_body = body_str.clone();
    log.user_response_body = body_str;
    log.user_response_headers = r#"{"content-type":"application/json"}"#.to_string();
    log.duration_ms = start.elapsed().as_millis() as i32;
    log.retry_count = (attempts.len() as i32 - 1).max(0);
    log.attempts = std::mem::take(attempts);
    upsert_log(state, log, log_settings).await;
    let mut response = (
        StatusCode::OK,
        [(axum::http::header::CONTENT_TYPE, "application/json")],
        body,
    )
        .into_response();
    inject_trace_header(&mut response);
    AttemptOutcome::Respond(response)
}

#[cfg(test)]
#[path = "test_tools.rs"]
mod test_tools;
