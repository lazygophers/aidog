//! [`AppCtx`] 的无界面内核实现（票 08）。
//!
//! 与桌面壳 `aidog_core::tauri_ctx::TauriCtx` 一一对应，区别只在「界面副作用」那半边：
//!
//! | 能力 | 桌面壳 | 内核 |
//! |---|---|---|
//! | db / middleware / proxy_handle | 同一份实例 | 同一份实例 |
//! | emit | Tauri `emit` 到 webview | 广播给 SSE `/events` 的订阅者 |
//! | 系统通知弹窗 | tauri-plugin-notification | 无桌面会话 → `false`（不是错误） |
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

/// 一条广播出去的界面事件（事件名 + payload）。
#[derive(Clone, Debug)]
pub struct KernelEvent {
    pub name: String,
    pub payload: Value,
}

/// 广播缓冲条数。订阅者跟不上时最老的事件被丢（`RecvError::Lagged`），
/// 而不是把代理热路径的 `emit` 堵住 —— 事件是「该刷新了」的提示，丢几条不影响正确性。
const EVENT_CHANNEL_CAP: usize = 256;

pub struct HeadlessCtx {
    db: Db,
    middleware: Arc<MiddlewareEngine>,
    proxy: ProxyHandle,
    events: broadcast::Sender<KernelEvent>,
}

impl HeadlessCtx {
    pub fn new(db: Db, middleware: Arc<MiddlewareEngine>) -> Self {
        let (events, _) = broadcast::channel(EVENT_CHANNEL_CAP);
        Self {
            db,
            middleware,
            proxy: ProxyHandle::new(),
            events,
        }
    }

    /// 订阅事件流（`/events` 每个连接一份）。
    pub fn subscribe(&self) -> broadcast::Receiver<KernelEvent> {
        self.events.subscribe()
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

    fn autolaunch_enabled(&self) -> Result<bool, String> {
        Err("autolaunch is managed by the service manager (systemd/launchd) in kernel mode".into())
    }

    fn set_autolaunch(&self, _enabled: bool) -> Result<(), String> {
        Err("autolaunch is managed by the service manager (systemd/launchd) in kernel mode".into())
    }
}
