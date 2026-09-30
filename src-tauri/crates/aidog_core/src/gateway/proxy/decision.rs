//! TypeSafe 决策请求（`/v1/systemone`，jev-decision-proxy §3.4）：
//! 经 router 选候选（复用过滤 / 冷却 / 熔断，不照抄 responses.rs 的绕过路由做法），
//! 原样转发 body 仅改写 `model`，响应剥非官方字段后回客户端，计费优先上游 `usage.cost`。
//!
//! 规则对照（spec §2）：R1 入口分流（handler.rs）→ R2–R6 router（candidates.rs）→
//! R7 本文件 route-fail 落库 → R8 模型改写（router 侧 resolve_decision_model）→
//! R9/R10 endpoint + URL → R11 原样转发 → R12 字段剥离 → R13 错误沿用 handle_non_success →
//! R14 est_cost 优先 usage.cost（无则由 process_upsert 回落 registry 价）→ R15 无新列。

use super::*;

/// 路由 Err 字符串 → 面向用户的 message（R7：措辞面向用户，非内部术语）。
/// `no_chat_platform` 分支由 handler.rs 的聊天 route-fail 路径复用。
pub(crate) fn kind_route_error_message(e: &str) -> String {
    match e {
        "no_decision_platform" => {
            "分组内没有可用的决策平台：请为平台配置 jev 决策模型槽位，或接入带 decision 能力模型的平台"
                .to_string()
        }
        "no_chat_platform" => {
            "分组内没有可用的聊天平台：平台仅支持决策请求，或模型不支持此类请求".to_string()
        }
        other => format!("route error: {other}"),
    }
}

/// 决策请求 route fail 落库（R7）：仿 peak 路径 —— `status_code=400`、
/// `blocked_by='router'`、`blocked_reason`=Err 字符串、`est_cost=0`。
#[allow(clippy::too_many_arguments)]
async fn route_fail_response(
    state: &Arc<ProxyState>,
    log: &mut ProxyLog,
    log_settings: &ProxyLogSettings,
    err: &str,
    start: std::time::Instant,
    lang: Lang,
) -> Response {
    log.blocked_by = "router".to_string();
    log.blocked_reason = err.to_string();
    log.status_code = 400;
    log.done = true;
    let msg = kind_route_error_message(err);
    log.response_body = msg.clone();
    log.duration_ms = start.elapsed().as_millis() as i32;
    upsert_log(state, log, log_settings).await;
    let mut r = (
        StatusCode::BAD_REQUEST,
        format!("{}: {}", i18n::t(lang, ErrorKey::Route), msg),
    )
        .into_response();
    inject_trace_header(&mut r);
    r
}

/// 决策响应（2xx）处理：取 `usage.cost` 计费（R14）→ 删非官方顶层字段（R12）→ 回客户端。
/// 返回 (发给客户端的 body 字节, 上游 usage.cost)。非 JSON body 原样返回（cost=None）。
fn process_decision_response(body: &[u8]) -> (Vec<u8>, Option<f64>) {
    let Ok(mut v) = serde_json::from_slice::<Value>(body) else {
        // 非 JSON（上游非标准但有内容）：原样返回，不计费（R12 仅约束 JSON 官方格式）
        return (body.to_vec(), None);
    };
    let cost = v
        .get("usage")
        .and_then(|u| u.get("cost"))
        .and_then(|c| c.as_f64());
    if let Some(obj) = v.as_object_mut() {
        obj.remove("id");
        obj.remove("provider");
    }
    if let Some(usage) = v.get_mut("usage").and_then(|u| u.as_object_mut()) {
        usage.remove("cost");
    }
    match serde_json::to_vec(&v) {
        Ok(bytes) => (bytes, cost),
        Err(_) => (body.to_vec(), cost),
    }
}

/// 决策请求主入口（handler.rs 在 count_tokens 分支后调用）。
#[allow(clippy::too_many_arguments)]
pub(crate) async fn handle_decision(
    state: &Arc<ProxyState>,
    log: &mut ProxyLog,
    log_settings: &ProxyLogSettings,
    group: &Group,
    bytes: &[u8],
    start: std::time::Instant,
    lang: Lang,
) -> Response {
    log.source_protocol = "typesafe".to_string();
    log.target_protocol = "typesafe".to_string();
    log.is_stream = false;

    // ── 解析请求 body（取 model 供路由 / 映射）──
    let req_value: Value = match serde_json::from_slice(bytes) {
        Ok(v) => v,
        Err(e) => {
            log.response_body = format!("parse request json error: {e}");
            log.status_code = 400;
            log.done = true;
            log.duration_ms = start.elapsed().as_millis() as i32;
            upsert_log(state, log, log_settings).await;
            let mut r = (
                StatusCode::BAD_REQUEST,
                format!("{}: {e}", i18n::t(lang, ErrorKey::ParseJson)),
            )
                .into_response();
            inject_trace_header(&mut r);
            return r;
        }
    };
    let requested_model = req_value
        .get("model")
        .and_then(|m| m.as_str())
        .unwrap_or("")
        .to_string();
    log.model = requested_model.clone();

    // ── 路由选候选（复用过滤 / 冷却 / 熔断；Decision 类型过滤见 candidates.rs）──
    let sched_settings = aidog_db::get_scheduling_settings(&state.db).await;
    let sched_ctx = ScheduleCtx {
        scheduler: &state.scheduler,
        sticky: &state.sticky,
        settings: &sched_settings,
        sticky_key: Some(format!("{}|decision", group.group_key)),
    };
    let candidate_set = match select_candidates_ctx(
        &state.db,
        group,
        &requested_model,
        Some(&sched_ctx),
        RequestKind::Decision,
    )
    .await
    {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!(group = %group.name, error = %e, "decision route failed");
            return route_fail_response(state, log, log_settings, &e, start, lang).await;
        }
    };
    let candidates = candidate_set.candidates;

    // ── 重试编排：仿 handler.rs 聊天循环（max_retries 上限 + attempts 记录）──
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

        // endpoint 选择（R9）：typesafe 优先，否则 OpenAI 系；都没有回退平台主 base_url。
        let ep = select_endpoint_for_decision(&route.platform.endpoints);
        let base_url = ep
            .map(|e| e.base_url.clone())
            .unwrap_or_else(|| route.platform.base_url.clone());
        // R10：URL = endpoint base_url（含 /v1）+ /systemone，禁额外拼接。
        let url = join_upstream_path(&base_url, &adapter::decision_api_path());

        // R8：模型改写已由 router 完成（jev 槽位 / available_models 推导），此处仅写入 body。
        let actual_model = route.target_model.clone();
        let mut upstream_body = req_value.clone();
        if let Some(obj) = upstream_body.as_object_mut() {
            obj.insert("model".to_string(), Value::String(actual_model.clone()));
        }
        let upstream_body_str = serde_json::to_string(&upstream_body).unwrap_or_default();

        log.platform_id = route.platform.id;
        log.actual_model = actual_model.clone();
        log.upstream_request_url = url.clone();
        log.upstream_request_headers = r#"{"authorization":"[REDACTED]","content-type":"application/json"}"#.to_string();
        log.upstream_request_body = if log_settings.log_upstream_request {
            format_pretty_json(&upstream_body_str)
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
            &proxy_client,
            req_timeout,
            conn_timeout,
            Some(&route.platform.extra),
            None,
        )
        .await;
        let rb = client
            .post(&url)
            .header("Content-Type", "application/json")
            .header("Authorization", format!("Bearer {}", route.platform.api_key))
            .body(upstream_body_str.clone());
        tracing::info!(group = %group.name, platform = %route.platform.name, model = %actual_model, url = %url, "decision upstream request");

        let resp = match rb.send().await {
            Ok(r) => r,
            Err(e) => {
                // transport 错误：对齐 forward.rs —— connect 失败计熔断 + 降权，其余仅降权。
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
                tracing::error!(url = %url, platform = %route.platform.name, error = %detail, "decision upstream request failed (502)");
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
                    continue;
                }
                let msg = format!("{}: {detail}", i18n::t(lang, ErrorKey::Upstream));
                return finalize_proxy_502(
                    state,
                    log,
                    &mut attempts,
                    route.platform.id,
                    upstream_err,
                    msg,
                    start,
                    log_settings,
                )
                .await;
            }
        };

        let status = resp.status();
        log.upstream_status_code = status.as_u16() as i32;
        log.upstream_response_headers = upstream_headers_to_json(resp.headers());

        // R13：非 2xx 沿用 handle_non_success（429 冷却 / 5xx·529 换平台计熔断 / 401 auth 冷却 / 400·422 直返）。
        if !status.is_success() {
            match handle_non_success(
                resp,
                status,
                state,
                log,
                &mut attempts,
                &route,
                group,
                &breaker_th,
                &url,
                start,
                attempt_start,
                attempt_ts,
                is_last_candidate,
                log_settings,
                &requested_model,
            )
            .await
            {
                AttemptOutcome::Respond(r) => return r,
                AttemptOutcome::Next { .. } => continue,
            }
        }

        // ── 2xx：非流式。取 usage.cost 计费（R14）→ 剥字段（R12）→ 成功记账 → 终态。──
        let body = resp.bytes().await.unwrap_or_default();
        let (client_body, upstream_cost) = process_decision_response(&body);
        let body_str = String::from_utf8_lossy(&body).to_string();

        // 成功记账（对齐 forward.rs commit_2xx_success：延迟 EMA + 熔断恢复 + auto_disabled 恢复）。
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

        // token 记账：usage.input_tokens / output_tokens（Anthropic 同名字段，extract_usage 已认）。
        let (input, output, cache, cache_write) = extract_usage(&body_str);
        log.input_tokens = input;
        log.output_tokens = output;
        log.cache_tokens = cache;
        log.cache_write_tokens = cache_write;
        // R14：上游显式 cost 直接采用（est_cost != 0 时 process_upsert 跳过 registry 价回落；
        // 无 cost 则留 0，由 process_upsert 按 registry 价 × tokens 计算，高峰倍率链照常生效）。
        if let Some(cost) = upstream_cost {
            log.est_cost = cost;
        }

        let client_str = String::from_utf8_lossy(&client_body).to_string();
        log.status_code = status.as_u16() as i32;
        log.done = true;
        log.response_body = body_str;
        log.user_response_body = client_str;
        log.user_response_headers = r#"{"content-type":"application/json"}"#.to_string();
        log.duration_ms = start.elapsed().as_millis() as i32;
        log.retry_count = (attempts.len() as i32 - 1).max(0);
        log.attempts = std::mem::take(&mut attempts);
        upsert_log(state, log, log_settings).await;
        let mut response = (StatusCode::OK, client_body).into_response();
        inject_trace_header(&mut response);
        return response;
    }

    // 候选耗尽且循环内未 return（同 handler.rs 兜底，理论不可达）。
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

#[cfg(test)]
#[path = "test_decision.rs"]
mod test_decision;
