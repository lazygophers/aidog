use super::*;
use std::sync::atomic::{AtomicU64, Ordering};

/// mock error_rate 命中判定：进程级请求计数器 + 乘法哈希打散，确定性伪随机（压测场景需可复现，故不引 rand crate）。
/// ponytail: 全局原子计数器 + 乘法哈希（splitmix64 常数）取模，分布均匀性弱于真随机数生成器，
/// 但避免了"每 SCALE 个请求里前 N 个连续命中"的突发簇集（纯取模无打散时会这样）；
/// 若未来需要跨进程可复现的独立种子控制，换 rand::SeedableRng 显式播种。
static MOCK_ERROR_COUNTER: AtomicU64 = AtomicU64::new(0);

fn mock_error_hit(error_rate: f64) -> bool {
    const SCALE: u64 = 10_000;
    let threshold = (error_rate.clamp(0.0, 1.0) * SCALE as f64) as u64;
    let n = MOCK_ERROR_COUNTER.fetch_add(1, Ordering::Relaxed);
    let scrambled = n.wrapping_mul(0x9E3779B97F4A7C15); // splitmix64 常数，打散连续计数器
    (scrambled % SCALE) < threshold
}

/// Mock 平台请求处理：本地生成可控假响应（非流式 JSON / 流式 SSE），填假 token 进 log。
#[allow(clippy::too_many_arguments)]
pub(crate) async fn handle_mock(
    state: Arc<ProxyState>,
    mut log: ProxyLog,
    log_settings: ProxyLogSettings,
    extra: &str,
    chat_req: &ChatRequest,
    req_value: &Value,
    source_protocol: &Protocol,
    requested_model: &str,
    is_stream: bool,
    start: std::time::Instant,
) -> Response {
    use aidog_adapter::mock;

    let cfg = mock::resolve_mock_config(extra, chat_req, req_value);

    // 真延迟（首包 TTFT，缺省回落 delay_ms）
    let ttft_ms = cfg.ttft_ms.unwrap_or(cfg.delay_ms);
    if ttft_ms > 0 {
        tokio::time::sleep(std::time::Duration::from_millis(ttft_ms)).await;
    }

    // 填假 token（最终生效值）
    log.input_tokens = cfg.input_tokens;
    log.output_tokens = cfg.output_tokens;
    log.cache_tokens = cfg.cache_tokens;

    // ── 错误 / 超时模拟 ──
    // error_rate 设置时先判概率命中，未命中本轮按 "none" 走成功路径；未设置 error_rate 时行为不变（每次都判 error_mode）。
    let effective_error_mode: &str = match cfg.error_rate {
        Some(rate) if !mock_error_hit(rate) => "none",
        _ => cfg.error_mode.as_str(),
    };
    match effective_error_mode {
        "http_error" => {
            tracing::warn!(platform_id = log.platform_id, status = cfg.status_code, "mock error_mode=http_error");
            let body = mock::build_error_body(source_protocol, cfg.status_code, "mock http_error");
            let body_str = serde_json::to_string(&body).unwrap_or_default();
            let status = StatusCode::from_u16(cfg.status_code).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
            log.status_code = cfg.status_code as i32;
            log.duration_ms = start.elapsed().as_millis() as i32;
            log.response_body = body_str.clone();
            log.user_response_body = body_str.clone();
            log.user_response_headers = r#"{"content-type":"application/json"}"#.to_string();
            upsert_log(&state, &log, &log_settings).await;
            return {
                let mut r = (status, [(axum::http::header::CONTENT_TYPE, "application/json")], body_str).into_response();
                inject_trace_header(&mut r);
                r
            };
        }
        "rate_limit_429" => {
            tracing::warn!(platform_id = log.platform_id, "mock error_mode=rate_limit_429 (429)");
            let body = mock::build_error_body(source_protocol, 429, "mock rate limit");
            let body_str = serde_json::to_string(&body).unwrap_or_default();
            log.status_code = 429;
            log.duration_ms = start.elapsed().as_millis() as i32;
            log.response_body = body_str.clone();
            log.user_response_body = body_str.clone();
            log.user_response_headers = r#"{"content-type":"application/json","retry-after":"5"}"#.to_string();
            upsert_log(&state, &log, &log_settings).await;
            return {
                let mut r = (
                    StatusCode::TOO_MANY_REQUESTS,
                    [
                        (axum::http::header::CONTENT_TYPE, "application/json"),
                        (axum::http::header::RETRY_AFTER, "5"),
                    ],
                    body_str,
                )
                    .into_response();
                inject_trace_header(&mut r);
                r
            };
        }
        "timeout" => {
            tracing::warn!(platform_id = log.platform_id, "mock error_mode=timeout (will sleep then 504)");
            // sleep 上限保护，不真 hang 连接
            tokio::time::sleep(std::time::Duration::from_secs(600)).await;
            let body = mock::build_error_body(source_protocol, 504, "mock timeout");
            let body_str = serde_json::to_string(&body).unwrap_or_default();
            log.status_code = 504;
            log.duration_ms = start.elapsed().as_millis() as i32;
            log.response_body = body_str.clone();
            log.user_response_body = body_str.clone();
            log.user_response_headers = r#"{"content-type":"application/json"}"#.to_string();
            upsert_log(&state, &log, &log_settings).await;
            return {
                let mut r = (StatusCode::GATEWAY_TIMEOUT, [(axum::http::header::CONTENT_TYPE, "application/json")], body_str)
                    .into_response();
                inject_trace_header(&mut r);
                r
            };
        }
        _ => {}
    }

    // 手动预算扣减（mock 也按用量预估扣减，与上游平台一致；仅成功路径，错误模式上方已 return）
    let mb_total = (log.input_tokens + log.output_tokens + log.cache_tokens) as f64;
    if mb_total > 0.0 {
        // mock 无 platform.extra / 无 preset 默认 → peak_hours multiplier 恒 1.0；
        // 传 0 / 0 跳过 peak_hours 查询（calc_est_cost 早退）。
        let now_ms = aidog_db::now();
        let est = crate::gateway::billing::calc_est_cost(&state.db, &log.actual_model, "mock", log.input_tokens, log.output_tokens, log.cache_tokens, 0, now_ms).await;
        let _ = super::manual_budget::apply_manual_budgets(&state.db, log.platform_id, est, mb_total, now_ms).await;
    }

    // stream_override 优先于请求 is_stream
    let stream = cfg.stream_override.unwrap_or(is_stream);

    if stream {
        let chunks = mock::build_sse_chunks(&cfg, source_protocol, requested_model);
        let inter_chunk_ms = cfg.inter_chunk_ms.unwrap_or(cfg.delay_ms);
        let body_stream = futures::stream::iter(chunks.into_iter().map(Ok::<_, std::io::Error>))
            .then(move |item| async move {
                if inter_chunk_ms > 0 {
                    tokio::time::sleep(std::time::Duration::from_millis(inter_chunk_ms)).await;
                }
                item
            });
        let body = Body::from_stream(body_stream);

        log.status_code = 200;
        log.duration_ms = start.elapsed().as_millis() as i32;
        log.response_body = "[mock stream]".to_string();
        log.user_response_body = "[mock stream]".to_string();
        log.user_response_headers = r#"{"content-type":"text/event-stream","cache-control":"no-cache","connection":"keep-alive"}"#.to_string();
        upsert_log(&state, &log, &log_settings).await;

        return {
            let mut r = (
                StatusCode::OK,
                [
                    (axum::http::header::CONTENT_TYPE, "text/event-stream"),
                    (axum::http::header::CACHE_CONTROL, "no-cache"),
                    (axum::http::header::CONNECTION, "keep-alive"),
                ],
                body,
            )
                .into_response();
            inject_trace_header(&mut r);
            r
        };
    }

    // 非流式 JSON
    let resp_body = mock::build_response(&cfg, source_protocol, requested_model);
    let body_str = serde_json::to_string(&resp_body).unwrap_or_default();
    let status = StatusCode::from_u16(cfg.status_code).unwrap_or(StatusCode::OK);
    log.status_code = cfg.status_code as i32;
    log.duration_ms = start.elapsed().as_millis() as i32;
    log.response_body = body_str.clone();
    log.user_response_body = body_str.clone();
    log.user_response_headers = r#"{"content-type":"application/json"}"#.to_string();
    upsert_log(&state, &log, &log_settings).await;

    let mut r = (status, [(axum::http::header::CONTENT_TYPE, "application/json")], body_str).into_response();
    inject_trace_header(&mut r);
    r
}

#[cfg(test)]
mod test_error_rate {
    use super::mock_error_hit;

    /// error_rate=0.05 跑 2000 次，命中比例应落在 5%±3%（2%~8%）内。
    #[test]
    fn error_rate_hit_ratio_within_tolerance() {
        let hits = (0..2000).filter(|_| mock_error_hit(0.05)).count();
        let ratio = hits as f64 / 2000.0;
        assert!((0.02..=0.08).contains(&ratio), "hit ratio {ratio} out of [0.02, 0.08]");
    }

    #[test]
    fn error_rate_zero_never_hits() {
        assert!((0..500).all(|_| !mock_error_hit(0.0)));
    }

    #[test]
    fn error_rate_one_always_hits() {
        assert!((0..500).all(|_| mock_error_hit(1.0)));
    }
}
