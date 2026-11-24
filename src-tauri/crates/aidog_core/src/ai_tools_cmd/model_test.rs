use crate::gateway;
use aidog_adapter as adapter;
use aidog_db::{self as db, Db};
use gateway::models::*;
use serde_json::Value;

// ── 测试上下文：准备阶段的聚合 ──
struct TestContext {
    platform: gateway::models::Platform,
    model: String,
    prompt: String,
    expected: Option<String>,
    chat_req: aidog_adapter::ChatRequest,
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
async fn prepare_test_context(db: &Db, req: &ModelTestRequest) -> Result<TestContext, String> {
    let platform = db::get_platform(db, req.platform_id)
        .await?
        .ok_or_else(|| {
            tracing::warn!(
                command = "model_test",
                platform_id = req.platform_id,
                "platform not found"
            );
            "platform not found".to_string()
        })?;

    let model = req
        .model
        .clone()
        .or(platform.models.default.first().cloned())
        .ok_or_else(|| {
            tracing::warn!(
                command = "model_test",
                platform_id = req.platform_id,
                "no model specified and no default model configured"
            );
            "no model specified and no default model configured".to_string()
        })?;

    // 工具调用探测（builtin-tool-compat T5）：固定提示词 + get_weather 工具定义，
    // 成功判据 = 响应含工具调用（见 extract_tool_call_name）。
    let (prompt, expected, tools) = if req.tool_test.unwrap_or(false) {
        (
            "What is the weather in Tokyo? You must call the get_weather tool.".to_string(),
            None,
            Some(vec![adapter::Tool {
                name: "get_weather".to_string(),
                description: Some("Get current weather for a city".to_string()),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": { "city": { "type": "string" } },
                    "required": ["city"]
                }),
                tool_type: None,
                cache_control: None,
                extra: None,
            }]),
        )
    } else {
        let (p, e) = match req.prompt.clone() {
            Some(p) => (p, None),
            None => {
                let (p, e) = random_test_challenge();
                (p, Some(e))
            }
        };
        (p, e, None)
    };

    let chat_req = adapter::ChatRequest {
        thinking_budget: None,
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
        tools,
        tool_choice: None,
        extra: None,
        thinking_mode: None,
    };

    Ok(TestContext {
        platform,
        model,
        prompt,
        expected,
        chat_req,
    })
}

// ── 阶段2：准备 HTTP 请求 ──
fn prepare_http_request(ctx: &TestContext) -> Result<HttpRequestContext, String> {
    let (target_protocol, target_base_url, client_type, coding_plan) =
        if !ctx.platform.endpoints.is_empty() {
            let ep = ctx
                .platform
                .endpoints
                .iter()
                .find(|ep| ep.coding_plan)
                .unwrap_or(&ctx.platform.endpoints[0]);
            (
                ep.protocol.clone(),
                ep.base_url.clone(),
                ep.client_type.clone(),
                ep.coding_plan,
            )
        } else {
            (
                ctx.platform.platform_type.clone(),
                ctx.platform.base_url.clone(),
                aidog_db::registry::derive_client_type_for_platform(
                    aidog_db::registry::PlatformCode(&ctx.platform.platform_type.wire_str()),
                ),
                false,
            )
        };

    // base_url 缺失 guard：OAuth 未回填 / 用户手建平台漏配 → 友好错误替代 reqwest builder error。
    // 空 base_url 拼 api_path 得无 host 相对 URL，reqwest builder 直接 error → client 502，错误不友好。
    if target_base_url.trim().is_empty() {
        tracing::warn!(
            command = "model_test",
            platform_id = ctx.platform.id,
            "base_url empty, cannot build upstream url"
        );
        return Err("base_url 缺失，请在平台 endpoints 配置上游地址".to_string());
    }

    let (mut req_body, api_path) =
        adapter::convert_request(&ctx.chat_req, &target_protocol, &ctx.platform.platform_type);
    if coding_plan {
        // 平台协议分派、wire 协议门控（与 forward.rs 同口径：prompt_cache_key 仅 openai wire 注入）
        gateway::proxy::inject_coding_plan_fields(&mut req_body, &ctx.platform.platform_type, &target_protocol);
    }
    let req_body_str = serde_json::to_string(&req_body).unwrap_or_default();
    let base_url = target_base_url.trim_end_matches('/');
    let url = format!("{}{}", base_url, api_path);

    let eff_api_key = gateway::proxy::resolve_opencode_zen_key(&ctx.platform);

    let upstream_headers = gateway::proxy::build_upstream_headers_for_model(
        &client_type,
        &target_protocol,
        &eff_api_key,
        &ctx.model,
        &axum::http::HeaderMap::new(),
    );
    let upstream_headers_json = serde_json::Value::Object(
        upstream_headers
            .iter()
            .map(|(k, v)| (k.clone(), serde_json::Value::String(v.clone())))
            .collect(),
    )
    .to_string();

    let start = std::time::Instant::now();
    let request_id = uuid::Uuid::new_v4().simple().to_string();
    let created_at = aidog_db::now();

    Ok(HttpRequestContext {
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
    })
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
        model: http_ctx.model.clone(),
        actual_model: http_ctx.model.clone(),
        target_protocol: format!("{:?}", target_protocol).to_lowercase(),
        platform_id,
        request_headers: r#"{"source":"model-test"}"#.into(),
        request_body: serde_json::to_string(
            &serde_json::json!({"messages":[{"role":"user","content":&http_ctx.prompt}]}),
        )
        .unwrap_or_default(),
        upstream_request_headers: http_ctx.upstream_headers_json.clone(),
        upstream_request_body: http_ctx.req_body_str.clone(),
        response_body: body_override.into(),
        request_url: format!("/model-test/{}", platform_id),
        upstream_request_url: http_ctx.url.clone(),
        upstream_response_headers: upstream_resp_headers.into(),
        upstream_status_code: upstream_status,
        user_response_body: user_resp_body.into(),
        status_code: user_status,
        duration_ms: http_ctx.start.elapsed().as_millis() as i32,
        input_tokens: in_tok,
        output_tokens: out_tok,
        created_at: http_ctx.created_at,
        updated_at: http_ctx.created_at,
        ..Default::default()
    }
    .out_of_band("[test]", "test")
    // 不经代理出站 body 构造 seam，无字段留痕（票 10）。
}

// ── 阶段4：Mock 平台处理 ──
fn handle_mock_test(ctx: &TestContext, http_ctx: &HttpRequestContext) -> Option<ModelTestResult> {
    if !matches!(http_ctx.target_protocol, Protocol::Mock) {
        return None;
    }

    let req_body: serde_json::Value =
        serde_json::from_str(&http_ctx.req_body_str).unwrap_or_default();
    let cfg = adapter::mock::resolve_mock_config(&ctx.platform.extra, &ctx.chat_req, &req_body);
    // mock 响应形态须match target_protocol（下游 handle_success_response 用同一 target_protocol 解析），而非字面量占位符。
    let target_protocol = &http_ctx.target_protocol;
    let (success, status_code, _resp_body, err_msg, in_tok, out_tok, preview): (
        bool,
        u16,
        String,
        String,
        i32,
        i32,
        String,
    ) = match cfg.error_mode.as_str() {
        "http_error" => {
            let body = adapter::mock::build_error_body(
                target_protocol,
                cfg.status_code,
                "mock http_error",
            );
            let body_str = serde_json::to_string(&body).unwrap_or_default();
            (
                false,
                cfg.status_code,
                body_str,
                format!("mock http_error (status {})", cfg.status_code),
                0,
                0,
                String::new(),
            )
        }
        "rate_limit_429" => {
            let body = adapter::mock::build_error_body(target_protocol, 429, "mock rate limit");
            let body_str = serde_json::to_string(&body).unwrap_or_default();
            (
                false,
                429,
                body_str,
                "mock rate_limit_429".to_string(),
                0,
                0,
                String::new(),
            )
        }
        "timeout" => {
            let body = adapter::mock::build_error_body(target_protocol, 504, "mock timeout");
            let body_str = serde_json::to_string(&body).unwrap_or_default();
            (
                false,
                504,
                body_str,
                "mock timeout".to_string(),
                0,
                0,
                String::new(),
            )
        }
        _ => {
            let body = adapter::mock::build_response(&cfg, target_protocol, &ctx.model);
            let body_str = serde_json::to_string(&body).unwrap_or_default();
            (
                true,
                200,
                body_str,
                String::new(),
                cfg.input_tokens,
                cfg.output_tokens,
                cfg.response_text.clone(),
            )
        }
    };

    tracing::info!(
        command = "model_test",
        platform_id = ctx.platform.id,
        mock = true,
        success,
        status = status_code,
        "model test mock response"
    );
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

    // 工具探测模式：成功判据 = 响应含工具调用；普通模式走关键词校验。
    let (success, error, preview) = if ctx.chat_req.tools.is_some() {
        match extract_tool_call_name(&resp_json, target_protocol) {
            Some(name) => (true, String::new(), format!("[tool_call] {name}")),
            None => (
                false,
                "模型未发起工具调用".to_string(),
                truncate_str(&response_text, 300),
            ),
        }
    } else {
        let ok = verify_test_response(&response_text, ctx.expected.as_deref());
        let err = if ok {
            String::new()
        } else {
            "响应内容校验失败".to_string()
        };
        (ok, err, truncate_str(&response_text, 300))
    };

    ModelTestResult {
        success,
        model: ctx.model.clone(),
        prompt_preview: truncate_str(&ctx.prompt, 100),
        response_preview: preview,
        duration_ms: http_ctx.start.elapsed().as_millis() as i32,
        input_tokens: in_tok,
        output_tokens: out_tok,
        error,
    }
}

crate::tauri_command! {
    pub async fn model_test(
        req: ModelTestRequest) -> Result<ModelTestResult, String> {
    let db = aidog_ctx::db();
        tracing::debug!(command = "model_test", platform_id = req.platform_id, "command invoked");

        // 阶段1：准备测试上下文
        let ctx = prepare_test_context(db, &req).await?;

        // 阶段2：准备 HTTP 请求
        let http_ctx = prepare_http_request(&ctx)?;

        // 阶段4：Mock 处理
        if let Some(result) = handle_mock_test(&ctx, &http_ctx) {
            let req_body: serde_json::Value = serde_json::from_str(&http_ctx.req_body_str).unwrap_or_default();
            let cfg = adapter::mock::resolve_mock_config(&ctx.platform.extra, &ctx.chat_req, &req_body);
            if cfg.delay_ms > 0 {
                tokio::time::sleep(std::time::Duration::from_millis(cfg.delay_ms)).await;
            }
            if let Err(le) = aidog_logs::upsert_request_log(db, build_test_proxy_log(
                &http_ctx, ctx.platform.id, &http_ctx.target_protocol,
                "", 200, 200, r#"{"content-type":"application/json"}"#, "",
                result.input_tokens, result.output_tokens,
            )).await {
                tracing::debug!(command = "model_test", error = %le, "upsert test proxy_log failed");
            }
            return Ok(result);
        }

        // 构建 HTTP 客户端（复用 proxy.rs 逻辑；非请求路径，现读 DB settings）
        let db_arc = std::sync::Arc::new(db.clone());
        let proxy_client_settings = gateway::http_client::load_proxy_client_settings(&db_arc).await;
        let client = gateway::http_client::build_http_client(
            &proxy_client_settings, 30, 10,
            gateway::http_client::platform_should_use_proxy(&proxy_client_settings, &ctx.platform.extra),
        ).await;

        tracing::info!(method = "POST", url = %http_ctx.url, "model test request");
        tracing::debug!(method = "POST", url = %http_ctx.url, body = %gateway::log_util::log_body_preview(&http_ctx.req_body_str), "model test request body");

        let req_builder = client
            .post(&http_ctx.url)
            .header("Content-Type", "application/json")
            .body(http_ctx.req_body_str.clone());
        let req_builder = gateway::proxy::apply_client_headers_for_model(req_builder, &http_ctx.client_type, &http_ctx.target_protocol, &http_ctx.eff_api_key, &http_ctx.model);

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
                if let Err(le) = aidog_logs::upsert_request_log(db, build_test_proxy_log(
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

        let body = match resp.text().await {
            Ok(b) => b,
            Err(e) => {
                let result = ModelTestResult {
                    success: false,
                    model: ctx.model.clone(),
                    prompt_preview: truncate_str(&ctx.prompt, 100),
                    response_preview: String::new(),
                    duration_ms: http_ctx.start.elapsed().as_millis() as i32,
                    input_tokens: 0,
                    output_tokens: 0,
                    error: format!("read upstream response failed: {e}"),
                };
                tracing::warn!(command = "model_test", platform_id = ctx.platform.id, error = %e, "read upstream response failed");
                if let Err(le) = aidog_logs::upsert_request_log(db, build_test_proxy_log(
                    &http_ctx, ctx.platform.id, &http_ctx.target_protocol,
                    &format!("upstream error: {e}"), upstream_status_code, 502,
                    &upstream_resp_headers, &format!("upstream error: {e}"), 0, 0,
                )).await {
                    tracing::debug!(command = "model_test", error = %le, "upsert test proxy_log failed");
                }
                return Ok(result);
            }
        };

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
            if let Err(le) = aidog_logs::upsert_request_log(db, build_test_proxy_log(
                &http_ctx, ctx.platform.id, &http_ctx.target_protocol,
                &body, upstream_status_code, upstream_status_code,
                &upstream_resp_headers, &body, 0, 0,
            )).await {
                tracing::debug!(command = "model_test", error = %le, "upsert test proxy_log failed");
            }
            return Ok(result);
        }

        let result = handle_success_response(&ctx, &http_ctx, &body, &http_ctx.target_protocol);

        if let Err(le) = aidog_logs::upsert_request_log(db, build_test_proxy_log(
            &http_ctx, ctx.platform.id, &http_ctx.target_protocol,
            &body, upstream_status_code, if result.success { 200 } else { 422 },
            &upstream_resp_headers, &body, result.input_tokens, result.output_tokens,
        )).await {
            tracing::debug!(command = "model_test", error = %le, "upsert test proxy_log failed");
        }

        Ok(result)
    }
}

#[allow(dead_code)]
pub(crate) fn truncate_str(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}\u{2026}", &s[..max])
    }
}

#[allow(dead_code)]
pub(crate) fn extract_response_text(v: &Value, protocol: &Protocol) -> String {
    match protocol {
        Protocol::Anthropic => v
            .get("content")
            .and_then(|c| c.get(0))
            .and_then(|b| b.get("text"))
            .and_then(|t| t.as_str())
            .unwrap_or("")
            .to_string(),
        _ => v
            .get("choices")
            .and_then(|c| c.get(0))
            .and_then(|c| c.get("message"))
            .and_then(|m| m.get("content"))
            .and_then(|t| t.as_str())
            .unwrap_or("")
            .to_string(),
    }
}

#[allow(dead_code)]
pub(crate) fn extract_tool_call_name(v: &Value, protocol: &Protocol) -> Option<String> {
    // anthropic：content blocks 内 type=tool_use → name；openai 系：message.tool_calls → function.name
    match protocol {
        Protocol::Anthropic => v.get("content")?.as_array()?.iter().find_map(|b| {
            (b.get("type").and_then(Value::as_str) == Some("tool_use"))
                .then(|| b.get("name").and_then(Value::as_str).map(str::to_string))
                .flatten()
        }),
        _ => v
            .get("choices")?
            .get(0)?
            .get("message")?
            .get("tool_calls")?
            .get(0)?
            .get("function")?
            .get("name")
            .and_then(Value::as_str)
            .map(str::to_string),
    }
}

#[allow(dead_code)]
pub(crate) fn extract_test_usage(v: &Value, protocol: &Protocol) -> (i32, i32) {
    let usage = v.get("usage");
    match protocol {
        Protocol::Anthropic => {
            let in_tok = usage
                .and_then(|u| u.get("input_tokens"))
                .and_then(|t| t.as_i64())
                .unwrap_or(0) as i32;
            let out_tok = usage
                .and_then(|u| u.get("output_tokens"))
                .and_then(|t| t.as_i64())
                .unwrap_or(0) as i32;
            (in_tok, out_tok)
        }
        _ => {
            let in_tok = usage
                .and_then(|u| u.get("prompt_tokens"))
                .and_then(|t| t.as_i64())
                .unwrap_or(0) as i32;
            let out_tok = usage
                .and_then(|u| u.get("completion_tokens"))
                .and_then(|t| t.as_i64())
                .unwrap_or(0) as i32;
            (in_tok, out_tok)
        }
    }
}

#[cfg(test)]
mod test_tool_call {
    use super::*;

    #[test]
    fn anthropic_tool_use_block_extracted() {
        let v = serde_json::json!({
            "content": [
                {"type": "text", "text": "let me check"},
                {"type": "tool_use", "id": "t1", "name": "get_weather", "input": {"city": "Tokyo"}}
            ]
        });
        assert_eq!(
            extract_tool_call_name(&v, &Protocol::Anthropic).as_deref(),
            Some("get_weather")
        );
    }

    #[test]
    fn openai_tool_calls_extracted() {
        let v = serde_json::json!({
            "choices": [{"message": {"tool_calls": [
                {"id": "c1", "type": "function", "function": {"name": "get_weather", "arguments": "{}"}}
            ]}}]
        });
        assert_eq!(
            extract_tool_call_name(&v, &Protocol::OpenAI).as_deref(),
            Some("get_weather")
        );
    }

    #[test]
    fn no_tool_call_returns_none() {
        let v = serde_json::json!({"content": [{"type": "text", "text": "hi"}]});
        assert_eq!(extract_tool_call_name(&v, &Protocol::Anthropic), None);
        let v = serde_json::json!({"choices": [{"message": {"content": "hi"}}]});
        assert_eq!(extract_tool_call_name(&v, &Protocol::OpenAI), None);
    }
}

// ── 生图连通性测试（生图-模型类型 票 10）──

crate::tauri_command! {
    /// 平台生图连通性测试（生图-模型类型 票 10）：平台 image 槽位模型发一次
    /// OpenAI Images 生成请求（`base_url + /images/generations`），
    /// 2xx 且响应 `data[]` 含可展示条目（url / b64_json）即成功；写 request_log（kind='test'），
    /// 平台卡「最近测试」徽章经 get_last_test_result 读同一张表。usage 提取与 /v1/images
    /// 出站同源（`gateway::proxy::image_usage`，票 03 token 口径）。
    pub async fn image_test(
        req: ModelTestRequest) -> Result<ModelTestResult, String> {
    let db = aidog_ctx::db();
        tracing::debug!(command = "image_test", platform_id = req.platform_id, "command invoked");

        let platform = db::get_platform(db, req.platform_id)
            .await?
            .ok_or_else(|| {
                tracing::warn!(command = "image_test", platform_id = req.platform_id, "platform not found");
                "platform not found".to_string()
            })?;

        let model = req
            .model
            .clone()
            .filter(|m| !m.trim().is_empty())
            .or_else(|| platform.models.image.first().filter(|m| !m.is_empty()).cloned())
            .ok_or_else(|| {
                tracing::warn!(command = "image_test", platform_id = req.platform_id, "no image model");
                "未配置生图模型（models.image 槽位为空）".to_string()
            })?;
        let prompt = req
            .prompt
            .clone()
            .filter(|p| !p.trim().is_empty())
            .unwrap_or_else(|| "a simple red circle on a white background".to_string());

        // 出站目标：gemini 方言平台走 Interactions 出站（票 06 同口径）；其余 OpenAI 系硬序
        //（与 /v1/images 出站同口径，candidates.rs RequestKind::Image 门一致）；零端点平台回平台主 base_url。
        let (gemini, base_url, client_type, target_protocol) = image_test_outbound(&platform);

        // base_url 缺失 guard：与 model_test 同文案（OAuth 未回填 / 手建平台漏配 → 友好错误）。
        if base_url.trim().is_empty() {
            tracing::warn!(command = "image_test", platform_id = platform.id, "base_url empty");
            return Err("base_url 缺失，请在平台 endpoints 配置上游地址".to_string());
        }

        let openai_body = serde_json::json!({ "model": model, "prompt": prompt, "n": 1 });
        // URL 构造对齐「URL 构造」约束：base_url 已含版本前缀，直接拼出站后缀（无版本段，无需 dedupe）。
        let url = if gemini {
            format!(
                "{}{}",
                base_url.trim_end_matches('/'),
                gateway::proxy::image_dialect::GEMINI_API_PATH
            )
        } else {
            format!("{}/images/generations", base_url.trim_end_matches('/'))
        };
        let eff_api_key = gateway::proxy::resolve_opencode_zen_key(&platform);
        // 出站 body / 请求头：gemini 方言先做 OpenAI→Interactions 等价转换（与网关 aux_attempt
        // 同转换；API key 走 x-goog-api-key），其余照 OpenAI Images 出站（headers 按模型客户端形态注入）。
        let (req_body_str, upstream_headers_json) = if gemini {
            let converted = gateway::proxy::image_dialect::generation_to_gemini(&openai_body, &model)?;
            (
                serde_json::to_string(&converted).unwrap_or_default(),
                serde_json::json!({
                    "x-goog-api-key": "[REDACTED]",
                    "content-type": "application/json",
                    "api-revision": gateway::proxy::image_dialect::GEMINI_API_REVISION,
                })
                .to_string(),
            )
        } else {
            let upstream_headers = gateway::proxy::build_upstream_headers_for_model(
                &client_type,
                &target_protocol,
                &eff_api_key,
                &model,
                &axum::http::HeaderMap::new(),
            );
            let headers_json = serde_json::Value::Object(
                upstream_headers
                    .iter()
                    .map(|(k, v)| (k.clone(), serde_json::Value::String(v.clone())))
                    .collect(),
            )
            .to_string();
            (serde_json::to_string(&openai_body).unwrap_or_default(), headers_json)
        };

        let http_ctx = HttpRequestContext {
            target_protocol: target_protocol.clone(),
            url,
            client_type,
            eff_api_key,
            request_id: uuid::Uuid::new_v4().simple().to_string(),
            created_at: aidog_db::now(),
            model: model.clone(),
            prompt: prompt.clone(),
            req_body_str,
            upstream_headers_json,
            start: std::time::Instant::now(),
        };

        #[allow(clippy::too_many_arguments)]
        async fn finish(
            db: &Db,
            http_ctx: &HttpRequestContext,
            platform_id: u64,
            body_override: &str,
            upstream_status: i32,
            user_status: i32,
            upstream_resp_headers: &str,
            user_resp_body: &str,
            in_tok: i32,
            out_tok: i32,
        ) {
            let mut log = build_test_proxy_log(
                http_ctx,
                platform_id,
                &http_ctx.target_protocol,
                body_override,
                upstream_status,
                user_status,
                upstream_resp_headers,
                user_resp_body,
                in_tok,
                out_tok,
            );
            // 生图测试留痕：路径与 source 与聊天测试区分（审计可分）。
            log.request_url = format!("/image-test/{}", platform_id);
            log.request_headers = r#"{"source":"image-test"}"#.into();
            log.request_body = http_ctx.req_body_str.clone();
            if let Err(le) = aidog_logs::upsert_request_log(db, log).await {
                tracing::debug!(command = "image_test", error = %le, "upsert test request_log failed");
            }
        }

        #[allow(clippy::too_many_arguments)]
        async fn fail(
            db: &Db,
            http_ctx: &HttpRequestContext,
            platform_id: u64,
            error: String,
            body_override: &str,
            upstream_status: i32,
            user_status: i32,
            upstream_resp_headers: &str,
            user_resp_body: &str,
        ) -> ModelTestResult {
            let result = ModelTestResult {
                success: false,
                model: http_ctx.model.clone(),
                prompt_preview: truncate_str(&http_ctx.prompt, 100),
                response_preview: truncate_str(body_override, 200),
                duration_ms: http_ctx.start.elapsed().as_millis() as i32,
                input_tokens: 0,
                output_tokens: 0,
                error,
            };
            finish(
                db, http_ctx, platform_id, body_override,
                upstream_status, user_status, upstream_resp_headers, user_resp_body, 0, 0,
            )
            .await;
            result
        }

        // 构建 HTTP 客户端（复用 proxy.rs 逻辑；非请求路径，现读 DB settings）
        let db_arc = std::sync::Arc::new(db.clone());
        let proxy_client_settings = gateway::http_client::load_proxy_client_settings(&db_arc).await;
        let client = gateway::http_client::build_http_client(
            &proxy_client_settings, 60, 10,
            gateway::http_client::platform_should_use_proxy(&proxy_client_settings, &platform.extra),
        ).await;

        tracing::info!(command = "image_test", method = "POST", url = %http_ctx.url, "image test request");

        let req_builder = client
            .post(&http_ctx.url)
            .header("Content-Type", "application/json")
            .body(http_ctx.req_body_str.clone());
        let req_builder = if gemini {
            // Gemini REST：API key 走 x-goog-api-key 头 + Api-Revision（与网关 aux_attempt 同口径）。
            req_builder
                .header("x-goog-api-key", &http_ctx.eff_api_key)
                .header("Api-Revision", gateway::proxy::image_dialect::GEMINI_API_REVISION)
        } else {
            gateway::proxy::apply_client_headers_for_model(
                req_builder,
                &http_ctx.client_type,
                &http_ctx.target_protocol,
                &http_ctx.eff_api_key,
                &http_ctx.model,
            )
        };

        let resp = match req_builder.send().await {
            Ok(r) => r,
            Err(e) => {
                tracing::warn!(command = "image_test", platform_id = platform.id, error = %e, "image test request failed");
                return Ok(fail(db, &http_ctx, platform.id, format!("request failed: {e}"),
                    &format!("upstream error: {e}"), 0, 502, "", &format!("upstream error: {e}")).await);
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
        let body = match resp.text().await {
            Ok(b) => b,
            Err(e) => {
                tracing::warn!(command = "image_test", platform_id = platform.id, error = %e, "image test read upstream response failed");
                return Ok(fail(db, &http_ctx, platform.id,
                    format!("read upstream response failed: {e}"),
                    &format!("upstream error: {e}"), upstream_status_code, 502,
                    &upstream_resp_headers, &format!("upstream error: {e}")).await);
            }
        };
        if !status.is_success() {
            tracing::warn!(command = "image_test", platform_id = platform.id, %status, "image test non-success upstream status");
            return Ok(fail(db, &http_ctx, platform.id, format!("HTTP {status}"),
                &body, upstream_status_code, upstream_status_code, &upstream_resp_headers, &body).await);
        }

        // gemini 方言：interaction → ImagesResponse 再走既有校验 / usage 提取（与网关同转换）；
        // 不可转换 = 上游契约不符，按失败落 request_log。
        let body = if gemini {
            match serde_json::from_str::<Value>(&body)
                .map_err(|e| e.to_string())
                .and_then(|v| gateway::proxy::image_dialect::interaction_to_openai(&v, &model))
            {
                Ok(v) => v.to_string(),
                Err(e) => {
                    tracing::warn!(command = "image_test", platform_id = platform.id, error = %e, "image test interaction not convertible");
                    return Ok(fail(
                        db, &http_ctx, platform.id,
                        format!("gemini interaction response not convertible: {e}"),
                        &body,
                        upstream_status_code, 502,
                        &upstream_resp_headers, &body,
                    ).await);
                }
            }
        } else {
            body
        };

        let (in_tok, out_tok) = gateway::proxy::image_usage(&body);
        let result = match validate_image_response(&body) {
            Ok((_n, summary)) => ModelTestResult {
                success: true,
                model,
                prompt_preview: truncate_str(&prompt, 100),
                response_preview: summary,
                duration_ms: http_ctx.start.elapsed().as_millis() as i32,
                input_tokens: in_tok,
                output_tokens: out_tok,
                error: String::new(),
            },
            Err(e) => {
                tracing::warn!(command = "image_test", platform_id = platform.id, error = %e, "image test response invalid");
                ModelTestResult {
                    success: false,
                    model,
                    prompt_preview: truncate_str(&prompt, 100),
                    response_preview: truncate_str(&body, 200),
                    duration_ms: http_ctx.start.elapsed().as_millis() as i32,
                    input_tokens: in_tok,
                    output_tokens: out_tok,
                    error: e,
                }
            }
        };

        finish(
            db, &http_ctx, platform.id, &body,
            upstream_status_code,
            if result.success { 200 } else { 422 },
            &upstream_resp_headers, &body, in_tok, out_tok,
        )
        .await;

        Ok(result)
    }
}

/// 生图测试响应校验：2xx 后 JSON `data[]` 至少一条带非空 `url` 或 `b64_json`。
/// 返回 (可展示张数, 摘要)；不满足 → Err(原因)。（口径与 aux_model::links_from_data 同：可展示条目）。
fn validate_image_response(body: &str) -> Result<(usize, String), String> {
    let v: Value = serde_json::from_str(body).map_err(|e| format!("response json parse error: {e}"))?;
    let data = v
        .get("data")
        .and_then(Value::as_array)
        .ok_or_else(|| "响应缺少 data 数组".to_string())?;
    let displayable = data
        .iter()
        .filter(|it| {
            let s = |k: &str| {
                it.get(k)
                    .and_then(Value::as_str)
                    .map(|s| !s.is_empty())
                    .unwrap_or(false)
            };
            s("url") || s("b64_json")
        })
        .count();
    if displayable == 0 {
        return Err("响应 data 条目缺少 url / b64_json".to_string());
    }
    Ok((displayable, format!("{displayable} image(s) generated")))
}

/// 生图测试出站目标：gemini 方言平台（`extra.image_dialect=gemini`）走 Interactions 出站
///（与 /v1/images 网关路径同口径，票 06）；其余 OpenAI 系硬序（与网关 RequestKind::Image
/// 门同口径）；零端点平台回平台主 base_url（aux_model 同兜底）。
/// 返回 (gemini 方言, base_url, endpoint client_type, 目标协议)。
fn image_test_outbound(platform: &gateway::models::Platform) -> (bool, String, String, Protocol) {
    let gemini = aidog_db::image_dialect_is_gemini(&platform.extra);
    if gemini {
        return (
            true,
            platform.base_url.clone(),
            String::new(),
            platform.platform_type.clone(),
        );
    }
    let (base_url, client_type, target_protocol) =
        gateway::proxy::select_endpoint_for_openai_family(&platform.endpoints)
            .map(|ep| (ep.base_url.clone(), ep.client_type.clone(), ep.protocol.clone()))
            .unwrap_or_else(|| {
                (
                    platform.base_url.clone(),
                    aidog_db::registry::derive_client_type_for_platform(
                        aidog_db::registry::PlatformCode(&platform.platform_type.wire_str()),
                    ),
                    platform.platform_type.clone(),
                )
            });
    (gemini, base_url, client_type, target_protocol)
}

#[cfg(test)]
mod test_image_outbound {
    use super::*;

    fn platform_from(extra: &str, endpoints: serde_json::Value) -> gateway::models::Platform {
        serde_json::from_value(serde_json::json!({
            "id": 1,
            "name": "test",
            "platform_type": "gemini",
            "base_url": "https://g.example",
            "api_key": "",
            "extra": extra,
            "models": {},
            "available_models": [],
            "created_at": 0,
            "updated_at": 0,
            "endpoints": endpoints,
        }))
        .unwrap()
    }

    /// gemini 方言平台 → Interactions 出站（平台主 base_url），不走 OpenAI Images 硬序
    ///（gemini-only 端点平台旧逻辑测试必败，而网关 /v1/images 经方言转换本可成功）。
    #[test]
    fn gemini_dialect_platform_uses_interactions_outbound() {
        let platform = platform_from(
            r#"{"image_dialect":"gemini"}"#,
            serde_json::json!([{"protocol":"gemini","base_url":"https://g.example/v1beta","client_type":"default","coding_plan":false}]),
        );
        let (gemini, base_url, _ct, proto) = image_test_outbound(&platform);
        assert!(gemini, "gemini 方言走 Interactions 出站");
        assert_eq!(base_url, "https://g.example", "gemini 走平台主 base_url");
        assert_eq!(proto, Protocol::Gemini);
    }

    /// 非 gemini 方言平台维持 OpenAI Images 硬序（既有行为不变）。
    #[test]
    fn openai_endpoint_platform_keeps_images_outbound() {
        let platform = platform_from(
            "",
            serde_json::json!([{"protocol":"openai","base_url":"https://o.example/v1","client_type":"default","coding_plan":false}]),
        );
        let (gemini, base_url, _ct, proto) = image_test_outbound(&platform);
        assert!(!gemini);
        assert_eq!(base_url, "https://o.example/v1");
        assert_eq!(proto, Protocol::OpenAI);
    }
}

#[cfg(test)]
mod test_image_response {
    use super::*;
    #[test]
    fn b64_entries_validate() {
        let body = serde_json::json!({
            "data": [{"b64_json": "aGVsbG8="}, {"url": "https://x/img.png"}]
        })
        .to_string();
        let (n, summary) = validate_image_response(&body).expect("both entries valid");
        assert_eq!(n, 2);
        assert!(summary.contains("2"));
    }

    #[test]
    fn empty_data_fails() {
        let body = serde_json::json!({ "data": [] }).to_string();
        assert!(validate_image_response(&body).is_err());
    }

    #[test]
    fn no_display_entry_fails() {
        let body = serde_json::json!({ "data": [{"revised_prompt": "x"}] }).to_string();
        let err = match validate_image_response(&body) {
            Err(e) => e,
            Ok(_) => panic!("expected validation failure"),
        };
        assert!(err.contains("url") || err.contains("b64_json"), "实际: {err}");
    }

    #[test]
    fn missing_data_array_fails() {
        assert!(validate_image_response(r#"{"ok":true}"#).is_err());
    }

    #[test]
    fn invalid_json_fails() {
        assert!(validate_image_response("not json").is_err());
    }
}

#[cfg(test)]
mod test_prepare_http_request {
    use super::*;
    use aidog_adapter::{ChatRequest, Message, MessageContent, Role};

    fn make_ctx(base_url: &str, endpoints: Vec<gateway::models::PlatformEndpoint>) -> TestContext {
        let platform = serde_json::from_value(serde_json::json!({
            "id": 1,
            "name": "test",
            "platform_type": "openai",
            "base_url": base_url,
            "api_key": "",
            "extra": "",
            "models": {},
            "available_models": [],
            "created_at": 0,
            "updated_at": 0,
            "endpoints": endpoints,
        }))
        .unwrap();
        TestContext {
            platform,
            model: "gpt-4".to_string(),
            prompt: "hi".to_string(),
            expected: None,
            chat_req: ChatRequest {
                thinking_budget: None,
                model: "gpt-4".to_string(),
                messages: vec![Message {
                    role: Role::User,
                    content: MessageContent::Text("hi".to_string()),
                }],
                system: None,
                max_tokens: Some(16),
                temperature: None,
                top_p: None,
                stream: Some(false),
                tools: None,
                tool_choice: None,
                extra: None,
                thinking_mode: None,
            },
        }
    }

    #[test]
    fn empty_base_url_no_endpoints_returns_friendly_error() {
        // 平台 base_url 空 + 无 endpoints → guard 触发返友好错误，非 reqwest builder error。
        let ctx = make_ctx("", vec![]);
        let err = prepare_http_request(&ctx).err().expect("expected error");
        assert!(
            err.contains("base_url"),
            "错误文案应含 base_url，实际: {err}"
        );
        assert!(
            !err.to_lowercase().contains("builder"),
            "不应是 reqwest builder error"
        );
    }

    #[test]
    fn whitespace_base_url_returns_friendly_error() {
        // 纯空白 base_url 视为缺失（trim 后空）。
        let ctx = make_ctx("   ", vec![]);
        let err = prepare_http_request(&ctx).err().expect("expected error");
        assert!(err.contains("base_url"));
    }

    #[test]
    fn empty_endpoint_base_url_falls_back_to_platform_then_errors() {
        // endpoints 存在但端点 base_url 空 + 平台 base_url 空 → 仍 guard 拦截。
        let ep = serde_json::from_value(serde_json::json!({
            "protocol": "openai", "base_url": "", "client_type": "default", "coding_plan": false,
        }))
        .unwrap();
        let ctx = make_ctx("", vec![ep]);
        let err = prepare_http_request(&ctx).err().expect("expected error");
        assert!(err.contains("base_url"));
    }

    #[test]
    fn non_empty_base_url_proceeds_past_guard() {
        // 正常 base_url → 通过 guard（后续 url 拼接成功，不验证完整 HTTP 链路）。
        let ctx = make_ctx("https://api.openai.com/v1", vec![]);
        let http_ctx = prepare_http_request(&ctx).expect("non-empty base_url should pass guard");
        assert!(http_ctx.url.starts_with("https://api.openai.com/v1"));
    }
}
