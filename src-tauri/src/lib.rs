mod gateway;
mod logging;

use gateway::db::{self, Db};
use gateway::models::*;
use tauri::State;
use serde_json::Value;
use std::sync::Arc;

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
async fn ensure_platform_groups(db: &Db) {
    let platforms = match db::list_platforms(db).await {
        Ok(p) => p,
        Err(e) => { tracing::error!("ensure_platform_groups: list_platforms failed: {e}"); return; }
    };
    // 一次性取出已有分组的 auto_from_platform 集合，避免循环内重复全表查询（N+1）
    let mut existing_auto: std::collections::HashSet<String> = db::list_groups(db).await
        .unwrap_or_default()
        .into_iter()
        .map(|g| g.auto_from_platform)
        .collect();
    for platform in &platforms {
        // 检查是否已存在关联此平台的分组
        let platform_id_str = platform.id.to_string();
        if existing_auto.contains(&platform_id_str) {
            continue;
        }
        // 自动创建分组 — path 用平台 ID 前缀避免同名协议冲突
        let protocol_str = format!("{:?}", platform.platform_type).to_lowercase();
        let group_path = format!("/{}-{}", protocol_str, platform.id);
        let group_name = slugify(&format!("{}-auto", platform.name));
        let group = match db::create_group(db, CreateGroup {
            name: group_name.clone(),
            path: group_path.clone(),
            routing_mode: RoutingMode::Failover,
            auto_from_platform: platform_id_str.clone(),
            request_timeout_secs: 0,
            connect_timeout_secs: 0,
            source_protocol: None,
            model_mappings: Vec::new(),
        }).await {
            Ok(g) => g,
            Err(e) => { tracing::error!("ensure_platform_groups: create_group failed for {}: {e}", platform.name); continue; }
        };
        existing_auto.insert(platform_id_str);
        // 将平台关联到自动分组
        if let Err(e) = db::set_group_platforms(db, group.id, &[GroupPlatformInput {
            platform_id: platform.id,
            priority: Some(0),
            weight: Some(1),
        }]).await {
            tracing::error!("ensure_platform_groups: set_group_platforms failed for {}: {e}", platform.name);
        }
        tracing::info!("ensure_platform_groups: created group '{}' path='{}' for platform '{}'", group_name, group_path, platform.name);
    }
}

// ─── Platform Commands ─────────────────────────────────────

#[tauri::command]
async fn platform_create(input: CreatePlatform, db: State<'_, Db>) -> Result<Platform, String> {
    let platform = db::create_platform(&db, input).await?;

    // 自动创建分组，path 按 protocol + 平台 ID 生成
    let protocol_str = format!("{:?}", platform.platform_type).to_lowercase();
    let group_path = format!("/{}-{}", protocol_str, platform.id);
    let group_name = slugify(&format!("{}-auto", platform.name));

    let group = db::create_group(
        &db,
        CreateGroup {
            name: group_name,
            path: group_path,
            routing_mode: RoutingMode::Failover,
            auto_from_platform: platform.id.to_string(),
            request_timeout_secs: 0,
            connect_timeout_secs: 0,
            source_protocol: None,
            model_mappings: Vec::new(),
        },
    ).await?;

    // 将平台关联到自动分组
    db::set_group_platforms(
        &db,
        group.id,
        &[GroupPlatformInput {
            platform_id: platform.id,
            priority: Some(0),
            weight: Some(1),
        }],
    ).await?;

    Ok(platform)
}

#[tauri::command]
async fn platform_list(db: State<'_, Db>) -> Result<Vec<Platform>, String> {
    db::list_platforms(&db).await
}

#[tauri::command]
async fn platform_get(id: u64, db: State<'_, Db>) -> Result<Option<Platform>, String> {
    db::get_platform(&db, id).await
}

#[tauri::command]
async fn platform_update(input: UpdatePlatform, db: State<'_, Db>) -> Result<Platform, String> {
    let platform = db::update_platform(&db, input).await?;
    // 确保该平台有关联分组，若无则自动创建
    let groups = db::list_groups(&db).await.unwrap_or_default();
    let platform_id_str = platform.id.to_string();
    let exists = groups.iter().any(|g| g.auto_from_platform == platform_id_str);
    if !exists {
        let protocol_str = format!("{:?}", platform.platform_type).to_lowercase();
        let group_path = format!("/{}-{}", protocol_str, platform.id);
        let group_name = slugify(&format!("{}-auto", platform.name));
        if let Ok(group) = db::create_group(&db, CreateGroup {
            name: group_name,
            path: group_path,
            routing_mode: RoutingMode::Failover,
            auto_from_platform: platform_id_str.clone(),
            request_timeout_secs: 0,
            connect_timeout_secs: 0,
            source_protocol: None,
            model_mappings: Vec::new(),
        }).await {
            let _ = db::set_group_platforms(&db, group.id, &[GroupPlatformInput {
                platform_id: platform.id,
                priority: Some(0),
                weight: Some(1),
            }]).await;
        }
    }
    Ok(platform)
}

#[tauri::command]
async fn platform_delete(id: u64, db: State<'_, Db>) -> Result<(), String> {
    db::delete_platform(&db, id).await
}

/// 设置 / 清除托盘展示平台（互斥单平台）。
/// enabled=true → 设 platform_id 为唯一展示平台（tray_display: "balance"|"coding"）；
/// enabled=false → 清空所有。改后刷新托盘。
#[tauri::command]
async fn platform_set_tray(
    platform_id: u64,
    tray_display: String,
    enabled: bool,
    db: State<'_, Db>,
    app: tauri::AppHandle,
) -> Result<(), String> {
    if enabled {
        db::set_tray_platform(&db, platform_id, &tray_display).await?;
    } else {
        db::clear_tray(&db).await?;
    }
    refresh_tray_menu(&app)?;
    Ok(())
}

/// 读取托盘配置。无配置时（首次/升级）从旧 show_in_tray 平台迁移生成默认。
#[tauri::command]
async fn tray_config_get(db: State<'_, Db>) -> Result<TrayConfig, String> {
    Ok(db::get_tray_config(&db).await?.unwrap_or_default())
}

/// 保存托盘配置并刷新托盘渲染。
#[tauri::command]
async fn tray_config_set(
    config: TrayConfig,
    db: State<'_, Db>,
    app: tauri::AppHandle,
) -> Result<(), String> {
    db::set_tray_config(&db, &config).await?;
    refresh_tray_menu(&app)?;
    Ok(())
}

/// 获取今日统计摘要（供前端预览使用）
#[tauri::command]
async fn tray_today_stats(db: State<'_, Db>) -> Result<db::TodayStats, String> {
    db::today_stats(&db).await
}

// ─── Group Commands ────────────────────────────────────────

#[tauri::command]
async fn group_create(mut input: CreateGroup, db: State<'_, Db>, app: tauri::AppHandle) -> Result<Group, String> {
    // Auto-slugify and validate group name
    input.name = slugify(&input.name);
    validate_group_name(&input.name)?;
    let result = db::create_group(&db, input).await?;
    try_sync_settings(&app, &db).await;
    Ok(result)
}

#[tauri::command]
async fn group_list(db: State<'_, Db>) -> Result<Vec<Group>, String> {
    db::list_groups(&db).await
}

#[tauri::command]
async fn group_get(id: u64, db: State<'_, Db>) -> Result<Option<Group>, String> {
    db::get_group(&db, id).await
}

#[tauri::command]
async fn group_update(mut input: UpdateGroup, db: State<'_, Db>, app: tauri::AppHandle) -> Result<Group, String> {
    // Auto-slugify and validate if name is being updated
    if let Some(ref name) = input.name {
        let slug = slugify(name);
        validate_group_name(&slug)?;
        input.name = Some(slug);
    }
    let result = db::update_group(&db, input).await?;
    try_sync_settings(&app, &db).await;
    Ok(result)
}

#[tauri::command]
async fn group_delete(id: u64, db: State<'_, Db>, app: tauri::AppHandle) -> Result<(), String> {
    db::delete_group(&db, id).await?;
    try_sync_settings(&app, &db).await;
    Ok(())
}

// ─── GroupPlatform Commands ────────────────────────────────

#[tauri::command]
async fn group_set_platforms(input: SetGroupPlatforms, db: State<'_, Db>, app: tauri::AppHandle) -> Result<(), String> {
    db::set_group_platforms(&db, input.group_id, &input.platforms).await?;
    try_sync_settings(&app, &db).await;
    Ok(())
}

#[tauri::command]
async fn group_get_platforms(
    group_id: u64,
    db: State<'_, Db>,
) -> Result<Vec<GroupPlatformDetail>, String> {
    db::get_group_platforms(&db, group_id).await
}

// ─── Aggregate ─────────────────────────────────────────────

#[tauri::command]
async fn group_detail(id: u64, db: State<'_, Db>) -> Result<Option<GroupDetail>, String> {
    db::get_group_detail(&db, id).await
}

#[tauri::command]
async fn group_detail_list(db: State<'_, Db>) -> Result<Vec<GroupDetail>, String> {
    db::list_group_details(&db).await
}

#[tauri::command]
async fn group_reorder(ordered_ids: Vec<u64>, db: State<'_, Db>, app: tauri::AppHandle) -> Result<(), String> {
    db::reorder_groups(&db, &ordered_ids).await?;
    try_sync_settings(&app, &db).await;
    Ok(())
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
    let proxy_db = Db::new(db_path.to_str().unwrap_or("")).await?;
    let proxy_db = std::sync::Arc::new(proxy_db);

    let (proxy_handle, actual_port) = gateway::proxy::start_proxy(proxy_db, port, Some(app.clone())).await?;

    {
        let mut h = handle.0.lock().map_err(|e| e.to_string())?;
        *h = Some(proxy_handle);
    }

    // 保存实际使用的端口到设置
    let saved = load_proxy_settings(&app).await.unwrap_or(ProxySettings { port: 9876, autostart: true, silent_launch: false });
    save_proxy_settings(&app, actual_port, true, saved.silent_launch).await?;

    // 同步所有分组的 settings 文件（端口可能变了）
    if let Some(db) = app.try_state::<Db>() {
        let _ = do_sync_group_settings(&db, actual_port).await;
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
    if let Ok(settings) = load_proxy_settings(&app).await {
        save_proxy_settings(&app, settings.port, false, settings.silent_launch).await?;
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
async fn proxy_get_settings(app: tauri::AppHandle) -> Result<ProxySettings, String> {
    load_proxy_settings(&app).await
}

#[tauri::command]
async fn proxy_set_autostart(app: tauri::AppHandle, enabled: bool) -> Result<(), String> {
    let current = load_proxy_settings(&app).await?;
    save_proxy_settings(&app, current.port, enabled, current.silent_launch).await?;
    Ok(())
}

#[tauri::command]
fn app_set_autolaunch(app: tauri::AppHandle, enabled: bool) -> Result<(), String> {
    use tauri_plugin_autostart::ManagerExt;
    let manager = app.autolaunch();
    if enabled {
        manager.enable().map_err(|e| format!("enable autolaunch: {e}"))?;
    } else {
        manager.disable().map_err(|e| format!("disable autolaunch: {e}"))?;
    }
    Ok(())
}

#[tauri::command]
fn app_get_autolaunch(app: tauri::AppHandle) -> Result<bool, String> {
    use tauri_plugin_autostart::ManagerExt;
    let manager = app.autolaunch();
    manager.is_enabled().map_err(|e| format!("get autolaunch: {e}"))
}

#[tauri::command]
async fn app_set_silent_launch(app: tauri::AppHandle, enabled: bool) -> Result<(), String> {
    let current = load_proxy_settings(&app).await?;
    save_proxy_settings(&app, current.port, current.autostart, enabled).await?;
    Ok(())
}

// ─── Proxy Client Settings (upstream HTTP proxy) ─────────────

#[tauri::command]
async fn proxy_client_get_settings(app: tauri::AppHandle) -> Result<gateway::models::ProxyClientSettings, String> {
    let db = app.try_state::<Db>()
        .map(|s| s.inner().clone())
        .ok_or("db not initialized")?;
    let settings = gateway::http_client::load_proxy_client_settings(&Arc::new(db)).await;
    Ok(settings)
}

#[tauri::command]
async fn proxy_client_set_settings(app: tauri::AppHandle, settings: gateway::models::ProxyClientSettings) -> Result<(), String> {
    let db = app.try_state::<Db>()
        .map(|s| s.inner())
        .ok_or("db not initialized")?;
    let value = serde_json::to_value(&settings)
        .map_err(|e| format!("serialize proxy client settings: {e}"))?;
    db::set_setting(db, gateway::models::SetSettingInput {
        scope: "proxy".to_string(),
        key: "proxy_client".to_string(),
        value,
    }).await
}

// ─── Platform Model Fetch ──────────────────────────────────

#[tauri::command]
async fn platform_fetch_models(
    protocol: Protocol,
    base_url: String,
    api_key: String,
    db: State<'_, Db>,
) -> Result<Vec<String>, String> {
    let db_arc = Arc::new(db.inner().clone());
    let client = gateway::http_client::build_http_client_system(&db_arc, 30, 10).await;
    let base = base_url.trim_end_matches('/');

    // Mock / Claude Code 透传平台无真实上游模型列表，不拉取模型
    if matches!(protocol, Protocol::Mock | Protocol::ClaudeCode) {
        return Ok(Vec::new());
    }

    let resp: Value = match protocol {
        Protocol::Mock | Protocol::ClaudeCode => unreachable!(),
        Protocol::Anthropic => {
            let url = format!("{base}/v1/models");
            tracing::info!(url = %url, "fetch models request");
            let resp = client
                .get(&url)
                .header("x-api-key", &api_key)
                .header("anthropic-version", "2023-06-01")
                .send()
                .await
                .map_err(|e| format!("fetch models: {e}"))?;
            let body = resp.text().await.map_err(|e| format!("read body: {e}"))?;
            tracing::debug!(url = %url, body = %body, "fetch models response body");
            serde_json::from_str::<Value>(&body).map_err(|e| format!("parse response: {e}"))?
        }
        Protocol::Bailian => {
            let url = format!("{base}/compatible-mode/v1/models");
            tracing::info!(url = %url, "fetch models request");
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
            tracing::info!(url = %url, %status, "fetch models response status");
            tracing::debug!(url = %url, body = %body, "fetch models response body");
            serde_json::from_str::<Value>(&body)
                .map_err(|e| {
                    tracing::error!("parse response failed: {e}, body={}", &body[..body.len().min(500)]);
                    format!("parse response: {e}")
                })?
        }
        Protocol::OpenAI | Protocol::Codex | Protocol::Glm | Protocol::GlmEn | Protocol::Kimi | Protocol::MiniMax | Protocol::MiniMaxEn | Protocol::Gemini | Protocol::OpenAIResponses | Protocol::OpenAICompletions | Protocol::BailianCoding | Protocol::DeepSeek | Protocol::StepFun | Protocol::StepFunEn | Protocol::Doubao | Protocol::DoubaoSeed | Protocol::BytePlus | Protocol::QianFan | Protocol::XiaomiMimo | Protocol::BaiLing | Protocol::Longcat | Protocol::OpenRouter | Protocol::SiliconFlow | Protocol::SiliconFlowEn | Protocol::AiHubMix | Protocol::DmxApi | Protocol::ModelScope | Protocol::ShengSuanYun | Protocol::AtlasCloud | Protocol::Novita | Protocol::TheRouter | Protocol::CherryIn | Protocol::PackyCode | Protocol::Cubence | Protocol::AiGoCode | Protocol::RightCode | Protocol::AiCodeMirror | Protocol::Nvidia | Protocol::Pateway | Protocol::CcSub | Protocol::ApiKeyFun | Protocol::ApiNebula | Protocol::SudoCode | Protocol::ClaudeApi | Protocol::ClaudeCN | Protocol::RunApi | Protocol::RelaxyCode | Protocol::CrazyRouter | Protocol::SssAiCode | Protocol::Compshare | Protocol::CompshareCoding | Protocol::Micu | Protocol::CTok | Protocol::EFlowCode | Protocol::LemonData | Protocol::PipeLlm | Protocol::OpenCode | Protocol::NewApi => {
            let url = format!("{base}/models");
            tracing::info!(url = %url, "fetch models request");
            let resp = client
                .get(&url)
                .header("Authorization", format!("Bearer {api_key}"))
                .send()
                .await
                .map_err(|e| format!("fetch models: {e}"))?;
            let body = resp.text().await.map_err(|e| format!("read body: {e}"))?;
            tracing::debug!(url = %url, body = %body, "fetch models response body");
            serde_json::from_str::<Value>(&body).map_err(|e| format!("parse response: {e}"))?
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
    db::query_stats(&db, &query).await
}

// ─── Model Testing ─────────────────────────────────────────

#[tauri::command]
async fn model_test(
    db: State<'_, Db>,
    req: ModelTestRequest,
) -> Result<ModelTestResult, String> {
    let platform = db::get_platform(&db, req.platform_id).await?
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
        (platform.platform_type.clone(), platform.base_url.clone(), ClientType::default())
    };

    let (req_body, api_path) = gateway::adapter::convert_request(&chat_req, &target_protocol, &platform.platform_type);
    let req_body_str = serde_json::to_string(&req_body).unwrap_or_default();
    let base_url = target_base_url.trim_end_matches('/');
    let url = format!("{}{}", base_url, api_path);

    // ── 使用与 proxy 相同的客户端 header 模拟逻辑 ──
    let upstream_headers = gateway::proxy::build_upstream_headers(&client_type, &target_protocol, &platform.api_key);

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
            created_at,
            updated_at: created_at,
            deleted_at: 0,
        }
    };

    tracing::info!(url = %url, "model test request");
    tracing::debug!(url = %url, body = %req_body_str, "model test request body");
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
            )).await;
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
        )).await;
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
    )).await;

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
async fn try_sync_settings(app: &tauri::AppHandle, db: &Db) {
    if let Ok(settings) = load_proxy_settings(app).await {
        let _ = do_sync_group_settings(db, settings.port).await;
    }
}

/// 为所有分组生成 settings.{group_name}.json 配置文件到 ~/.aidog/ 目录
/// 核心逻辑：可被多个触发点调用
async fn do_sync_group_settings(db: &Db, port: u16) -> Result<Vec<String>, String> {
    let groups = gateway::db::list_groups(db).await?;

    let aidog_dir = dirs::home_dir()
        .ok_or("cannot resolve home directory")?
        .join(".aidog");

    // Ensure ~/.aidog/ exists
    std::fs::create_dir_all(&aidog_dir)
        .map_err(|e| format!("create .aidog dir: {e}"))?;

    // Load base claude code config from app settings (scope=global, key=claude_code)
    // Fallback to compiled-in defaults when DB has no config
    let base_config: serde_json::Value = gateway::db::get_setting(db, "global", "claude_code").await
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

        // Strip internal aidog UI state — not real Claude Code fields.
        if let Some(obj) = config.as_object_mut() {
            obj.remove("_aidog_statusline");
            obj.remove("_aidog_subagent_statusline");
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

        // Codex profile：为该分组生成 `$CODEX_HOME/<group>.config.toml`
        //（profile 文件 = 用户级层，可含 model_providers）。与 Claude Code
        // json 生成并行，互不影响。失败仅记录、不中断（Codex 未装也不应阻塞）。
        match gateway::codex::write_group_profile(group_name, port) {
            Ok(Some(p)) => written.push(p),
            Ok(None) => {}
            Err(e) => eprintln!("codex profile sync for {group_name}: {e}"),
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

    // Cleanup: remove Codex profile files for deleted groups（用户级 config.toml 不动）。
    if let Err(e) = gateway::codex::cleanup_group_profiles(&group_names) {
        eprintln!("codex profile cleanup: {e}");
    }

    Ok(written)
}

/// Tauri command — manual sync from UI
#[tauri::command]
async fn sync_group_settings(app: tauri::AppHandle, db: State<'_, Db>) -> Result<Vec<String>, String> {
    let proxy_settings = load_proxy_settings(&app).await?;
    do_sync_group_settings(&db, proxy_settings.port).await
}

// ─── Proxy Log Commands ────────────────────────────────────

use gateway::models::{ProxyLog, ProxyLogSummary, ProxyLogSettings, ProxyLogFilter};

#[tauri::command]
async fn proxy_log_list(db: State<'_, Db>, limit: u32, offset: u32) -> Result<Vec<ProxyLogSummary>, String> {
    gateway::db::list_proxy_logs(&db, limit, offset).await
}

#[tauri::command]
async fn proxy_log_list_filtered(
    db: State<'_, Db>,
    filter: ProxyLogFilter,
    limit: u32,
    offset: u32,
) -> Result<Vec<ProxyLogSummary>, String> {
    gateway::db::filtered_list_proxy_logs(&db, &filter, limit, offset).await
}

#[tauri::command]
async fn proxy_log_count_filtered(
    db: State<'_, Db>,
    filter: ProxyLogFilter,
) -> Result<u32, String> {
    gateway::db::filtered_count_proxy_logs(&db, &filter).await
}

#[tauri::command]
async fn proxy_log_get(id: String, db: State<'_, Db>) -> Result<Option<ProxyLog>, String> {
    gateway::db::get_proxy_log(&db, &id).await
}

#[tauri::command]
async fn proxy_log_clear(db: State<'_, Db>) -> Result<(), String> {
    gateway::db::clear_proxy_logs(&db).await
}

#[tauri::command]
async fn proxy_log_count(db: State<'_, Db>) -> Result<u32, String> {
    gateway::db::count_proxy_logs(&db).await
}

#[tauri::command]
async fn platform_usage_stats(platform_id: u64, db: State<'_, Db>) -> Result<gateway::models::PlatformUsageStats, String> {
    gateway::db::get_platform_usage_stats(&db, platform_id).await
}

#[tauri::command]
async fn group_usage_stats(group_name: String, db: State<'_, Db>) -> Result<gateway::models::PlatformUsageStats, String> {
    gateway::db::get_group_usage_stats(&db, &group_name).await
}

#[tauri::command]
async fn proxy_log_settings_get(db: State<'_, Db>) -> Result<ProxyLogSettings, String> {
    let val = gateway::db::get_setting(&db, "proxy", "logging").await
        .ok()
        .flatten()
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or_default();
    Ok(val)
}

#[tauri::command]
async fn proxy_log_settings_set(db: State<'_, Db>, settings: ProxyLogSettings) -> Result<(), String> {
    let value = serde_json::to_value(&settings)
        .map_err(|e| format!("serialize log settings: {e}"))?;
    gateway::db::set_setting(&db, gateway::models::SetSettingInput {
        scope: "proxy".into(),
        key: "logging".into(),
        value,
    }).await?;
    // Run field-level cleanup for user/upstream request data
    let _ = gateway::db::cleanup_user_request_fields(&db, settings.user_request_retention_days).await;
    let _ = gateway::db::cleanup_upstream_request_fields(&db, settings.upstream_request_retention_days).await;
    // Delete entire log rows older than overall retention
    if settings.retention_days > 0 {
        let _ = gateway::db::cleanup_proxy_logs(&db, settings.retention_days).await;
    }
    Ok(())
}

// ─── Proxy Timeout Settings ─────────────────────────────────

use gateway::models::ProxyTimeoutSettings;

#[tauri::command]
async fn proxy_timeout_get(db: State<'_, Db>) -> Result<ProxyTimeoutSettings, String> {
    Ok(gateway::db::get_setting(&db, "proxy", "timeout").await
        .ok()
        .flatten()
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or_default())
}

#[tauri::command]
async fn proxy_timeout_set(db: State<'_, Db>, settings: ProxyTimeoutSettings) -> Result<(), String> {
    gateway::db::set_setting(&db, SetSettingInput {
        scope: "proxy".to_string(),
        key: "timeout".to_string(),
        value: serde_json::to_value(&settings).map_err(|e| format!("serialize: {e}"))?,
    }).await
}

// ─── Platform Quota (Balance & Coding Plan) ────────────────

use gateway::quota::PlatformQuota;

#[tauri::command]
async fn platform_query_quota(
    base_url: String, api_key: String,
    platform_id: Option<u64>, db: State<'_, Db>,
) -> Result<PlatformQuota, String> {
    let q = gateway::quota::query_quota(Some(&Arc::new(db.inner().clone())), &base_url, &api_key).await;
    if q.success {
        persist_quota_to_db(&db, platform_id, &q).await;
    }
    Ok(q)
}

/// New API 专用余额查询（两步：先查 token quota 类型，再按需查用户余额）
#[tauri::command]
async fn platform_query_quota_newapi(
    base_url: String, api_key: String, extra: String,
    platform_id: Option<u64>, db: State<'_, Db>,
) -> Result<PlatformQuota, String> {
    let q = gateway::quota::query_quota_newapi(Some(&Arc::new(db.inner().clone())), &base_url, &api_key, &extra).await;
    if q.success {
        persist_quota_to_db(&db, platform_id, &q).await;
    }
    Ok(q)
}

/// 将 quota 真查结果写回 platform 表，并作为一次「校准」严格对齐 est = 真实。
/// 走 estimate::calibrate_from_quota：est_coding_plan 写入正确的 EstCodingPlan 形态
/// （est_utilization=真实 util、util_at_last_real=真实、tokens_since_real=0、拟合 coef），
/// 并重置 last_real_query_at + estimate_count。
/// 这修复了旧路径直写 raw CodingPlanInfo JSON（字段 utilization≠est_utilization）→ tray est 显 0/偏差大的根因，
/// 同时保证「真查发生时 est 立即对齐真实」。
async fn persist_quota_to_db(db: &Db, platform_id: Option<u64>, q: &PlatformQuota) {
    let Some(pid) = platform_id else { return };
    let is_coding_plan = q.coding_plan.is_some();
    gateway::estimate::calibrate_from_quota(db, pid, q, is_coding_plan).await;
}

/// 冷启动 est 初始化：对 tray 中启用、且从未真查过（last_real_query_at==0）的平台，
/// 后台触发一次真查并校准对齐 est=真实。避免冷启动 tray 显 0/旧偏差大。
/// 不阻塞：每平台 spawn 独立 async（锁外 await 真查，calibrate_from_quota 短持锁写）。
/// 真查完成后发 tray-refresh，让主线程刷新托盘显示。
async fn cold_start_init_tray_estimates(app: &tauri::AppHandle) {
    let Some(db_state) = app.try_state::<Db>() else { return };
    let Ok(Some(config)) = db::get_tray_config(&db_state).await else { return };
    // 收集 tray 启用、platform 类型、且 last_real_query_at==0 的平台
    let mut targets: Vec<gateway::models::Platform> = Vec::new();
    for item in config.items.iter().filter(|i| i.enabled && i.item_type == "platform") {
        let Some(pid) = item.platform_id else { continue };
        if let Ok(Some(p)) = db::get_platform(&db_state, pid).await {
            if p.last_real_query_at == 0 {
                targets.push(p);
            }
        }
    }
    for p in targets {
        let handle = app.clone();
        tauri::async_runtime::spawn(async move {
            let Some(db) = handle.try_state::<Db>() else { return };
            let db_arc = Arc::new(db.inner().clone());
            let is_newapi = matches!(p.platform_type, gateway::models::Protocol::NewApi);
            // 锁外 async 真查
            let q = if is_newapi {
                gateway::quota::query_quota_newapi(Some(&db_arc), &p.base_url, &p.api_key, &p.extra).await
            } else {
                gateway::quota::query_quota(Some(&db_arc), &p.base_url, &p.api_key).await
            };
            if !q.success {
                return; // 失败保留，下次再试（不重置 last_real_query_at）
            }
            let is_coding_plan = q.coding_plan.is_some();
            gateway::estimate::calibrate_from_quota(&db, p.id, &q, is_coding_plan).await;
            use tauri::Emitter;
            let _ = handle.emit("tray-refresh", ());
        });
    }
}

#[tauri::command]
async fn platform_reorder(ordered_ids: Vec<u64>, db: State<'_, Db>) -> Result<(), String> {
    db::reorder_platforms(&db, &ordered_ids).await
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
async fn settings_get(
    scope: String,
    key: String,
    db: State<'_, Db>,
) -> Result<Option<serde_json::Value>, String> {
    db::get_setting(&db, &scope, &key).await
}

#[tauri::command]
async fn settings_set(input: SetSettingInput, db: State<'_, Db>, app: tauri::AppHandle) -> Result<(), String> {
    db::set_setting(&db, input).await?;
    // Auto-sync group settings files when claude code config changes
    try_sync_settings(&app, &db).await;
    Ok(())
}

#[tauri::command]
async fn settings_delete(scope: String, key: String, db: State<'_, Db>) -> Result<(), String> {
    db::delete_setting(&db, &scope, &key).await
}

#[tauri::command]
async fn settings_list(scope: String, db: State<'_, Db>) -> Result<Vec<String>, String> {
    db::list_setting_keys(&db, &scope).await
}

// ─── StatusLine Script Generation ──────────────────────────

/// Generate statusline script file in ~/.aidog/ and return absolute path.
/// `script_type`: "statusline" | "subagent"
#[tauri::command]
fn generate_statusline_script(
    script_type: String,
    content: String,
) -> Result<String, String> {
    let aidog_dir = aidog_data_dir()?;
    let filename = if script_type == "subagent" {
        "aidog-subagent-statusline.sh"
    } else {
        "aidog-statusline.sh"
    };
    let path = aidog_dir.join(filename);
    std::fs::write(&path, &content).map_err(|e| format!("write script: {e}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&path).map_err(|e| format!("stat script: {e}"))?.permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&path, perms).map_err(|e| format!("chmod script: {e}"))?;
    }
    Ok(path.to_string_lossy().to_string())
}

// ─── Settings Persistence ──────────────────────────────────

/// 统一数据目录：~/.aidog/
fn aidog_data_dir() -> Result<std::path::PathBuf, String> {
    let home = dirs::home_dir().ok_or("cannot resolve home directory")?;
    let dir = home.join(".aidog");
    std::fs::create_dir_all(&dir).map_err(|e| format!("create ~/.aidog: {e}"))?;
    Ok(dir)
}

#[tauri::command]
fn read_claude_code_settings() -> Result<serde_json::Value, String> {
    let home = dirs::home_dir().ok_or("cannot resolve home directory")?;
    let path = home.join(".claude").join("settings.json");
    if !path.exists() {
        return Err("~/.claude/settings.json not found".into());
    }
    let content = std::fs::read_to_string(&path).map_err(|e| format!("read settings: {e}"))?;
    serde_json::from_str(&content).map_err(|e| format!("parse settings: {e}"))
}

/// Load app log settings from DB (must be called after init_tables)
async fn load_app_log_settings_from_db(db: &Db) -> logging::AppLogSettings {
    db::get_setting(db, "app", "logging").await
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
async fn app_log_settings_get(db: State<'_, Db>) -> Result<logging::AppLogSettings, String> {
    Ok(load_app_log_settings_from_db(&db).await)
}

#[tauri::command]
async fn app_log_settings_set(settings: logging::AppLogSettings, db: State<'_, Db>) -> Result<(), String> {
    let value = serde_json::to_value(&settings).map_err(|e| e.to_string())?;
    db::set_setting(&db, SetSettingInput { scope: "app".to_string(), key: "logging".to_string(), value }).await?;
    // Also persist to file so it's available before DB init on next startup
    if let Some(dir) = dirs::home_dir() {
        let path = dir.join(".aidog").join("log_settings.json");
        let _ = std::fs::write(&path, serde_json::to_string_pretty(&settings).unwrap_or_default());
    }
    Ok(())
}

// ─── Model Price Commands ──────────────────────────────────

#[tauri::command]
async fn model_price_list(db: State<'_, Db>, limit: u32, offset: u32) -> Result<Vec<gateway::models::ModelPriceSummary>, String> {
    gateway::db::list_model_prices(&db, limit, offset).await
}

#[tauri::command]
async fn model_price_count(db: State<'_, Db>) -> Result<u32, String> {
    gateway::db::count_model_prices(&db).await
}

#[tauri::command]
async fn model_price_search(db: State<'_, Db>, query: String, limit: u32) -> Result<Vec<gateway::models::ModelPriceSummary>, String> {
    gateway::db::search_model_prices(&db, &query, limit).await
}

#[tauri::command]
async fn model_price_list_filtered(
    db: State<'_, Db>,
    query: Option<String>,
    source: Option<String>,
    limit: u32,
    offset: u32,
) -> Result<Vec<gateway::models::ModelPriceSummary>, String> {
    gateway::db::filtered_list_model_prices(&db, query.as_deref(), source.as_deref(), limit, offset).await
}

#[tauri::command]
async fn model_price_count_filtered(
    db: State<'_, Db>,
    query: Option<String>,
    source: Option<String>,
) -> Result<u32, String> {
    gateway::db::filtered_count_model_prices(&db, query.as_deref(), source.as_deref()).await
}

#[tauri::command]
async fn model_price_delete(db: State<'_, Db>, model_name: String) -> Result<(), String> {
    gateway::db::delete_model_price(&db, &model_name).await
}

#[tauri::command]
async fn model_price_upsert(
    db: State<'_, Db>,
    model_name: String,
    source: String,
    price_data: String,
) -> Result<(), String> {
    gateway::db::upsert_model_price(&db, &model_name, &source, &price_data).await
}

#[tauri::command]
async fn model_price_resolve(
    db: State<'_, Db>,
    model_name: String,
    platform_type: String,
) -> Result<gateway::models::ResolvedPrice, String> {
    let settings = gateway::price_sync::get_sync_settings(&db).await;
    gateway::db::resolve_price(&db, &model_name, &platform_type, settings.fallback_input_price, settings.fallback_output_price).await
}

#[tauri::command]
async fn model_price_sync(db: State<'_, Db>) -> Result<gateway::models::PriceSyncResult, String> {
    gateway::price_sync::sync_litellm_prices(&db).await
}

#[tauri::command]
async fn price_sync_settings_get(db: State<'_, Db>) -> Result<gateway::models::PriceSyncSettings, String> {
    Ok(gateway::price_sync::get_sync_settings(&db).await)
}

#[tauri::command]
async fn price_sync_settings_set(db: State<'_, Db>, settings: gateway::models::PriceSyncSettings) -> Result<(), String> {
    gateway::price_sync::save_sync_settings(&db, &settings).await;
    Ok(())
}

#[derive(serde::Serialize, serde::Deserialize, Clone)]
struct ProxySettings {
    port: u16,
    autostart: bool,
    #[serde(default)]
    silent_launch: bool,
}

/// 从 DB 读取 proxy settings；首次运行时自动迁移 proxy_settings.json 文件
async fn load_proxy_settings(app: &tauri::AppHandle) -> Result<ProxySettings, String> {
    let db = app.try_state::<Db>()
        .map(|s| s.inner())
        .ok_or("db not initialized")?;

    // 从 DB 读取
    if let Some(val) = db::get_setting(db, "proxy", "settings").await? {
        let s: ProxySettings = serde_json::from_value(val)
            .map_err(|e| format!("parse proxy settings: {e}"))?;
        return Ok(s);
    }

    // DB 无记录：尝试从旧文件迁移
    let file_path = aidog_data_dir()?.join("proxy_settings.json");
    if file_path.exists() {
        if let Ok(content) = std::fs::read_to_string(&file_path) {
            if let Ok(s) = serde_json::from_str::<ProxySettings>(&content) {
                // 迁移到 DB
                let _ = save_proxy_settings_to_db(db, &s).await;
                // 删除旧文件
                let _ = std::fs::remove_file(&file_path);
                return Ok(s);
            }
        }
    }

    // 默认值
    Ok(ProxySettings { port: 9876, autostart: true, silent_launch: false })
}

async fn save_proxy_settings_to_db(db: &Db, settings: &ProxySettings) -> Result<(), String> {
    let value = serde_json::to_value(settings)
        .map_err(|e| format!("serialize proxy settings: {e}"))?;
    db::set_setting(db, gateway::models::SetSettingInput {
        scope: "proxy".to_string(),
        key: "settings".to_string(),
        value,
    }).await
}

async fn save_proxy_settings(
    app: &tauri::AppHandle,
    port: u16,
    autostart: bool,
    silent_launch: bool,
) -> Result<(), String> {
    let db = app.try_state::<Db>()
        .map(|s| s.inner())
        .ok_or("db not initialized")?;
    let settings = ProxySettings { port, autostart, silent_launch };
    save_proxy_settings_to_db(db, &settings).await
}

// ─── Tray ──────────────────────────────────────────────────

use tauri::menu::{MenuBuilder, MenuItemBuilder};
use tauri::tray::TrayIconBuilder;

/// 托盘渲染（多 item）。从 settings tray config 读取 enabled items（按 order），
/// 每项独立颜色（三态）/ 字号 / line_mode，作为「一列」参与列对齐。
/// 多平台两行模式（iStat Menus 式）：第一行各列标签横排、第二行各列值横排，列上下对齐。
/// 列对齐用 NSTextTab（NSParagraphStyle tabStops），每列一个 tab stop（按列宽估位置）。
/// 全部 single 且无 two_line 列 → 退单行横排（separator 拼接）。
/// 纯文字无 emoji。GUI 实际渲染（暗亮模式对比度 / 列对齐 / 垂直居中）留用户验证。
///
/// 托盘单列：name（标签）+ value（值）+ 颜色（三态）+ 字号 + two_line（该列是否两行展示）。
/// - two_line=true：第一行该列显 name，第二行该列显 value。
/// - two_line=false（single）：第一行该列显 "name value"，第二行该列留空（tab 占位）。
#[derive(Debug, Clone)]
struct TrayColumn {
    name: String,
    value: String,
    color: TrayColor,
    font_size: f64,
    two_line: bool,
    /// "left" | "center" | "right"
    align: String,
    /// 两行模式第二行对齐，None = 跟随 align
    align_row2: Option<String>,
}

/// 托盘渲染布局：columns（数据列）+ gaps（列间间隙）。
/// gaps[i] = columns[i] 与 columns[i+1] 之间的间隙；None = 默认 2px 空白。
struct TrayLayout {
    columns: Vec<TrayColumn>,
    /// 长度 = columns.len() - 1（若 columns.len() ≥ 2）。
    /// None = 默认空白间隙；Some(text) = 自定义分隔符文本。
    gaps: Vec<Option<String>>,
}

/// 计算单个 platform item 的（名, 值）二元组。
/// display="coding" 或平台具 coding plan → 值=`{%}%`（剩余百分比）；否则 值=`{balance:.2}`。
fn platform_item_parts(platform: &Platform, display: &str) -> (String, String) {
    let name = platform.name.clone();
    let plan = gateway::estimate::EstCodingPlan::from_json(&platform.est_coding_plan);
    let first_tier = plan.tiers.first();
    let is_coding = display == "coding" || first_tier.is_some();
    let value = if is_coding {
        let util = first_tier.map(|t| t.est_utilization).unwrap_or(0.0);
        format!("{:.0}%", (100.0 - util).max(0.0))
    } else {
        format!("${}", trim_trailing_zeros(&format!("{:.2}", platform.est_balance_remaining)))
    };
    (name, value)
}

/// 从托盘配置生成有序渲染布局（已按 order 排序、跳过 disabled、跳过取数失败项）。
/// separator items 不生成列，而是作为相邻数据列之间的间隙。
/// gaps[i] = columns[i] 与 columns[i+1] 之间的间隙；None = 默认空白。
async fn tray_layout(app: &tauri::AppHandle) -> TrayLayout {
    let empty = TrayLayout { columns: Vec::new(), gaps: Vec::new() };
    let Some(db) = app.try_state::<Db>() else { return empty; };
    let Ok(Some(config)) = db::get_tray_config(&db).await else { return empty; };
    let mut items: Vec<&TrayItem> = config.items.iter().filter(|i| i.enabled).collect();
    items.sort_by_key(|i| i.order);

    let mut columns: Vec<TrayColumn> = Vec::new();
    let mut gaps: Vec<Option<String>> = Vec::new();
    let mut pending_sep: Option<String> = None;

    for item in items {
        if item.item_type == "separator" {
            pending_sep = Some(if item.display.is_empty() { "·".to_string() } else { item.display.clone() });
            continue;
        }

        // Non-separator item → compute column data
        if !columns.is_empty() {
            gaps.push(pending_sep.take());
        }

        let two_line = item.line_mode == "two";
        let (name, value) = match item.item_type.as_str() {
            "platform" => {
                let Some(pid) = item.platform_id else { continue };
                let Ok(Some(platform)) = db::get_platform(&db, pid).await else { continue };
                platform_item_parts(&platform, &item.display)
            }
            "today_usage" => {
                let stats = db::today_stats(&db).await.unwrap_or(db::TodayStats {
                    tokens: 0, cache_rate: 0.0, cost: 0.0, total_requests: 0,
                });
                let metric = item.metric.as_deref().unwrap_or("tokens");
                let (label, val) = match metric {
                    "cache_rate" => ("Cache".to_string(), format!("{:.0}%", stats.cache_rate)),
                    "cost" => {
                        let d = item.decimals.unwrap_or(5) as usize;
                        ("花费".to_string(), format!("${}", trim_trailing_zeros(&format!("{:.d$}", stats.cost, d = d))))
                    }
                    "requests" => ("请求".to_string(), format!("{}", stats.total_requests)),
                    _ => ("今日".to_string(), format!("{} tok", stats.tokens)),
                };
                (label, val)
            }
            _ => continue,
        };
        if name.is_empty() && value.is_empty() {
            continue;
        }
        // 自定义 label 优先
        let name = item.label.clone().unwrap_or(name);
        columns.push(TrayColumn {
            name, value,
            color: item.color.clone(),
            font_size: item.font_size,
            two_line,
            align: item.align.clone(),
            align_row2: item.align_row2.clone(),
        });
    }

    TrayLayout { columns, gaps }
}

/// 托盘配置的分隔符（多 item 横排间隔）。
async fn tray_separator(app: &tauri::AppHandle) -> String {
    if let Some(db) = app.try_state::<Db>() {
        if let Ok(Some(config)) = db::get_tray_config(&db).await {
            return config.separator;
        }
    }
    default_separator_str()
}

fn default_separator_str() -> String { "  ".to_string() }

/// 菜单内 quota 项的纯文字概要（无颜色/字号，separator 拼接；每列横排 "名 值"）。
fn tray_quota_text(app: &tauri::AppHandle) -> Option<String> {
    let layout = tauri::async_runtime::block_on(tray_layout(app));
    if layout.columns.is_empty() {
        return None;
    }
    let default_sep = tauri::async_runtime::block_on(tray_separator(app));
    let mut texts: Vec<String> = Vec::new();
    for (i, col) in layout.columns.iter().enumerate() {
        if i > 0 {
            let gap = layout.gaps.get(i - 1).and_then(|g| g.clone()).unwrap_or_else(|| " ".to_string());
            texts.push(gap);
        }
        texts.push(format!("{} {}", col.name, col.value));
    }
    Some(texts.join(&default_sep))
}

fn build_tray_menu(app: &tauri::AppHandle) -> Result<tauri::menu::Menu<tauri::Wry>, String> {
    let running = {
        let handle = app.state::<ProxyHandle>();
        let h = handle.0.lock().map_err(|e| e.to_string())?;
        h.is_some()
    };

    let settings = tauri::async_runtime::block_on(load_proxy_settings(app))?;
    let status_text = if running {
        format!("● Proxy Running :{}", settings.port)
    } else {
        "○ Proxy Stopped".to_string()
    };

    let toggle_id = if running { "proxy_stop" } else { "proxy_start" };
    let toggle_text = if running { "Stop Proxy" } else { "Start Proxy" };

    let mut builder = MenuBuilder::new(app)
        .item(&MenuItemBuilder::with_id("status", status_text).enabled(false).build(app).map_err(|e| e.to_string())?);

    // tray quota 详情项（选定平台余额 / coding%）
    if let Some(quota_text) = tray_quota_text(app) {
        builder = builder
            .item(&MenuItemBuilder::with_id("tray_quota", quota_text).enabled(false).build(app).map_err(|e| e.to_string())?);
    }

    let menu = builder
        .separator()
        .item(&MenuItemBuilder::with_id(toggle_id, toggle_text).build(app).map_err(|e| e.to_string())?)
        .separator()
        .item(&MenuItemBuilder::with_id("show", "Show Window").build(app).map_err(|e| e.to_string())?)
        .item(&MenuItemBuilder::with_id("quit", "Quit").build(app).map_err(|e| e.to_string())?)
        .build().map_err(|e| e.to_string())?;

    Ok(menu)
}

/// macOS 菜单栏 tray 文字字号（pt）。默认 set_title 用系统字号（偏大），
/// 这里用 NSStatusItem button 的 attributedTitle 设小号 NSFont（参考菜单栏紧凑文字，约 9pt）。
/// 两行（\n）由 NSFont 行高决定，配合居中段落样式保持紧凑。
#[cfg(target_os = "macos")]
const TRAY_FONT_SIZE: f64 = 9.0;

/// 去除浮点数格式化尾部多余的零：10.10 → "10.1", 0.00 → "0", 965.80 → "965.8"
fn trim_trailing_zeros(s: &str) -> String {
    if let Some(_pos) = s.find('.') {
        let trimmed = s.trim_end_matches('0').trim_end_matches('.');
        if trimmed.is_empty() { "0".to_string() } else { trimmed.to_string() }
    } else {
        s.to_string()
    }
}

/// 将 TrayColor（三态）解析为 NSColor：
/// - follow → labelColor（系统自适应明暗）
/// - preset red/green/orange → systemRed/Green/Orange（自适应明暗）
/// - custom → 解析 hex（#RRGGBB / RRGGBB），sRGB 构造；解析失败回退 labelColor
///   注意：custom 固定色在某些菜单栏主题下可读性差（前端已警告）。
#[cfg(target_os = "macos")]
fn resolve_tray_color(color: &TrayColor) -> objc2::rc::Retained<objc2_app_kit::NSColor> {
    use objc2_app_kit::NSColor;
    match color.mode.as_str() {
        "preset" => match color.value.as_str() {
            "red" => NSColor::systemRedColor(),
            "green" => NSColor::systemGreenColor(),
            "orange" => NSColor::systemOrangeColor(),
            _ => NSColor::labelColor(),
        },
        "custom" => {
            let hex = color.value.trim().trim_start_matches('#');
            if hex.len() == 6 {
                if let (Ok(r), Ok(g), Ok(b)) = (
                    u8::from_str_radix(&hex[0..2], 16),
                    u8::from_str_radix(&hex[2..4], 16),
                    u8::from_str_radix(&hex[4..6], 16),
                ) {
                    return NSColor::colorWithSRGBRed_green_blue_alpha(
                        r as f64 / 255.0,
                        g as f64 / 255.0,
                        b as f64 / 255.0,
                        1.0,
                    );
                }
            }
            NSColor::labelColor()
        }
        // "follow" 及未知 → labelColor
        _ => NSColor::labelColor(),
    }
}

/// 估算列宽（pt）：以最长一行字符数 × 估字宽 + padding。
/// menuBarFont 近似等宽（CJK 全角约 1 字宽 = fontSize，ASCII 半角约 fontSize*0.6）。
/// 精确测量文本渲染宽度：用 AppKit sizeWithAttributes 返回实际像素宽。
/// 需要 MainThread（AppKit 要求），调用方已在主线程闭包内。
#[cfg(target_os = "macos")]
fn measure_text_width(text: &str, font_size: f64) -> f64 {
    use objc2::rc::Retained;
    use objc2::runtime::AnyObject;
    use objc2_app_kit::{NSFont, NSFontAttributeName, NSStringDrawing};
    use objc2_foundation::{NSDictionary, NSString};

    let ns_text = NSString::from_str(text);
    let font = NSFont::boldSystemFontOfSize(font_size);
    let font_key: &NSString = unsafe { NSFontAttributeName };
    let font_obj: &AnyObject = (*font).as_ref();
    let attrs: Retained<NSDictionary<NSString, AnyObject>> =
        NSDictionary::from_slices(&[font_key], &[font_obj]);
    // SAFETY: attrs 类型正确（NSFontAttributeName → NSFont）。
    unsafe { ns_text.sizeWithAttributes(Some(&attrs)) }.width
}

/// macOS：用 attributedTitle 给 tray button 设多列小字（每列独立颜色/字号）。
/// Tauri/tray-icon 的 set_title 走 button.setTitle(NSString) 无字号/颜色控制，故直连 NSStatusItem button。
/// 通过 tauri TrayIcon::with_inner_tray_icon 拿 tray_icon::TrayIcon，再 ns_status_item() 取底层 NSStatusItem。
/// 闭包在主线程执行（with_inner_tray_icon 保证），满足 AppKit 主线程约束。
///
/// 布局（iStat Menus 式）：
/// - 有任一 two_line 列 → **两行多列模式**：
///   - 第一行各列：two_line→name；single→"name value"
///   - 第二行各列：two_line→value；single→""（占位，tab 推进保持列对齐）
///   - 列间 `\t`，行间一个 `\n`；NSParagraphStyle.tabStops 每列一个 NSTextTab(left, 累加列宽)
///   - per-column 着色/字号：逐 cell 用 make_part 构造带 attributes 的子串 append，
///     tab/换行字符用 follow 颜色（无 range:setAttributes，规避 utf16 偏移坑）。
/// - 无 two_line 列 → **单行模式**：沿用 separator 横排拼接（无回归）。
///   整串套用同一 NSParagraphStyle（tabStops + 固定行高居中）+ baselineOffset 垂直居中。
#[cfg(target_os = "macos")]
fn set_tray_attributed_title(
    tray: &tauri::tray::TrayIcon,
    columns: Vec<TrayColumn>,
    gaps: Vec<Option<String>>,
    _separator: String,
) -> Result<(), String> {
    use objc2::rc::Retained;
    use objc2_app_kit::{NSFont, NSFontAttributeName, NSForegroundColorAttributeName, NSParagraphStyleAttributeName};
    use objc2_app_kit::{NSMutableParagraphStyle, NSTextAlignment, NSTextTab, NSTextTabType};
    use objc2_app_kit::NSBaselineOffsetAttributeName;
    use objc2_foundation::{NSArray, NSAttributedString, NSMutableAttributedString, NSDictionary, NSNumber, NSString};
    use objc2::AnyThread;

    tray.with_inner_tray_icon(move |inner| -> Result<(), String> {
        // SAFETY: with_inner_tray_icon 在主线程执行闭包，AppKit 调用满足主线程要求。
        let status_item = inner
            .ns_status_item()
            .ok_or_else(|| "ns_status_item unavailable".to_string())?;
        // MainThreadMarker：闭包已在主线程，断言获取。
        let mtm = objc2_foundation::MainThreadMarker::new()
            .ok_or_else(|| "not on main thread".to_string())?;
        let button = status_item
            .button(mtm)
            .ok_or_else(|| "status item has no button".to_string())?;

        let two_line_mode = columns.iter().any(|c| c.two_line);

        // 段落样式：两行模式压缩行高（min==max）让两行紧凑；单行模式不压缩，字号更大。
        // 两行：9pt × 2 行 ≈ 20pt，贴近菜单栏 ~22pt 高度。
        // 单行：13pt × 1 行，充分利用菜单栏垂直空间。
        let para = NSMutableParagraphStyle::new();
        // 两行模式用左对齐（tabStops 控制列位置）；单行模式居中。
        para.setAlignment(if two_line_mode {
            NSTextAlignment::Left
        } else {
            NSTextAlignment::Center
        });
        let line_h = if two_line_mode {
            TRAY_FONT_SIZE + 5.0 // 两行模式，行间距 10px
        } else {
            0.0 // 单行不压缩行高，使用系统默认
        };
        if two_line_mode {
            para.setMinimumLineHeight(line_h);
            para.setMaximumLineHeight(line_h);
            para.setLineSpacing(0.0);
        }

        // 两行模式：两行共用同一个段落样式（para），均使用 LeftTabStopType。
        // 列宽 = max(第一行该列文字, 第二行该列文字) 实测宽 + padding；位置累加（loc = 各列右边界）。
        // 对齐：通过在文本前填充空格实现右/居中对齐（精确测量 + 空格宽度推算）。
        // 两行都用 left tab @列右边界 → 同一列两行起始位置相同 → 列边界对齐。
        let mut col_widths: Vec<f64> = Vec::new();
        if two_line_mode {
            const COL_PADDING: f64 = 5.0; // 列间最小间距 5px
            let mut left_tabs: Vec<Retained<NSTextTab>> = Vec::new();
            let mut loc: f64 = 0.0;
            for col in columns.iter() {
                let line1 = if col.two_line {
                    col.name.clone()
                } else {
                    format!("{} {}", col.name, col.value)
                };
                let line2 = if col.two_line { col.value.clone() } else { String::new() };
                let w1 = measure_text_width(&line1, TRAY_FONT_SIZE);
                let w2 = measure_text_width(&line2, TRAY_FONT_SIZE + 3.0);
                let col_w = w1.max(w2) + COL_PADDING;
                col_widths.push(col_w);
                loc += col_w;
                left_tabs.push(NSTextTab::initWithType_location(
                    NSTextTab::alloc(),
                    NSTextTabType::LeftTabStopType,
                    loc,
                ));
            }
            let left_array: Retained<NSArray<NSTextTab>> = NSArray::from_retained_slice(&left_tabs);
            para.setTabStops(Some(&left_array));
        }

        // 根据对齐设置在文本前填充空格：right → 左侧填充至列宽；center → 两侧填充。
        let align_text = |text: &str, col_w: f64, font_size: f64, align: &str| -> String {
            if align == "left" || text.is_empty() {
                return text.to_string();
            }
            let text_w = measure_text_width(text, font_size);
            let space_w = measure_text_width(" ", font_size);
            if space_w <= 0.0 { return text.to_string(); }
            let extra = (col_w - text_w).max(0.0);
            let n_spaces = (extra / space_w).round() as usize;
            match align {
                "right" => format!("{}{}", " ".repeat(n_spaces), text),
                "center" => {
                    let half = n_spaces / 2;
                    format!("{}{}{}", " ".repeat(half), text, " ".repeat(n_spaces - half))
                }
                _ => text.to_string(),
            }
        };

        // baselineOffset：两行模式需要负偏移下推居中；单行模式无需偏移。
        let baseline_offset = NSNumber::new_f64(if two_line_mode { -7.0 } else { -5.0 });

        // 单行模式：每列字号覆盖为更大值（只有一行，充分利用菜单栏高度）。
        let single_line_font_size: f64 = 13.0;

        use objc2::runtime::AnyObject;
        let para_key: &NSString = unsafe { NSParagraphStyleAttributeName };
        let baseline_key: &NSString = unsafe { NSBaselineOffsetAttributeName };
        let font_key: &NSString = unsafe { NSFontAttributeName };
        let color_key: &NSString = unsafe { NSForegroundColorAttributeName };

        // 构造单段 attributed string（文字 + 字号 + 颜色 + 指定段落/baseline）。
        // 两行模式：标签行和值行共用 `para`（LeftTabStopType），列边界自然对齐。
        let make_part = |text: &str, font_size: f64, color: &TrayColor, para_style: &NSMutableParagraphStyle| -> Retained<NSAttributedString> {
            let ns_text = NSString::from_str(text);
            let font: Retained<NSFont> = NSFont::boldSystemFontOfSize(font_size);
            let ns_color = resolve_tray_color(color);

            let keys: [&NSString; 4] = [font_key, color_key, para_key, baseline_key];
            let font_obj: &AnyObject = (*font).as_ref();
            let color_obj: &AnyObject = (*ns_color).as_ref();
            let para_obj: &AnyObject = para_style.as_ref();
            let baseline_obj: &AnyObject = (*baseline_offset).as_ref();
            let objects: [&AnyObject; 4] = [font_obj, color_obj, para_obj, baseline_obj];
            let attrs: Retained<NSDictionary<NSString, objc2::runtime::AnyObject>> =
                NSDictionary::from_slices(&keys, &objects);
            // SAFETY: attrs 键为 NSAttributedStringKey(NSString)、值为合法 AppKit 对象，类型正确。
            unsafe {
                NSAttributedString::initWithString_attributes(
                    NSAttributedString::alloc(),
                    &ns_text,
                    Some(&attrs),
                )
            }
        };

        let follow_color = TrayColor::default(); // mode=follow（tab/换行/separator 用）
        let result = NSMutableAttributedString::new();

        if two_line_mode {
            let _default_gap = " ".to_string();
            // 第一行（标签行）：各列首段，列间 \t + gap 文字。整行用 `para`（left tab）。
            for (idx, col) in columns.iter().enumerate() {
                if idx > 0 {
                    result.appendAttributedString(&make_part("\t", TRAY_FONT_SIZE, &follow_color, &para));
                    let gap_text = gaps.get(idx - 1)
                        .and_then(|g| g.clone())
                        .unwrap_or_default();
                    if !gap_text.is_empty() {
                        result.appendAttributedString(&make_part(&gap_text, TRAY_FONT_SIZE, &follow_color, &para));
                    }
                }
                let line1 = if col.two_line {
                    col.name.clone()
                } else {
                    format!("{} {}", col.name, col.value)
                };
                let col_w = col_widths.get(idx).copied().unwrap_or(0.0);
                let aligned = align_text(&line1, col_w, TRAY_FONT_SIZE, &col.align);
                result.appendAttributedString(&make_part(&aligned, TRAY_FONT_SIZE, &col.color, &para));
            }
            // 行间换行
            let nl_font = columns.first().map(|c| c.font_size).unwrap_or(TRAY_FONT_SIZE);
            result.appendAttributedString(&make_part("\n", nl_font, &follow_color, &para));
            // 第二行（值行）：与标签行相同结构，对齐取 align_row2（fallback align）。字体比标签行大1pt。
            for (idx, col) in columns.iter().enumerate() {
                let row2_font = TRAY_FONT_SIZE + 3.0;
                if idx > 0 {
                    result.appendAttributedString(&make_part("\t", row2_font, &follow_color, &para));
                    let gap_text = gaps.get(idx - 1)
                        .and_then(|g| g.clone())
                        .unwrap_or_default();
                    if !gap_text.is_empty() {
                        result.appendAttributedString(&make_part(&gap_text, row2_font, &follow_color, &para));
                    }
                }
                let line2 = if col.two_line { col.value.clone() } else { String::new() };
                if !line2.is_empty() {
                    let row2_align = col.align_row2.as_deref().unwrap_or(&col.align);
                    let col_w = col_widths.get(idx).copied().unwrap_or(0.0);
                    let aligned = align_text(&line2, col_w, row2_font, row2_align);
                    result.appendAttributedString(&make_part(&aligned, row2_font, &col.color, &para));
                }
            }
        } else {
            // 单行模式：每列 "名 值"，列间用 gap 拼接。字号加大（只有一行，充分利用菜单栏高度）。
            let default_gap = " ".to_string();
            let join_font = single_line_font_size;
            for (idx, col) in columns.iter().enumerate() {
                if idx > 0 {
                    let gap_text = gaps.get(idx - 1)
                        .and_then(|g| g.clone())
                        .unwrap_or_else(|| default_gap.clone());
                    result.appendAttributedString(&make_part(&gap_text, join_font, &follow_color, &para));
                }
                let text = format!("{} {}", col.name, col.value);
                result.appendAttributedString(&make_part(&text, single_line_font_size, &col.color, &para));
            }
        }

        button.setAttributedTitle(&result);
        Ok(())
    })
    .map_err(|e| e.to_string())?
}

fn refresh_tray_menu(app: &tauri::AppHandle) -> Result<(), String> {
    let tray = app.tray_by_id("main").ok_or("tray not found")?;
    let menu = build_tray_menu(app)?;
    tray.set_menu(Some(menu)).map_err(|e| e.to_string())?;
    // macOS 菜单栏：有 quota 值时隐藏 logo + 两行小字 title；无值时恢复 logo + 清 title。
    // 非 macOS 平台仅 menu item 降级（不调 set_title / set_icon）。
    #[cfg(target_os = "macos")]
    {
        let layout = tauri::async_runtime::block_on(tray_layout(app));
        if layout.columns.is_empty() {
            tray.set_icon(app.default_window_icon().cloned())
                .map_err(|e| e.to_string())?;
            tray.set_title(None::<&str>).map_err(|e| e.to_string())?;
        } else {
            let separator = tauri::async_runtime::block_on(tray_separator(app));
            tray.set_icon(None).map_err(|e| e.to_string())?;
            // 兜底文字：各列 "名 值"，间隙用 separator
            let fallback_text = layout.columns
                .iter()
                .map(|c| format!("{} {}", c.name, c.value))
                .collect::<Vec<_>>()
                .join(separator.as_str());
            tray.set_title(Some(&fallback_text)).map_err(|e| e.to_string())?;
            if let Err(e) = set_tray_attributed_title(&tray, layout.columns, layout.gaps, separator) {
                tracing::warn!("tray attributed title failed, fallback to default font: {e}");
            }
        }
    }
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
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .setup(|app| {
            // 初始化日志（尽早，在 DB 之前）
            let data_dir = aidog_data_dir().expect("failed to resolve data dir");
            let log_settings = load_app_log_settings();
            logging::init_logging(&data_dir, &log_settings);
            logging::cleanup_old_logs(&data_dir, log_settings.retention_hours);

            // 在 data dir 创建 SQLite
            let db_path = data_dir.join("aidog.db");
            let db = tauri::async_runtime::block_on(async {
                let db = Db::new(db_path.to_str().unwrap()).await.expect("failed to open database");
                db.init_tables().await.expect("failed to init tables");
                // 为所有平台确保存在关联分组（一个平台一个）
                ensure_platform_groups(&db).await;
                db
            });
            app.manage(db);

            // 启动时同步所有 settings 文件（检查不一致并更新）
            {
                let handle = app.handle();
                let db_state = app.state::<Db>();
                tauri::async_runtime::block_on(try_sync_settings(handle, &db_state));
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
                        let settings = tauri::async_runtime::block_on(load_proxy_settings(app)).unwrap_or(ProxySettings {
                            port: 9876,
                            autostart: true,
                            silent_launch: false,
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

            // 监听后台预估发出的 tray-refresh 事件，在主线程刷新托盘（避免后台线程直接操作 tray）
            {
                use tauri::Listener;
                let handle = app.handle().clone();
                app.listen("tray-refresh", move |_| {
                    let _ = refresh_tray_menu(&handle);
                });
            }

            // 自动启动代理
            let settings = tauri::async_runtime::block_on(load_proxy_settings(app.handle()))?;
            if settings.autostart {
                let port = settings.port;
                let handle = app.handle().clone();
                tauri::async_runtime::spawn(async move {
                    let _ = proxy_start(port, handle).await;
                });
            }

            // 冷启动 est 初始化：tray 平台从未真查（last_real_query_at==0）→ 后台真查对齐 est=真实。
            {
                let handle = app.handle().clone();
                tauri::async_runtime::spawn(async move {
                    cold_start_init_tray_estimates(&handle).await;
                });
            }

            // 静默启动：隐藏主窗口，仅托盘运行
            if settings.silent_launch {
                if let Some(w) = app.get_webview_window("main") {
                    let _ = w.hide();
                }
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
            platform_set_tray,
            platform_fetch_models,
            // Tray Config
            tray_config_get,
            tray_config_set,
            tray_today_stats,
            // Group
            group_create,
            group_list,
            group_get,
            group_update,
            group_delete,
            // GroupPlatform
            group_set_platforms,
            group_get_platforms,
            // Aggregate
            group_detail,
            group_detail_list,
            group_reorder,
            // Proxy
            proxy_start,
            proxy_stop,
            proxy_status,
            proxy_get_settings,
            proxy_set_autostart,
            app_set_autolaunch,
            app_get_autolaunch,
            app_set_silent_launch,
            // Proxy Client Settings
            proxy_client_get_settings,
            proxy_client_set_settings,
            // Config Export
            export_claude_config,
            sync_group_settings,
            // Proxy Logs
            proxy_log_list,
            proxy_log_list_filtered,
            proxy_log_count_filtered,
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
            generate_statusline_script,
            read_claude_code_settings,
            // Codex Config
            gateway::codex::codex_config_read,
            gateway::codex::codex_config_write,
            gateway::codex::codex_config_path,
            // Statistics
            stats_query,
            model_test,
            // Platform Usage
            platform_usage_stats,
            group_usage_stats,
            // Platform Quota
            platform_query_quota,
            platform_query_quota_newapi,
            platform_reorder,
            // Model Prices
            model_price_list,
            model_price_count,
            model_price_search,
model_price_list_filtered,
model_price_count_filtered,
            model_price_delete,
            model_price_upsert,
            model_price_resolve,
            model_price_sync,
            price_sync_settings_get,
            price_sync_settings_set,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
