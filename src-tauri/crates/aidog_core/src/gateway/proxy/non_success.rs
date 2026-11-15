use super::*;
use super::failure_verdict::{apply, judge, Failure, JudgeCtx, Next};

/// attempt 级杂项上下文（`handle_non_success` 入参打包）：URL、时间戳对、候选位次、
/// 日志开关、客户端请求的模型名（remap 前，中间件 applies_to.models 的匹配对象）。
pub(crate) struct NonSuccessCtx<'a> {
    pub url: &'a str,
    pub start: std::time::Instant,
    pub attempt_start: std::time::Instant,
    pub attempt_ts: i64,
    pub is_last_candidate: bool,
    pub log_settings: &'a ProxyLogSettings,
    pub requested_model: &'a str,
    /// 命中槽位的模型链上是否还有下一个模型（票 06）。
    pub has_next_model: bool,
    /// 置 true = 本次为模型类失败，已留痕未记账，调用方换下一个模型重发。
    pub switched: &'a mut bool,
    /// 本次尝试的首字节耗时（毫秒，响应头已到）。
    pub ttfb_ms: Option<i64>,
    /// 当前排序里的候选数（connect 失败熔断的本地网络保护分母，apply 用）。
    pub candidate_total: usize,
}

/// 失败后继续（换组合）时给最后一条尝试记录打排除范围：服务类 → 整个平台，其余（模型类 / 未知类）→ 只该组合。
/// 范围取 `Verdict.scope`（judge 已按类别裁定）。
fn mark_exclusion(attempts: &mut [ProxyAttempt], scope: ExcludeScope) {
    if let Some(a) = attempts.last_mut() {
        a.exclusion = Some(scope);
    }
}

/// 上游返回非 2xx 时的处理薄壳：解析响应 → `judge`（纯判定，见 `failure_verdict`）→
/// `apply`（scheduler / DB 记账唯一落地点）→ 按 `Verdict::next` 换模型 / 换候选 / 回客户端。
/// 本函数只做 I/O（读 body、中间件规则分类、日志落库），判定逻辑全部在 judge。
#[allow(clippy::too_many_arguments)]
pub(crate) async fn handle_non_success(
    resp: reqwest::Response,
    status: reqwest::StatusCode,
    state: &Arc<ProxyState>,
    log: &mut ProxyLog,
    attempts: &mut Vec<ProxyAttempt>,
    route: &RouteResult,
    group: &Group,
    breaker_th: &super::scheduling::BreakerThresholds,
    ctx: NonSuccessCtx<'_>,
) -> AttemptOutcome {
    let NonSuccessCtx { url, start, attempt_start, attempt_ts, is_last_candidate, log_settings, requested_model, has_next_model, switched, ttfb_ms, candidate_total } = ctx;
    // Retry-After 须在 resp 被 text() 消费前取（429 配额冷却用，见 judge）
    let retry_after = resp
        .headers()
        .get(reqwest::header::RETRY_AFTER)
        .and_then(|v| v.to_str().ok())
        .map(str::to_string);
    // 中间件 response_headers 条件用；同样须在 text() 消费 resp 前取。
    let resp_headers_json = upstream_headers_to_json(resp.headers());
    // 回客户端时透传用；同样须在 text() 消费 resp 前取。
    let upstream_resp_headers = resp.headers().clone();
    // 限流头（含 anthropic-ratelimit-unified-*）在 429 上最有价值：后台落库，不阻塞响应。
    spawn_rate_limit(state, route.platform.id, &upstream_resp_headers, tracing::Span::current());
    let body = resp.text().await.unwrap_or_default();
    let duration_ms = start.elapsed().as_millis() as i64;
    let code = status.as_u16();
    tracing::warn!(
        url = %url, platform = %route.platform.name, status = code,
        duration_ms, "upstream returned non-success status"
    );
    tracing::debug!(url = %url, status = code, body = %super::log_util::log_body_preview(&body), "upstream error response body");
    let attempt_err = truncate_attempt_error(&body);
    // 中间件 error_rule 分类是 I/O（读设置缓存 + 规则匹配），留薄壳；判定面收成 rule_non_retryable。
    let err_class = {
        let mw_settings = state
            .settings_cache
            .read()
            .await
            .middleware_settings
            .clone();
        state.middleware.classify_error(
            &mw_settings,
            code,
            &body,
            Some(&group.group_key),
            Some(route.platform.id as i64),
            requested_model,
            Some(&resp_headers_json),
        )
    };
    let code_meaning = registry_code_meaning(&route.platform.platform_type.wire_str(), &body);
    let censorship_blocked = is_censorship_blocked(&body);
    let failure = Failure::Http {
        code,
        body: &body,
        retry_after: retry_after.as_deref(),
        code_meaning: code_meaning.as_deref(),
    };
    let v = judge(
        &failure,
        &JudgeCtx {
            health_aware: breaker_th.health_aware,
            has_next_model,
            is_last_candidate,
            rule_non_retryable: err_class.as_ref().is_some_and(|c| !c.retryable),
            now_ms: aidog_db::now(),
        },
    );
    // 留痕：链上有下一个模型但没换 → 服务类（chain_stop=service）/ 规则拦截（chain_stop=rule），
    // 画布显示「跳过」；换模型与链尽两条早退路径不标。
    let chain_stop = (has_next_model && !matches!(v.next, Next::SwitchModel))
        .then(|| if v.class == UpstreamErrorClass::Service { "service" } else { "rule" }.to_string());
    attempts.push(ProxyAttempt {
        chain_stop,
        error_class: Some(v.class.as_str().to_string()),
        code_meaning: code_meaning.clone(),
        error_rule: err_class.as_ref().map(|c| c.matched_by.clone()),
        ttfb_ms,
        platform_id: route.platform.id,
        platform_name: route.platform.name.clone(),
        status_code: code as i32,
        error: attempt_err,
        duration_ms: attempt_start.elapsed().as_millis() as i64,
        ts: attempt_ts,
        model: Some(route.target_model.clone()),
        ..Default::default()
    });
    apply(&v, state, route, breaker_th, &failure, candidate_total).await;
    if let Some(a) = attempts.last_mut() {
        a.effects = v.effects.iter().map(|e| e.as_str().to_string()).collect();
    }

    match v.next {
        Next::SwitchModel => {
            tracing::info!(
                platform = %route.platform.name, status = code, class = ?v.class, model = %route.target_model,
                "model-class upstream error, switching to next model on same platform"
            );
            *switched = true;
            mark_exclusion(attempts, v.scope);
            return AttemptOutcome::Next { connect_failed: v.connect_failed };
        }
        Next::SwitchPlatform => {
            // 链尽换平台（blame=false 且零效果）与服务类 / 规则放行的主路径换候选在此合流；
            // 仅前者有专属留痕日志。
            if v.class == UpstreamErrorClass::Model && v.effects.is_empty() {
                tracing::info!(
                    platform = %route.platform.name, status = code, model = %route.target_model,
                    "model-class upstream error with chain exhausted, failing over to next platform without platform blame"
                );
            }
            mark_exclusion(attempts, v.scope);
            return AttemptOutcome::Next { connect_failed: v.connect_failed };
        }
        Next::Return => {}
    }

    // ── 终态：中间件 error_rule override_status/body（若有）回客户端 ──
    if let Some(ref c) = err_class {
        tracing::info!(
            matched_by = %c.matched_by, category = %c.category, retryable = c.retryable,
            status = code, "middleware error_rule classified upstream error"
        );
    }
    let status_retryable =
        is_status_retryable(code) || censorship_blocked || code_meaning.as_deref() == Some("overloaded");
    if !status_retryable && !censorship_blocked {
        tracing::info!(
            status = code, platform = %route.platform.name,
            "decision-A: hard request error (400/422), not retrying next platform"
        );
    }

    let body_overridden = err_class
        .as_ref()
        .is_some_and(|c| c.override_body.is_some());
    let (out_code, out_body) = match err_class {
        Some(c) => (
            c.override_status.unwrap_or(code),
            c.override_body.unwrap_or_else(|| body.clone()),
        ),
        None => (code, body.clone()),
    };
    // 透传上游响应头（与 2xx 路径同一条黑名单），理由见 error_response_headers 的文档注释。
    let filtered = error_response_headers(&upstream_resp_headers, body_overridden);

    log.platform_id = route.platform.id;
    log.response_body = body;
    log.status_code = out_code as i32;
    // 终态：真实 HTTP 状态码已定、不再换候选、之后不会再写这一行（票 06 的 done 列语义）。
    // 漏置位会让 upsert_log 的 is_terminal 恒 false → 既不 emit 也不进 stats_agg。
    log.done = true;
    log.user_response_body = out_body.clone();
    // 日志字段 = 实际发回客户端的头集合（与 finish.rs 的 2xx 路径同语义）
    log.user_response_headers = resp_headers_to_log_json(&filtered);
    log.duration_ms = duration_ms as i32;
    log.retry_count = aidog_db::models::retry_count_of(attempts);
    log.attempts = std::mem::take(attempts);
    upsert_log(state, log, log_settings).await;
    let mut r = (
        StatusCode::from_u16(out_code).unwrap_or(StatusCode::BAD_GATEWAY),
        out_body,
    )
        .into_response();
    // into_response 对 String 写死 content-type: text/plain；HeaderMap::extend 是 append 语义，
    // 直接 extend 会留下重复的 content-type（text/plain + 真实值），故先 remove 再 extend。
    r.headers_mut().remove(axum::http::header::CONTENT_TYPE);
    r.headers_mut().extend(filtered);
    inject_trace_header(&mut r);
    AttemptOutcome::Respond(r)
}
