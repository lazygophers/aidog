//! [`AppCtx`] 的无界面内核实现（票 08）。
//!
//! 与桌面壳 `aidog_core::tauri_ctx::TauriCtx` 一一对应，区别只在「界面副作用」那半边：
//!
//! | 能力 | 桌面壳 | 内核 |
//! |---|---|---|
//! | db / middleware / proxy_handle | 同一份实例 | 同一份实例 |
//! | emit | Tauri `emit` 到 webview | 广播给 SSE `/events` 的订阅者 |
//! | 系统通知弹窗 | tauri-plugin-notification | 广播 `notif-popup` 给外壳，由外壳去弹 |
//! | TTS 交给界面读 | emit 给 webview | 纯内核无界面 → `false`（进程内 `say` 后端不经本 trait） |
//! | 开机自启 | tauri-plugin-autostart | 由 systemd 一类服务管理器负责 → `Err` |
//!
//! **纯内核形态（不带 `--ui`）下 emit 一样在跑**，只是没有订阅者，广播即丢弃。
//! 这样命令体与代理热路径不必知道自己跑在哪种形态里。

use std::sync::Arc;

use aidog_ctx::{AppCtx, ProxyHandle};
use aidog_db::Db;
use aidog_middleware::MiddlewareEngine;
use serde_json::Value;
use tokio::sync::broadcast;

pub struct HeadlessCtx {
    db: Db,
    middleware: Arc<MiddlewareEngine>,
    proxy: ProxyHandle,
}

impl HeadlessCtx {
    pub fn new(db: Db, middleware: Arc<MiddlewareEngine>) -> Self {
        Self {
            db,
            middleware,
            proxy: ProxyHandle::new(),
        }
    }

    /// 订阅事件流（测试用；生产订阅者是代理上的 `/api/events`）。
    pub fn subscribe(&self) -> tokio::sync::broadcast::Receiver<aidog_ctx::UiEvent> {
        aidog_ctx::subscribe_events()
    }
}

impl AppCtx for HeadlessCtx {
    fn db(&self) -> &Db {
        &self.db
    }

    fn middleware(&self) -> &Arc<MiddlewareEngine> {
        &self.middleware
    }

    fn proxy_handle(&self) -> &ProxyHandle {
        &self.proxy
    }

    fn emit(&self, event: &str, payload: Value) {
        // 没有订阅者时 `send` 返回 Err —— 与桌面壳「没有 webview 在听不是错误」同语义，
        // 丢弃即可（纯内核形态下这是常态，不该刷日志）。
        let _ = self.events.send(KernelEvent {
            name: event.to_string(),
            payload,
        });
    }

    /// 内核自己没有桌面会话，但外壳有。把弹窗请求原样广播出去（事件名
    /// [`aidog_notification::NOTIF_POPUP`]），外壳用自己那套通知 API 弹。
    ///
    /// 返回 `true` 的含义与 `speak_via_ui` 一致：**请求已交出去**。内核无从知道外壳最后
    /// 弹没弹（用户可能没授权），返回 `false` 会让 `DispatchResult.popup` 一律报「没弹」，
    /// 比乐观返回更失真。
    fn show_popup(&self, title: &str, body: &str) -> bool {
        self.emit(
            aidog_notification::NOTIF_POPUP,
            serde_json::json!({ "title": title, "body": body }),
        );
        true
    }

    fn autolaunch_enabled(&self) -> Result<bool, String> {
        Err("autolaunch is managed by the service manager (systemd/launchd) in kernel mode".into())
    }

    fn set_autolaunch(&self, _enabled: bool) -> Result<(), String> {
        Err("autolaunch is managed by the service manager (systemd/launchd) in kernel mode".into())
    }
}
