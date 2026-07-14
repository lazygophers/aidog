use aidog_core::gateway::{self, db::{self, Db}, adapter, models::Protocol};
use gateway::models::*;
use tauri::State;
use serde_json::Value;

// ── 测试上下文：准备阶段的聚合 ──
struct TestContext {
    platform: gateway::models::Platform,
    model: String,
    prompt: String,
    expected: Option<String>,
    chat_req: gateway::adapter::ChatRequest,
}

// ── HTTP 请求上下文：请求准备阶段的聚合 ──
struct HttpRequestContext {
    target_protocol: Protocol,
    url: String,
    client_type: String,
    eff_api_key: String,
    request_id: String,
    created_at: i64,
    model: String,
    prompt: String,
    req_body_str: String,
    upstream_headers_json: String,
    start: std::time::Instant,
}

// ── 阶段1：准备测试上下文 ──
async fn prepare_test_context(
    db: &Db,
    req: &ModelTestRequest,
) -> Result<TestContext, String> {
        let platform = db::get_platform(db, req.platform_id).await?
            .ok_or_else(|| {
                tracing::warn!(command = "model_test", platform_id = req.platform_id, "platform not found");
                "platform not found".to_string()
            })?;

        let model = req.model.clone()
            .or(platform.models.default.clone())
            .ok_or_else(|| {
                tracing::warn!(command = "model_test", platform_id = req.platform_id, "no model specified and no default model configured");
                "no model specified and no default model configured".to_string()
            })?;

        let (prompt, expected) = match req.prompt.clone() {
            Some(p) => (p, None),
            None => {
                let (p, e) = random_test_challenge();
                (p, Some(e))
            }
        };

        let chat_req = adapter::ChatRequest {
            model: model.clone(),
            messages: vec![adapter::Message {
                role: adapter::Role::User,
                content: adapter::MessageContent::Text(prompt.clone()),
            }],
            system: None,
            max_tokens: Some(req.max_tokens.unwrap_or(1024)),
            temperature: None,
            top_p: None,
            stream: Some(false),
            tools: None,
            tool_choice: None,
            extra: None,
        };

        Ok(TestContext { platform, model, prompt, expected, chat_req })
}

// ── 阶段2：准备 HTTP 请求 ──
fn prepare_http_request(
    ctx: &TestContext,
) -> HttpRequestContext {
    let (target_protocol, target_base_url, client_type, coding_plan) = if !ctx.platform.endpoints.is_empty() {
        let ep = ctx.platform.endpoints.iter()
            .find(|ep| ep.coding_plan)
            .unwrap_or(&ctx.platform.endpoints[0]);
        (ep.protocol.clone(), ep.base_url.clone(), ep.client_type.clone(), ep.coding_plan)
    } else {
        (ctx.platform.platform_type.clone(), ctx.platform.base_url.clone(), "default".to_string(), false)
    };

    let (mut req_body, mut api_path) = adapter::convert_request(&ctx.chat_req, &target_protocol, &ctx.platform.platform_type);
    if coding_plan {
        gateway::proxy::inject_coding_plan_fields(&mut req_body, &target_protocol);
        gateway::proxy::override_coding_plan_path(&mut api_path, &target_protocol);
    }
    let req_body_str = serde_json::to_string(&req_body).unwrap_or_default();
    let base_url = target_base_url.trim_end_matches('/');
    let url = format!("{}{}", base_url, api_path);

    let eff_api_key = gateway::proxy::resolve_opencode_zen_key(&ctx.platform);

    let upstream_headers = gateway::proxy::build_upstream_headers(
        &client_type, &target_protocol, &eff_api_key,
        &axum::http::HeaderMap::new(), &url
    );
    let upstream_headers_json = serde_json::Value::Object(
        upstream_headers.iter()
            .map(|(k, v)| (k.clone(), serde_json::Value::String(v.clone())))
            .collect()
    ).to_string();

    let start = std::time::Instant::now();
    let request_id = uuid::Uuid::new_v4().simple().to_string();
    let created_at = gateway::db::now();

    HttpRequestContext {
        target_protocol,
        url,
        client_type,
        eff_api_key,
        request_id,
        created_at,
        model: ctx.model.clone(),
        prompt: ctx.prompt.clone(),
        req_body_str,
        upstream_headers_json,
        start,
    }
}

// ── 阶段3：构造 ProxyLog ──
#[allow(clippy::too_many_arguments)]
fn build_test_proxy_log(
    http_ctx: &HttpRequestContext,
    platform_id: u64,
    target_protocol: &Protocol,
    body_override: &str,
    upstream_status: i32,
    user_status: i32,
    upstream_resp_headers: &str,
    user_resp_body: &str,
    in_tok: i32,
    out_tok: i32,
) -> gateway::models::ProxyLog {
    gateway::models::ProxyLog {
        id: http_ctx.request_id.clone(),
        group_key: "[test]".into(),
        model: http_ctx.model.clone(),
        actual_model: http_ctx.model.clone(),
        source_protocol: "test".into(),
        target_protocol: format!("{:?}", target_protocol).to_lowercase(),
        platform_id,
        request_headers: r#"{"source":"model-test"}"#.into(),
        request_body: serde_json::to_string(&serde_json::json!({"messages":[{"role":"user","content":&http_ctx.prompt}]})).unwrap_or_default(),
        upstream_request_headers: http_ctx.upstream_headers_json.clone(),
        upstream_request_body: http_ctx.req_body_str.clone(),
        response_body: body_override.into(),
        request_url: format!("/model-test/{}", platform_id),
        upstream_request_url: http_ctx.url.clone(),
        upstream_response_headers: upstream_resp_headers.into(),
        upstream_status_code: upstream_status,
        user_response_headers: r#"{"content-type":"application/json"}"#.to_string(),
        user_response_body: user_resp_body.into(),
        status_code: user_status,
        duration_ms: http_ctx.start.elapsed().as_millis() as i32,
        input_tokens: in_tok,
        output_tokens: out_tok,
        cache_tokens: 0,
        est_cost: 0.0,
        is_stream: false,
        attempts: Vec::new(),
        retry_count: 0,
        blocked_by: String::new(),
        blocked_reason: String::new(),
        created_at: http_ctx.created_at,
        updated_at: http_ctx.created_at,
        deleted_at: 0,
    }
}

// ── 阶段4：Mock 平台处理 ──
fn handle_mock_test(
    ctx: &TestContext,
    http_ctx: &HttpRequestContext,
) -> Option<ModelTestResult> {
    if !matches!(http_ctx.target_protocol, Protocol::Mock) {
        return None;
    }

    let req_body: serde_json::Value = serde_json::from_str(&http_ctx.req_body_str).unwrap_or_default();
    let cfg = adapter::mock::resolve_mock_config(&ctx.platform.extra, &ctx.chat_req, &req_body);
    let source_proto_str = "test";
    let (success, status_code, _resp_body, err_msg, in_tok, out_tok, preview): (bool, u16, String, String, i32, i32, String) = match cfg.error_mode.as_str() {
        "http_error" => {
            let body = adapter::mock::build_error_body(source_proto_str, cfg.status_code, "mock http_error");
            let body_str = serde_json::to_string(&body).unwrap_or_default();
            (false, cfg.status_code, body_str, format!("mock http_error (status {})", cfg.status_code), 0, 0, String::new())
        }
        "rate_limit_429" => {
            let body = adapter::mock::build_error_body(source_proto_str, 429, "mock rate limit");
            let body_str = serde_json::to_string(&body).unwrap_or_default();
            (false, 429, body_str, "mock rate_limit_429".to_string(), 0, 0, String::new())
        }
        "timeout" => {
            let body = adapter::mock::build_error_body(source_proto_str, 504, "mock timeout");
            let body_str = serde_json::to_string(&body).unwrap_or_default();
            (false, 504, body_str, "mock timeout".to_string(), 0, 0, String::new())
        }
        _ => {
            let body = adapter::mock::build_response(&cfg, source_proto_str, &ctx.model);
            let body_str = serde_json::to_string(&body).unwrap_or_default();
            (true, 200, body_str, String::new(), cfg.input_tokens, cfg.output_tokens, cfg.response_text.clone())
        }
    };

    tracing::info!(command = "model_test", platform_id = ctx.platform.id, mock = true, success, status = status_code, "model test mock response");
    Some(ModelTestResult {
        success,
        model: ctx.model.clone(),
        prompt_preview: truncate_str(&ctx.prompt, 100),
        response_preview: preview,
        duration_ms: http_ctx.start.elapsed().as_millis() as i32,
        input_tokens: in_tok,
        output_tokens: out_tok,
        error: err_msg,
    })
}

// ── 阶段5：成功响应处理 ──
fn handle_success_response(
    ctx: &TestContext,
    http_ctx: &HttpRequestContext,
    body: &str,
    target_protocol: &Protocol,
) -> ModelTestResult {
    let resp_json: serde_json::Value = serde_json::from_str(body).unwrap_or_default();
    let response_text = extract_response_text(&resp_json, target_protocol);
    let (in_tok, out_tok) = extract_test_usage(&resp_json, target_protocol);

    let success = verify_test_response(&response_text, ctx.expected.as_deref());
    let error = if success { String::new() } else { "响应内容校验失败".to_string() };

    ModelTestResult {
        success,
        model: ctx.model.clone(),
        prompt_preview: truncate_str(&ctx.prompt, 100),
        response_preview: truncate_str(&response_text, 300),
        duration_ms: http_ctx.start.elapsed().as_millis() as i32,
        input_tokens: in_tok,
        output_tokens: out_tok,
        error,
    }
}

#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %aidog_core::logging::new_trace_id()))]
pub async fn model_test(
    db: State<'_, Db>,
    req: ModelTestRequest,
) -> Result<ModelTestResult, String> {
    tracing::debug!(command = "model_test", platform_id = req.platform_id, "command invoked");

    // 阶段1：准备测试上下文
    let ctx = prepare_test_context(&db, &req).await?;

    // 阶段2：准备 HTTP 请求
    let http_ctx = prepare_http_request(&ctx);

    // 阶段4：Mock 处理
    if let Some(result) = handle_mock_test(&ctx, &http_ctx) {
        let req_body: serde_json::Value = serde_json::from_str(&http_ctx.req_body_str).unwrap_or_default();
        let cfg = adapter::mock::resolve_mock_config(&ctx.platform.extra, &ctx.chat_req, &req_body);
        if cfg.delay_ms > 0 {
            tokio::time::sleep(std::time::Duration::from_millis(cfg.delay_ms)).await;
        }
        if let Err(le) = db::upsert_proxy_log(&db, build_test_proxy_log(
            &http_ctx, ctx.platform.id, &http_ctx.target_protocol,
            "", 200, 200, r#"{"content-type":"application/json"}"#, "",
            result.input_tokens, result.output_tokens,
        )).await {
            tracing::debug!(command = "model_test", error = %le, "upsert test proxy_log failed");
        }
        return Ok(result);
    }

    // 构建 HTTP 客户端（复用 proxy.rs 逻辑；非请求路径，现读 DB settings）
    let db_arc = std::sync::Arc::new(db.inner().clone());
    let proxy_client_settings = gateway::http_client::load_proxy_client_settings(&db_arc).await;
    let client = gateway::http_client::build_http_client(
        &proxy_client_settings, 30, 10, Some(&ctx.platform.extra), None,
    ).await;

    tracing::info!(method = "POST", url = %http_ctx.url, "model test request");
    tracing::debug!(method = "POST", url = %http_ctx.url, body = %gateway::log_util::log_body_preview(&http_ctx.req_body_str), "model test request body");

    let req_builder = client
        .post(&http_ctx.url)
        .header("Content-Type", "application/json")
        .body(http_ctx.req_body_str.clone());
    let req_builder = gateway::proxy::apply_client_headers(req_builder, &http_ctx.client_type, &http_ctx.target_protocol, &http_ctx.eff_api_key);

    let resp = match req_builder.send().await {
        Ok(r) => r,
        Err(e) => {
            let result = ModelTestResult {
                success: false,
                model: ctx.model.clone(),
                prompt_preview: truncate_str(&ctx.prompt, 100),
                response_preview: String::new(),
                duration_ms: http_ctx.start.elapsed().as_millis() as i32,
                input_tokens: 0,
                output_tokens: 0,
                error: format!("request failed: {e}"),
            };
            tracing::warn!(command = "model_test", platform_id = ctx.platform.id, error = %e, "model test request failed");
            if let Err(le) = db::upsert_proxy_log(&db, build_test_proxy_log(
                &http_ctx, ctx.platform.id, &http_ctx.target_protocol,
                &format!("upstream error: {e}"), 0, 502, "", &format!("upstream error: {e}"), 0, 0,
            )).await {
                tracing::debug!(command = "model_test", error = %le, "upsert test proxy_log failed");
            }
            return Ok(result);
        }
    };

    let upstream_status_code = resp.status().as_u16() as i32;
    let status = resp.status();

    let upstream_resp_headers = {
        let mut h = serde_json::Map::new();
        for (k, v) in resp.headers() {
            if let Ok(s) = v.to_str() {
                h.insert(k.as_str().to_string(), serde_json::Value::String(s.to_string()));
            }
        }
        serde_json::Value::Object(h).to_string()
    };

    let body = resp.text().await.unwrap_or_default();

    if !status.is_success() {
        let result = ModelTestResult {
            success: false,
            model: ctx.model.clone(),
            prompt_preview: truncate_str(&ctx.prompt, 100),
            response_preview: truncate_str(&body, 200),
            duration_ms: http_ctx.start.elapsed().as_millis() as i32,
            input_tokens: 0,
            output_tokens: 0,
            error: format!("HTTP {}", status),
        };
        tracing::warn!(command = "model_test", platform_id = ctx.platform.id, %status, "model test non-success upstream status");
        if let Err(le) = db::upsert_proxy_log(&db, build_test_proxy_log(
            &http_ctx, ctx.platform.id, &http_ctx.target_protocol,
            &body, upstream_status_code, upstream_status_code,
            &upstream_resp_headers, &body, 0, 0,
        )).await {
            tracing::debug!(command = "model_test", error = %le, "upsert test proxy_log failed");
        }
        return Ok(result);
    }

    let result = handle_success_response(&ctx, &http_ctx, &body, &http_ctx.target_protocol);

    if let Err(le) = db::upsert_proxy_log(&db, build_test_proxy_log(
        &http_ctx, ctx.platform.id, &http_ctx.target_protocol,
        &body, upstream_status_code, if result.success { 200 } else { 422 },
        &upstream_resp_headers, &body, result.input_tokens, result.output_tokens,
    )).await {
        tracing::debug!(command = "model_test", error = %le, "upsert test proxy_log failed");
    }

    Ok(result)
}

#[allow(dead_code)]
pub(crate) fn truncate_str(s: &str, max: usize) -> String {
    if s.len() <= max { s.to_string() } else { format!("{}\u{2026}", &s[..max]) }
}

#[allow(dead_code)]
pub(crate) fn extract_response_text(v: &Value, protocol: &Protocol) -> String {
    match protocol {
        Protocol::Anthropic => {
            v.get("content").and_then(|c| c.get(0)).and_then(|b| b.get("text"))
                .and_then(|t| t.as_str()).unwrap_or("").to_string()
        }
        _ => {
            v.get("choices").and_then(|c| c.get(0))
                .and_then(|c| c.get("message")).and_then(|m| m.get("content"))
                .and_then(|t| t.as_str()).unwrap_or("").to_string()
        }
    }
}

#[allow(dead_code)]
pub(crate) fn extract_test_usage(v: &Value, protocol: &Protocol) -> (i32, i32) {
    let usage = v.get("usage");
    match protocol {
        Protocol::Anthropic => {
            let in_tok = usage.and_then(|u| u.get("input_tokens")).and_then(|t| t.as_i64()).unwrap_or(0) as i32;
            let out_tok = usage.and_then(|u| u.get("output_tokens")).and_then(|t| t.as_i64()).unwrap_or(0) as i32;
            (in_tok, out_tok)
        }
        _ => {
            let in_tok = usage.and_then(|u| u.get("prompt_tokens")).and_then(|t| t.as_i64()).unwrap_or(0) as i32;
            let out_tok = usage.and_then(|u| u.get("completion_tokens")).and_then(|t| t.as_i64()).unwrap_or(0) as i32;
            (in_tok, out_tok)
        }
    }
}
