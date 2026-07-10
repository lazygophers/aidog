use aidog_core::gateway::{self, db::{self, Db}};
#[allow(unused_imports)]
use aidog_core::logging;
#[allow(unused_imports)]
use gateway::models::*;
#[allow(unused_imports)]
use tauri::State;
#[allow(unused_imports)]
use serde_json::Value;
#[allow(unused_imports)]
use std::sync::Arc;
#[allow(unused_imports)]
use tauri::Manager;


#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %aidog_core::logging::new_trace_id()))]
pub async fn model_test(
    db: State<'_, Db>,
    req: ModelTestRequest,
) -> Result<ModelTestResult, String> {
    tracing::debug!(command = "model_test", platform_id = req.platform_id, "command invoked");
    let platform = db::get_platform(&db, req.platform_id).await?
        .ok_or_else(|| { tracing::warn!(command = "model_test", platform_id = req.platform_id, "platform not found"); "platform not found".to_string() })?;

    let model = req.model.clone().or(platform.models.default.clone())
        .ok_or_else(|| { tracing::warn!(command = "model_test", platform_id = req.platform_id, "no model specified and no default model configured"); "no model specified and no default model configured".to_string() })?;

    // prompt 来源二选一：
    //   - req.prompt 有值 → 自定义模式（ModelTestPanel）：跳过随机 + 跳过内容校验，success=响应非空。
    //   - req.prompt 为空 → 默认/快速测试：随机生成可校验题（算术/常识轮换），expected 用于子串校验。
    let (prompt, expected) = match req.prompt.clone() {
        Some(p) => (p, None),
        None => {
            let (p, e) = random_test_challenge();
            (p, Some(e))
        }
    };

    let chat_req = gateway::adapter::ChatRequest {
        model: model.clone(),
        messages: vec![gateway::adapter::Message {
            role: gateway::adapter::Role::User,
            content: gateway::adapter::MessageContent::Text(prompt.clone()),
        }],
        system: None,
        // 默认 max_tokens 需容纳推理模型（如 MiniMax-M3）的 <think> 前导：
        //   只给 16 token 会被思维链吃光，finish_reason=length，答案（expected 子串）
        //   永不出现 → 内容校验失败 → 健康模型被误判 422。给足 1024 让答案有空间产出。
        //   自定义模式（req.prompt 有值、跳过内容校验）调用方仍可显式传 req.max_tokens 收窄。
        max_tokens: Some(req.max_tokens.unwrap_or(1024)),
        // 不强制 temperature：部分模型（如 Kimi coding plan）只允许 temperature=1，
        // 发任何其他值会被上游 400 拒绝。省略让上游用模型默认值，避开所有挑剔 temperature 的模型。
        temperature: None,
        top_p: None,
        stream: Some(false),
        tools: None,
        tool_choice: None,
        extra: None,
    };

    // 优先使用 endpoint 匹配（同 proxy 逻辑），回退到平台主配置
    // model-test 优先选 coding_plan endpoint（测试 coding 端点更有意义），否则取第一个
    let (target_protocol, target_base_url, client_type, coding_plan) = if !platform.endpoints.is_empty() {
        let ep = platform.endpoints.iter().find(|ep| ep.coding_plan)
            .unwrap_or(&platform.endpoints[0]);
        (ep.protocol.clone(), ep.base_url.clone(), ep.client_type.clone(), ep.coding_plan)
    } else {
        (platform.platform_type.clone(), platform.base_url.clone(), ClientType::default(), false)
    };

    let (mut req_body, mut api_path) = gateway::adapter::convert_request(&chat_req, &target_protocol, &platform.platform_type);
    // coding plan 注入（与 proxy.rs 对齐）
    if coding_plan {
        gateway::proxy::inject_coding_plan_fields(&mut req_body, &target_protocol);
        gateway::proxy::override_coding_plan_path(&mut api_path, &target_protocol);
    }
    let req_body_str = serde_json::to_string(&req_body).unwrap_or_default();
    let base_url = target_base_url.trim_end_matches('/');
    let url = format!("{}{}", base_url, api_path);

    // OpenCode Zen api_key 兜底（与 proxy.rs 路径对齐，model-test proxy parity）。
    let eff_api_key = gateway::proxy::resolve_opencode_zen_key(&platform);

    // ── 使用与 proxy 相同的客户端 header 模拟逻辑 ──
    // model_test 无入站请求头（平台测试），传空 HeaderMap —— 仅 apply 模拟头，无透传。
    let upstream_headers = gateway::proxy::build_upstream_headers(&client_type, &target_protocol, &eff_api_key, &axum::http::HeaderMap::new(), &url);

    let db_arc = Arc::new(db.inner().clone());
    let client = gateway::http_client::build_http_client(
        &db_arc, 30, 10, Some(&platform.extra), None,
    ).await;

    let start = std::time::Instant::now();
    let request_id = uuid::Uuid::new_v4().simple().to_string();
    let created_at = gateway::db::now();

    let req_builder = client
        .post(&url)
        .header("Content-Type", "application/json")
        .body(req_body_str.clone());
    let req_builder = gateway::proxy::apply_client_headers(req_builder, &client_type, &target_protocol, &eff_api_key);

    // ── 辅助: 构造测试日志 ──
    let make_log = |body_override: &str, upstream_status: i32, user_status: i32,
                     upstream_resp_headers: &str, user_resp_body: &str,
                     in_tok: i32, out_tok: i32| -> gateway::models::ProxyLog {
        gateway::models::ProxyLog {
            id: request_id.clone(),
            group_key: "[test]".into(),
            model: model.clone(),
            actual_model: model.clone(),
            source_protocol: "test".into(),
            target_protocol: format!("{:?}", target_protocol).to_lowercase(),
            platform_id: platform.id,
            request_headers: r#"{"source":"model-test"}"#.into(),
            request_body: serde_json::to_string(&serde_json::json!({"messages":[{"role":"user","content":prompt}]})).unwrap_or_default(),
            upstream_request_headers: serde_json::Value::Object(
                upstream_headers.iter().map(|(k, v)| (k.clone(), serde_json::Value::String(v.clone()))).collect()
            ).to_string(),
            upstream_request_body: req_body_str.clone(),
            response_body: body_override.into(),
            request_url: format!("/model-test/{}", platform.id),
            upstream_request_url: url.clone(),
            upstream_response_headers: upstream_resp_headers.into(),
            upstream_status_code: upstream_status,
            user_response_headers: r#"{"content-type":"application/json"}"#.to_string(),
            user_response_body: user_resp_body.into(),
            status_code: user_status,
            duration_ms: start.elapsed().as_millis() as i32,
            input_tokens: in_tok,
            output_tokens: out_tok,
            cache_tokens: 0,
            est_cost: 0.0,
            is_stream: false,
            attempts: Vec::new(),
            retry_count: 0,
            blocked_by: String::new(),
            blocked_reason: String::new(),
            created_at,
            updated_at: created_at,
            deleted_at: 0,
        }
    };

    // ── Mock 平台：本地生成响应（不发真实 HTTP），与 proxy handle_mock 对齐。
    //   model_test 入站协议固定 "test"；mock build_response 对未知协议走默认 Anthropic 格式。
    //   response_preview 直接取 cfg.response_text（mock 配置的响应文本），无需解析响应体。
    if matches!(target_protocol, Protocol::Mock) {
        let cfg = gateway::adapter::mock::resolve_mock_config(&platform.extra, &chat_req, &req_body);
        if cfg.delay_ms > 0 {
            tokio::time::sleep(std::time::Duration::from_millis(cfg.delay_ms)).await;
        }
        let source_proto_str = "test";
        let (success, status_code, resp_body, err_msg, in_tok, out_tok, preview): (bool, u16, String, String, i32, i32, String) = match cfg.error_mode.as_str() {
            "http_error" => {
                let body = gateway::adapter::mock::build_error_body(source_proto_str, cfg.status_code, "mock http_error");
                let body_str = serde_json::to_string(&body).unwrap_or_default();
                (false, cfg.status_code, body_str, format!("mock http_error (status {})", cfg.status_code), 0, 0, String::new())
            }
            "rate_limit_429" => {
                let body = gateway::adapter::mock::build_error_body(source_proto_str, 429, "mock rate limit");
                let body_str = serde_json::to_string(&body).unwrap_or_default();
                (false, 429, body_str, "mock rate_limit_429".to_string(), 0, 0, String::new())
            }
            "timeout" => {
                // model_test 不真 hang（proxy 里 sleep 600s 是为让客户端超时）；直接返回 504。
                let body = gateway::adapter::mock::build_error_body(source_proto_str, 504, "mock timeout");
                let body_str = serde_json::to_string(&body).unwrap_or_default();
                (false, 504, body_str, "mock timeout".to_string(), 0, 0, String::new())
            }
            _ => {
                let body = gateway::adapter::mock::build_response(&cfg, source_proto_str, &model);
                let body_str = serde_json::to_string(&body).unwrap_or_default();
                (true, 200, body_str, String::new(), cfg.input_tokens, cfg.output_tokens, cfg.response_text.clone())
            }
        };
        let duration_ms = start.elapsed().as_millis() as i32;
        let log_entry = make_log(&resp_body, status_code as i32, status_code as i32, r#"{"content-type":"application/json"}"#, &resp_body, in_tok, out_tok);
        if let Err(le) = db::upsert_proxy_log(&db, log_entry).await {
            tracing::warn!(command = "model_test", platform_id = platform.id, error = %le, "persist mock test log failed");
        }
        tracing::info!(command = "model_test", platform_id = platform.id, mock = true, success, status = status_code, "model test mock response");
        return Ok(ModelTestResult {
            success,
            model: model.clone(),
            prompt_preview: truncate_str(&prompt, 100),
            response_preview: preview,
            duration_ms,
            input_tokens: in_tok,
            output_tokens: out_tok,
            error: err_msg,
        });
    }

    tracing::info!(method = "POST", url = %url, "model test request");
    tracing::debug!(method = "POST", url = %url, body = %gateway::log_util::log_body_preview(&req_body_str), "model test request body");
    let resp = match req_builder.send().await {
        Ok(r) => r,
        Err(e) => {
            let result = ModelTestResult {
                success: false,
                model: model.clone(),
                prompt_preview: truncate_str(&prompt, 100),
                response_preview: String::new(),
                duration_ms: start.elapsed().as_millis() as i32,
                input_tokens: 0,
                output_tokens: 0,
                error: format!("request failed: {e}"),
            };
            tracing::warn!(command = "model_test", platform_id = platform.id, error = %e, "model test request failed");
            if let Err(le) = db::upsert_proxy_log(&db, make_log(
                &format!("upstream error: {e}"), 0, 502, "", &format!("upstream error: {e}"), 0, 0,
            )).await {
                tracing::debug!(command = "model_test", error = %le, "upsert test proxy_log failed");
            }
            return Ok(result);
        }
    };

    let duration_ms = start.elapsed().as_millis() as i32;
    let upstream_status_code = resp.status().as_u16() as i32;
    let status = resp.status();

    // 捕获上游响应头
    let upstream_resp_headers = {
        let mut h = serde_json::Map::new();
        for (k, v) in resp.headers() {
            if let Ok(s) = v.to_str() {
                h.insert(k.to_string(), serde_json::Value::String(s.to_string()));
            }
        }
        serde_json::Value::Object(h).to_string()
    };

    let body = resp.text().await.unwrap_or_default();

    if !status.is_success() {
        let result = ModelTestResult {
            success: false,
            model: model.clone(),
            prompt_preview: truncate_str(&prompt, 100),
            response_preview: truncate_str(&body, 200),
            duration_ms,
            input_tokens: 0,
            output_tokens: 0,
            error: format!("HTTP {}", status),
        };
        tracing::warn!(command = "model_test", platform_id = platform.id, %status, "model test non-success upstream status");
        if let Err(le) = db::upsert_proxy_log(&db, make_log(
            &body, upstream_status_code, upstream_status_code,
            &upstream_resp_headers, &body, 0, 0,
        )).await {
            tracing::debug!(command = "model_test", error = %le, "upsert test proxy_log failed");
        }
        return Ok(result);
    }

    let resp_json: serde_json::Value = serde_json::from_str(&body).unwrap_or_default();
    let response_text = extract_response_text(&resp_json, &target_protocol);
    let (in_tok, out_tok) = extract_test_usage(&resp_json, &target_protocol);

    // 内容校验：
    //   - expected 有值（随机可校验题）：归一化后响应须含 expected 子串，否则失败。
    //     容忍模型自然长答（含解释/标点），只要关键词出现即算通过。
    //   - expected 为 None（自定义 prompt）：跳过内容校验，仅要求响应非空。
    let success = verify_test_response(&response_text, expected.as_deref());
    let error = if success { String::new() } else { "响应内容校验失败".to_string() };

    let result = ModelTestResult {
        success,
        model: model.clone(),
        prompt_preview: truncate_str(&prompt, 100),
        response_preview: truncate_str(&response_text, 300),
        duration_ms,
        input_tokens: in_tok,
        output_tokens: out_tok,
        error,
    };

    if let Err(le) = db::upsert_proxy_log(&db, make_log(
        &body, upstream_status_code, if success { 200 } else { 422 },
        &upstream_resp_headers, &body, in_tok, out_tok,
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
