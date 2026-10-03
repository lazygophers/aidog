//! 菜单栏一帧数据的**取数侧**（票 I20）。
//!
//! 与渲染侧（`crate::menubar`，macOS + AppKit + 主线程）分家的理由只有一个：
//! Flutter 壳里这两件事不在同一个进程里。DB 归 `aidog-kernel`，`NSStatusItem` 归
//! Runner（NSApp 只有它有）。所以取数在这边、渲染在那边，中间隔着一次 JSON：
//!
//! ```text
//! kernel: collect_state(db) ──JSON──► Dart ──method channel──► Swift ──C ABI──► menubar::render
//! ```
//!
//! Tauri 壳里两侧同进程，`menubar::collect_state` 仍是本模块的 re-export，调用点一字未改。

use crate::tray_render::{TrayLayout, tray_layout, tray_quota_text, tray_separator};
use aidog_db::Db;

/// 一次渲染所需的全部数据。DB 取数在 [`collect_state`]（任意线程 await），
/// 取完再跳主线程交给 `menubar::render`——AppKit 调用里不做 IO。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MenuBarState {
    pub layout: TrayLayout,
    pub separator: String,
    /// 菜单首行状态文字，如 `"● Proxy Running :9890"`。
    pub status_text: String,
    /// 菜单第二行的余额 / 配额概要；None = 不加该项。
    pub quota_text: Option<String>,
    pub proxy_running: bool,
}

/// 取一次渲染所需的数据。可在任意线程 await。
pub async fn collect_state(db: &Db) -> MenuBarState {
    let proxy_running = aidog_ctx::ctx().proxy_handle().is_running();
    let port = crate::shared::load_proxy_settings(db)
        .await
        .map(|s| s.port)
        .unwrap_or(9890);
    MenuBarState {
        layout: tray_layout(db).await,
        separator: tray_separator(db).await,
        status_text: status_text(proxy_running, port),
        quota_text: tray_quota_text(db).await,
        proxy_running,
    }
}

/// 菜单首行的代理状态文字（与原 Tauri 菜单 `build_tray_menu` 的文案一致）。
pub fn status_text(running: bool, port: u16) -> String {
    if running {
        format!("● Proxy Running :{port}")
    } else {
        "○ Proxy Stopped".to_string()
    }
}

crate::tauri_command! {
    /// 菜单栏一帧数据。Flutter 壳用它喂 Runner 里的 `NSStatusItem`；
    /// Tauri 壳同进程直接调 [`collect_state`]，不走这条 RPC。
    pub async fn menu_bar_state() -> Result<MenuBarState, String> {
        Ok(collect_state(aidog_ctx::db()).await)
    }
}
