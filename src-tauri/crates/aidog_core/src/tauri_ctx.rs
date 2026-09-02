//! [`AppCtx`] 的桌面壳实现（票 06）。
//!
//! 这里是**唯一**允许把 `AppHandle` 与命令层联系起来的地方：命令体只认
//! `aidog_ctx::ctx()`，`AppHandle` 到此为止。无界面内核（票 08）写一个等价的
//! `HeadlessCtx` 即可复用全部命令体。
//!
//! 托盘渲染不走这里——`tray_render` 的 `build_tray_menu` / `refresh_tray_menu`
//! 仍直接吃 `AppHandle`，由 root bin 调用（票 06 明确不解耦托盘）。

use std::sync::Arc;

use aidog_ctx::{AppCtx, ProxyHandle};
use aidog_db::Db;
use aidog_middleware::MiddlewareEngine;
use tauri::Emitter;

pub struct TauriCtx {
    app: tauri::AppHandle,
    db: Db,
    middleware: Arc<MiddlewareEngine>,
    proxy: ProxyHandle,
}

impl TauriCtx {
    /// `db` / `middleware` 传的必须是 app_setup 里那两个实例本体（`Db` 克隆共享同一后台
    /// 连接、`Arc<MiddlewareEngine>` 共享同一桶），禁在此新建。
    pub fn new(app: tauri::AppHandle, db: Db, middleware: Arc<MiddlewareEngine>) -> Self {
        Self {
            app,
            db,
            middleware,
            proxy: ProxyHandle::new(),
        }
    }

    /// 桌面壳专用逃生口：托盘 / 窗口 / 插件等**不进 `AppCtx`** 的能力从这里取 `AppHandle`。
    pub fn app_handle(&self) -> &tauri::AppHandle {
        &self.app
    }
}

impl AppCtx for TauriCtx {
    fn db(&self) -> &Db {
        &self.db
    }

    fn middleware(&self) -> &Arc<MiddlewareEngine> {
        &self.middleware
    }

    fn proxy_handle(&self) -> &ProxyHandle {
        &self.proxy
    }

    fn emit(&self, event: &str, payload: serde_json::Value) {
        // 迁移前各调用点一律 `let _ = app.emit(...)`：没有 webview 在听不是错误。
        let _ = self.app.emit(event, payload);
    }

    fn show_popup(&self, title: &str, body: &str) -> bool {
        use tauri_plugin_notification::NotificationExt;
        match self
            .app
            .notification()
            .builder()
            .title(title)
            .body(body)
            .show()
        {
            Ok(()) => true,
            Err(e) => {
                tracing::warn!(error = %e, "notify: popup show failed");
                false
            }
        }
    }

    fn speak_via_ui(&self, text: &str) -> bool {
        let _ = self.app.emit(aidog_notification::NOTIF_SPEAK, text);
        true
    }

    fn autolaunch_enabled(&self) -> Result<bool, String> {
        use tauri_plugin_autostart::ManagerExt;
        self.app
            .autolaunch()
            .is_enabled()
            .map_err(|e| format!("get autolaunch: {e}"))
    }

    fn set_autolaunch(&self, enabled: bool) -> Result<(), String> {
        use tauri_plugin_autostart::ManagerExt;
        let manager = self.app.autolaunch();
        if enabled {
            manager.enable().map_err(|e| format!("enable autolaunch: {e}"))
        } else {
            manager
                .disable()
                .map_err(|e| format!("disable autolaunch: {e}"))
        }
    }
}

/// 建 `TauriCtx` 并装进进程单例。app_setup 在 `Db` 与 `MiddlewareEngine` 就绪后调一次。
pub fn install(app: tauri::AppHandle, db: Db, middleware: Arc<MiddlewareEngine>) {
    aidog_ctx::install(Arc::new(TauriCtx::new(app, db, middleware)));
}
