use aidog_core::shared::*;
use aidog_core::sync_settings::do_sync_group_settings;
use aidog_core::tray_render::refresh_tray_menu;
use aidog_core::gateway::middleware::MiddlewareEngine;
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
pub(crate) async fn proxy_start(
    port: u16,
    app: tauri::AppHandle,
) -> Result<String, String> {
    tracing::debug!(command = "proxy_start", port, "command invoked");
    // 检查是否已运行
    let handle = app.state::<ProxyHandle>();
    {
        let h = handle.0.lock().map_err(|e| e.to_string())?;
        if h.is_some() {
            tracing::warn!(command = "proxy_start", "proxy already running");
            return Err("proxy already running".to_string());
        }
    }

    // 获取 DB 的路径并克隆一份连接
    let db_path = aidog_data_dir()?.join("aidog.db");
    let proxy_db = Db::new(db_path.to_str().unwrap_or("")).await
        .map_err(|e| { tracing::error!(command = "proxy_start", error = %e, "open proxy db failed"); e })?;
    let proxy_db = std::sync::Arc::new(proxy_db);

    // 读取绑定模式（0.0.0.0 LAN / 127.0.0.1 本机）；地址只在 bind 时读取一次。
    let saved = load_proxy_settings(&app).await.unwrap_or(ProxySettings { port: 9876, autostart: true, silent_launch: false, bind_lan: true });

    // 复用 setup 阶段 app.manage 的同一 MiddlewareEngine 单例（CRUD reload 与代理消费同源）。
    let middleware = app.state::<Arc<MiddlewareEngine>>().inner().clone();
    let (proxy_handle, actual_port) = gateway::proxy::start_proxy(proxy_db, port, Some(app.clone()), middleware, saved.bind_lan).await
        .map_err(|e| { tracing::error!(command = "proxy_start", port, error = %e, "start_proxy failed"); e })?;

    {
        let mut h = handle.0.lock().map_err(|e| e.to_string())?;
        *h = Some(proxy_handle);
    }

    // 保存实际使用的端口到设置
    save_proxy_settings(&app, actual_port, true, saved.silent_launch, saved.bind_lan).await?;

    // 同步所有分组的 settings 文件（端口可能变了）
    if let Some(db) = app.try_state::<Db>() {
        if let Err(e) = do_sync_group_settings(&db, actual_port).await {
            tracing::warn!(command = "proxy_start", port = actual_port, error = %e, "sync group settings after start failed");
        }
    }

    // 更新托盘菜单
    refresh_tray_menu(&app, &super::tray::TrayMenuBuildImpl).await?;

    let msg = if actual_port != port {
        format!("proxy started on port {} ({} was occupied)", actual_port, port)
    } else {
        format!("proxy started on port {}", actual_port)
    };
    tracing::info!(command = "proxy_start", port = actual_port, "proxy started");
    Ok(msg)
}

#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %aidog_core::logging::new_trace_id()))]
pub(crate) async fn proxy_stop(app: tauri::AppHandle) -> Result<(), String> {
    tracing::debug!(command = "proxy_stop", "command invoked");
    let handle = app.state::<ProxyHandle>();
    {
        let mut h = handle.0.lock().map_err(|e| e.to_string())?;
        if let Some(jh) = h.take() {
            jh.abort();
        }
    }

    // 更新设置
    if let Ok(settings) = load_proxy_settings(&app).await {
        save_proxy_settings(&app, settings.port, false, settings.silent_launch, settings.bind_lan).await
            .map_err(|e| { tracing::error!(command = "proxy_stop", error = %e, "persist proxy settings failed"); e })?;
    }

    refresh_tray_menu(&app, &super::tray::TrayMenuBuildImpl).await?;
    tracing::info!(command = "proxy_stop", "proxy stopped");
    Ok(())
}

#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %aidog_core::logging::new_trace_id()))]
pub fn proxy_status(app: tauri::AppHandle) -> Result<bool, String> {
    tracing::debug!(command = "proxy_status", "command invoked");
    let handle = app.state::<ProxyHandle>();
    let h = handle.0.lock().map_err(|e| e.to_string())?;
    Ok(h.is_some())
}

#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %aidog_core::logging::new_trace_id()))]
pub async fn proxy_get_settings(app: tauri::AppHandle) -> Result<ProxySettings, String> {
    tracing::debug!(command = "proxy_get_settings", "command invoked");
    load_proxy_settings(&app).await
}

#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %aidog_core::logging::new_trace_id()))]
pub async fn proxy_set_autostart(app: tauri::AppHandle, enabled: bool) -> Result<(), String> {
    tracing::debug!(command = "proxy_set_autostart", enabled, "command invoked");
    let current = load_proxy_settings(&app).await?;
    save_proxy_settings(&app, current.port, enabled, current.silent_launch, current.bind_lan).await
        .map_err(|e| { tracing::error!(command = "proxy_set_autostart", error = %e, "persist proxy settings failed"); e })?;
    Ok(())
}

#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %aidog_core::logging::new_trace_id()))]
pub async fn proxy_set_bind_lan(app: tauri::AppHandle, enabled: bool) -> Result<(), String> {
    tracing::debug!(command = "proxy_set_bind_lan", enabled, "command invoked");
    let current = load_proxy_settings(&app).await?;
    save_proxy_settings(&app, current.port, current.autostart, current.silent_launch, enabled).await
        .map_err(|e| { tracing::error!(command = "proxy_set_bind_lan", error = %e, "persist proxy settings failed"); e })?;
    // 绑定地址只在 bind 时读取 → 若代理在跑，重启使新地址生效。
    if proxy_status(app.clone())? {
        proxy_stop(app.clone()).await?;
        proxy_start(current.port, app.clone()).await?;
    }
    Ok(())
}

#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %aidog_core::logging::new_trace_id()))]
pub fn app_set_autolaunch(app: tauri::AppHandle, enabled: bool) -> Result<(), String> {
    tracing::debug!(command = "app_set_autolaunch", enabled, "command invoked");
    use tauri_plugin_autostart::ManagerExt;
    let manager = app.autolaunch();
    if enabled {
        manager.enable().map_err(|e| { tracing::error!(command = "app_set_autolaunch", error = %e, "enable autolaunch failed"); format!("enable autolaunch: {e}") })?;
    } else {
        manager.disable().map_err(|e| { tracing::error!(command = "app_set_autolaunch", error = %e, "disable autolaunch failed"); format!("disable autolaunch: {e}") })?;
    }
    Ok(())
}

#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %aidog_core::logging::new_trace_id()))]
pub fn app_get_autolaunch(app: tauri::AppHandle) -> Result<bool, String> {
    tracing::debug!(command = "app_get_autolaunch", "command invoked");
    use tauri_plugin_autostart::ManagerExt;
    let manager = app.autolaunch();
    manager.is_enabled().map_err(|e| { tracing::warn!(command = "app_get_autolaunch", error = %e, "get autolaunch failed"); format!("get autolaunch: {e}") })
}

#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %aidog_core::logging::new_trace_id()))]
pub async fn app_set_silent_launch(app: tauri::AppHandle, enabled: bool) -> Result<(), String> {
    tracing::debug!(command = "app_set_silent_launch", enabled, "command invoked");
    let current = load_proxy_settings(&app).await?;
    save_proxy_settings(&app, current.port, current.autostart, enabled, current.bind_lan).await
        .map_err(|e| { tracing::error!(command = "app_set_silent_launch", error = %e, "persist proxy settings failed"); e })?;
    Ok(())
}

// ─── Proxy Client Settings (upstream HTTP proxy) ─────────────

#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %aidog_core::logging::new_trace_id()))]
pub async fn proxy_client_get_settings(app: tauri::AppHandle) -> Result<gateway::models::ProxyClientSettings, String> {
    tracing::debug!(command = "proxy_client_get_settings", "command invoked");
    let db = app.try_state::<Db>()
        .map(|s| s.inner().clone())
        .ok_or_else(|| { tracing::error!(command = "proxy_client_get_settings", "db not initialized"); "db not initialized".to_string() })?;
    let settings = gateway::http_client::load_proxy_client_settings(&Arc::new(db)).await;
    Ok(settings)
}

#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %aidog_core::logging::new_trace_id()))]
pub async fn proxy_client_set_settings(app: tauri::AppHandle, settings: gateway::models::ProxyClientSettings) -> Result<(), String> {
    tracing::debug!(command = "proxy_client_set_settings", "command invoked");
    let db = app.try_state::<Db>()
        .map(|s| s.inner())
        .ok_or_else(|| { tracing::error!(command = "proxy_client_set_settings", "db not initialized"); "db not initialized".to_string() })?;
    let value = serde_json::to_value(&settings)
        .map_err(|e| format!("serialize proxy client settings: {e}"))?;
    db::set_setting(db, gateway::models::SetSettingInput {
        scope: "proxy".to_string(),
        key: "proxy_client".to_string(),
        value,
    }).await
        .map_err(|e| { tracing::error!(command = "proxy_client_set_settings", error = %e, "persist proxy client settings failed"); e })
}
