use crate::shared::*;
use crate::sync_settings::do_sync_group_settings;
use aidog_middleware::MiddlewareEngine;
// 托盘刷新经 Tauri event 解耦：emit "tray-refresh"，app crate setup() 内已有 listener
// (app_setup.rs:395) 调 refresh_tray_menu + TrayMenuBuildImpl。复用现有事件 +
// listener（同 proxy/log.rs:164 同域 precedent），避 commands_proxy → commands_platform
// 跨 command 依赖 + 零新 wiring 代码。
use crate::gateway;
use aidog_db::{self as db, Db};
use std::sync::Arc;
use tauri::{Emitter, Manager};

/// proxy_start 命令的结构化错误（proxy-port-no-drift s2：区分「端口占用」与「其他绑定失败」，
/// 供前端错误条 / s3 系统通知据此判别，禁靠字符串前缀匹配）。`message` 是英文调试信息，
/// **禁前端直接展示给用户** —— 用户可见文案走 i18n，按 `kind` + `port` 拼模板。
#[derive(Debug, Clone, serde::Serialize)]
pub struct ProxyStartError {
    pub kind: ProxyStartErrorKind,
    pub port: u16,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProxyStartErrorKind {
    AddrInUse,
    Other,
}

impl std::fmt::Display for ProxyStartError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl From<gateway::proxy::ProxyBindError> for ProxyStartError {
    fn from(e: gateway::proxy::ProxyBindError) -> Self {
        let port = e.port();
        let kind = match e {
            gateway::proxy::ProxyBindError::AddrInUse(_) => ProxyStartErrorKind::AddrInUse,
            gateway::proxy::ProxyBindError::Other(..) => ProxyStartErrorKind::Other,
        };
        Self {
            kind,
            port,
            message: e.to_string(),
        }
    }
}

// proxy_set_bind_lan（在 tauri_command! 宏内，错误类型固定 String）内部用 `?` 转调本命令，
// 需要 ProxyStartError -> String 的转换支撑该 `?`。
impl From<ProxyStartError> for String {
    fn from(e: ProxyStartError) -> Self {
        e.message
    }
}

// 结构化错误类型无法套 tauri_command! 宏（宏固定 Result<_, String>），手写等价的
// instrument + debug 日志。
#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %crate::logging::new_trace_id()))]
pub async fn proxy_start(port: u16, app: tauri::AppHandle) -> Result<String, ProxyStartError> {
    tracing::debug!(command = "proxy_start", port, "command invoked");
    let other_err = |port: u16, message: String| ProxyStartError {
        kind: ProxyStartErrorKind::Other,
        port,
        message,
    };

    // 检查是否已运行
    let handle = app.state::<ProxyHandle>();
    {
        let h = handle
            .0
            .lock()
            .map_err(|e| other_err(port, e.to_string()))?;
        if h.is_some() {
            tracing::warn!(command = "proxy_start", "proxy already running");
            return Err(other_err(port, "proxy already running".to_string()));
        }
    }

    // 复用主 Db 实例（app.manage 在 app_setup 注入，含 proxy_log / stats_agg 独立 handle +
    // 读池 + 进程内缓存）。禁独立 Db::new：会开第二条 log.db 写连接，与主实例争锁且
    // 绕过 proxy_log/settings 缓存，致数据不一致。clone 廉价（Arc 引用计数，共享同一后台线程）。
    let proxy_db = std::sync::Arc::new(app.state::<Db>().inner().clone());

    // 读取绑定模式（0.0.0.0 LAN / 127.0.0.1 本机）；地址只在 bind 时读取一次。
    let saved = load_proxy_settings(&app).await.unwrap_or(ProxySettings {
        port: 9890,
        autostart: true,
        silent_launch: false,
        bind_lan: false,
    });

    // 复用 setup 阶段 app.manage 的同一 MiddlewareEngine 单例（CRUD reload 与代理消费同源）。
    let middleware = app.state::<Arc<MiddlewareEngine>>().inner().clone();
    let proxy_handle = gateway::proxy::start_proxy(
        proxy_db,
        port,
        Some(app.clone()),
        middleware,
        saved.bind_lan,
    )
    .await
    .map_err(|e| {
        let err: ProxyStartError = e.into();
        tracing::error!(command = "proxy_start", port, error = %err, "start_proxy failed");
        err
    })?;

    {
        let mut h = handle
            .0
            .lock()
            .map_err(|e| other_err(port, e.to_string()))?;
        *h = Some(proxy_handle);
    }

    // 端口是用户设定值，不是启动流程的输出 —— 不回写设置（proxy-port-no-drift 根因 2）。

    // 同步所有分组的 settings 文件
    if let Some(db) = app.try_state::<Db>()
        && let Err(e) = do_sync_group_settings(&db, port).await
    {
        tracing::warn!(command = "proxy_start", port, error = %e, "sync group settings after start failed");
    }

    // 通知 app crate 刷新托盘菜单（emit "tray-refresh"，listener 在 app_setup.rs:395）
    let _ = app.emit("tray-refresh", ());

    tracing::info!(command = "proxy_start", port, "proxy started");
    Ok(format!("proxy started on port {port}"))
}

crate::tauri_command! {
pub async fn proxy_stop(app: tauri::AppHandle) -> Result<(), String> {
    let handle = app.state::<ProxyHandle>();
    {
        let mut h = handle.0.lock().map_err(|e| e.to_string())?;
        if let Some(jh) = h.take() {
            jh.abort();
        }
    }

    // 通知 app crate 刷新托盘菜单（emit "tray-refresh"，listener 在 app_setup.rs:395）
    let _ = app.emit("tray-refresh", ());
    tracing::info!(command = "proxy_stop", "proxy stopped");
    Ok(())
}
}

#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %crate::logging::new_trace_id()))]
pub fn proxy_status(app: tauri::AppHandle) -> Result<bool, String> {
    tracing::debug!(command = "proxy_status", "command invoked");
    let handle = app.state::<ProxyHandle>();
    let h = handle.0.lock().map_err(|e| e.to_string())?;
    Ok(h.is_some())
}

crate::tauri_command! {
pub async fn proxy_get_settings(app: tauri::AppHandle) -> Result<ProxySettings, String> {
    load_proxy_settings(&app).await
}
}

crate::tauri_command! {
pub async fn proxy_set_autostart(app: tauri::AppHandle, enabled: bool) -> Result<(), String> {
    tracing::debug!(command = "proxy_set_autostart", enabled, "command invoked");
    let current = load_proxy_settings(&app).await?;
    save_proxy_settings(&app, current.port, enabled, current.silent_launch, current.bind_lan).await
        .map_err(|e| { tracing::error!(command = "proxy_set_autostart", error = %e, "persist proxy settings failed"); e })?;
    Ok(())
}
}

crate::tauri_command! {
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
}

#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %crate::logging::new_trace_id()))]
pub fn app_set_autolaunch(app: tauri::AppHandle, enabled: bool) -> Result<(), String> {
    tracing::debug!(command = "app_set_autolaunch", enabled, "command invoked");
    use tauri_plugin_autostart::ManagerExt;
    let manager = app.autolaunch();
    if enabled {
        manager.enable().map_err(|e| {
            tracing::error!(command = "app_set_autolaunch", error = %e, "enable autolaunch failed");
            format!("enable autolaunch: {e}")
        })?;
    } else {
        manager.disable().map_err(|e| { tracing::error!(command = "app_set_autolaunch", error = %e, "disable autolaunch failed"); format!("disable autolaunch: {e}") })?;
    }
    Ok(())
}

#[tauri::command]
#[tracing::instrument(skip_all, fields(trace_id = %crate::logging::new_trace_id()))]
pub fn app_get_autolaunch(app: tauri::AppHandle) -> Result<bool, String> {
    tracing::debug!(command = "app_get_autolaunch", "command invoked");
    use tauri_plugin_autostart::ManagerExt;
    let manager = app.autolaunch();
    manager.is_enabled().map_err(|e| {
        tracing::warn!(command = "app_get_autolaunch", error = %e, "get autolaunch failed");
        format!("get autolaunch: {e}")
    })
}

crate::tauri_command! {
pub async fn app_set_silent_launch(app: tauri::AppHandle, enabled: bool) -> Result<(), String> {
    tracing::debug!(command = "app_set_silent_launch", enabled, "command invoked");
    let current = load_proxy_settings(&app).await?;
    save_proxy_settings(&app, current.port, current.autostart, enabled, current.bind_lan).await
        .map_err(|e| { tracing::error!(command = "app_set_silent_launch", error = %e, "persist proxy settings failed"); e })?;
    Ok(())
}
}

// ─── Proxy Client Settings (upstream HTTP proxy) ─────────────

crate::tauri_command! {
pub async fn proxy_client_get_settings(app: tauri::AppHandle) -> Result<gateway::models::ProxyClientSettings, String> {
    let db = app.try_state::<Db>()
        .map(|s| s.inner().clone())
        .ok_or_else(|| { tracing::error!(command = "proxy_client_get_settings", "db not initialized"); "db not initialized".to_string() })?;
    let settings = gateway::http_client::load_proxy_client_settings(&Arc::new(db)).await;
    Ok(settings)
}
}

crate::tauri_command! {
pub async fn proxy_client_set_settings(app: tauri::AppHandle, settings: gateway::models::ProxyClientSettings) -> Result<(), String> {
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
}
