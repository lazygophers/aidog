use super::*;

/// 非流式 2xx 成功响应处理：usage 提取 + 跨协议转换 + 模型回填 + 出站中间件 + 响应头透传。
/// commit_2xx_success! 已在调用方执行（log.attempts 已填充）。返回最终客户端响应。
#[allow(clippy::too_many_arguments)]
pub(crate) async fn finish_nonstream(
    state: &Arc<ProxyState>,
    log: &mut ProxyLog,
    log_settings: &ProxyLogSettings,
    group: &Group,
    route: &RouteResult,
    source_protocol: &str,
    requested_model: &str,
    actual_model: &str,
    eff_api_key: &str,
    target_protocol_enum: &Protocol,
    same_protocol_passthrough: bool,
    needs_model_remap: bool,
    coding_plan: bool,
    // 校准/预估链路用的 base_url：endpoint 真 base_url（coding plan 平台级 base_url 恒空，
    // 用它 dispatch query_quota 子串匹配才命中）。空则回退平台级（等价现状）。
    quota_base_url: String,
    upstream_resp_headers: &reqwest::header::HeaderMap,
    start: std::time::Instant,
    body: Bytes,
) -> Response {
    let quota_base_url = if quota_base_url.trim().is_empty() {
        route.platform.base_url.clone()
    } else {
        quota_base_url
    };
    // usage 借用：lossy 不经 to_string 中转
        let (input_tokens, output_tokens, cache_tokens) =
            extract_usage(String::from_utf8_lossy(&body).as_ref());

        // ── record gate（与 finish_stream :186-187 对称）：上游侧 body 受 log_upstream_request，
        //   客户端侧 body 受 log_user_request。body 先不分配——gate 开才走 cap_nonstream_body 截断 + 落库。──
        let record_upstream_body = log_settings.enabled && log_settings.log_upstream_request;
        let upstream_body_str: String = if record_upstream_body {
            cap_nonstream_body(&body)
        } else {
            String::new()
        };

        log.response_body = upstream_body_str;
        log.status_code = 200;
        log.duration_ms = start.elapsed().as_millis() as i32;
        log.input_tokens = input_tokens;
        log.output_tokens = output_tokens;
        log.cache_tokens = cache_tokens;

        // ── 非流式跨协议响应转换 ──
        // 流式路径靠 parse_sse→to_client_sse 转换响应格式，但非流式分支历史上**直接透传上游 body**，
        // 致 source≠target 且非同协议透传时（如 anthropic 客户端 ↔ openai 平台），CC 收到上游原生
        // openai chat completion JSON（含 tool_calls）而非 anthropic messages → "empty or malformed (200)"。
        // 这里补齐：同协议透传跳过；否则按 (wire=target, client=source) 转换。返回 None 表示无需转换，透传原文。
        let body = if !same_protocol_passthrough {
            let upstream_json: Value = serde_json::from_slice(&body).unwrap_or(Value::Null);
            match adapter::convert_response(
                &upstream_json,
                target_protocol_enum,
                source_protocol,
                requested_model,
            ) {
                Some(converted) => serde_json::to_vec(&converted).unwrap_or_else(|_| body.to_vec()),
                None => body.to_vec(),
            }
        } else {
            body.to_vec()
        };
        let body = Bytes::from(body);

        // Replace model in response back to original if remapped
        let body = if needs_model_remap {
            replace_model_in_json(&body, requested_model)
        } else {
            body.to_vec()
        };

        // ── 中间件出站规则（非流式 2xx）：response_override/redaction/content_filter 改写 body。
        //   在 usage 提取后改写（脱敏不影响计费/统计）；与入站脱敏幂等。
        //   总开关/子开关 OFF 时为 no-op。error_rule 不在此（仅非 2xx 路径分类）。──
        let body = {
            let mut s = String::from_utf8_lossy(&body).to_string();
            let mw_settings = state.settings_cache.read().await.middleware_settings.clone();
            state.middleware.apply_outbound(
                &mw_settings, &mut s,
                Some(&group.group_key), Some(route.platform.id as i64),
            );
            s.into_bytes()
        };
        // 客户端侧 body gate（受 log_user_request）+ 16MB cap
        let record_client_body = log_settings.enabled && log_settings.log_user_request;
        log.user_response_body = if record_client_body {
            cap_nonstream_body(&body)
        } else {
            String::new()
        };

        // ── 透传上游响应头（黑名单剔除 content-encoding/content-length/hop-by-hop）──
        let mut filtered = filter_upstream_resp_headers(upstream_resp_headers, false);
        // 上游缺 content-type 时回退默认 application/json
        if !filtered
            .iter()
            .any(|(n, _)| n == axum::http::header::CONTENT_TYPE)
        {
            filtered.push((
                axum::http::header::CONTENT_TYPE,
                axum::http::HeaderValue::from_static("application/json"),
            ));
        }
        // 日志字段 = 实际发回客户端的头集合（不再写死 content-type）
        log.user_response_headers = resp_headers_to_log_json(&filtered);

        tracing::info!(
            platform = %route.platform.name, model = %actual_model, status = 200, stream = false,
            duration_ms = log.duration_ms, input_tokens, output_tokens, cache_tokens,
            "request completed"
        );
        upsert_log(state, log, log_settings).await;

        // ── 请求驱动预估（后台，不阻塞响应）──
        spawn_estimate(
            state,
            route.platform.id,
            &route.platform.platform_type,
            quota_base_url,
            eff_api_key.to_string(),
            actual_model.to_string(),
            route.platform.extra.clone(),
            input_tokens,
            output_tokens,
            cache_tokens,
            coding_plan,
            tracing::Span::current(),
        );

        let mut response = (StatusCode::OK, body.to_vec()).into_response();
        // into_response 对 Vec<u8> 写死 content-type: application/octet-stream；
        // HeaderMap::extend 用 append 语义，直接 extend 会产生重复 content-type（octet-stream + 真实值）。
        // 故先 remove 默认 content-type，再 extend（filtered 已含真实 content-type 或回退 application/json）。
        response
            .headers_mut()
            .remove(axum::http::header::CONTENT_TYPE);
        response.headers_mut().extend(filtered);
        inject_trace_header(&mut response);
        response
}

/// 流式 2xx 成功响应处理：peek 已确认有内容，此处构建 StreamLogGuard + SSE relay/转换闭包。
/// commit_2xx_success! 已在调用方执行（log.attempts 已填充）。upstream_stream 为 peek 后剩余流，
/// peek_buf 为已缓冲首批 chunk（prepend 回流）。返回 SSE 流式响应。
#[allow(clippy::too_many_arguments)]
pub(crate) async fn finish_stream<S>(
    upstream_stream: S,
    peek_buf: Vec<Bytes>,
    state: &Arc<ProxyState>,
    log: &mut ProxyLog,
    log_settings: &ProxyLogSettings,
    group: &Group,
    route: &RouteResult,
    source_protocol: &str,
    requested_model: &str,
    actual_model: &str,
    eff_api_key: &str,
    target_protocol_enum: &Protocol,
    same_protocol_passthrough: bool,
    needs_model_remap: bool,
    coding_plan: bool,
    // 校准/预估链路用的 base_url：endpoint 真 base_url（见 finish_nonstream 注）。空则回退平台级。
    quota_base_url: String,
    upstream_resp_headers: &reqwest::header::HeaderMap,
    start: std::time::Instant,
) -> Response
where
    S: futures::Stream<Item = reqwest::Result<Bytes>> + Unpin + Send + 'static,
{

    let quota_base_url = if quota_base_url.trim().is_empty() {
        route.platform.base_url.clone()
    } else {
        quota_base_url
    };

    // 流式：转换 SSE 格式为 Anthropic 格式返回
    // 同协议透传时（passthrough_response），下方闭包内原样 relay 上游 SSE，仅提取 usage。
    let passthrough_response = same_protocol_passthrough;
    let protocol = target_protocol_enum.clone();
    let client_protocol = source_protocol.to_string();
    let model_for_sse = requested_model.to_string();
    let model_for_response = if needs_model_remap {
        requested_model.to_string()
    } else {
        String::new()
    };

    // ── 中间件出站流式逐块改写上下文：在构建 stream 闭包前读取 settings（闭包在 req span 外轮询，
    //   不可再 await DB）。引擎 Arc clone 进闭包，每 chunk 文本应用 mask/override/sensitive。
    //   error 已由上游 HTTP 状态码在 forward 后判定（非 2xx 不会走到这里，故流式无需再判 error）。──
    let mw_engine = state.middleware.clone();
    let mw_settings = state.settings_cache.read().await.middleware_settings.clone();
    let mw_active = mw_settings.enabled;
    let mw_group = group.group_key.clone();
    let mw_platform_id = route.platform.id as i64;

    // ── 旁路聚合器：累积 token + 上游 SSE 原文 + 转换后下发客户端的 SSE。
    // 闭包内对其加同步锁是短临界区（push），禁持锁跨 await（闭包本身同步，不 await）。──
    let agg = Arc::new(StreamAggregator::new());
    let est_fired = Arc::new(std::sync::atomic::AtomicBool::new(false));

    // 闭包由 axum 在 req span 外轮询（Response 返回后），故此处捕获当前 req span 链回 trace_id。
    let req_span = tracing::Span::current();

    // ── body 记录受 ProxyLogSettings 开关控制：仅相应开关开启才聚合，零开关时不耗内存。
    // OOM 止血：response_body(上游) 改受 log_upstream_request 同侧控制（默认关 → 流式不累积上游原文，
    // 内存占用与开关语义一致；upstream response_body 仍按 settings 二次脱敏写库）。
    // user_response_body 受 log_user_request 控制。master switch(enabled) 仍由 upsert_log 早退兜底。──
    let record_upstream_body = log_settings.enabled && log_settings.log_upstream_request;
    let record_client_body = log_settings.enabled && log_settings.log_user_request;

    // ── 最终回写 guard：[DONE] 正常结束 或 客户端断连 Drop 时回写聚合 token/body（幂等）。──
    let guard = StreamLogGuard {
        agg: agg.clone(),
        est_fired: est_fired.clone(),
        log: log.clone(),
        state: state.clone(),
        settings: log_settings.clone(),
        start,
        record_upstream_body,
        record_client_body,
        req_span: req_span.clone(),
        est: Some(StreamEstCtx {
            platform_id: route.platform.id,
            platform_type: route.platform.platform_type.clone(),
            base_url: quota_base_url,
            api_key: eff_api_key.to_string(),
            model: actual_model.to_string(),
            extra: route.platform.extra.clone(),
            coding_plan,
        }),
    };

    // guard 被 move 进闭包，随 stream 生命周期存活；stream 被 Drop（含客户端断连）时 guard.drop 触发兜底 flush。
    // 决策 B：把 peek 阶段已缓冲的首批 chunk 原样 prepend 回流（不能吞首块），再接上游剩余流；
    // 下游闭包对缓冲块与后续块一视同仁（token 聚合 / 转换 / finalize 不受影响）。
    let buffered_head = futures::stream::iter(
        peek_buf.into_iter().map(Ok::<Bytes, reqwest::Error>),
    );
    let upstream_rest = buffered_head.chain(upstream_stream);
    let stream = upstream_rest.map(move |chunk_result| {
        let chunk = match chunk_result {
            Ok(c) => c,
            Err(e) => {
                // 上游流中途断裂（如 GLM ~60s 截断）：不向客户端报错，仅记日志 +
                // 按客户端协议合成干净的 Stop 终止事件收尾，已输出内容保留。
                // （不再注入 `event: error`，避免 CC 显示 "API Error: error decoding response body"。）
                tracing::warn!(error = %e, "SSE upstream stream chunk error; closing stream gracefully");
                let stop = adapter::to_client_sse(&ChatStreamEvent::Stop {
                    finish_reason: Some("end_turn".to_string()),
                }, &client_protocol, &model_for_sse).unwrap_or_default();
                return Ok::<_, std::io::Error>(Bytes::from(stop));
            }
        };

        // 旁路累积上游响应原文（受 master 开关控制；锁为同步短临界区）
        if record_upstream_body
            && let Ok(mut up) = guard.agg.upstream_body.lock() {
                up.push(chunk.clone());
            }

        let text = String::from_utf8_lossy(&chunk);

        // ── 同协议透传：跳过 parse_sse→to_client_sse 格式转换，原样 relay 上游 SSE 字节。
        // usage 提取仍保留（accumulate_sse_usage），est_cost / 统计不丢。
        // 注意：透传下不改写响应 model 字段（保持上游原文，与请求体 model=actual_model 一致）。──
        let out_bytes = if passthrough_response {
            // 跨 chunk 行重组后累计 usage（逐 chunk .lines() 会因 data: 行被切断而丢 usage）。
            guard.agg.feed_sse_usage(&text);
            chunk.clone()
        } else {
            // token 累计走跨 chunk 行重组（逐 chunk .lines() 会因 data: 行被切断丢 usage）。
            // 协议转换仍逐 chunk 处理（输出格式转换路径，行为不变）。
            guard.agg.feed_sse_usage(&text);
            let mut output = String::new();
            for line in text.lines() {
                if let Some(data) = line.strip_prefix("data: ") {
                    if data.trim() == "[DONE]" {
                        output.push_str(&adapter::to_client_sse(&ChatStreamEvent::Stop {
                            finish_reason: Some("end_turn".to_string()),
                        }, &client_protocol, &model_for_sse).unwrap_or_default());
                        continue;
                    }

                    if let Ok(json) = serde_json::from_str::<Value>(data)
                        && let Some(event) = adapter::parse_sse(&json, &protocol) {
                            let event = if !model_for_response.is_empty() {
                                match event {
                                    ChatStreamEvent::Start { id, model: _ } => ChatStreamEvent::Start {
                                        id,
                                        model: model_for_response.clone(),
                                    },
                                    other => other,
                                }
                            } else {
                                event
                            };
                            if let Some(sse) = adapter::to_client_sse(&event, &client_protocol, &model_for_sse) {
                                output.push_str(&sse);
                            }
                        }
                }
            }
            Bytes::from(output)
        };

        // ── 中间件出站流式逐块改写：对下发客户端的 chunk 文本应用 mask/override/sensitive。
        //   逐块正则替换；跨 chunk 边界的密钥/敏感词可能漏匹配（已知限制，滑窗后续）。
        //   总开关 OFF 时跳过。在记录 client_body 前改写，确保审计与下发一致（脱敏后版本）。──
        let out_bytes = if mw_active && !out_bytes.is_empty() {
            let original = String::from_utf8_lossy(&out_bytes);
            let rewritten = mw_engine.apply_outbound_stream_chunk(
                &mw_settings, &original, Some(&mw_group), Some(mw_platform_id),
            );
            if rewritten == original.as_ref() {
                out_bytes
            } else {
                Bytes::from(rewritten)
            }
        } else {
            out_bytes
        };

        // 旁路累积下发客户端的 SSE（受 log_user_request 开关控制）
        if record_client_body && !out_bytes.is_empty()
            && let Ok(mut cl) = guard.agg.client_body.lock() {
                cl.push(out_bytes.clone());
            }
        // 正常结束：本 chunk 含 [DONE] 即触发 flush（token 已累加完整）；否则由断连 Drop 兜底。
        // flush 幂等（est_fired 守卫），[DONE] 与 Drop 二者只生效一次。flush 内仅 tokio::spawn，不阻塞转发。
        guard.flush_if_done(&text);

        Ok(out_bytes)
    });

    let body = Body::from_stream(stream);

    // Upsert（返回 stream 前的占位）：标记流进行中，token=0、body 占位；
    // 最终态由 guard.flush（[DONE] 或断连 Drop）覆盖。
    // ── SSE 三自管头（content-type/cache-control/connection）+ 叠加筛选上游头（is_stream=true 额外剔这三者，防上游覆盖）──
    let sse_self_managed: [(axum::http::HeaderName, axum::http::HeaderValue); 3] = [
        (axum::http::header::CONTENT_TYPE, axum::http::HeaderValue::from_static("text/event-stream")),
        (axum::http::header::CACHE_CONTROL, axum::http::HeaderValue::from_static("no-cache")),
        (axum::http::header::CONNECTION, axum::http::HeaderValue::from_static("keep-alive")),
    ];
    let stream_filtered = filter_upstream_resp_headers(upstream_resp_headers, true);
    // 日志字段 = 实发头 = SSE 三自管头 + 透传上游头
    let mut all_stream_headers: Vec<(axum::http::HeaderName, axum::http::HeaderValue)> =
        sse_self_managed.to_vec();
    all_stream_headers.extend(stream_filtered.iter().cloned());

    log.status_code = 200;
    log.response_body = "[stream]".to_string();
    log.user_response_body = "[stream]".to_string();
    log.user_response_headers = resp_headers_to_log_json(&all_stream_headers);
    log.duration_ms = start.elapsed().as_millis() as i32;
    upsert_log(state, log, log_settings).await;

    let mut response = (StatusCode::OK, body).into_response();
    {
        let h = response.headers_mut();
        for (n, v) in sse_self_managed {
            h.insert(n, v);
        }
        h.extend(stream_filtered);
    }
    inject_trace_header(&mut response);
    response
}
