mod gateway;

use gateway::db::{self, Db};
use gateway::models::*;
use tauri::State;

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

    Ok(format!("proxy started on port {}", port))
}

#[tauri::command]
async fn proxy_stop(app: tauri::AppHandle) -> Result<(), String> {
    let handle = app.state::<ProxyHandle>();
    let mut h = handle.0.lock().map_err(|e| e.to_string())?;
    if let Some(jh) = h.take() {
        jh.abort();
    }
    Ok(())
}

#[tauri::command]
fn proxy_status(app: tauri::AppHandle) -> Result<bool, String> {
    let handle = app.state::<ProxyHandle>();
    let h = handle.0.lock().map_err(|e| e.to_string())?;
    Ok(h.is_some())
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
            app.manage(db);
            app.manage(ProxyHandle(StdMutex::new(None)));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // Platform
            platform_create,
            platform_list,
            platform_get,
            platform_update,
            platform_delete,
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
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
