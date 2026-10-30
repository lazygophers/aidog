//! 辅助模型请求（`/v1/embeddings`、`/v1/rerank`）的共享转发骨架，仿 `decision.rs`：
//! 解析 JSON → `select_candidates_ctx(spec.kind)` → 重试编排 → OpenAI 系 endpoint →
//! `join_upstream_path(base_url, spec.api_path)` → 仅改写 `model` 为槽位值 → 转发 →
//! 2xx 回写响应 `model` 为客户端原名 → 记账。不支持流式。
//!
//! 种类差异全部收在 [`AuxSpec`]（路由种类 / 出站后缀 / 日志协议标签 / usage 提取器）；
//! 路由失败的 `blocked_reason` 取 `kind.no_platform_err()`，由 `decision::route_fail_response` 落库。

use super::decision::route_fail_response;
use super::*;

/// 一种辅助模型请求的差异点（新增 rerank = 新增一个 const，不改骨架）。
pub(crate) struct AuxSpec {
    pub kind: RequestKind<'static>,
    /// 出站后缀，拼在 endpoint `base_url`（已含版本前缀）之后，如 `/embeddings`。
    pub api_path: &'static str,
    /// `source_protocol` / `target_protocol` 日志标签。
    pub label: &'static str,
    /// 2xx 响应体 → (input_tokens, output_tokens)。
    pub extract_usage: fn(&str) -> (i32, i32),
    /// 2xx 响应体 → 按次计量次数（rerank：Cohere `search_units`，缺省 1）；None = 该种类不按次计量。
    pub extract_units: Option<fn(&str) -> i64>,
}

/// embedding：`usage.prompt_tokens` → input，output 恒 0（OpenAI 规范无输出 token）。
fn embedding_usage(body: &str) -> (i32, i32) {
    let (input, _output, ..) = extract_usage(body);
    (input, 0)
}

pub(crate) const EMBEDDING: AuxSpec = AuxSpec {
    kind: RequestKind::Embedding,
    api_path: "/embeddings",
    label: "embeddings",
    extract_usage: embedding_usage,
    extract_units: None,
};

/// rerank：Jina `usage.total_tokens`（记 input）→ SiliconFlow `meta.tokens` →
/// Cohere `meta.billed_units` 的 token 字段；全缺 → (0, 0)（Cohere 按次计量见 [`rerank_search_units`]）。
fn rerank_usage(body: &str) -> (i32, i32) {
    let v: Value = serde_json::from_str(body).unwrap_or_default();
    let n = |p: &Value, k: &str| p.get(k).and_then(Value::as_i64).unwrap_or(0) as i32;
    if let Some(t) = v.pointer("/usage/total_tokens").and_then(Value::as_i64) {
        return (t as i32, 0);
    }
    for path in ["/meta/tokens", "/meta/billed_units"] {
        if let Some(m) = v.pointer(path) {
            let (i, o) = (n(m, "input_tokens"), n(m, "output_tokens"));
            if i > 0 || o > 0 {
                return (i, o);
            }
        }
    }
    (0, 0)
}

/// rerank 按次计量次数：`meta.billed_units.search_units`，缺省按 1 次/请求。
fn rerank_search_units(body: &str) -> i64 {
    serde_json::from_str::<Value>(body)
        .ok()
        .and_then(|v| v.pointer("/meta/billed_units/search_units").and_then(Value::as_i64))
        .unwrap_or(1)
}

pub(crate) const RERANK: AuxSpec = AuxSpec {
    kind: RequestKind::Rerank,
    api_path: "/rerank",
    label: "rerank",
    extract_usage: rerank_usage,
    extract_units: Some(rerank_search_units),
};

/// 辅助模型请求主入口（handler.rs 在 count_tokens / 决策分流旁调用）。
#[allow(clippy::too_many_arguments)]
pub(crate) async fn handle_aux_model(
    spec: &AuxSpec,
    state: &Arc<ProxyState>,
    log: &mut ProxyLog,
    log_settings: &ProxyLogSettings,
    group: &Group,
    bytes: &[u8],
    start: std::time::Instant,
    lang: Lang,
) -> Response {
    log.source_protocol = spec.label.to_string();
    log.target_protocol = spec.label.to_string();
    log.is_stream = false;

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

    let sched_settings = aidog_db::get_scheduling_settings(&state.db).await;
    let sched_ctx = ScheduleCtx {
        scheduler: &state.scheduler,
        sticky: &state.sticky,
        settings: &sched_settings,
        sticky_key: Some(format!("{}|{}", group.group_key, spec.label)),
    };
    let candidate_set = match select_candidates_ctx(
        &state.db,
        group,
        &requested_model,
        Some(&sched_ctx),
        spec.kind,
    )
    .await
    {
        Ok(c) => {
            log.routing = Some(c.routing.clone());
            c
        }
        Err(e) => {
            log.routing = Some(e.routing.clone());
            tracing::warn!(group = %group.name, kind = spec.label, error = %e, "aux route failed");
            return route_fail_response(state, log, log_settings, &e, start, lang).await;
        }
    };
    let candidates = candidate_set.candidates;

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

        match aux_attempt(
            spec, state, log, &mut attempts, log_settings, group, &route, &req_value,
            &requested_model, &proxy_client, req_timeout, conn_timeout, &sched_settings, start,
            lang, attempt_start, attempt_ts, is_last_candidate, candidate_total,
        )
        .await
        {
            AttemptOutcome::Respond(r) => return r,
            AttemptOutcome::Next { .. } => continue,
        }
    }

    // 候选耗尽且循环内未 return（同 decision.rs / handler.rs 兜底，理论不可达）。
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

/// 单候选尝试（endpoint 选择 → 建请求 → 发送 → 非 2xx 沿用 handle_non_success → 2xx 记账终态）。
#[allow(clippy::too_many_arguments)]
async fn aux_attempt(
    spec: &AuxSpec,
    state: &Arc<ProxyState>,
    log: &mut ProxyLog,
    attempts: &mut Vec<ProxyAttempt>,
    log_settings: &ProxyLogSettings,
    group: &Group,
    route: &RouteResult,
    req_value: &Value,
    requested_model: &str,
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
    // endpoint：OpenAI 系优先，否则回退平台主 base_url（零端点平台）。
    let ep = select_endpoint_for_openai_family(&route.platform.endpoints);
    let base_url = ep
        .map(|e| e.base_url.clone())
        .unwrap_or_else(|| route.platform.base_url.clone());
    // URL = endpoint base_url（含版本前缀）+ 出站后缀，禁额外拼接。
    let url = join_upstream_path(&base_url, spec.api_path);

    // 模型改写已由 router 完成（槽位值），此处仅写入 body，其余字段原样。
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
        upstream_body_str.clone()
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
    let rb = client
        .post(&url)
        .header("Content-Type", "application/json")
        .header("Authorization", format!("Bearer {}", route.platform.api_key))
        .body(upstream_body_str.clone());
    tracing::info!(group = %group.name, platform = %route.platform.name, model = %actual_model, url = %url, kind = spec.label, "aux upstream request");

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
            tracing::error!(url = %url, platform = %route.platform.name, error = %detail, "aux upstream request failed (502)");
            let upstream_err = format!("upstream error: {detail}");
            attempts.push(ProxyAttempt {
                platform_id: route.platform.id,
                platform_name: route.platform.name.clone(),
                status_code: 0,
                error: upstream_err.clone(),
                duration_ms: attempt_start.elapsed().as_millis() as i64,
                ts: attempt_ts,
                model: Some(route.target_model.clone()),
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
            return AttemptOutcome::Respond(
                finalize_proxy_502(
                    state,
                    log,
                    attempts,
                    route.platform.id,
                    upstream_err,
                    msg,
                    start,
                    log_settings,
                )
                .await,
            );
        }
    };

    let status = resp.status();
    log.upstream_status_code = status.as_u16() as i32;
    log.upstream_response_headers = upstream_headers_to_json(resp.headers());

    // 非 2xx 沿用 handle_non_success（429 冷却 / 5xx·529 换平台计熔断 / 401 auth 冷却 / 400·422 直返）。
    if !status.is_success() {
        return match handle_non_success(
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
                requested_model,
            },
        )
        .await
        {
            AttemptOutcome::Respond(r) => AttemptOutcome::Respond(r),
            AttemptOutcome::Next { .. } => AttemptOutcome::Next { connect_failed: false },
        };
    }

    // ── 2xx：非流式。响应 model 回写客户端原名，其余字段不动。──
    let body = resp.bytes().await.unwrap_or_default();
    let body_str = String::from_utf8_lossy(&body).to_string();
    let client_body = if requested_model.is_empty() {
        body.to_vec()
    } else {
        replace_model_in_json(&body, requested_model)
    };

    // 成功记账（对齐 decision.rs / forward.rs commit_2xx_success）。
    let attempt_latency_ms = attempt_start.elapsed().as_millis() as i64;
    state
        .scheduler
        .record_success(route.platform.id, attempt_latency_ms);
    if !route.platform.last_error.is_empty() {
        let _ = aidog_db::set_platform_last_error(&state.db, route.platform.id, None).await;
    }
    attempts.push(ProxyAttempt {
        platform_id: route.platform.id,
        platform_name: route.platform.name.clone(),
        status_code: status.as_u16() as i32,
        error: String::new(),
        duration_ms: attempt_latency_ms,
        ts: attempt_ts,
        model: Some(route.target_model.clone()),
    });
    if route.platform.status == super::models::PlatformStatus::AutoDisabled {
        if let Err(e) = aidog_db::recover_platform_auto_disabled(&state.db, route.platform.id).await
        {
            tracing::error!(platform_id = route.platform.id, error = %e, "recover auto-disabled platform failed");
        } else {
            aidog_ctx::emit("proxy-log-updated", route.platform.id.into());
        }
    }

    // token 记账；est_cost 留 0，由 upsert_log 按 actual_model 的 registry 价 × tokens 计算
    // （全 0 价未标 free → 兜底价；free:true → 0，见 log.rs est_cost 回落）。
    let (input, output) = (spec.extract_usage)(&body_str);
    log.input_tokens = input;
    log.output_tokens = output;
    if let Some(units) = spec.extract_units {
        // 按次计量次数经内存 ProxyLog 透传，calc_est_cost 仅在条目 `price.unit=="request"` 时消费。
        log.tool_call_counts
            .push((crate::gateway::billing::SEARCH_UNITS_KEY.to_string(), units(&body_str)));
    }

    log.status_code = status.as_u16() as i32;
    log.done = true;
    log.response_body = body_str;
    log.user_response_body = String::from_utf8_lossy(&client_body).to_string();
    log.user_response_headers = r#"{"content-type":"application/json"}"#.to_string();
    log.duration_ms = start.elapsed().as_millis() as i32;
    log.retry_count = (attempts.len() as i32 - 1).max(0);
    log.attempts = std::mem::take(attempts);
    upsert_log(state, log, log_settings).await;
    let mut response = (
        StatusCode::OK,
        [(axum::http::header::CONTENT_TYPE, "application/json")],
        client_body,
    )
        .into_response();
    inject_trace_header(&mut response);
    AttemptOutcome::Respond(response)
}

#[cfg(test)]
#[path = "test_aux_model.rs"]
mod test_aux_model;
