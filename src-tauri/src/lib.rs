mod gateway;

use gateway::db::{self, Db};
use gateway::models::*;
use tauri::State;
use serde_json::Value;

// ─── Platform Commands ─────────────────────────────────────

#[tauri::command]
fn platform_create(input: CreatePlatform, db: State<'_, Db>) -> Result<Platform, String> {
    db::create_platform(&db, input)
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
    db::update_platform(&db, input)
}

#[tauri::command]
fn platform_delete(id: String, db: State<'_, Db>) -> Result<(), String> {
    db::delete_platform(&db, &id)
}

// ─── Group Commands ────────────────────────────────────────

#[tauri::command]
fn group_create(input: CreateGroup, db: State<'_, Db>) -> Result<Group, String> {
    db::create_group(&db, input)
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
fn group_update(input: UpdateGroup, db: State<'_, Db>) -> Result<Group, String> {
    db::update_group(&db, input)
}

#[tauri::command]
fn group_delete(id: String, db: State<'_, Db>) -> Result<(), String> {
    db::delete_group(&db, &id)
}

// ─── GroupPlatform Commands ────────────────────────────────

#[tauri::command]
fn group_set_platforms(input: SetGroupPlatforms, db: State<'_, Db>) -> Result<(), String> {
    db::set_group_platforms(&db, &input.group_id, &input.platforms)
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
fn mapping_create(input: CreateModelMapping, db: State<'_, Db>) -> Result<ModelMapping, String> {
    db::create_model_mapping(&db, input)
}

#[tauri::command]
fn mapping_list(group_id: String, db: State<'_, Db>) -> Result<Vec<ModelMapping>, String> {
    db::list_model_mappings(&db, &group_id)
}

#[tauri::command]
fn mapping_update(input: UpdateModelMapping, db: State<'_, Db>) -> Result<ModelMapping, String> {
    db::update_model_mapping(&db, input)
}

#[tauri::command]
fn mapping_delete(id: String, db: State<'_, Db>) -> Result<(), String> {
    db::delete_model_mapping(&db, &id)
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
    let db_path = {
        let app_dir = app
            .path()
            .app_data_dir()
            .map_err(|e| e.to_string())?;
        app_dir.join("aidog.db")
    };
    let proxy_db = Db::new(db_path.to_str().unwrap_or(""))?;
    let proxy_db = std::sync::Mutex::new(proxy_db);

    let proxy_handle = gateway::proxy::start_proxy(proxy_db, port).await?;

    {
        let mut h = handle.0.lock().map_err(|e| e.to_string())?;
        *h = Some(proxy_handle);
    }

    // 保存端口到设置
    save_proxy_settings(&app, port, true)?;

    // 更新托盘菜单
    refresh_tray_menu(&app)?;

    Ok(format!("proxy started on port {}", port))
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
        Protocol::Anthropic | Protocol::ClaudeCode => {
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
        Protocol::OpenAI | Protocol::Codex | Protocol::GLM | Protocol::Kimi | Protocol::MiniMax => {
            let url = format!("{base}/models");
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

// ─── Settings Persistence ──────────────────────────────────

#[derive(serde::Serialize, serde::Deserialize, Clone)]
struct ProxySettings {
    port: u16,
    autostart: bool,
}

fn settings_path(app: &tauri::AppHandle) -> Result<std::path::PathBuf, String> {
    let app_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| e.to_string())?;
    Ok(app_dir.join("proxy_settings.json"))
}

fn load_proxy_settings(app: &tauri::AppHandle) -> Result<ProxySettings, String> {
    let path = settings_path(app)?;
    if path.exists() {
        let content = std::fs::read_to_string(&path)
            .map_err(|e| format!("read settings: {e}"))?;
        serde_json::from_str(&content).map_err(|e| format!("parse settings: {e}"))
    } else {
        Ok(ProxySettings { port: 8080, autostart: false })
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
        .setup(|app| {
            // 在 app data dir 创建 SQLite
            let app_dir = app
                .path()
                .app_data_dir()
                .expect("failed to resolve app data dir");
            std::fs::create_dir_all(&app_dir).ok();
            let db_path = app_dir.join("aidog.db");
            let db = Db::new(db_path.to_str().unwrap()).expect("failed to open database");
            db.init_tables().expect("failed to init tables");
            db.run_migrations().expect("failed to run migrations");
            app.manage(db);
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
                            port: 8080,
                            autostart: false,
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
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
