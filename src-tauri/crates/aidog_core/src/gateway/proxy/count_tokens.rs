use super::*;

pub(crate) fn is_count_tokens_endpoint(path: &str) -> bool {
    let api_path = match path.find("/v1/") {
        Some(idx) => &path[idx..],
        None => return false,
    };
    api_path
        .trim_end_matches('/')
        .ends_with("/v1/messages/count_tokens")
}

/// 本地近似估算 anthropic count_tokens body 的 input_tokens（透传失败兜底）。
/// 启发式：累计 system + 全部 messages 文本 + tools 定义的字符数，按 ~4 字符/token 折算
/// （英文经验值；中文偏低但 count_tokens 仅用于客户端预估，可接受偏差）。
/// 计费口径：proxy_log 单行仍保留估算的 input_tokens + est_cost（供单行审计可见），
/// 但聚合路径（log.rs first_agg gate 按 request_url 识别 count_tokens）跳过 stats_agg，
/// 故不计入 Stats 页/托盘总统计。
/// 拿不到任何文本字段 → 返回保底 1（避免返回 0 误导客户端流程）。
pub(crate) fn estimate_input_tokens(body: &Value) -> i64 {
    fn collect_text(v: &Value, acc: &mut usize) {
        match v {
            Value::String(s) => *acc += s.len(),
            Value::Array(arr) => arr.iter().for_each(|e| collect_text(e, acc)),
            Value::Object(map) => map.values().for_each(|e| collect_text(e, acc)),
            _ => {}
        }
    }
    let mut chars = 0usize;
    if let Some(obj) = body.as_object() {
        for key in ["system", "messages", "tools"] {
            if let Some(v) = obj.get(key) {
                collect_text(v, &mut chars);
            }
        }
    }
    let tokens = chars.div_ceil(4) as i64;
    tokens.max(1)
}

/// Anthropic `/v1/messages/count_tokens` 子端点：透传优先 + 本地估算兜底（方案 X）。
/// 1. 复用 select_candidates_ctx 选首选平台 + 拿模型映射（claude-opus-4-8 → glm-5.1）。
/// 2. 取该平台 anthropic 端点 base_url（无则回退平台主 base_url），URL = base_url + `/v1/messages/count_tokens`
///    （遵 url-construction-rule：anthropic base_url 不含 /v1，仅拼 endpoint 后缀，与 build_models_url 同款）。
/// 3. 透传客户端原始 body（仅 patch model 字段为路由目标模型），x-api-key + anthropic-version 鉴权 POST。
/// 4. 上游 2xx → 原样回客户端（anthropic count_tokens 响应 schema）。
/// 5. 上游 4xx/5xx 或连接失败（平台不支持该端点）→ 本地估算 `{"input_tokens": N}` 返 200，
///    不返回错误，避免 claude-cli 预估流程被上游 500/404 阻断。
///
/// proxy_log：source/target protocol=anthropic，upstream_request_url 含尾段，status 记真实结果。
pub(crate) async fn handle_count_tokens(
    state: &Arc<ProxyState>,
    log: &mut ProxyLog,
    log_settings: &ProxyLogSettings,
    group: &Group,
    bytes: &[u8],
    start: std::time::Instant,
) -> Response {
    log.source_protocol = "anthropic".to_string();
    log.target_protocol = "anthropic".to_string();

    // 原始 body（用于透传 + 估算兜底）+ 入站 model
    let raw_body: Value = serde_json::from_slice(bytes).unwrap_or(Value::Null);
    let requested_model = raw_body
        .get("model")
        .and_then(|m| m.as_str())
        .unwrap_or_default()
        .to_string();
    log.model = requested_model.clone();

    // 本地估算值（透传失败时回客户端；提前算好，避免分支重复）
    let est_tokens = estimate_input_tokens(&raw_body);
    let est_body = serde_json::json!({ "input_tokens": est_tokens }).to_string();
    // 兜底响应：返回本地估算 `{"input_tokens":N}` 200，并把回客户端正文记入 log.user_response_body
    // （与 handle_responses_subendpoint 成功路径一致：客户端实际收到的正文落库）。
    let est_response = |body: &str| -> Response {
        let mut r = (
            StatusCode::OK,
            [(axum::http::header::CONTENT_TYPE, "application/json")],
            body.to_string(),
        )
            .into_response();
        inject_trace_header(&mut r);
        r
    };
    // 在各兜底分支统一回写 log 的客户端响应正文/头（est_response 闭包不可借 &mut log，故在调用点写 log）。
    macro_rules! fallback_log {
        () => {{
            log.input_tokens = est_tokens as i32;
            log.user_response_body = est_body.clone();
            log.user_response_headers = r#"{"content-type":"application/json"}"#.to_string();
            log.duration_ms = start.elapsed().as_millis() as i32;
        }};
    }

    // 路由选平台（复用 group→platform 选择，拿模型映射目标）
    let sched_settings = super::db::get_scheduling_settings(&state.db).await;
    let sched_ctx = ScheduleCtx {
        scheduler: &state.scheduler,
        sticky: &state.sticky,
        settings: &sched_settings,
        sticky_key: Some(format!("{}|count_tokens", group.group_key)),
    };
    let candidate_set =
        match select_candidates_ctx(&state.db, group, &requested_model, Some(&sched_ctx)).await {
            Ok(c) => c,
            Err(e) => {
                // 路由失败 → 本地估算兜底（不阻断 claude-cli）
                tracing::warn!(group = %group.name, model = %requested_model, error = %e, "count_tokens: route failed, falling back to local estimate");
                log.status_code = 200;
                log.response_body = format!("route error (local estimate fallback): {e}");
                fallback_log!();
                upsert_log(state, log, log_settings).await;
                return est_response(&est_body);
            }
        };
    let route = match candidate_set.candidates.into_iter().next() {
        Some(r) => r,
        None => {
            tracing::warn!(group = %group.name, "count_tokens: no candidate platform, local estimate fallback");
            log.status_code = 200;
            log.response_body = "no candidate platform (local estimate fallback)".to_string();
            fallback_log!();
            upsert_log(state, log, log_settings).await;
            return est_response(&est_body);
        }
    };

    let actual_model = route.target_model.clone();
    log.platform_id = route.platform.id;
    log.actual_model = actual_model.clone();

    // 取 anthropic 端点 base_url（无则回退平台主 base_url）
    let base_url = route
        .platform
        .endpoints
        .iter()
        .find(|ep| matches!(ep.protocol, Protocol::Anthropic))
        .map(|ep| ep.base_url.clone())
        .unwrap_or_else(|| route.platform.base_url.clone());
    // URL：base_url + /v1/messages/count_tokens（anthropic base_url 不含 /v1，与 build_models_url 同款拼接）
    let url = format!(
        "{}/v1/messages/count_tokens",
        base_url.trim_end_matches('/')
    );
    log.upstream_request_url = url.clone();
    log.upstream_request_headers =
        r#"{"x-api-key":"[REDACTED]","anthropic-version":"2023-06-01"}"#.to_string();

    // 透传 body：仅 patch model 字段为路由目标模型
    let mut upstream_body = raw_body.clone();
    if let Some(obj) = upstream_body.as_object_mut() {
        obj.insert("model".to_string(), Value::String(actual_model.clone()));
    }
    let upstream_body_str = serde_json::to_string(&upstream_body).unwrap_or_default();
    // ponytail: pretty 序列化仅当 log_upstream_request 开启时执行，关日志零开销
    log.upstream_request_body = if log_settings.log_upstream_request {
        format_pretty_json(&upstream_body_str)
    } else {
        String::new()
    };

    let (system_timeout, proxy_client) = {
        let c = state.settings_cache.read().await;
        (c.system_timeout.clone(), c.proxy_client.clone())
    };
    let req_timeout = if system_timeout.request_timeout_secs > 0 { system_timeout.request_timeout_secs } else { 60 };
    let conn_timeout = if system_timeout.connect_timeout_secs > 0 { system_timeout.connect_timeout_secs } else { 10 };
    let client = super::http_client::build_http_client(
        &proxy_client, req_timeout, conn_timeout, Some(&route.platform.extra), None,
    )
    .await;

    let rb = client
        .post(&url)
        .header("Content-Type", "application/json")
        .header("x-api-key", &route.platform.api_key)
        .header("anthropic-version", "2023-06-01")
        .body(upstream_body_str.clone());
    tracing::info!(group = %group.name, platform = %route.platform.name, model = %actual_model, url = %url, "count_tokens upstream request");

    let resp = match rb.send().await {
        Ok(r) => r,
        Err(e) => {
            // 连接失败 / 超时 → 本地估算兜底（不阻断 claude-cli）
            tracing::warn!(url = %url, error = %e, "count_tokens upstream request failed, local estimate fallback");
            log.upstream_status_code = 0;
            log.status_code = 200;
            log.response_body = format!("upstream error (local estimate fallback): {e}");
            fallback_log!();
            upsert_log(state, log, log_settings).await;
            return est_response(&est_body);
        }
    };

    let status = resp.status();
    log.upstream_status_code = status.as_u16() as i32;
    let body = resp.bytes().await.unwrap_or_default();
    let body_str = String::from_utf8_lossy(&body).to_string();

    if status.is_success() {
        // 上游支持 count_tokens → 原样回客户端真实值
        log.status_code = status.as_u16() as i32;
        log.response_body = body_str.clone();
        log.user_response_body = body_str;
        log.user_response_headers = r#"{"content-type":"application/json"}"#.to_string();
        log.input_tokens = serde_json::from_slice::<Value>(&body)
            .ok()
            .and_then(|v| v.get("input_tokens").and_then(|t| t.as_i64()))
            .unwrap_or(0) as i32;
        log.duration_ms = start.elapsed().as_millis() as i32;
        tracing::info!(url = %url, status = status.as_u16(), "count_tokens upstream responded (passthrough)");
        upsert_log(state, log, log_settings).await;
        let mut response = (StatusCode::OK, body.to_vec()).into_response();
        response.headers_mut().insert(
            axum::http::header::CONTENT_TYPE,
            axum::http::HeaderValue::from_static("application/json"),
        );
        inject_trace_header(&mut response);
        return response;
    }

    // 上游不支持该端点（4xx/5xx）→ 本地估算兜底，返回 200 而非透传错误
    tracing::warn!(url = %url, upstream_status = status.as_u16(), "count_tokens upstream unsupported, local estimate fallback");
    log.status_code = 200;
    log.response_body = format!("upstream {} (local estimate fallback): {}", status.as_u16(), body_str);
    fallback_log!();
    upsert_log(state, log, log_settings).await;
    est_response(&est_body)
}

#[cfg(test)]
#[path = "test_count_tokens.rs"]
mod test_count_tokens;
