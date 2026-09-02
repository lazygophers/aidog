use crate::shared::*;
use crate::sync_settings::do_sync_group_settings;
// 托盘刷新经 Tauri event 解耦：emit "tray-refresh"，app crate setup() 内已有 listener
// (app_setup.rs:395) 调 refresh_tray_menu + TrayMenuBuildImpl。复用现有事件 +
// listener（同 proxy/log.rs:164 同域 precedent），避 commands_proxy → commands_platform
// 跨 command 依赖 + 零新 wiring 代码。
use crate::gateway;
use aidog_db as db;
use std::sync::Arc;

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

// 结构化错误类型走 tauri_command! 的兜底分支（不自动补 Err 日志，错误分支已手写）。
crate::tauri_command! {
pub async fn proxy_start(port: u16) -> Result<String, ProxyStartError> {
    tracing::debug!(command = "proxy_start", port, "command invoked");
    let other_err = |port: u16, message: String| ProxyStartError {
        kind: ProxyStartErrorKind::Other,
        port,
        message,
    };

    let ctx = aidog_ctx::ctx();

    // 检查是否已运行
    let handle = ctx.proxy_handle();
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

    // 复用主 Db 实例（AppCtx 持有的那一个，含 proxy_log / stats_agg 独立 handle +
    // 读池 + 进程内缓存）。禁独立 Db::new：会开第二条 log.db 写连接，与主实例争锁且
    // 绕过 proxy_log/settings 缓存，致数据不一致。clone 廉价（Arc 引用计数，共享同一后台线程）。
    let proxy_db = std::sync::Arc::new(ctx.db().clone());

    // 读取绑定模式（0.0.0.0 LAN / 127.0.0.1 本机）；地址只在 bind 时读取一次。
    let saved = load_proxy_settings(ctx.db()).await.unwrap_or(ProxySettings {
        port: 9890,
        autostart: true,
        silent_launch: false,
        bind_lan: false,
    });

    // 复用 setup 阶段建的同一 MiddlewareEngine 单例（CRUD reload 与代理消费同源）。
    let middleware = ctx.middleware().clone();
    let proxy_handle = gateway::proxy::start_proxy(
        proxy_db,
        port,
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
    if let Err(e) = do_sync_group_settings(ctx.db(), port).await {
        tracing::warn!(command = "proxy_start", port, error = %e, "sync group settings after start failed");
    }

    // 通知桌面壳刷新托盘菜单（emit "tray-refresh"，listener 在 app_setup.rs）
    ctx.emit("tray-refresh", serde_json::Value::Null);

    tracing::info!(command = "proxy_start", port, "proxy started");
    Ok(format!("proxy started on port {port}"))
}
}

crate::tauri_command! {
pub async fn proxy_stop() -> Result<(), String> {
    let handle = aidog_ctx::ctx().proxy_handle();
    {
        let mut h = handle.0.lock().map_err(|e| e.to_string())?;
        if let Some(jh) = h.take() {
            jh.abort();
        }
    }

    // 通知桌面壳刷新托盘菜单（emit "tray-refresh"，listener 在 app_setup.rs）
    aidog_ctx::emit_unit("tray-refresh");
    tracing::info!(command = "proxy_stop", "proxy stopped");
    Ok(())
}
}

crate::tauri_command! {
pub fn proxy_status() -> Result<bool, String> {
    let handle = aidog_ctx::ctx().proxy_handle();
    let h = handle.0.lock().map_err(|e| e.to_string())?;
    Ok(h.is_some())
}
}

crate::tauri_command! {
pub async fn proxy_get_settings() -> Result<ProxySettings, String> {
    load_proxy_settings(aidog_ctx::db()).await
}
}

crate::tauri_command! {
pub async fn proxy_set_autostart(enabled: bool) -> Result<(), String> {
    tracing::debug!(command = "proxy_set_autostart", enabled, "command invoked");
    let db = aidog_ctx::db();
    let current = load_proxy_settings(db).await?;
    save_proxy_settings(db, current.port, enabled, current.silent_launch, current.bind_lan).await
        .map_err(|e| { tracing::error!(command = "proxy_set_autostart", error = %e, "persist proxy settings failed"); e })?;
    Ok(())
}
}

crate::tauri_command! {
pub async fn proxy_set_bind_lan(enabled: bool) -> Result<(), String> {
    tracing::debug!(command = "proxy_set_bind_lan", enabled, "command invoked");
    let db = aidog_ctx::db();
    let current = load_proxy_settings(db).await?;
    save_proxy_settings(db, current.port, current.autostart, current.silent_launch, enabled).await
        .map_err(|e| { tracing::error!(command = "proxy_set_bind_lan", error = %e, "persist proxy settings failed"); e })?;
    // 绑定地址只在 bind 时读取 → 若代理在跑，重启使新地址生效。
    if proxy_status()? {
        proxy_stop().await?;
        proxy_start(current.port).await?;
    }
    Ok(())
}
}

crate::tauri_command! {
pub fn app_set_autolaunch(enabled: bool) -> Result<(), String> {
    tracing::debug!(command = "app_set_autolaunch", enabled, "command invoked");
    aidog_ctx::ctx().set_autolaunch(enabled)
}
}

crate::tauri_command! {
pub fn app_get_autolaunch() -> Result<bool, String> {
    aidog_ctx::ctx().autolaunch_enabled()
}
}

crate::tauri_command! {
pub async fn app_set_silent_launch(enabled: bool) -> Result<(), String> {
    tracing::debug!(command = "app_set_silent_launch", enabled, "command invoked");
    let db = aidog_ctx::db();
    let current = load_proxy_settings(db).await?;
    save_proxy_settings(db, current.port, current.autostart, enabled, current.bind_lan).await
        .map_err(|e| { tracing::error!(command = "app_set_silent_launch", error = %e, "persist proxy settings failed"); e })?;
    Ok(())
}
}

// ─── Proxy Client Settings (upstream HTTP proxy) ─────────────

crate::tauri_command! {
pub async fn proxy_client_get_settings() -> Result<gateway::models::ProxyClientSettings, String> {
    let db = Arc::new(aidog_ctx::db().clone());
    Ok(gateway::http_client::load_proxy_client_settings(&db).await)
}
}

crate::tauri_command! {
pub async fn proxy_client_set_settings(settings: gateway::models::ProxyClientSettings) -> Result<(), String> {
    let db = aidog_ctx::db();
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
