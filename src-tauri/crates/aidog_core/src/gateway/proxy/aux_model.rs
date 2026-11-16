//! 辅助模型请求（`/v1/embeddings`、`/v1/rerank`）的共享转发骨架，仿 `decision.rs`：
//! 解析 JSON → `select_candidates_gated(spec.kind)` → 重试编排 → OpenAI 系 endpoint →
//! `join_upstream_path(base_url, spec.api_path)` → 仅改写 `model` 为槽位值 → 转发 →
//! 2xx 回写响应 `model` 为客户端原名 → 记账。不支持流式。
//!
//! 种类差异全部收在 [`AuxSpec`]（路由种类 / 出站后缀 / 日志协议标签 / usage 提取器）；
//! 路由失败的 `blocked_reason` 取 `kind.no_platform_err()`，由 `decision::route_fail_response` 落库。

use super::attempt_plan::AttemptLoop;
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
pub(crate) async fn handle_aux_model(
    spec: &AuxSpec,
    state: &Arc<ProxyState>,
    log: &mut ProxyLog,
    group: &Group,
    bytes: &[u8],
    ctx: super::call_scope::ReqCtx<'_>,
) -> Response {
    let super::call_scope::ReqCtx { log_settings, start, lang, client } = ctx;
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
        rate_body: Some(&req_value),
    };
    let candidate_set = match select_candidates_gated(
        &state.db,
        group,
        &requested_model,
        Some(&sched_ctx),
        spec.kind,
        client.clone(),
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

    let mut attempt_loop = AttemptLoop::new(group.max_retries, spec.kind, &requested_model, client, candidates);
    let mut attempts: Vec<ProxyAttempt> = Vec::new();
    // 平台限频占位跨调用保存：同平台连续调用复用（平台级只占一次），换平台时释放上一个。
    let mut rate_slot = RateSlot::default();

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

    while let Some(pick) = attempt_loop.next(&state.db, group, &sched_ctx, log, attempts.len()).await {
        let route = pick.route;
        let attempt_start = std::time::Instant::now();
        let attempt_ts = aidog_db::now();

        // 平台限频占位（与 forward_attempt 同口径）：begin 占平台级，enter_model 占模型级；
        // 占位失败不发请求（不计次数）：平台级排除整个平台，模型级只排除该组合。
        let _access = match rate_slot.enter(
            state,
            log,
            &mut attempts,
            RateSlotCtx {
                route: &route,
                requested_model: &requested_model,
                req_value: &req_value,
                model_alt: pick.model_alt,
                label: spec.label,
            },
        ) {
            Ok(access) => access,
            Err(()) => {
                attempt_loop.failed(&mut attempts, false);
                continue;
            }
        };
        // 额度窗口闸门（extra.quota_windows 执行）：超限不发上游请求（不计调用次数），排除整个平台。
        if !crate::gateway::quota_window::gate_attempt(
            &state.db,
            log,
            &mut attempts,
            &route.platform,
            &route.target_model,
            pick.model_alt,
        )
        .await
        {
            attempt_loop.failed(&mut attempts, false);
            continue;
        }
        let capped = attempt_loop.begin_call(log, &attempts, route.platform.id);
        let scope = super::call_scope::CallScope {
            state,
            group,
            route: &route,
            log_settings,
            lang,
            start,
            attempt_start,
            attempt_ts,
            is_last_candidate: capped,
            candidate_total: pick.candidate_total,
            has_next_model: pick.model_alt && !capped,
            breaker_th: super::scheduling::BreakerThresholds::resolve(&sched_settings, &route.platform, group.routing_mode),
            proxy_client: &proxy_client,
            req_timeout,
            conn_timeout,
        };
        let out = aux_attempt(&scope, spec, log, &mut attempts, &req_value, &requested_model, &mut false)
            .await;
        match out {
            AttemptOutcome::Respond(r) => return r,
            AttemptOutcome::Next { connect_failed } => attempt_loop.failed(&mut attempts, connect_failed),
        }
    }

    AttemptLoop::exhausted(state, log, &mut attempts, start, log_settings, lang).await
}

/// 单候选尝试（endpoint 选择 → 建请求 → 发送 → 非 2xx 沿用 handle_non_success → 2xx 记账终态）。
async fn aux_attempt(
    scope: &super::call_scope::CallScope<'_>,
    spec: &AuxSpec,
    log: &mut ProxyLog,
    attempts: &mut Vec<ProxyAttempt>,
    req_value: &Value,
    requested_model: &str,
    switched: &mut bool,
) -> AttemptOutcome {
    let (route, log_settings, group) = (scope.route, scope.log_settings, scope.group);
    let (is_last_candidate, has_next_model) = (scope.is_last_candidate, scope.has_next_model);
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

    let client = scope.build_client().await;
    let rb = client
        .post(&url)
        .header("Content-Type", "application/json")
        .header("Authorization", format!("Bearer {}", route.platform.api_key))
        .body(upstream_body_str.clone());
    tracing::info!(group = %group.name, platform = %route.platform.name, model = %actual_model, url = %url, kind = spec.label, "aux upstream request");

    let resp = match rb.send().await {
        Ok(r) => r,
        // transport 错误：judge(Transport) + apply 记账（connect 计熔断 + 降权，其余仅降权）。
        Err(e) => return scope.transport_failed(log, attempts, &e, &url).await,
    };

    let status = resp.status();
    log.upstream_status_code = status.as_u16() as i32;
    log.upstream_response_headers =
        upstream_headers_json_for_log(resp.headers(), log_settings.log_upstream_request);

    // 非 2xx 沿用 handle_non_success（429 冷却 / 5xx·529 换平台计熔断 / 401 auth 冷却 / 400·422 直返）。
    if !status.is_success() {
        return match handle_non_success(
            resp,
            status,
            scope.state,
            log,
            attempts,
            route,
            group,
            &scope.breaker_th,
            NonSuccessCtx {
                url: &url,
                start: scope.start,
                attempt_start: scope.attempt_start,
                attempt_ts: scope.attempt_ts,
                is_last_candidate,
                log_settings,
                requested_model,
                has_next_model,
                switched,
                ttfb_ms: None,
                candidate_total: scope.candidate_total,
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

    // 成功记账（CallScope 统一：延迟 EMA + last_error 清空 + auto_disabled 恢复 + attempt 留痕，
    // 对齐 decision.rs / forward.rs commit_2xx_success）。
    scope.succeeded(attempts, status.as_u16()).await;

    // token 记账；est_cost 留 0，由 upsert_log 按 actual_model 的 registry 价 × tokens 计算
    // （全 0 价未标 free → 兜底价；free:true → 0，见 log.rs est_cost 回落）。
    let (input, output) = (spec.extract_usage)(&body_str);
    log.input_tokens = input;
    log.output_tokens = output;
    // 按次计量次数（rerank：search_units，缺省 1；embedding None → 0）。
    let request_units = match spec.extract_units {
        Some(units) => {
            let n = units(&body_str);
            // 按量计数经内存 ProxyLog 透传，calc_est_cost 按条目 `price.unit` 消费
            // （request/image/mp 按次；char/second 按字符数/时长换算，issue 157）。
            log.tool_call_counts
                .push((crate::gateway::billing::SEARCH_UNITS_KEY.to_string(), n));
            n
        }
        None => 0,
    };

    // issue #61：embedding / rerank 请求入余额与手动预算预估（此前完全不触发）。
    // spawn_estimate 后台执行，不阻塞响应；base_url / api_key 仅供校准真查用。
    spawn_estimate(
        scope.state,
        route.platform.id,
        &route.platform.platform_type,
        route.platform.base_url.clone(),
        route.platform.api_key.clone(),
        actual_model.clone(),
        route.platform.extra.clone(),
        input,
        output,
        0, // embedding / rerank 无缓存 token
        0,
        route.platform.endpoints.iter().any(|ep| ep.coding_plan),
        log.created_at,
        0.0, // tools 按次费与 aux 请求无关
        request_units,
        true, // 额度窗口执行范围：向量化 / 重排序计入
        tracing::Span::current(),
    );

    log.status_code = status.as_u16() as i32;
    log.done = true;
    log.response_body = body_str;
    log.user_response_body = String::from_utf8_lossy(&client_body).to_string();
    log.user_response_headers = r#"{"content-type":"application/json"}"#.to_string();
    log.duration_ms = scope.start.elapsed().as_millis() as i32;
    log.retry_count = aidog_db::models::retry_count_of(attempts);
    log.attempts = std::mem::take(attempts);
    upsert_log(scope.state, log, log_settings).await;
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
