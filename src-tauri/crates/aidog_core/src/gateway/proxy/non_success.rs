use super::*;

/// 上游返回非 2xx 时的处理：记录 attempt、熔断计数、401/403 auto_disable、
/// 中间件 error_rule 分类、决策 A 硬错圈定，决定 failover(Next) 还是返回客户端(Respond)。
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
    url: &str,
    start: std::time::Instant,
    attempt_start: std::time::Instant,
    attempt_ts: i64,
    is_last_candidate: bool,
    log_settings: &ProxyLogSettings,
) -> AttemptOutcome {
        let body = resp.text().await.unwrap_or_default();
        let duration_ms = start.elapsed().as_millis() as i64;
        let code = status.as_u16();
        tracing::warn!(
            url = %url, platform = %route.platform.name, status = code,
            duration_ms, "upstream returned non-success status"
        );
        tracing::debug!(url = %url, status = code, body = %super::log_util::log_body_preview(&body), "upstream error response body");
        let attempt_err = truncate_attempt_error(&body);
        attempts.push(ProxyAttempt {
            platform_id: route.platform.id,
            platform_name: route.platform.name.clone(),
            status_code: code as i32,
            error: attempt_err.clone(),
            duration_ms: attempt_start.elapsed().as_millis() as i64,
            ts: attempt_ts,
        });

        // 错误体提取人类可读 message（嵌套 error.message / 顶层 message），命中则 last_error
        // 与 429 分类都基于它；未命中回退 truncate_attempt_error 摘要 / body 原文。
        let extracted_msg = extract_error_message(&body);

        // 记本平台最近一次错误（卡片展示，非请求记录实时取）。本平台失败即覆盖，
        // 其自身下次成功时清空（commit_2xx）。换候选成功不清失败平台的 last_error。
        let last_error_detail = extracted_msg.clone().unwrap_or_else(|| attempt_err.clone());
        let _ = super::db::set_platform_last_error(
            &state.db, route.platform.id, Some(format!("HTTP {code}: {last_error_detail}")),
        ).await;

        // ── 429 分类（只看 message 文本，禁按 error.type）：配额耗尽 vs 限流 transient ──
        //   分类用于熔断计数（见下），不再触发 auto_disable：429 统一走 failover 换下个候选。
        let is_429_quota_exhausted = code == 429
            && classify_429(extracted_msg.as_deref().unwrap_or(&body));

        // ── 熔断计数：5xx 或 429-限流 计一次失败；401/403/402/429-配额/其他客户端 4xx 不计熔断（仅 inflight-1）。
        //   熔断与 auto_disabled 解耦：走 auto_disabled 的（401/403/402）不参与熔断。──
        if code >= 500 || (code == 429 && !is_429_quota_exhausted) {
            state.scheduler.record_failure(route.platform.id, breaker_th, super::db::now());
        } else {
            state.scheduler.record_ignored(route.platform.id);
        }

        // ── 自动禁用（指数退避，换下个候选）：仅 401/403 鉴权失败、402 余额不足 ──
        //   401/403/402 单次即禁用；429（无论配额耗尽还是限流）不再触发 auto_disable，
        //   统一按决策 A 走 failover 换下个候选。熔断仍按 classify_429 区分配额/限流（见上）。
        //   其它状态码（含 404/405/429）不自动禁用，仅按决策 A 走 failover 重试。
        if code == 401 || code == 403 || code == 402 {
            match super::db::set_platform_auto_disabled(&state.db, route.platform.id).await {
                Ok(until) if until > 0 => tracing::warn!(
                    platform = %route.platform.name, platform_id = route.platform.id, status = code,
                    auto_disabled_until = until, "platform auto-disabled (auth/balance)"
                ),
                Ok(_) => {} // 用户手动 disabled，不动
                Err(e) => tracing::error!(platform_id = route.platform.id, error = %e, "auto-disable platform failed"),
            }
        }

        // ── 中间件 error_rule 分类（出站）：按规则将上游错误分类为 retryable/non-retryable。
        //   non-retryable → 立即返回不换候选（用 override_status/body 若有）。
        //   retryable     → 走默认重试语义（换下个候选）。
        //   无命中        → 默认重试语义不变（is_last_candidate 决定）。
        //   熔断器不在本树：此处只产标记驱动现有重试循环，不引入任何熔断状态。──
        let err_class = {
            let mw_settings = state.settings_cache.read().await.middleware_settings.clone();
            state.middleware.classify_error(
                &mw_settings, code, &body,
                Some(&group.group_key), Some(route.platform.id as i64),
            )
        };
        // ── 决策 A：状态码硬错圈定 ──
        //   400 / 422（请求体本身非法）→ 不重试，直接返客户端（换平台无用，避免无谓遍历）。
        //   其余非 2xx（401/403/404/405/429/5xx/未知）→ 默认可重试（换下个候选）。
        //   400/422 的硬停优先于中间件 error_rule 的 retryable 分类（status 硬错语义不可被覆盖回可重试）。
        let status_retryable = is_status_retryable(code);
        // 中间件 error_rule：仅在 status 本身可重试时，允许其将错误显式降级为 non-retryable（缩小重试面）；
        //   不允许把硬错（400/422）反向放大为可重试。
        let mw_non_retryable = err_class.as_ref().map(|c| !c.retryable).unwrap_or(false);
        let non_retryable = !status_retryable || mw_non_retryable;
        if let Some(ref c) = err_class {
            tracing::info!(
                matched_by = %c.matched_by, category = %c.category, retryable = c.retryable,
                status = code, "middleware error_rule classified upstream error"
            );
        }
        if !status_retryable {
            tracing::info!(
                status = code, platform = %route.platform.name,
                "decision-A: hard request error (400/422), not retrying next platform"
            );
        }

        // 可重试（非 400/422 硬错 且 中间件未标 non-retryable）→ 换下个候选；
        // 候选耗尽 / 超 max_retries 则返回最后一次错误。non-retryable → 立即返回（不换候选）。
        if !non_retryable && !is_last_candidate {
            return AttemptOutcome::Next;
        }

        // ── 应用 error_rule override_status/body（若有）回客户端 ──
        let (out_code, out_body) = match err_class {
            Some(c) => (
                c.override_status.unwrap_or(code),
                c.override_body.unwrap_or_else(|| body.clone()),
            ),
            None => (code, body.clone()),
        };
        log.platform_id = route.platform.id;
        log.response_body = body.clone();
        log.status_code = out_code as i32;
        log.user_response_body = out_body.clone();
        log.user_response_headers = log.upstream_response_headers.clone();
        log.duration_ms = duration_ms as i32;
        log.retry_count = (attempts.len() as i32 - 1).max(0);
        log.attempts = std::mem::take(attempts);
        upsert_log(state, log, log_settings).await;
        let mut r = (StatusCode::from_u16(out_code).unwrap_or(StatusCode::BAD_GATEWAY), out_body).into_response();
        inject_trace_header(&mut r);
        AttemptOutcome::Respond(r)
}
