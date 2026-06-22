use super::*;

    fn rq_headers(pairs: &[(&str, &str)]) -> reqwest::header::HeaderMap {
        let mut m = reqwest::header::HeaderMap::new();
        for (k, v) in pairs {
            let name = reqwest::header::HeaderName::from_bytes(k.as_bytes()).unwrap();
            m.append(name, reqwest::header::HeaderValue::from_str(v).unwrap());
        }
        m
    }

    fn has(out: &[(axum::http::HeaderName, axum::http::HeaderValue)], name: &str) -> bool {
        out.iter().any(|(n, _)| n.as_str().eq_ignore_ascii_case(name))
    }
    fn val_of<'a>(
        out: &'a [(axum::http::HeaderName, axum::http::HeaderValue)],
        name: &str,
    ) -> Option<&'a str> {
        out.iter()
            .find(|(n, _)| n.as_str().eq_ignore_ascii_case(name))
            .and_then(|(_, v)| v.to_str().ok())
    }
    fn count_of(out: &[(axum::http::HeaderName, axum::http::HeaderValue)], name: &str) -> usize {
        out.iter().filter(|(n, _)| n.as_str().eq_ignore_ascii_case(name)).count()
    }

    #[test]
    fn is_stream_request_false_but_upstream_sse() {
        // 中转站对未声明 stream 的请求强制以 SSE 响应 → 必须判为流式（修复账目零 token bug）。
        assert!(resolve_is_stream(false, "text/event-stream"));
        assert!(resolve_is_stream(false, "text/event-stream; charset=utf-8"));
    }

    #[test]
    fn is_stream_request_true_kept() {
        // 既有正常流式路径不回归。
        assert!(resolve_is_stream(true, "application/json"));
        assert!(resolve_is_stream(true, "text/event-stream"));
    }

    #[test]
    fn is_stream_non_stream_json() {
        // 非流式 JSON 响应保持非流式（走 JSON usage 解析路径）。
        assert!(!resolve_is_stream(false, "application/json"));
        assert!(!resolve_is_stream(false, ""));
    }

    // ── 决策 A：failover 重试状态码圈定 is_status_retryable ──

    #[test]
    fn retry_hard_request_errors_not_retried() {
        // 400 / 422：请求体本身非法，换平台也没用 → 不重试，直接返客户端。
        assert!(!is_status_retryable(400));
        assert!(!is_status_retryable(422));
    }

    #[test]
    fn retry_auth_dead_endpoint_retried() {
        // 401/403（鉴权→auto_disabled）、404/405（端点/方法错，仅 failover 不禁用）均重试下一平台。
        assert!(is_status_retryable(401));
        assert!(is_status_retryable(403));
        assert!(is_status_retryable(404));
        assert!(is_status_retryable(405));
    }

    #[test]
    fn retry_rate_limit_and_server_errors_retried() {
        // 429（限流/配额，换平台可能成功）+ 所有 5xx（上游故障）→ 重试。
        assert!(is_status_retryable(429));
        assert!(is_status_retryable(500));
        assert!(is_status_retryable(502));
        assert!(is_status_retryable(503));
        assert!(is_status_retryable(504));
        assert!(is_status_retryable(599));
    }

    #[test]
    fn retry_other_4xx_retried_conservatively() {
        // 其余未列举 4xx（非 400/422 硬错）保守重试，不误把上游临时拒绝当硬错。
        assert!(is_status_retryable(408)); // request timeout
        assert!(is_status_retryable(409)); // conflict
        assert!(is_status_retryable(425)); // too early
        assert!(is_status_retryable(402)); // 注：本路径不含 manual budget 402（那条在 forward 前短路返回）
    }

    // ── 决策 B：流式 200 首块判定 classify_stream_first ──

    #[test]
    fn peek_anthropic_message_start_is_meaningful() {
        let text = "event: message_start\ndata: {\"type\":\"message_start\",\"message\":{\"id\":\"m1\"}}\n\n";
        assert_eq!(classify_stream_first(text, false), StreamPeek::Meaningful);
    }

    #[test]
    fn peek_openai_choices_delta_is_meaningful() {
        let text = "data: {\"choices\":[{\"delta\":{\"content\":\"hi\"}}]}\n\n";
        assert_eq!(classify_stream_first(text, false), StreamPeek::Meaningful);
    }

    #[test]
    fn peek_immediate_done_is_empty() {
        // 任何内容前先 [DONE] → 空响应，应重试。
        assert_eq!(classify_stream_first("data: [DONE]\n\n", false), StreamPeek::EmptyOrError);
    }

    #[test]
    fn peek_event_error_is_empty() {
        // event: error 行即判错（无需等 data）。
        let text = "event: error\ndata: {\"type\":\"error\",\"error\":{\"message\":\"boom\"}}\n\n";
        assert_eq!(classify_stream_first(text, false), StreamPeek::EmptyOrError);
    }

    #[test]
    fn peek_error_json_is_empty() {
        // 无 event 行、直接 data 为 error 结构 → 失败。
        let text = "data: {\"error\":{\"message\":\"bad\"}}\n\n";
        assert_eq!(classify_stream_first(text, false), StreamPeek::EmptyOrError);
        let text2 = "data: {\"type\":\"error\",\"message\":\"bad\"}\n\n";
        assert_eq!(classify_stream_first(text2, false), StreamPeek::EmptyOrError);
    }

    #[test]
    fn peek_keepalive_only_needs_more() {
        // 仅 SSE 注释 / event 名行 / 空行 → 尚不足以判定，继续缓冲。
        assert_eq!(classify_stream_first(": ping\n\n", false), StreamPeek::NeedMore);
        assert_eq!(classify_stream_first("event: message_start\n", false), StreamPeek::NeedMore);
        assert_eq!(classify_stream_first("", false), StreamPeek::NeedMore);
    }

    #[test]
    fn peek_partial_json_frame_needs_more() {
        // data 帧 JSON 跨 chunk 截断（尚不可解析）→ 等更多，不误判。
        let text = "data: {\"choices\":[{\"delta\":{\"cont";
        assert_eq!(classify_stream_first(text, false), StreamPeek::NeedMore);
    }

    #[test]
    fn peek_stream_ended_no_content_is_empty() {
        // 流秒断 / 空 body（结束时仍无有效内容事件）→ 空响应，重试。
        assert_eq!(classify_stream_first("", true), StreamPeek::EmptyOrError);
        assert_eq!(classify_stream_first(": ping\n\n", true), StreamPeek::EmptyOrError);
    }

    // ── 决策 B：非流式 200 body 有效性 is_nonstream_body_valid ──

    #[test]
    fn nonstream_empty_body_invalid() {
        assert!(!is_nonstream_body_valid(""));
        assert!(!is_nonstream_body_valid("   "));
    }

    #[test]
    fn nonstream_error_body_invalid() {
        assert!(!is_nonstream_body_valid("{\"error\":{\"message\":\"bad\"}}"));
        assert!(!is_nonstream_body_valid("{\"type\":\"error\",\"message\":\"x\"}"));
    }

    #[test]
    fn nonstream_valid_openai_and_anthropic() {
        assert!(is_nonstream_body_valid(
            "{\"choices\":[{\"message\":{\"role\":\"assistant\",\"content\":\"hi\"}}]}"
        ));
        assert!(is_nonstream_body_valid(
            "{\"content\":[{\"type\":\"text\",\"text\":\"hi\"}],\"role\":\"assistant\"}"
        ));
    }

    #[test]
    fn nonstream_empty_choices_or_content_invalid() {
        // 200 但 choices/content 为空数组 → 无实质内容，重试。
        assert!(!is_nonstream_body_valid("{\"choices\":[]}"));
        assert!(!is_nonstream_body_valid("{\"content\":[]}"));
    }

    #[test]
    fn nonstream_non_json_treated_valid() {
        // 非 JSON 但有内容（上游非标准 200）→ 保守视为有效，不误杀。
        assert!(is_nonstream_body_valid("plain text response"));
    }

    #[test]
    fn filter_resp_drops_blacklist() {
        let src = rq_headers(&[
            ("content-encoding", "gzip"),
            ("content-length", "123"),
            ("transfer-encoding", "chunked"),
            ("connection", "keep-alive"),
            ("keep-alive", "timeout=5"),
            ("date", "Tue, 17 Jun 2026 00:00:00 GMT"),
        ]);
        let out = filter_upstream_resp_headers(&src, false);
        assert!(!has(&out, "content-encoding"));
        assert!(!has(&out, "content-length"));
        assert!(!has(&out, "transfer-encoding"));
        assert!(!has(&out, "connection"));
        assert!(!has(&out, "keep-alive"));
        // 非黑名单保留
        assert!(has(&out, "date"));
    }

    #[test]
    fn filter_resp_keeps_business_headers() {
        let src = rq_headers(&[
            ("date", "Tue, 17 Jun 2026 00:00:00 GMT"),
            ("x-log-id", "abc123"),
            ("x-process-time", "0.042"),
            ("vary", "Accept-Encoding"),
            ("set-cookie", "sid=1; Path=/"),
            ("content-type", "application/json; charset=utf-8"),
        ]);
        let out = filter_upstream_resp_headers(&src, false);
        assert_eq!(val_of(&out, "date"), Some("Tue, 17 Jun 2026 00:00:00 GMT"));
        assert_eq!(val_of(&out, "x-log-id"), Some("abc123"));
        assert_eq!(val_of(&out, "x-process-time"), Some("0.042"));
        assert_eq!(val_of(&out, "vary"), Some("Accept-Encoding"));
        assert_eq!(val_of(&out, "set-cookie"), Some("sid=1; Path=/"));
        assert_eq!(val_of(&out, "content-type"), Some("application/json; charset=utf-8"));
    }

    #[test]
    fn filter_resp_keeps_multiple_set_cookie() {
        let src = rq_headers(&[
            ("set-cookie", "a=1; Path=/"),
            ("set-cookie", "b=2; Path=/"),
            ("set-cookie", "c=3; Path=/"),
        ]);
        let out = filter_upstream_resp_headers(&src, false);
        assert_eq!(count_of(&out, "set-cookie"), 3, "多值 set-cookie 不得丢值");
    }

    #[test]
    fn filter_resp_blacklist_case_insensitive() {
        let src = rq_headers(&[
            ("Content-Encoding", "gzip"),
            ("Transfer-Encoding", "chunked"),
            ("X-Log-Id", "keep-me"),
        ]);
        let out = filter_upstream_resp_headers(&src, false);
        assert!(!has(&out, "content-encoding"), "大小写混合仍须剔除");
        assert!(!has(&out, "transfer-encoding"));
        assert!(has(&out, "x-log-id"));
    }

    #[test]
    fn filter_resp_stream_drops_sse_self_managed() {
        let src = rq_headers(&[
            ("content-type", "application/json"),
            ("cache-control", "max-age=60"),
            ("connection", "close"),
            ("x-log-id", "from-upstream"),
            ("date", "Tue, 17 Jun 2026 00:00:00 GMT"),
        ]);
        let out = filter_upstream_resp_headers(&src, true);
        // SSE 自管头不得来自上游
        assert!(!has(&out, "content-type"));
        assert!(!has(&out, "cache-control"));
        assert!(!has(&out, "connection"));
        // 透传价值头仍保留
        assert_eq!(val_of(&out, "x-log-id"), Some("from-upstream"));
        assert!(has(&out, "date"));
    }

    #[test]
    fn filter_resp_stream_sse_headers_take_self_managed_values() {
        // 模拟流式实发头组装：SSE 三自管头 + filter(is_stream=true)
        let src = rq_headers(&[
            ("content-type", "application/json"),  // 上游值，须被 SSE 自管覆盖
            ("cache-control", "max-age=60"),
            ("connection", "close"),
            ("x-log-id", "from-upstream"),
        ]);
        let sse: [(axum::http::HeaderName, axum::http::HeaderValue); 3] = [
            (axum::http::header::CONTENT_TYPE, axum::http::HeaderValue::from_static("text/event-stream")),
            (axum::http::header::CACHE_CONTROL, axum::http::HeaderValue::from_static("no-cache")),
            (axum::http::header::CONNECTION, axum::http::HeaderValue::from_static("keep-alive")),
        ];
        let mut all: Vec<(axum::http::HeaderName, axum::http::HeaderValue)> = sse.to_vec();
        all.extend(filter_upstream_resp_headers(&src, true));
        assert_eq!(val_of(&all, "content-type"), Some("text/event-stream"));
        assert_eq!(val_of(&all, "cache-control"), Some("no-cache"));
        assert_eq!(val_of(&all, "connection"), Some("keep-alive"));
        assert_eq!(val_of(&all, "x-log-id"), Some("from-upstream"));
    }

    #[test]
    fn resp_headers_log_json_first_value_and_format() {
        let headers = vec![
            (axum::http::header::CONTENT_TYPE, axum::http::HeaderValue::from_static("application/json")),
            (
                axum::http::HeaderName::from_static("x-log-id"),
                axum::http::HeaderValue::from_static("xyz"),
            ),
        ];
        let json = resp_headers_to_log_json(&headers);
        let v: Value = serde_json::from_str(&json).unwrap();
        assert_eq!(v.get("content-type").and_then(|x| x.as_str()), Some("application/json"));
        assert_eq!(v.get("x-log-id").and_then(|x| x.as_str()), Some("xyz"));
    }

    #[test]
    fn nonstream_response_has_single_content_type() {
        // 复现非流式 2xx 成功路径头组装：(StatusCode, Vec<u8>).into_response() 默认写死
        // content-type: application/octet-stream，须先 remove 再 extend，避免重复 content-type。
        use axum::response::IntoResponse;
        let src = rq_headers(&[
            ("content-type", "application/json; charset=utf-8"),
            ("x-log-id", "abc"),
        ]);
        let filtered = filter_upstream_resp_headers(&src, false);
        let mut response = (StatusCode::OK, b"{}".to_vec()).into_response();
        response
            .headers_mut()
            .remove(axum::http::header::CONTENT_TYPE);
        response.headers_mut().extend(filtered);
        let cts: Vec<_> = response
            .headers()
            .get_all(axum::http::header::CONTENT_TYPE)
            .iter()
            .map(|v| v.to_str().unwrap().to_string())
            .collect();
        assert_eq!(cts, vec!["application/json; charset=utf-8".to_string()], "须单一 content-type 取上游真实值");
        assert!(response.headers().contains_key("x-log-id"));
    }

    // ── 透传 URL 拼接：base_url(host 根) + 客户端原始 path(+query) ──

