use super::*;

/// forward_attempt 里一次路由决策产出的协议/模型元数据，供 finish_nonstream / finish_stream 共用。
/// 收拢原先散落的 5 个协议相关参数（source_protocol / target_protocol_enum /
/// same_protocol_passthrough / needs_model_remap / coding_plan）+ 模型对 + api_key/base_url，
/// 把 finish_* 参数个数从 16/18 压到 ≤9（含 state/log/log_settings/group/route/upstream_resp_headers/start）。
pub(crate) struct AttemptCtx {
    pub source_protocol: Protocol,
    pub target_protocol: Protocol,
    pub same_protocol_passthrough: bool,
    pub coding_plan: bool,
    pub requested_model: String,
    pub actual_model: String,
    pub eff_api_key: String,
    // 校准/预估链路用的 base_url：endpoint 真 base_url（coding plan 平台级 base_url 恒空，
    // 用它 dispatch query_quota 子串匹配才命中）。空则回退平台级，见 finish_nonstream/finish_stream 开头。
    pub quota_base_url: String,
    /// 客户端显式禁用思考（请求体 `disable_thinking: true`）。出站已写显式禁用参数，
    /// 但 MiniMax-M2 等内置思考模型照发思维链回来 → 响应侧再剥一次，兑现「禁用」语义。
    pub disable_thinking: bool,
}

/// 非流式 2xx 成功响应处理：usage 提取 + 跨协议转换 + 模型回填 + 出站中间件 + 响应头透传。
/// commit_2xx_success! 已在调用方执行（log.attempts 已填充）。返回最终客户端响应。
#[allow(clippy::too_many_arguments)]
pub(crate) async fn finish_nonstream(
    state: &Arc<ProxyState>,
    log: &mut ProxyLog,
    log_settings: &ProxyLogSettings,
    group: &Group,
    route: &RouteResult,
    ctx: &AttemptCtx,
    upstream_resp_headers: &reqwest::header::HeaderMap,
    start: std::time::Instant,
    body: Bytes,
) -> Response {
    let source_protocol = &ctx.source_protocol;
    let target_protocol_enum = &ctx.target_protocol;
    let same_protocol_passthrough = ctx.same_protocol_passthrough;
    let coding_plan = ctx.coding_plan;
    let requested_model = ctx.requested_model.as_str();
    let actual_model = ctx.actual_model.as_str();
    let eff_api_key = ctx.eff_api_key.as_str();
    let quota_base_url = if ctx.quota_base_url.trim().is_empty() {
        route.platform.base_url.clone()
    } else {
        ctx.quota_base_url.clone()
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
    log.done = true;
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

    // ── 禁用思考的响应侧兑现（转换与透传两分支共用本 seam）──
    // 出站已按目标协议写了显式禁用参数，但 MiniMax-M2 这类内置思考模型不认，照回思维链
    // （实测 request 3ed5a698：整条响应只有 thinking 块）。按客户端协议剥掉思维链载体，
    // 客户端拿到干净正文；上游已花的思考 token 无法追回，仍照实计费。
    let body = if ctx.disable_thinking {
        let mut json: Value = serde_json::from_slice(&body).unwrap_or(Value::Null);
        if adapter::strip_thinking_in_body(&mut json, source_protocol) {
            tracing::info!(model = %actual_model, "disable_thinking: stripped thinking from upstream response");
            Bytes::from(serde_json::to_vec(&json).unwrap_or_else(|_| body.to_vec()))
        } else {
            body
        }
    } else {
        body
    };

    // 下发 model 始终回填客户端请求的模型名（含未 remap 但上游自报名不符的场景）
    let body = if !requested_model.is_empty() {
        replace_model_in_json(&body, requested_model)
    } else {
        body.to_vec()
    };

    // ── 中间件出站规则（非流式 2xx）：response_override/redaction/content_filter 改写 body。
    //   在 usage 提取后改写（脱敏不影响计费/统计）；与入站脱敏幂等。
    //   总开关/子开关 OFF 时为 no-op。error_rule 不在此（仅非 2xx 路径分类）。──
    let body = {
        let mut s = String::from_utf8_lossy(&body).to_string();
        let mw_settings = state
            .settings_cache
            .read()
            .await
            .middleware_settings
            .clone();
        state.middleware.apply_outbound(
            &mw_settings,
            &mut s,
            Some(&group.group_key),
            Some(route.platform.id as i64),
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
/// commit_2xx_success! 已在调用方执行（log.attempts 已填充）。stream 为调用方已把 peek 阶段
/// 缓冲的首批 chunk prepend 回上游剩余流后的完整流（见 forward.rs 调用点）。返回 SSE 流式响应。
#[allow(clippy::too_many_arguments)]
pub(crate) async fn finish_stream<S>(
    stream: S,
    state: &Arc<ProxyState>,
    log: &mut ProxyLog,
    log_settings: &ProxyLogSettings,
    group: &Group,
    route: &RouteResult,
    ctx: &AttemptCtx,
    upstream_resp_headers: &reqwest::header::HeaderMap,
    start: std::time::Instant,
) -> Response
where
    S: futures::Stream<Item = reqwest::Result<Bytes>> + Unpin + Send + 'static,
{
    let source_protocol = &ctx.source_protocol;
    let requested_model = ctx.requested_model.as_str();
    let actual_model = ctx.actual_model.as_str();
    let eff_api_key = ctx.eff_api_key.as_str();
    let target_protocol_enum = &ctx.target_protocol;
    let same_protocol_passthrough = ctx.same_protocol_passthrough;
    let coding_plan = ctx.coding_plan;
    let quota_base_url = if ctx.quota_base_url.trim().is_empty() {
        route.platform.base_url.clone()
    } else {
        ctx.quota_base_url.clone()
    };

    // 流式：转换 SSE 格式为 Anthropic 格式返回
    // 同协议透传时（passthrough_response），下方闭包内原样 relay 上游 SSE，仅提取 usage。
    let passthrough_response = same_protocol_passthrough;
    let protocol = target_protocol_enum.clone();
    let client_protocol = source_protocol.clone();
    let model_for_sse = requested_model.to_string();

    // ── 中间件出站流式逐块改写上下文：在构建 stream 闭包前读取 settings（闭包在 req span 外轮询，
    //   不可再 await DB）。引擎 Arc clone 进闭包，每 chunk 文本应用 mask/override/sensitive。
    //   error 已由上游 HTTP 状态码在 forward 后判定（非 2xx 不会走到这里，故流式无需再判 error）。──
    let mw_engine = state.middleware.clone();
    let mw_settings = state
        .settings_cache
        .read()
        .await
        .middleware_settings
        .clone();
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
    // 决策 B（peek 阶段已缓冲的首批 chunk prepend 回流）已在调用方 forward.rs 完成，此处直接消费完整流。
    // utf8_buf：跨 chunk 字节重组器（红线 2 修复），逐 chunk 顺序 poll、closure 内独占可变捕获，无需加锁。
    let mut utf8_buf = Utf8ChunkReassembler::new();
    // sse_line_buf：内容路径跨 chunk 行重组（design.md sse-chunk-line-reassembly，与
    // guard.agg.feed_sse_usage 的 sse_line_buf 同型 idiom）。只在转换分支（!passthrough_response）
    // 使用；passthrough 分支原样 relay 字节，无需按行解析，零改动。
    let mut sse_line_buf = SseLineReassembler::new();
    // Anthropic 系客户端流式渲染状态机：跨 chunk 维护 content block index 分配与开块表
    // （tool/thinking 块完整 content_block_start·stop 序列；ticket 08）。
    let mut client_sse_state = adapter::AnthropicSseState::default();
    // 行内思维链标签（`<thinking>` / `<think>`）跨 chunk 分离器：部分上游把思考写进正文文本而非
    // 结构化思维链字段，不分流则标签原样渲染到客户端界面。与 client_sse_state 同生命周期
    // （逐 chunk 顺序 poll，闭包内独占可变捕获）。
    let mut inline_reasoning = adapter::InlineReasoningSplitter::new();
    // 禁用思考（客户端 disable_thinking=true）时的流式剥离：
    // 透传分支按客户端 wire 逐帧剔思维链帧（含 Anthropic block index 重编号），
    // 转换分支直接丢 ReasoningDelta 事件（不必过状态机）。
    let disable_thinking = ctx.disable_thinking;
    let mut sse_thinking_stripper =
        disable_thinking.then(|| adapter::SseThinkingStripper::new(client_protocol.clone()));
    // 上游流自然耗尽哨兵：chain 在 map 之前，上游 Stream 返 None 时置 exhausted 位，使 Drop
    // 兜底 flush 能区分「上游读完（无 [DONE]/message_stop 也算正常收尾，如 Gemini）」与
    // 「客户端提前断连」。poll_fn 恒返 Ready(None)，不产 item，对下游 map / 客户端字节零影响。
    let agg_end = agg.clone();
    let stream = stream.chain(futures::stream::poll_fn(move |_| {
        agg_end.mark_exhausted();
        std::task::Poll::Ready(None)
    }));
    let stream = stream.map(move |chunk_result| {
        let chunk = match chunk_result {
            Ok(c) => c,
            Err(e) => {
                // 上游流中途断裂（如 cometapi 10-12s 掐断）：按客户端协议合成 error 帧 +
                // Stop 终止事件收尾，已输出内容保留。2026-08-21 用户决策恢复 error 帧：
                // 完整性明确感知优先于 CC 显示 "API Error" 的干扰（静默截断会让客户端
                // 把半截文本当完整答案，比报错更糟）。
                tracing::warn!(error = %e, "SSE upstream stream chunk error; sending error frame");
                // 终态标记：本次流是被上游掐断的（flush 回写 502，禁再记 200 成功）。
                guard.agg.mark_upstream_err();
                let mut out = upstream_break_error_frame(&client_protocol);
                out.push_str(
                    &adapter::to_client_sse(
                        &ChatStreamEvent::Stop {
                            finish_reason: Some("end_turn".to_string()),
                        },
                        &client_protocol,
                        &model_for_sse,
                    )
                    .unwrap_or_default(),
                );
                return Ok::<_, std::io::Error>(Bytes::from(out));
            }
        };

        // 旁路累积上游响应原文（受 master 开关控制；push_upstream 内部 O(1) 判断是否已达
        // STREAM_BODY_MAX_BYTES 上限，达上限后跳过不再增长，累积期本身有界）
        if record_upstream_body {
            guard.agg.push_upstream(&chunk);
        }

        // 跨 chunk 字节层重组：被 chunk 边界切断的多字节字符尾部留到下次拼接后再解码一次，
        // 避免逐 chunk 独立 lossy 解码把半个字符替换为 U+FFFD（原 `String::from_utf8_lossy(&chunk)`）。
        let text = utf8_buf.feed(&chunk);

        // ── 同协议透传：跳过 parse_sse→to_client_sse 格式转换，relay 上游 SSE 字节。
        // usage 提取仍保留（accumulate_sse_usage），est_cost / 统计不丢。
        // 下发 model 与客户端请求对齐：完整行内改写 model 字段（跨 chunk 切断的行经
        // sse_line_buf 拼接后完整，同转换分支 idiom；字节内容除 model 外原样 relay）。──
        let out_bytes = if passthrough_response {
            // 跨 chunk 行重组后累计 usage（逐 chunk .lines() 会因 data: 行被切断而丢 usage）。
            guard.agg.feed_sse_usage(&text);
            let line_ready_text = sse_line_buf.feed(&text);
            // 禁用思考：逐帧剔上游思维链帧（帧被 chunk 切断时由 stripper 内部缓冲）。
            // 末帧可能不带结尾空行 → 见到终止哨兵时冲刷残留，避免吞掉 message_stop / [DONE]。
            let line_ready_text = match sse_thinking_stripper.as_mut() {
                Some(s) => {
                    let mut out = s.push(&line_ready_text);
                    if line_ready_text.contains("[DONE]")
                        || line_ready_text.contains("message_stop")
                    {
                        out.push_str(&s.finish());
                    }
                    out
                }
                None => line_ready_text,
            };
            if line_ready_text.contains("\"model\"") {
                Bytes::from(replace_model_in_sse_text(&line_ready_text, &model_for_sse))
            } else {
                Bytes::from(line_ready_text)
            }
        } else {
            // token 累计走跨 chunk 行重组（逐 chunk .lines() 会因 data: 行被切断丢 usage）。
            guard.agg.feed_sse_usage(&text);
            // 内容路径同型跨 chunk 行重组：完整行立即随本 chunk 下发（不攒批，避免首 token
            // 时延退化），不完整尾行留 sse_line_buf 等下个 chunk 拼接（design.md 修法）。
            let line_ready_text = sse_line_buf.feed(&text);
            let mut output = String::new();
            // 上游帧格式（`data: ` 分帧 / DONE 哨兵 / 各协议 JSON 解析）知识全部收在 adapter 侧，
            // 此处只负责 model 字段改写 + 按客户端协议渲染下发。
            for event in adapter::parse_upstream_sse(&line_ready_text, &protocol) {
                let event = if !model_for_sse.is_empty() {
                    match event {
                        ChatStreamEvent::Start { id, model: _ } => ChatStreamEvent::Start {
                            id,
                            model: model_for_sse.clone(),
                        },
                        other => other,
                    }
                } else {
                    event
                };
                // 正文里的行内思维链标签 → ReasoningDelta（出口与结构化思维链一致：
                // Anthropic 客户端得 thinking 块，OpenAI 客户端得 reasoning_content）。
                for event in adapter::split_stream_inline_reasoning(event, &mut inline_reasoning) {
                    // 禁用思考：思维链事件（含行内标签分出来的）不下发
                    if disable_thinking && matches!(event, ChatStreamEvent::ReasoningDelta { .. }) {
                        continue;
                    }
                    if let Some(sse) = adapter::to_client_sse_stateful(
                        &event,
                        &mut client_sse_state,
                        &client_protocol,
                        &model_for_sse,
                    ) {
                        output.push_str(&sse);
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
                &mw_settings,
                &original,
                Some(&mw_group),
                Some(mw_platform_id),
            );
            if rewritten == original.as_ref() {
                out_bytes
            } else {
                Bytes::from(rewritten)
            }
        } else {
            out_bytes
        };

        // 旁路累积下发客户端的 SSE（受 log_user_request 开关控制；push_client 同 O(1) 上限判断）
        if record_client_body && !out_bytes.is_empty() {
            guard.agg.push_client(&out_bytes);
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
        (
            axum::http::header::CONTENT_TYPE,
            axum::http::HeaderValue::from_static("text/event-stream"),
        ),
        (
            axum::http::header::CACHE_CONTROL,
            axum::http::HeaderValue::from_static("no-cache"),
        ),
        (
            axum::http::header::CONNECTION,
            axum::http::HeaderValue::from_static("keep-alive"),
        ),
    ];
    let stream_filtered = filter_upstream_resp_headers(upstream_resp_headers, true);
    // 日志字段 = 实发头 = SSE 三自管头 + 透传上游头
    let mut all_stream_headers: Vec<(axum::http::HeaderName, axum::http::HeaderValue)> =
        sse_self_managed.to_vec();
    all_stream_headers.extend(stream_filtered.iter().cloned());

    log.status_code = 200;
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

/// 上游流中途断裂时按客户端协议合成的 error 帧（纯函数，便于单测）。
/// - Anthropic 系（含平台变体）→ `event: error`（Messages SSE 规范的终端错误事件）
/// - OpenAI 系（openai/openai_responses/openai_completions）→ error chunk + `[DONE]`
/// - Gemini → Gemini generateContent SSE 错误载荷
pub(crate) fn upstream_break_error_frame(client_protocol: &aidog_db::models::Protocol) -> String {
    use aidog_db::models::Protocol;
    match client_protocol {
        Protocol::OpenAI | Protocol::OpenAIResponses | Protocol::OpenAICompletions => {
            "data: {\"error\":{\"message\":\"upstream stream interrupted\",\"type\":\"server_error\",\"code\":502}}\n\ndata: [DONE]\n\n".to_string()
        }
        Protocol::Gemini => {
            "data: {\"error\":{\"code\":502,\"message\":\"upstream stream interrupted\",\"status\":\"UNAVAILABLE\"}}\n\n".to_string()
        }
        _ => {
            "event: error\ndata: {\"type\":\"error\",\"error\":{\"type\":\"overloaded_error\",\"message\":\"upstream stream interrupted\"}}\n\n".to_string()
        }
    }
}

#[cfg(test)]
mod test_break_frame {
    use super::upstream_break_error_frame;
    use aidog_db::models::Protocol;

    #[test]
    fn anthropic_family_gets_error_event() {
        let f = upstream_break_error_frame(&Protocol::Anthropic);
        assert!(f.starts_with("event: error\n"));
        assert!(f.contains("overloaded_error"));
        // 平台变体同走 anthropic 格式
        assert!(upstream_break_error_frame(&Protocol::Glm).starts_with("event: error\n"));
    }

    #[test]
    fn openai_family_gets_error_chunk_and_done() {
        for p in [
            Protocol::OpenAI,
            Protocol::OpenAIResponses,
            Protocol::OpenAICompletions,
        ] {
            let f = upstream_break_error_frame(&p);
            assert!(f.contains("\"error\""));
            assert!(f.ends_with("data: [DONE]\n\n"));
        }
    }

    #[test]
    fn gemini_gets_unavailable_payload() {
        let f = upstream_break_error_frame(&Protocol::Gemini);
        assert!(f.contains("\"status\":\"UNAVAILABLE\""));
    }
}
