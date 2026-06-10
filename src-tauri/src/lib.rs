mod gateway;
mod logging;

use gateway::db::{self, Db};
use gateway::models::*;
use tauri::State;
use serde_json::Value;

// ─── Helpers ───────────────────────────────────────────────

/// Convert any string to a slug: lowercase, alphanumeric + hyphens only.
/// Chinese/special chars are transliterated or stripped.
fn slugify(input: &str) -> String {
    input
        .to_lowercase()
        .replace(" ", "-")
        .replace("（", "-")
        .replace("）", "")
        .replace("(", "-")
        .replace(")", "")
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else if c == ' ' {
                '-'
            } else {
                // Strip non-ASCII non-alphanumeric (Chinese chars etc.)
                '\0'
            }
        })
        .filter(|c| *c != '\0')
        .collect::<String>()
        // Collapse multiple hyphens
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

/// Validate group name is a valid slug (lowercase alphanumeric + hyphen)
fn validate_group_name(name: &str) -> Result<(), String> {
    if name.is_empty() {
        return Err("group name cannot be empty".to_string());
    }
    if !name.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-') {
        return Err(format!(
            "group name '{}' must contain only lowercase letters, digits, and hyphens",
            name
        ));
    }
    if name.starts_with('-') || name.ends_with('-') {
        return Err("group name cannot start or end with hyphen".to_string());
    }
    Ok(())
}

/// 为所有平台确保存在关联的自动分组（一个平台一个，相互独立）
fn ensure_platform_groups(db: &Db) {
    let platforms = match db::list_platforms(db) {
        Ok(p) => p,
        Err(e) => { tracing::error!("ensure_platform_groups: list_platforms failed: {e}"); return; }
    };
    for platform in &platforms {
        // 检查是否已存在关联此平台的分组
        let groups = db::list_groups(db).unwrap_or_default();
        let exists = groups.iter().any(|g| g.auto_from_platform.as_deref() == Some(&platform.id));
        if exists {
            continue;
        }
        // 自动创建分组 — path 用平台 ID 前缀避免同名协议冲突
        let protocol_str = format!("{:?}", platform.protocol).to_lowercase();
        let short_id = &platform.id[..8.min(platform.id.len())];
        let group_path = format!("/{}-{}", protocol_str, short_id);
        let group_name = slugify(&format!("{}-auto", platform.name));
        let group = match db::create_group(db, CreateGroup {
            name: group_name.clone(),
            path: group_path.clone(),
            routing_mode: RoutingMode::Failover,
            auto_from_platform: Some(platform.id.clone()),
            request_timeout_secs: 0,
            connect_timeout_secs: 0,
            source_protocol: None,
        }) {
            Ok(g) => g,
            Err(e) => { tracing::error!("ensure_platform_groups: create_group failed for {}: {e}", platform.name); continue; }
        };
        // 将平台关联到自动分组
        if let Err(e) = db::set_group_platforms(db, &group.id, &[GroupPlatformInput {
            platform_id: platform.id.clone(),
            priority: Some(0),
            weight: Some(1),
        }]) {
            tracing::error!("ensure_platform_groups: set_group_platforms failed for {}: {e}", platform.name);
        }
        tracing::info!("ensure_platform_groups: created group '{}' path='{}' for platform '{}'", group_name, group_path, platform.name);
    }
}

// ─── Platform Commands ─────────────────────────────────────

#[tauri::command]
fn platform_create(input: CreatePlatform, db: State<'_, Db>) -> Result<Platform, String> {
    let platform = db::create_platform(&db, input)?;

    // 自动创建分组，path 按 protocol + 平台短 ID 生成
    let protocol_str = format!("{:?}", platform.protocol).to_lowercase();
    let short_id = &platform.id[..8.min(platform.id.len())];
    let group_path = format!("/{}-{}", protocol_str, short_id);
    let group_name = slugify(&format!("{}-auto", platform.name));

    let group = db::create_group(
        &db,
        CreateGroup {
            name: group_name,
            path: group_path,
            routing_mode: RoutingMode::Failover,
            auto_from_platform: Some(platform.id.clone()),
            request_timeout_secs: 0,
            connect_timeout_secs: 0,
            source_protocol: None,
        },
    )?;

    // 将平台关联到自动分组
    db::set_group_platforms(
        &db,
        &group.id,
        &[GroupPlatformInput {
            platform_id: platform.id.clone(),
            priority: Some(0),
            weight: Some(1),
        }],
    )?;

    Ok(platform)
}

#[tauri::command]
fn platform_list(db: State<'_, Db>) -> Result<Vec<Platform>, String> {
    db::list_platforms(&db)
}

#[tauri::command]
fn platform_get(id: String, db: State<'_, Db>) -> Result<Option<Platform>, String> {
    db::get_platform(&db, &id)
}

#[tauri::command]
fn platform_update(input: UpdatePlatform, db: State<'_, Db>) -> Result<Platform, String> {
    let platform = db::update_platform(&db, input)?;
    // 确保该平台有关联分组，若无则自动创建
    let groups = db::list_groups(&db).unwrap_or_default();
    let exists = groups.iter().any(|g| g.auto_from_platform.as_deref() == Some(&platform.id));
    if !exists {
        let protocol_str = format!("{:?}", platform.protocol).to_lowercase();
        let short_id = &platform.id[..8.min(platform.id.len())];
        let group_path = format!("/{}-{}", protocol_str, short_id);
        let group_name = slugify(&format!("{}-auto", platform.name));
        if let Ok(group) = db::create_group(&db, CreateGroup {
            name: group_name,
            path: group_path,
            routing_mode: RoutingMode::Failover,
            auto_from_platform: Some(platform.id.clone()),
            request_timeout_secs: 0,
            connect_timeout_secs: 0,
            source_protocol: None,
        }) {
            let _ = db::set_group_platforms(&db, &group.id, &[GroupPlatformInput {
                platform_id: platform.id.clone(),
                priority: Some(0),
                weight: Some(1),
            }]);
        }
    }
    Ok(platform)
}

#[tauri::command]
fn platform_delete(id: String, db: State<'_, Db>) -> Result<(), String> {
    db::delete_platform(&db, &id)
}

// ─── Group Commands ────────────────────────────────────────

#[tauri::command]
fn group_create(mut input: CreateGroup, db: State<'_, Db>, app: tauri::AppHandle) -> Result<Group, String> {
    // Auto-slugify and validate group name
    input.name = slugify(&input.name);
    validate_group_name(&input.name)?;
    let result = db::create_group(&db, input)?;
    try_sync_settings(&app, &db);
    Ok(result)
}

#[tauri::command]
fn group_list(db: State<'_, Db>) -> Result<Vec<Group>, String> {
    db::list_groups(&db)
}

#[tauri::command]
fn group_get(id: String, db: State<'_, Db>) -> Result<Option<Group>, String> {
    db::get_group(&db, &id)
}

#[tauri::command]
fn group_update(mut input: UpdateGroup, db: State<'_, Db>, app: tauri::AppHandle) -> Result<Group, String> {
    // Auto-slugify and validate if name is being updated
    if let Some(ref name) = input.name {
        let slug = slugify(name);
        validate_group_name(&slug)?;
        input.name = Some(slug);
    }
    let result = db::update_group(&db, input)?;
    try_sync_settings(&app, &db);
    Ok(result)
}

#[tauri::command]
fn group_delete(id: String, db: State<'_, Db>, app: tauri::AppHandle) -> Result<(), String> {
    db::delete_group(&db, &id)?;
    try_sync_settings(&app, &db);
    Ok(())
}

// ─── GroupPlatform Commands ────────────────────────────────

#[tauri::command]
fn group_set_platforms(input: SetGroupPlatforms, db: State<'_, Db>, app: tauri::AppHandle) -> Result<(), String> {
    db::set_group_platforms(&db, &input.group_id, &input.platforms)?;
    try_sync_settings(&app, &db);
    Ok(())
}

#[tauri::command]
fn group_get_platforms(
    group_id: String,
    db: State<'_, Db>,
) -> Result<Vec<GroupPlatformDetail>, String> {
    db::get_group_platforms(&db, &group_id)
}

// ─── ModelMapping Commands ─────────────────────────────────

#[tauri::command]
fn mapping_create(input: CreateModelMapping, db: State<'_, Db>, app: tauri::AppHandle) -> Result<ModelMapping, String> {
    let result = db::create_model_mapping(&db, input)?;
    try_sync_settings(&app, &db);
    Ok(result)
}

#[tauri::command]
fn mapping_list(group_id: String, db: State<'_, Db>) -> Result<Vec<ModelMapping>, String> {
    db::list_model_mappings(&db, &group_id)
}

#[tauri::command]
fn mapping_update(input: UpdateModelMapping, db: State<'_, Db>, app: tauri::AppHandle) -> Result<ModelMapping, String> {
    let result = db::update_model_mapping(&db, input)?;
    try_sync_settings(&app, &db);
    Ok(result)
}

#[tauri::command]
fn mapping_delete(id: String, db: State<'_, Db>, app: tauri::AppHandle) -> Result<(), String> {
    db::delete_model_mapping(&db, &id)?;
    try_sync_settings(&app, &db);
    Ok(())
}

// ─── Aggregate ─────────────────────────────────────────────

#[tauri::command]
fn group_detail(id: String, db: State<'_, Db>) -> Result<Option<GroupDetail>, String> {
    db::get_group_detail(&db, &id)
}

#[tauri::command]
fn group_detail_list(db: State<'_, Db>) -> Result<Vec<GroupDetail>, String> {
    db::list_group_details(&db)
}

// ─── Proxy Commands ────────────────────────────────────────

use std::sync::Mutex as StdMutex;
use tokio::task::JoinHandle;

/// 代理服务器状态
struct ProxyHandle(StdMutex<Option<JoinHandle<()>>>);

#[tauri::command]
async fn proxy_start(
    port: u16,
    app: tauri::AppHandle,
) -> Result<String, String> {
    // 检查是否已运行
    let handle = app.state::<ProxyHandle>();
    {
        let h = handle.0.lock().map_err(|e| e.to_string())?;
        if h.is_some() {
            return Err("proxy already running".to_string());
        }
    }

    // 获取 DB 的路径并克隆一份连接
    let db_path = aidog_data_dir()?.join("aidog.db");
    let proxy_db = Db::new(db_path.to_str().unwrap_or(""))?;
    let proxy_db = std::sync::Mutex::new(proxy_db);

    let (proxy_handle, actual_port) = gateway::proxy::start_proxy(proxy_db, port).await?;

    {
        let mut h = handle.0.lock().map_err(|e| e.to_string())?;
        *h = Some(proxy_handle);
    }

    // 保存实际使用的端口到设置
    save_proxy_settings(&app, actual_port, true)?;

    // 同步所有分组的 settings 文件（端口可能变了）
    if let Some(db) = app.try_state::<Db>() {
        let _ = do_sync_group_settings(&db, actual_port);
    }

    // 更新托盘菜单
    refresh_tray_menu(&app)?;

    let msg = if actual_port != port {
        format!("proxy started on port {} ({} was occupied)", actual_port, port)
    } else {
        format!("proxy started on port {}", actual_port)
    };
    Ok(msg)
}

#[tauri::command]
async fn proxy_stop(app: tauri::AppHandle) -> Result<(), String> {
    let handle = app.state::<ProxyHandle>();
    {
        let mut h = handle.0.lock().map_err(|e| e.to_string())?;
        if let Some(jh) = h.take() {
            jh.abort();
        }
    }

    // 更新设置
    if let Ok(settings) = load_proxy_settings(&app) {
        save_proxy_settings(&app, settings.port, false)?;
    }

    refresh_tray_menu(&app)?;
    Ok(())
}

#[tauri::command]
fn proxy_status(app: tauri::AppHandle) -> Result<bool, String> {
    let handle = app.state::<ProxyHandle>();
    let h = handle.0.lock().map_err(|e| e.to_string())?;
    Ok(h.is_some())
}

#[tauri::command]
fn proxy_get_settings(app: tauri::AppHandle) -> Result<ProxySettings, String> {
    load_proxy_settings(&app)
}

#[tauri::command]
fn proxy_set_autostart(app: tauri::AppHandle, enabled: bool) -> Result<(), String> {
    let current = load_proxy_settings(&app)?;
    save_proxy_settings(&app, current.port, enabled)?;
    Ok(())
}

// ─── Platform Model Fetch ──────────────────────────────────

#[tauri::command]
async fn platform_fetch_models(
    protocol: Protocol,
    base_url: String,
    api_key: String,
) -> Result<Vec<String>, String> {
    let client = reqwest::Client::new();
    let base = base_url.trim_end_matches('/');

    let resp: Value = match protocol {
        Protocol::Anthropic => {
            let url = format!("{base}/v1/models");
            client
                .get(&url)
                .header("x-api-key", &api_key)
                .header("anthropic-version", "2023-06-01")
                .send()
                .await
                .map_err(|e| format!("fetch models: {e}"))?
                .json()
                .await
                .map_err(|e| format!("parse response: {e}"))?
        }
        Protocol::Bailian => {
            let url = format!("{base}/compatible-mode/v1/models");
            tracing::info!("fetch models: {url}");
            let resp = client
                .get(&url)
                .header("Authorization", format!("Bearer {api_key}"))
                .send()
                .await
                .map_err(|e| {
                    tracing::error!("fetch models request failed: {e}");
                    format!("fetch models: {e}")
                })?;
            let status = resp.status();
            let body = resp.text().await.map_err(|e| format!("read body: {e}"))?;
            tracing::info!("fetch models response status={status}, body_len={}", body.len());
            serde_json::from_str::<Value>(&body)
                .map_err(|e| {
                    tracing::error!("parse response failed: {e}, body={}", &body[..body.len().min(500)]);
                    format!("parse response: {e}")
                })?
        }
        Protocol::OpenAI | Protocol::Codex | Protocol::Glm | Protocol::GlmEn | Protocol::Kimi | Protocol::MiniMax | Protocol::MiniMaxEn | Protocol::Gemini | Protocol::OpenAIResponses | Protocol::OpenAICompletions | Protocol::BailianCoding | Protocol::DeepSeek | Protocol::StepFun | Protocol::StepFunEn | Protocol::Doubao | Protocol::DoubaoSeed | Protocol::BytePlus | Protocol::QianFan | Protocol::XiaomiMimo | Protocol::BaiLing | Protocol::Longcat | Protocol::OpenRouter | Protocol::SiliconFlow | Protocol::SiliconFlowEn | Protocol::AiHubMix | Protocol::DmxApi | Protocol::ModelScope | Protocol::ShengSuanYun | Protocol::AtlasCloud | Protocol::Novita | Protocol::TheRouter | Protocol::CherryIn | Protocol::PackyCode | Protocol::Cubence | Protocol::AiGoCode | Protocol::RightCode | Protocol::AiCodeMirror | Protocol::Nvidia | Protocol::Pateway | Protocol::CcSub | Protocol::ApiKeyFun | Protocol::ApiNebula | Protocol::SudoCode | Protocol::ClaudeApi | Protocol::ClaudeCN | Protocol::RunApi | Protocol::RelaxyCode | Protocol::CrazyRouter | Protocol::SssAiCode | Protocol::Compshare | Protocol::CompshareCoding | Protocol::Micu | Protocol::CTok | Protocol::EFlowCode | Protocol::LemonData | Protocol::PipeLlm | Protocol::OpenCode => {
            let url = format!("{base}/models");
            tracing::info!("fetch models: {url}");
            client
                .get(&url)
                .header("Authorization", format!("Bearer {api_key}"))
                .send()
                .await
                .map_err(|e| format!("fetch models: {e}"))?
                .json()
                .await
                .map_err(|e| format!("parse response: {e}"))?
        }
    };

    // 解析 {"data": [{"id": "..."}, ...]} 格式
    let models = resp
        .get("data")
        .and_then(|d| d.as_array())
        .map(|arr| {
            let mut ids: Vec<String> = arr
                .iter()
                .filter_map(|item| item.get("id").and_then(|v| v.as_str()).map(String::from))
                .collect();
            ids.sort();
            ids
        })
        .unwrap_or_default();

    Ok(models)
}

// ─── Statistics ─────────────────────────────────────────────

#[tauri::command]
async fn stats_query(
    db: State<'_, Db>,
    query: StatsQuery,
) -> Result<StatsResult, String> {
    db::query_stats(&db, &query)
}

// ─── Model Testing ─────────────────────────────────────────

#[tauri::command]
async fn model_test(
    db: State<'_, Db>,
    req: ModelTestRequest,
) -> Result<ModelTestResult, String> {
    let platform = db::get_platform(&db, &req.platform_id)?
        .ok_or("platform not found")?;

    let model = req.model.clone().or(platform.models.default.clone())
        .ok_or("no model specified and no default model configured")?;

    let prompt = req.prompt.clone().unwrap_or_else(|| {
        let idx = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as usize % TEST_PROMPTS.len();
        TEST_PROMPTS[idx].to_string()
    });

    let chat_req = gateway::adapter::ChatRequest {
        model: model.clone(),
        messages: vec![gateway::adapter::Message {
            role: gateway::adapter::Role::User,
            content: gateway::adapter::MessageContent::Text(prompt.clone()),
        }],
        system: None,
        max_tokens: Some(req.max_tokens.unwrap_or(64)),
        temperature: Some(0.0),
        top_p: None,
        stream: Some(false),
        tools: None,
        tool_choice: None,
        extra: None,
    };

    // 优先使用 endpoint 匹配（同 proxy 逻辑），回退到平台主配置
    let (target_protocol, target_base_url, client_type) = if !platform.endpoints.is_empty() {
        let ep = &platform.endpoints[0];
        (ep.protocol.clone(), ep.base_url.clone(), ep.client_type.clone())
    } else {
        (platform.protocol.clone(), platform.base_url.clone(), ClientType::default())
    };

    let (req_body, api_path) = gateway::adapter::convert_request(&chat_req, &target_protocol, &platform.protocol);
    let req_body_str = serde_json::to_string(&req_body).unwrap_or_default();
    let base_url = target_base_url.trim_end_matches('/');
    let url = format!("{}{}", base_url, api_path);

    // ── 使用与 proxy 相同的客户端 header 模拟逻辑 ──
    let upstream_headers = gateway::proxy::build_upstream_headers(&client_type, &target_protocol, &platform.api_key);

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .unwrap_or_else(|_| reqwest::Client::new());

    let start = std::time::Instant::now();
    let request_id = uuid::Uuid::new_v4().to_string();
    let created_at = chrono::Utc::now().to_rfc3339();

    let req_builder = client
        .post(&url)
        .header("Content-Type", "application/json")
        .body(req_body_str.clone());
    let req_builder = gateway::proxy::apply_client_headers(req_builder, &client_type, &target_protocol, &platform.api_key);

    // ── 辅助: 构造测试日志 ──
    let make_log = |body_override: &str, upstream_status: i32, user_status: i32,
                     upstream_resp_headers: &str, user_resp_body: &str,
                     in_tok: i32, out_tok: i32| -> gateway::models::ProxyLog {
        gateway::models::ProxyLog {
            id: request_id.clone(),
            group_name: "[test]".into(),
            model: model.clone(),
            actual_model: model.clone(),
            source_protocol: "test".into(),
            target_protocol: format!("{:?}", target_protocol).to_lowercase(),
            platform_id: platform.id.clone(),
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
            created_at: created_at.clone(),
        }
    };

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
            let _ = db::upsert_proxy_log(&db, &make_log(
                &format!("upstream error: {e}"), 0, 502, "", &format!("upstream error: {e}"), 0, 0,
            ));
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
        let _ = db::upsert_proxy_log(&db, &make_log(
            &body, upstream_status_code, upstream_status_code,
            &upstream_resp_headers, &body, 0, 0,
        ));
        return Ok(result);
    }

    let resp_json: serde_json::Value = serde_json::from_str(&body).unwrap_or_default();
    let response_text = extract_response_text(&resp_json, &target_protocol);
    let (in_tok, out_tok) = extract_test_usage(&resp_json, &target_protocol);

    let result = ModelTestResult {
        success: true,
        model: model.clone(),
        prompt_preview: truncate_str(&prompt, 100),
        response_preview: truncate_str(&response_text, 300),
        duration_ms,
        input_tokens: in_tok,
        output_tokens: out_tok,
        error: String::new(),
    };

    let _ = db::upsert_proxy_log(&db, &make_log(
        &body, upstream_status_code, 200,
        &upstream_resp_headers, &body, in_tok, out_tok,
    ));

    Ok(result)
}

#[allow(dead_code)]
fn truncate_str(s: &str, max: usize) -> String {
    if s.len() <= max { s.to_string() } else { format!("{}\u{2026}", &s[..max]) }
}

#[allow(dead_code)]
fn extract_response_text(v: &Value, protocol: &Protocol) -> String {
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
fn extract_test_usage(v: &Value, protocol: &Protocol) -> (i32, i32) {
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

// ─── Claude Code Config Export ─────────────────────────────

#[tauri::command]
fn export_claude_config(port: u16, _app: tauri::AppHandle) -> Result<String, String> {
    let base_url = format!("http://localhost:{}/claude/v1/messages", port);
    let config_path = dirs::home_dir()
        .ok_or("cannot resolve home directory")?
        .join(".claude.json");

    // 读取已有配置
    let mut config: serde_json::Value = if config_path.exists() {
        let content = std::fs::read_to_string(&config_path)
            .map_err(|e| format!("read config: {e}"))?;
        serde_json::from_str(&content).unwrap_or(serde_json::Value::Object(Default::default()))
    } else {
        serde_json::Value::Object(Default::default())
    };

    // 设置 ANTHROPIC_BASE_URL
    if let Some(obj) = config.as_object_mut() {
        obj.insert(
            "ANTHROPIC_BASE_URL".to_string(),
            serde_json::Value::String(base_url.clone()),
        );
    }

    let content = serde_json::to_string_pretty(&config)
        .map_err(|e| format!("serialize config: {e}"))?;
    std::fs::write(&config_path, content)
        .map_err(|e| format!("write config: {e}"))?;

    Ok(config_path.to_string_lossy().to_string())
}

/// Helper: attempt sync, log errors but don't propagate
fn try_sync_settings(app: &tauri::AppHandle, db: &Db) {
    if let Ok(settings) = load_proxy_settings(app) {
        let _ = do_sync_group_settings(db, settings.port);
    }
}

/// 为所有分组生成 settings.{group_name}.json 配置文件到 ~/.aidog/ 目录
/// 核心逻辑：可被多个触发点调用
fn do_sync_group_settings(db: &Db, port: u16) -> Result<Vec<String>, String> {
    let groups = gateway::db::list_groups(db)?;

    let aidog_dir = dirs::home_dir()
        .ok_or("cannot resolve home directory")?
        .join(".aidog");

    // Ensure ~/.aidog/ exists
    std::fs::create_dir_all(&aidog_dir)
        .map_err(|e| format!("create .aidog dir: {e}"))?;

    // Load base claude code config from app settings (scope=global, key=claude_code)
    // Fallback to compiled-in defaults when DB has no config
    let base_config: serde_json::Value = gateway::db::get_setting(db, "global", "claude_code")
        .ok()
        .flatten()
        .filter(|v| v.is_object() && v.as_object().is_some_and(|o| !o.is_empty()))
        .unwrap_or_else(|| {
            serde_json::from_str(include_str!("../defaults/settings.json"))
                .unwrap_or(serde_json::Value::Object(Default::default()))
        });

    // Collect current group names for cleanup
    let group_names: std::collections::HashSet<String> = groups.iter().map(|g| g.name.clone()).collect();

    let mut written = Vec::new();

    for group in &groups {
        let group_name = &group.name;

        let mut config = base_config.clone();

        // Set proxy routing fields inside env
        if let Some(obj) = config.as_object_mut() {
            if !obj.contains_key("env") {
                obj.insert("env".into(), serde_json::Value::Object(Default::default()));
            }
            if let Some(env_map) = obj.get_mut("env").and_then(|v| v.as_object_mut()) {
                env_map.insert(
                    "ANTHROPIC_BASE_URL".to_string(),
                    serde_json::Value::String(format!("http://127.0.0.1:{}/proxy", port)),
                );
                env_map.insert(
                    "ANTHROPIC_AUTH_TOKEN".to_string(),
                    serde_json::Value::String(group_name.clone()),
                );
            }
        }

        let file_path = aidog_dir.join(format!("settings.{}.json", group_name));
        let content = serde_json::to_string_pretty(&config)
            .map_err(|e| format!("serialize config for {}: {e}", group_name))?;

        // Diff check: only write when content differs
        let existing = std::fs::read_to_string(&file_path).unwrap_or_default();
        if existing != content {
            std::fs::write(&file_path, &content)
                .map_err(|e| format!("write config for {}: {e}", group_name))?;
            written.push(file_path.to_string_lossy().to_string());
        }
    }

    // Cleanup: remove settings files for deleted groups
    if let Ok(entries) = std::fs::read_dir(&aidog_dir) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if let Some(group_name) = name.strip_prefix("settings.").and_then(|s| s.strip_suffix(".json")) {
                if !group_names.contains(group_name) {
                    let _ = std::fs::remove_file(entry.path());
                }
            }
        }
    }

    Ok(written)
}

/// Tauri command — manual sync from UI
#[tauri::command]
fn sync_group_settings(app: tauri::AppHandle, db: State<'_, Db>) -> Result<Vec<String>, String> {
    let proxy_settings = load_proxy_settings(&app)?;
    do_sync_group_settings(&db, proxy_settings.port)
}

// ─── Proxy Log Commands ────────────────────────────────────

use gateway::models::{ProxyLog, ProxyLogSummary, ProxyLogSettings};

#[tauri::command]
fn proxy_log_list(db: State<'_, Db>, limit: u32, offset: u32) -> Result<Vec<ProxyLogSummary>, String> {
    gateway::db::list_proxy_logs(&db, limit, offset)
}

#[tauri::command]
fn proxy_log_get(id: String, db: State<'_, Db>) -> Result<Option<ProxyLog>, String> {
    gateway::db::get_proxy_log(&db, &id)
}

#[tauri::command]
fn proxy_log_clear(db: State<'_, Db>) -> Result<(), String> {
    gateway::db::clear_proxy_logs(&db)
}

#[tauri::command]
fn proxy_log_count(db: State<'_, Db>) -> Result<u32, String> {
    gateway::db::count_proxy_logs(&db)
}

#[tauri::command]
fn platform_usage_stats(platform_id: String, db: State<'_, Db>) -> Result<gateway::models::PlatformUsageStats, String> {
    gateway::db::get_platform_usage_stats(&db, &platform_id)
}

#[tauri::command]
fn group_usage_stats(group_name: String, db: State<'_, Db>) -> Result<gateway::models::PlatformUsageStats, String> {
    gateway::db::get_group_usage_stats(&db, &group_name)
}

#[tauri::command]
fn proxy_log_settings_get(db: State<'_, Db>) -> Result<ProxyLogSettings, String> {
    let val = gateway::db::get_setting(&db, "proxy", "logging")
        .ok()
        .flatten()
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or_default();
    Ok(val)
}

#[tauri::command]
fn proxy_log_settings_set(db: State<'_, Db>, settings: ProxyLogSettings) -> Result<(), String> {
    let value = serde_json::to_value(&settings)
        .map_err(|e| format!("serialize log settings: {e}"))?;
    gateway::db::set_setting(&db, gateway::models::SetSettingInput {
        scope: "proxy".into(),
        key: "logging".into(),
        value,
    })?;
    // When disabled, also run cleanup so stale logs don't accumulate
    if !settings.enabled && settings.retention_days > 0 {
        let _ = gateway::db::cleanup_proxy_logs(&db, settings.retention_days);
    }
    Ok(())
}

// ─── Proxy Timeout Settings ─────────────────────────────────

use gateway::models::ProxyTimeoutSettings;

#[tauri::command]
fn proxy_timeout_get(db: State<'_, Db>) -> Result<ProxyTimeoutSettings, String> {
    Ok(gateway::db::get_setting(&db, "proxy", "timeout")
        .ok()
        .flatten()
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or_default())
}

#[tauri::command]
fn proxy_timeout_set(db: State<'_, Db>, settings: ProxyTimeoutSettings) -> Result<(), String> {
    gateway::db::set_setting(&db, SetSettingInput {
        scope: "proxy".to_string(),
        key: "timeout".to_string(),
        value: serde_json::to_value(&settings).map_err(|e| format!("serialize: {e}"))?,
    })
}

// ─── Platform Quota (Balance & Coding Plan) ────────────────

use gateway::quota::PlatformQuota;

#[tauri::command]
async fn platform_query_quota(base_url: String, api_key: String) -> Result<PlatformQuota, String> {
    Ok(gateway::quota::query_quota(&base_url, &api_key).await)
}

// ─── Path Autocomplete ─────────────────────────────────────

use serde::Serialize;

#[derive(Serialize)]
struct PathEntry {
    name: String,
    full_path: String,
    is_dir: bool,
    /// Unix timestamp (seconds)
    modified: i64,
}

/// Expand `~` to home directory and resolve path
fn expand_path(input: &str) -> std::path::PathBuf {
    if input.starts_with("~/") || input == "~" {
        if let Some(home) = dirs::home_dir() {
            if input == "~" {
                return home;
            }
            return home.join(&input[2..]);
        }
    }
    std::path::PathBuf::from(input)
}

#[tauri::command]
fn fs_autocomplete(input: String) -> Result<Vec<PathEntry>, String> {
    let path = expand_path(input.trim());

    // Determine parent dir and prefix filter
    let (parent, prefix) = if input.ends_with('/') || input == "~" || input.ends_with('~') {
        (path.clone(), "".to_string())
    } else {
        let file_name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        let parent = path.parent().map(|p| p.to_path_buf()).unwrap_or_else(|| {
            std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("/"))
        });
        (parent, file_name)
    };

    if !parent.exists() || !parent.is_dir() {
        return Ok(vec![]);
    }

    let entries: Vec<PathEntry> = std::fs::read_dir(&parent)
        .map_err(|e| e.to_string())?
        .filter_map(|entry| {
            let entry = entry.ok()?;
            let name = entry.file_name().to_string_lossy().to_string();

            // Filter by prefix
            if !prefix.is_empty() && !name.to_lowercase().starts_with(&prefix.to_lowercase()) {
                return None;
            }

            let metadata = entry.metadata().ok()?;
            let modified = metadata
                .modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs() as i64)
                .unwrap_or(0);

            let full_path = entry.path().to_string_lossy().to_string();

            Some(PathEntry {
                name,
                full_path,
                is_dir: metadata.is_dir(),
                modified,
            })
        })
        .collect();

    // Sort: directories first, then by modification time descending
    let mut sorted = entries;
    sorted.sort_by(|a, b| {
        match (a.is_dir, b.is_dir) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => b.modified.cmp(&a.modified),
        }
    });

    // Limit results
    sorted.truncate(20);

    Ok(sorted)
}

// ─── Settings Commands ─────────────────────────────────────

use gateway::models::SetSettingInput;

#[tauri::command]
fn settings_get(
    scope: String,
    key: String,
    db: State<'_, Db>,
) -> Result<Option<serde_json::Value>, String> {
    db::get_setting(&db, &scope, &key)
}

#[tauri::command]
fn settings_set(input: SetSettingInput, db: State<'_, Db>, app: tauri::AppHandle) -> Result<(), String> {
    db::set_setting(&db, input)?;
    // Auto-sync group settings files when claude code config changes
    try_sync_settings(&app, &db);
    Ok(())
}

#[tauri::command]
fn settings_delete(scope: String, key: String, db: State<'_, Db>) -> Result<(), String> {
    db::delete_setting(&db, &scope, &key)
}

#[tauri::command]
fn settings_list(scope: String, db: State<'_, Db>) -> Result<Vec<String>, String> {
    db::list_setting_keys(&db, &scope)
}

// ─── Settings Persistence ──────────────────────────────────

/// 统一数据目录：~/.aidog/
fn aidog_data_dir() -> Result<std::path::PathBuf, String> {
    let home = dirs::home_dir().ok_or("cannot resolve home directory")?;
    let dir = home.join(".aidog");
    std::fs::create_dir_all(&dir).map_err(|e| format!("create ~/.aidog: {e}"))?;
    Ok(dir)
}

/// Load app log settings from DB (must be called after init_tables)
fn load_app_log_settings_from_db(db: &Db) -> logging::AppLogSettings {
    db::get_setting(db, "app", "logging")
        .ok()
        .flatten()
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or_default()
}

/// Load app log settings from file (pre-DB, uses defaults + env)
fn load_app_log_settings() -> logging::AppLogSettings {
    // Try loading from a simple JSON file before DB is available
    let path = dirs::home_dir()
        .unwrap_or_default()
        .join(".aidog")
        .join("log_settings.json");
    if path.exists() {
        if let Ok(data) = std::fs::read_to_string(&path) {
            if let Ok(s) = serde_json::from_str(&data) {
                return s;
            }
        }
    }
    logging::AppLogSettings::default()
}

#[tauri::command]
fn app_log_settings_get(db: State<'_, Db>) -> Result<logging::AppLogSettings, String> {
    Ok(load_app_log_settings_from_db(&db))
}

#[tauri::command]
fn app_log_settings_set(settings: logging::AppLogSettings, db: State<'_, Db>) -> Result<(), String> {
    let value = serde_json::to_value(&settings).map_err(|e| e.to_string())?;
    db::set_setting(&db, SetSettingInput { scope: "app".to_string(), key: "logging".to_string(), value })?;
    // Also persist to file so it's available before DB init on next startup
    if let Some(dir) = dirs::home_dir() {
        let path = dir.join(".aidog").join("log_settings.json");
        let _ = std::fs::write(&path, serde_json::to_string_pretty(&settings).unwrap_or_default());
    }
    Ok(())
}

#[derive(serde::Serialize, serde::Deserialize, Clone)]
struct ProxySettings {
    port: u16,
    autostart: bool,
}

fn settings_path(_app: &tauri::AppHandle) -> Result<std::path::PathBuf, String> {
    Ok(aidog_data_dir()?.join("proxy_settings.json"))
}

fn load_proxy_settings(app: &tauri::AppHandle) -> Result<ProxySettings, String> {
    let path = settings_path(app)?;
    if path.exists() {
        let content = std::fs::read_to_string(&path)
            .map_err(|e| format!("read settings: {e}"))?;
        serde_json::from_str(&content).map_err(|e| format!("parse settings: {e}"))
    } else {
        Ok(ProxySettings { port: 9876, autostart: true })
    }
}

fn save_proxy_settings(
    app: &tauri::AppHandle,
    port: u16,
    autostart: bool,
) -> Result<(), String> {
    let path = settings_path(app)?;
    let settings = ProxySettings { port, autostart };
    let content = serde_json::to_string_pretty(&settings)
        .map_err(|e| format!("serialize settings: {e}"))?;
    std::fs::write(&path, content)
        .map_err(|e| format!("write settings: {e}"))?;
    Ok(())
}

// ─── Tray ──────────────────────────────────────────────────

use tauri::menu::{MenuBuilder, MenuItemBuilder};
use tauri::tray::TrayIconBuilder;

fn build_tray_menu(app: &tauri::AppHandle) -> Result<tauri::menu::Menu<tauri::Wry>, String> {
    let running = {
        let handle = app.state::<ProxyHandle>();
        let h = handle.0.lock().map_err(|e| e.to_string())?;
        h.is_some()
    };

    let settings = load_proxy_settings(app)?;
    let status_text = if running {
        format!("● Proxy Running :{}", settings.port)
    } else {
        "○ Proxy Stopped".to_string()
    };

    let toggle_id = if running { "proxy_stop" } else { "proxy_start" };
    let toggle_text = if running { "Stop Proxy" } else { "Start Proxy" };

    let menu = MenuBuilder::new(app)
        .item(&MenuItemBuilder::with_id("status", status_text).enabled(false).build(app).map_err(|e| e.to_string())?)
        .separator()
        .item(&MenuItemBuilder::with_id(toggle_id, toggle_text).build(app).map_err(|e| e.to_string())?)
        .separator()
        .item(&MenuItemBuilder::with_id("show", "Show Window").build(app).map_err(|e| e.to_string())?)
        .item(&MenuItemBuilder::with_id("quit", "Quit").build(app).map_err(|e| e.to_string())?)
        .build().map_err(|e| e.to_string())?;

    Ok(menu)
}

fn refresh_tray_menu(app: &tauri::AppHandle) -> Result<(), String> {
    let tray = app.tray_by_id("main").ok_or("tray not found")?;
    let menu = build_tray_menu(app)?;
    tray.set_menu(Some(menu)).map_err(|e| e.to_string())?;
    Ok(())
}

// ─── App Entry ─────────────────────────────────────────────

use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            // 初始化日志（尽早，在 DB 之前）
            let data_dir = aidog_data_dir().expect("failed to resolve data dir");
            let log_settings = load_app_log_settings();
            logging::init_logging(&data_dir, &log_settings);
            logging::cleanup_old_logs(&data_dir, log_settings.retention_hours);

            // 在 data dir 创建 SQLite
            let db_path = data_dir.join("aidog.db");
            let db = Db::new(db_path.to_str().unwrap()).expect("failed to open database");
            db.init_tables().expect("failed to init tables");
            db.fix_group_names();
            // 为所有平台确保存在关联分组（一个平台一个）
            ensure_platform_groups(&db);
            app.manage(db);

            // 启动时同步所有 settings 文件（检查不一致并更新）
            {
                let handle = app.handle();
                let db_state = app.state::<Db>();
                try_sync_settings(handle, &db_state);
            }

            app.manage(ProxyHandle(StdMutex::new(None)));

            // 系统托盘
            let menu = build_tray_menu(app.handle())?;
            TrayIconBuilder::with_id("main")
                .icon(app.default_window_icon().cloned().unwrap())
                .menu(&menu)
                .tooltip("AiDog — AI API Gateway")
                .on_menu_event(|app, event| match event.id().as_ref() {
                    "proxy_start" => {
                        let settings = load_proxy_settings(app).unwrap_or(ProxySettings {
                            port: 9876,
                            autostart: true,
                        });
                        let port = settings.port;
                        tauri::async_runtime::block_on(async {
                            let _ = proxy_start(port, app.clone()).await;
                        });
                    }
                    "proxy_stop" => {
                        tauri::async_runtime::block_on(async {
                            let _ = proxy_stop(app.clone()).await;
                        });
                    }
                    "show" => {
                        if let Some(w) = app.get_webview_window("main") {
                            let _ = w.show();
                            let _ = w.set_focus();
                        }
                    }
                    "quit" => {
                        app.exit(0);
                    }
                    _ => {}
                })
                .build(app).map_err(|e| e.to_string())?;

            // 自动启动代理
            let settings = load_proxy_settings(app.handle())?;
            if settings.autostart {
                let port = settings.port;
                let handle = app.handle().clone();
                tauri::async_runtime::spawn(async move {
                    let _ = proxy_start(port, handle).await;
                });
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // Platform
            platform_create,
            platform_list,
            platform_get,
            platform_update,
            platform_delete,
            platform_fetch_models,
            // Group
            group_create,
            group_list,
            group_get,
            group_update,
            group_delete,
            // GroupPlatform
            group_set_platforms,
            group_get_platforms,
            // ModelMapping
            mapping_create,
            mapping_list,
            mapping_update,
            mapping_delete,
            // Aggregate
            group_detail,
            group_detail_list,
            // Proxy
            proxy_start,
            proxy_stop,
            proxy_status,
            proxy_get_settings,
            proxy_set_autostart,
            // Config Export
            export_claude_config,
            sync_group_settings,
            // Proxy Logs
            proxy_log_list,
            proxy_log_get,
            proxy_log_clear,
            proxy_log_count,
            proxy_log_settings_get,
            proxy_log_settings_set,
            // Proxy Timeout
            proxy_timeout_get,
            proxy_timeout_set,
            // App Logging
            app_log_settings_get,
            app_log_settings_set,
            // Settings
            fs_autocomplete,
            settings_get,
            settings_set,
            settings_delete,
            settings_list,
            // Statistics
            stats_query,
            model_test,
            // Platform Usage
            platform_usage_stats,
            group_usage_stats,
            // Platform Quota
            platform_query_quota,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
