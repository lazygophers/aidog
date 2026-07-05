use super::*;

pub(crate) fn is_responses_subendpoint(path: &str) -> bool {
    let api_path = match path.find("/v1/") {
        Some(idx) => &path[idx..],
        None => return false,
    };
    // strip 末尾斜杠后，必须严格长于裸 `/v1/responses`（即 `/v1/responses/<seg>...`）才算子端点。
    // 裸 `/v1/responses` 或 `/v1/responses/`（create，无后续段）→ false。
    let trimmed = api_path.trim_end_matches('/');
    trimmed.starts_with("/v1/responses/") && trimmed.len() > "/v1/responses".len()
}

/// Responses API 子端点透传：选分组首个支持 responses 的平台，原样转发 method/body 到上游 + 平台凭证。
/// 不做转换 / model mapping / 重试（子端点是对上游 response 对象的操作，无 chat 语义）。
/// 平台选择：分组首个 enabled 且 endpoint 协议含 OpenAIResponses 的平台；无则回退首个 enabled 平台。
/// 上游 URL：取该平台 responses 端点 base_url + 子路径（api_path 去 `/v1` 前缀，如 `/responses/{id}/cancel`），
///   镜像 create same_protocol_passthrough 的 `base_url.trim_end('/') + api_path` 构造，base_url 已含 /v1 禁重复拼。
/// 鉴权：平台凭证 `Authorization: Bearer <api_key>` + `OpenAI-Beta: responses=experimental`（不透传客户端 group token）。
/// 已知限制：response_id→platform 无持久映射，多 responses 平台分组下取首个平台，若 create 落到非首个 → 上游可能 404
///   （单 responses 平台分组安全，Codex 常见场景）。此限制在 prd 失败处理已标注，log 记录真实 status。
#[allow(clippy::too_many_arguments)]
pub(crate) async fn handle_responses_subendpoint(
    state: &Arc<ProxyState>,
    log: &mut ProxyLog,
    log_settings: &ProxyLogSettings,
    group: &Group,
    orig_method: &axum::http::Method,
    bytes: &[u8],
    path: &str,
    start: std::time::Instant,
    lang: Lang,
) -> Response {
    log.source_protocol = "openai_responses".to_string();
    log.target_protocol = "openai_responses".to_string();

    // 分组平台列表
    let group_platforms = match super::db::get_group_platforms(&state.db, group.id).await {
        Ok(p) => p,
        Err(e) => {
            tracing::warn!(group = %group.name, error = %e, "responses subendpoint: get_group_platforms failed");
            log.response_body = format!("group platforms error: {e}");
            log.status_code = 503;
            log.duration_ms = start.elapsed().as_millis() as i32;
            upsert_log(state, log, log_settings).await;
            return {
                let mut r = (StatusCode::SERVICE_UNAVAILABLE, format!("{}: {e}", i18n::t(lang, ErrorKey::Route))).into_response();
                inject_trace_header(&mut r);
                r
            };
        }
    };

    // 平台选择：首个 enabled 且含 OpenAIResponses 端点的平台 → 取其 responses 端点 base_url。
    // 回退：首个 enabled 平台（取其首个端点或平台主配置 base_url）。
    let selected = group_platforms.iter().find_map(|gp| {
        if !gp.platform.enabled {
            return None;
        }
        gp.platform
            .endpoints
            .iter()
            .find(|ep| matches!(ep.protocol, Protocol::OpenAIResponses))
            .map(|ep| (gp.platform.clone(), ep.base_url.clone()))
    });
    let (platform, base_url) = match selected {
        Some(p) => p,
        None => {
            // 回退：首个 enabled 平台
            match group_platforms.iter().find(|gp| gp.platform.enabled) {
                Some(gp) => {
                    let base = gp
                        .platform
                        .endpoints
                        .first()
                        .map(|ep| ep.base_url.clone())
                        .unwrap_or_else(|| gp.platform.base_url.clone());
                    (gp.platform.clone(), base)
                }
                None => {
                    tracing::warn!(group = %group.name, "responses subendpoint: no enabled platform in group");
                    log.response_body = "no responses-capable or enabled platform for responses subendpoint".to_string();
                    log.status_code = 503;
                    log.duration_ms = start.elapsed().as_millis() as i32;
                    upsert_log(state, log, log_settings).await;
                    return {
                        let mut r = (StatusCode::SERVICE_UNAVAILABLE, i18n::t(lang, ErrorKey::Route)).into_response();
                        inject_trace_header(&mut r);
                        r
                    };
                }
            }
        }
    };

    // 上游子路径：api_path（strip /proxy+group 前缀，同 detect_source_protocol）去 `/v1` 前缀。
    // base_url 已含版本前缀（如 .../v1）→ 子路径只保留 `/responses/...`，禁重复拼 /v1（url-construction-rule）。
    let api_path = match path.find("/v1/") {
        Some(idx) => &path[idx..],
        None => path,
    };
    let sub_path = api_path.strip_prefix("/v1").unwrap_or(api_path);
    let url = format!("{}{}", base_url.trim_end_matches('/'), sub_path);

    log.platform_id = platform.id;
    log.upstream_request_url = url.clone();
    log.upstream_request_headers = r#"{"authorization":"[REDACTED]","openai-beta":"responses=experimental"}"#.to_string();

    let system_timeout = get_system_timeout(&state.db).await;
    let req_timeout = if system_timeout.request_timeout_secs > 0 { system_timeout.request_timeout_secs } else { 60 };
    let conn_timeout = if system_timeout.connect_timeout_secs > 0 { system_timeout.connect_timeout_secs } else { 10 };
    let client = super::http_client::build_http_client(&state.db, req_timeout, conn_timeout, Some(&platform.extra), None).await;

    // 保留原始 method + 原样转发 body（GET/DELETE 无 body；POST cancel/compact 原样）。
    let mut rb = client
        .request(orig_method.clone(), &url)
        .header("Authorization", format!("Bearer {}", platform.api_key))
        .header("OpenAI-Beta", "responses=experimental");
    if !bytes.is_empty() {
        rb = rb.header("Content-Type", "application/json").body(bytes.to_vec());
        log.upstream_request_body = String::from_utf8_lossy(bytes).to_string();
    }
    tracing::info!(group = %group.name, platform = %platform.name, method = %orig_method, url = %url, "responses subendpoint upstream request");

    let resp = match rb.send().await {
        Ok(r) => r,
        Err(e) => {
            tracing::error!(url = %url, error = %e, "responses subendpoint upstream request failed (502)");
            log.response_body = format!("upstream error: {e}");
            log.status_code = 502;
            log.upstream_status_code = 0;
            log.user_response_body = format!("{}: {e}", i18n::t(lang, ErrorKey::Upstream));
            log.duration_ms = start.elapsed().as_millis() as i32;
            upsert_log(state, log, log_settings).await;
            return {
                let mut r = (StatusCode::BAD_GATEWAY, format!("{}: {e}", i18n::t(lang, ErrorKey::Upstream))).into_response();
                inject_trace_header(&mut r);
                r
            };
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
    tracing::info!(url = %url, status = status.as_u16(), "responses subendpoint upstream responded");
    upsert_log(state, log, log_settings).await;

    let resp_status = StatusCode::from_u16(status.as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);
    let mut response = (resp_status, body.to_vec()).into_response();
    if let Ok(hv) = axum::http::HeaderValue::from_str(&content_type) {
        response.headers_mut().insert(axum::http::header::CONTENT_TYPE, hv);
    }
    inject_trace_header(&mut response);
    response
}

#[cfg(test)]
#[path = "test_responses.rs"]
mod test_responses;
