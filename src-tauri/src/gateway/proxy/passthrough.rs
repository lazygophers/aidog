use super::*;

/// Claude Code 订阅平台纯透传：把客户端原始请求 1:1 relay 到 base_url，原样返回响应，记 proxy_log。
/// 不做任何协议 / header / 认证转换；客户端自带订阅 OAuth header。
#[allow(clippy::too_many_arguments)]
pub(crate) async fn handle_passthrough(
    state: &Arc<ProxyState>,
    log: &mut ProxyLog,
    log_settings: &ProxyLogSettings,
    orig_method: axum::http::Method,
    orig_uri: axum::http::Uri,
    orig_headers: axum::http::HeaderMap,
    bytes: axum::body::Bytes,
    base_url: &str,
    start: std::time::Instant,
    lang: Lang,
) -> Response {
    // 透传不转换协议，source/target 都标 claude_code
    log.source_protocol = "claude_code".to_string();
    log.target_protocol = "claude_code".to_string();

    // 目标 URL = base_url(host 根) + 客户端原始 path(+query)
    let url = build_passthrough_url(base_url, &orig_uri);
    log.upstream_request_url = url.clone();

    // 解析超时（系统级；透传无 group/model mapping 覆盖）
    let system_timeout = get_system_timeout(&state.db).await;
    // passthrough 透明 relay：禁用总超时——reqwest .timeout 覆盖「连接→响应头→body 全部读完」，
    // 会砍断长 SSE 流（thinking/tool_use body 读取 > request_timeout_secs）致无 message_stop → 客户端
    // JSON Parse error / 内容残缺。透传语义上不替客户端施加任意 body 超时；connect_timeout 仍保护连接期，
    // 客户端自有超时兜底，上游真断由 stream-error-graceful-passthrough 合成 message_stop 兜底。
    let req_timeout = 0u64;
    let conn_timeout = if system_timeout.connect_timeout_secs > 0 { system_timeout.connect_timeout_secs } else { 10 };
    let client = super::http_client::build_http_client(
        &state.db, req_timeout, conn_timeout,
        None, None,
    ).await;

    // 原样转发 header，剔除 hop-by-hop（Host / Content-Length 由 reqwest 按目标 URL + body 重设）
    let fwd_headers = passthrough_headers(&orig_headers);
    // 记录上游请求头（透传 redact authorization）
    log.upstream_request_headers = {
        let mut h = serde_json::Map::new();
        for (k, v) in &fwd_headers {
            let name = k.as_str();
            if is_sensitive_auth_header(name) {
                h.insert(name.to_string(), Value::String("[REDACTED]".into()));
            } else if let Ok(s) = v.to_str() {
                h.insert(name.to_string(), Value::String(s.to_string()));
            }
        }
        Value::Object(h).to_string()
    };
    log.upstream_request_body = String::from_utf8_lossy(&bytes).to_string();
    tracing::info!(method = %orig_method, url = %url, "passthrough upstream request");
    tracing::debug!(method = %orig_method, url = %url, body = %super::log_util::log_body_preview(&log.upstream_request_body), "passthrough upstream request body");

    let method = match reqwest::Method::from_bytes(orig_method.as_str().as_bytes()) {
        Ok(m) => m,
        Err(_) => reqwest::Method::POST,
    };
    let mut req_builder = client.request(method, &url).body(bytes.to_vec());
    req_builder = req_builder.headers(fwd_headers);

    let resp = match req_builder.send().await {
        Ok(r) => r,
        Err(e) => {
            tracing::error!(url = %url, error = %e, duration_ms = start.elapsed().as_millis() as i64, "passthrough upstream request failed (502)");
            log.response_body = format!("upstream error: {e}");
            log.status_code = 502;
            log.user_response_body = format!("{}: {e}", i18n::t(lang, ErrorKey::Upstream));
            log.user_response_headers = r#"{"content-type":"text/plain"}"#.to_string();
            log.duration_ms = start.elapsed().as_millis() as i32;
            upsert_log(state, log, log_settings).await;
            return (StatusCode::BAD_GATEWAY, format!("{}: {e}", i18n::t(lang, ErrorKey::Upstream))).into_response();
        }
    };

    let status = resp.status();
    log.upstream_status_code = status.as_u16() as i32;
    tracing::info!(url = %url, status = status.as_u16(), duration_ms = start.elapsed().as_millis() as i64, "passthrough upstream responded");

    // 捕获上游响应头（原样照搬给客户端）
    let mut resp_header_map = axum::http::HeaderMap::new();
    {
        let mut h = serde_json::Map::new();
        for (k, v) in resp.headers() {
            if let Ok(s) = v.to_str() {
                h.insert(k.to_string(), Value::String(s.to_string()));
            }
            // 剔除 hop-by-hop / 长度类，由 axum 按 body 重设
            let name = k.as_str();
            if name.eq_ignore_ascii_case("content-length")
                || name.eq_ignore_ascii_case("transfer-encoding")
                || name.eq_ignore_ascii_case("connection")
            {
                continue;
            }
            if let (Ok(hn), Ok(hv)) = (
                axum::http::HeaderName::from_bytes(k.as_str().as_bytes()),
                axum::http::HeaderValue::from_bytes(v.as_bytes()),
            ) {
                resp_header_map.insert(hn, hv);
            }
        }
        log.upstream_response_headers = Value::Object(h).to_string();
    }

    let content_type = resp
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();
    let is_stream = content_type.contains("text/event-stream")
        || resp
            .headers()
            .get(reqwest::header::TRANSFER_ENCODING)
            .and_then(|v| v.to_str().ok())
            .map(|s| s.contains("chunked"))
            .unwrap_or(false);

    let resp_status = StatusCode::from_u16(status.as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);

    // ── 非流式：原样 relay bytes ──
    if !is_stream {
        let body = resp.bytes().await.unwrap_or_default();
        let resp_str = String::from_utf8_lossy(&body).to_string();
        let (input_tokens, output_tokens, cache_tokens) = extract_usage(&resp_str);

        log.response_body = resp_str.clone();
        log.status_code = status.as_u16() as i32;
        log.duration_ms = start.elapsed().as_millis() as i32;
        log.input_tokens = input_tokens;
        log.output_tokens = output_tokens;
        log.cache_tokens = cache_tokens;
        log.user_response_body = resp_str;
        log.user_response_headers = log.upstream_response_headers.clone();
        upsert_log(state, log, log_settings).await;

        let mut response = (resp_status, body.to_vec()).into_response();
        *response.headers_mut() = resp_header_map;
        return response;
    }

    // ── 流式：原样透传 SSE bytes，不解析不转换；旁路累计 token + 聚合 body，[DONE]/断连回写 ──
    log.is_stream = true;
    log.status_code = status.as_u16() as i32;

    let agg = Arc::new(StreamAggregator::new());
    let est_fired = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let req_span = tracing::Span::current();

    // 透传原样 relay：response_body == user_response_body == 上游 SSE 原文。
    // response_body 受 master(enabled) 控制；user_response_body 受 log_user_request 控制。
    let record_upstream_body = log_settings.enabled;
    let record_client_body = log_settings.enabled && log_settings.log_user_request;

    // 透传分支无协议转换 → user_response_body 复用 upstream 原文（不单独聚合 client_body）。
    // 透传 user_response_body == upstream 原文：当 log_user_request 开启时（record_client_body=true）
    // 闭包把上游 chunk 同步 push 进 client_body，flush 即从 client_body 写 user_response_body。
    // 故 guard.record_client_body 必须 == record_client_body（曾误设 false，导致 flush 跳过
    // user_response_body 回写，透传日志的 user_response_body 永不落内容）。
    let passthrough_user_body = record_client_body;
    let guard = StreamLogGuard {
        agg: agg.clone(),
        est_fired: est_fired.clone(),
        log: log.clone(),
        state: state.clone(),
        settings: log_settings.clone(),
        start,
        record_upstream_body,
        record_client_body: passthrough_user_body,
        req_span: req_span.clone(),
        // 透传分支历史上不做请求驱动预估，保持现状
        est: None,
    };

    // guard 被 move 进闭包；stream 被 Drop（含客户端断连）时 guard.drop 触发兜底 flush。
    let stream = resp.bytes_stream().map(move |chunk_result| {
        let chunk = match chunk_result {
            Ok(c) => c,
            Err(e) => {
                // 上游流中途断裂：不向客户端报错（避免 CC "error decoding response body"），
                // 仅记日志 + 合成 anthropic message_stop 干净收尾（claude_code relay wire = anthropic）。
                tracing::warn!(error = %e, "passthrough upstream stream chunk error; closing stream gracefully");
                return Ok::<_, std::io::Error>(Bytes::from(
                    "event: message_stop\ndata: {\"type\":\"message_stop\"}\n\n",
                ));
            }
        };
        // 旁路累积上游 SSE 原文（受 master 开关控制）
        if record_upstream_body {
            if let Ok(mut up) = guard.agg.upstream_body.lock() {
                up.push(chunk.clone());
            }
        }
        // 透传 user_response_body == upstream 原文：受 log_user_request 控制时同步聚合到 client_body
        if passthrough_user_body {
            if let Ok(mut cl) = guard.agg.client_body.lock() {
                cl.push(chunk.clone());
            }
        }
        // 尽力从 SSE data 累计 usage（Anthropic / OpenAI 兼容字段，含 message.usage 兜底），不改写 chunk。
        // 跨 chunk 行重组：data: 行被切到两个 chunk 时逐 chunk .lines() 会丢 usage。
        let text = String::from_utf8_lossy(&chunk);
        guard.agg.feed_sse_usage(&text);
        guard.flush_if_done(&text);
        Ok::<_, std::io::Error>(chunk)
    });

    let body = Body::from_stream(stream);

    // 返回 stream 前的占位 upsert：标记流进行中，最终态由 guard.flush（[DONE]/断连）覆盖。
    log.duration_ms = start.elapsed().as_millis() as i32;
    log.response_body = "[stream]".to_string();
    log.user_response_body = "[stream]".to_string();
    log.user_response_headers = log.upstream_response_headers.clone();
    upsert_log(state, log, log_settings).await;

    let mut response = (resp_status, body).into_response();
    *response.headers_mut() = resp_header_map;
    response
}

/// 模型列表端点 relay：选分组首个启用平台，按平台协议拉上游 /models 并原样 relay status + body。
/// 不做 model mapping / 重试 / 转换（模型列表无此语义，取第一个可用平台即可）。
/// 鉴权注入平台凭证（非客户端 group token，上游不认）；URL 遵 url-construction-rule。
pub(crate) async fn handle_models_passthrough(
    state: &Arc<ProxyState>,
    log: &mut ProxyLog,
    log_settings: &ProxyLogSettings,
    group: &Group,
    start: std::time::Instant,
    lang: Lang,
) -> Response {
    // 选分组首个启用平台（endpoint 优先取首个端点协议/URL，否则平台主配置）。
    // Mock 平台无真实上游（base_url 空），不能 relay 模型列表 —— 跳过，否则
    // build_models_url 产无 scheme 的相对 URL，reqwest .send() → builder error → 502。
    let group_platforms = match super::db::get_group_platforms(&state.db, group.id).await {
        Ok(p) => p,
        Err(e) => {
            tracing::warn!(group = %group.name, error = %e, "models: get_group_platforms failed");
            log.response_body = format!("group platforms error: {e}");
            log.status_code = 503;
            log.duration_ms = start.elapsed().as_millis() as i32;
            upsert_log(state, log, log_settings).await;
            return (StatusCode::SERVICE_UNAVAILABLE, format!("{}: {e}", i18n::t(lang, ErrorKey::Route))).into_response();
        }
    };
    let platform = match group_platforms
        .iter()
        .find(|gp| gp.platform.enabled && !matches!(gp.platform.platform_type, Protocol::Mock))
    {
        Some(gp) => gp.platform.clone(),
        None => {
            tracing::warn!(group = %group.name, "models: no enabled upstream platform in group (mock skipped)");
            log.response_body = "no enabled upstream platform for models endpoint".to_string();
            log.status_code = 503;
            log.duration_ms = start.elapsed().as_millis() as i32;
            upsert_log(state, log, log_settings).await;
            return (StatusCode::SERVICE_UNAVAILABLE, i18n::t(lang, ErrorKey::Route)).into_response();
        }
    };

    // endpoint 优先（首个端点协议/URL），否则平台主配置。api_key 始终取平台凭证。
    let (protocol, base_url) = if let Some(ep) = platform.endpoints.first() {
        (ep.protocol.clone(), ep.base_url.clone())
    } else {
        (platform.platform_type.clone(), platform.base_url.clone())
    };
    let url = build_models_url(&protocol, &base_url);

    log.platform_id = platform.id;
    log.target_protocol = format!("{:?}", protocol).to_lowercase();
    log.upstream_request_url = url.clone();
    log.upstream_request_headers = r#"{"authorization":"[REDACTED]"}"#.to_string();

    let system_timeout = get_system_timeout(&state.db).await;
    let req_timeout = if system_timeout.request_timeout_secs > 0 { system_timeout.request_timeout_secs } else { 60 };
    let conn_timeout = if system_timeout.connect_timeout_secs > 0 { system_timeout.connect_timeout_secs } else { 10 };
    let client = super::http_client::build_http_client(&state.db, req_timeout, conn_timeout, None, None).await;

    // OpenCode Zen 同款兜底：/v1/models 无 auth 也能列，留空时注入 $opencode 与 chat 路径一致。
    let models_api_key = resolve_opencode_zen_key(&platform);
    let rb = apply_models_auth(client.get(&url), &protocol, &models_api_key);
    tracing::info!(group = %group.name, platform = %platform.name, url = %url, "models endpoint upstream request");

    let resp = match rb.send().await {
        Ok(r) => r,
        Err(e) => {
            tracing::error!(url = %url, error = %e, "models endpoint upstream request failed (502)");
            log.response_body = format!("upstream error: {e}");
            log.status_code = 502;
            log.upstream_status_code = 0;
            log.user_response_body = format!("{}: {e}", i18n::t(lang, ErrorKey::Upstream));
            log.duration_ms = start.elapsed().as_millis() as i32;
            upsert_log(state, log, log_settings).await;
            return (StatusCode::BAD_GATEWAY, format!("{}: {e}", i18n::t(lang, ErrorKey::Upstream))).into_response();
        }
    };

    let status = resp.status();
    log.upstream_status_code = status.as_u16() as i32;
    let content_type = resp
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("application/json")
        .to_string();
    let body = resp.bytes().await.unwrap_or_default();
    let body_str = String::from_utf8_lossy(&body).to_string();

    log.status_code = status.as_u16() as i32;
    log.response_body = body_str.clone();
    log.user_response_body = body_str;
    log.user_response_headers = format!(r#"{{"content-type":"{}"}}"#, content_type);
    log.duration_ms = start.elapsed().as_millis() as i32;
    tracing::info!(url = %url, status = status.as_u16(), "models endpoint upstream responded");
    upsert_log(state, log, log_settings).await;

    let resp_status = StatusCode::from_u16(status.as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);
    let mut response = (resp_status, body.to_vec()).into_response();
    if let Ok(hv) = axum::http::HeaderValue::from_str(&content_type) {
        response.headers_mut().insert(axum::http::header::CONTENT_TYPE, hv);
    }
    response
}

/// 透传目标 URL 拼接：base_url(去尾斜杠) + 客户端原始 path(+query)
pub(crate) fn build_passthrough_url(base_url: &str, uri: &axum::http::Uri) -> String {
    let base = base_url.trim_end_matches('/');
    let pq = uri
        .path_and_query()
        .map(|pq| pq.as_str())
        .unwrap_or_else(|| uri.path());
    format!("{}{}", base, pq)
}

/// 判定请求 path（已含 group/proxy 前缀）是否为模型列表端点。
/// strip 任意前缀后尾段为 `/v1/models` | `/models`（openai/anthropic 同名）→ true。
/// gemini `/v1beta/models` 本期不在代理 relay 范围（标 TODO，见 prd 失败处理）。
pub(crate) fn is_models_endpoint(path: &str) -> bool {
    let p = path.trim_end_matches('/');
    // gemini /v1beta/models 本期不在代理 relay 范围（鉴权/响应格式不同），显式排除。
    if p.contains("/v1beta/") {
        return false;
    }
    p.ends_with("/v1/models") || p.ends_with("/models")
}

/// 按平台协议构造上游模型列表端点 URL（遵 url-construction-rule：base_url 已含版本前缀，仅 trim 尾 `/` + 端点后缀，禁额外拼版本）。
/// 三类后缀：Anthropic → `/v1/models`（base_url 通常不含 /v1）；Bailian → `/compatible-mode/v1/models`；
/// 其余 OpenAI 兼容（含 glm `.../api/paas/v4`、openai `.../v1`）→ `/models`。
/// 与 lib.rs `platform_fetch_models` 单一事实源，避免按协议拉 /models 的 URL 构造重复腐化。
pub fn build_models_url(protocol: &Protocol, base_url: &str) -> String {
    let base = base_url.trim_end_matches('/');
    match protocol {
        Protocol::Anthropic => format!("{base}/v1/models"),
        Protocol::Bailian => format!("{base}/compatible-mode/v1/models"),
        _ => format!("{base}/models"),
    }
}

/// 按平台协议给上游模型列表请求注入鉴权头（平台凭证，非客户端 group token）。
/// Anthropic → `x-api-key` + `anthropic-version`；其余 OpenAI 兼容 → `Authorization: Bearer`。
/// 与 lib.rs `platform_fetch_models` 鉴权风格对齐。
pub fn apply_models_auth(
    rb: reqwest::RequestBuilder,
    protocol: &Protocol,
    api_key: &str,
) -> reqwest::RequestBuilder {
    match protocol {
        Protocol::Anthropic => rb
            .header("x-api-key", api_key)
            .header("anthropic-version", "2023-06-01"),
        // openai/兼容：Bearer 之外叠加 api-key 头（小米 token-plan openai 端点要求），其他上游忽略未知头。
        _ => rb
            .header("Authorization", format!("Bearer {api_key}"))
            .header("api-key", api_key),
    }
}

#[cfg(test)]
#[path = "test_passthrough.rs"]
mod test_passthrough;
