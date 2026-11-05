//! 主界面（主窗口 webview）生命周期：关窗即销毁，托盘常驻；需要时按配置重建。
//!
//! 单一真值：`OPEN` 标志而非 `get_webview_window`——`destroy()` 后窗口从表里摘除要等
//! `RunEvent::WindowEvent::Destroyed`（同 `popover_window.rs` 模块注释），MockRuntime 还从不派发它。
//! macOS Dock 图标与顶部菜单由同一个激活策略开关控制：界面开 = Regular，关 = Accessory；
//! `POLICY` 记录当前策略，MockRuntime 下真实调用不可观测，测试断言该记录。
//! Windows / Linux 任务栏图标随窗口销毁自然消失，策略记录仅为跨平台统一状态。

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use tauri::{AppHandle, Manager, Runtime};

pub(crate) const LABEL: &str = "main";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Policy {
    Regular,
    Accessory,
}

static OPEN: AtomicBool = AtomicBool::new(false);
static POLICY: AtomicU8 = AtomicU8::new(0);

/// 界面是否存在。后续票的内核查询会消费，目前仅测试用。
#[allow(dead_code)]
pub(crate) fn exists() -> bool {
    OPEN.load(Ordering::SeqCst)
}

/// 当前记录的激活策略（仅测试断言用）。
#[cfg(test)]
pub(crate) fn policy() -> Policy {
    if POLICY.load(Ordering::SeqCst) == 0 {
        Policy::Regular
    } else {
        Policy::Accessory
    }
}

fn set_policy<R: Runtime>(app: &AppHandle<R>, p: Policy) {
    POLICY.store((p == Policy::Accessory) as u8, Ordering::SeqCst);
    #[cfg(target_os = "macos")]
    {
        let ap = match p {
            Policy::Regular => tauri::ActivationPolicy::Regular,
            Policy::Accessory => tauri::ActivationPolicy::Accessory,
        };
        if let Err(e) = app.set_activation_policy(ap) {
            tracing::warn!(error = %e, ?p, "set activation policy failed");
        }
    }
    #[cfg(not(target_os = "macos"))]
    let _ = app;
}

/// 打开界面：不存在则按主窗口配置重建，存在则 show + focus；策略切 Regular。
pub(crate) fn open<R: Runtime>(app: &AppHandle<R>) {
    set_policy(app, Policy::Regular);
    let w = match app.get_webview_window(LABEL) {
        Some(w) => w,
        None => {
            let cfg = app
                .config()
                .app
                .windows
                .iter()
                .find(|c| c.label == LABEL)
                .cloned()
                .unwrap_or_default();
            match tauri::WebviewWindowBuilder::from_config(app, &cfg).and_then(|b| b.build()) {
                Ok(w) => w,
                Err(e) => {
                    tracing::error!(error = %e, "create main window failed");
                    return;
                }
            }
        }
    };
    OPEN.store(true, Ordering::SeqCst);
    let _ = w.show();
    let _ = w.set_focus();
}

/// 启动入口：静默启动不建窗口、直接 Accessory；否则打开界面。
pub(crate) fn start<R: Runtime>(app: &AppHandle<R>, silent: bool) {
    if silent {
        set_policy(app, Policy::Accessory);
    } else {
        open(app);
    }
}

/// macOS 再次启动 / 点 Dock 图标（`RunEvent::Reopen`）。
#[allow(dead_code)] // 仅 macOS 的 RunEvent::Reopen 分支调用
pub(crate) fn on_reopen<R: Runtime>(app: &AppHandle<R>) {
    open(app);
}

/// Windows / Linux 第二实例启动：首实例打开界面（aidog:// 参数由 single-instance 的
/// deep-link 特性转给 deep-link 插件 → `dispatch_urls`）。
#[allow(dead_code)] // 仅非 macOS 的单实例回调调用
pub(crate) fn on_second_instance<R: Runtime>(app: &AppHandle<R>) {
    open(app);
}

/// 界面已关闭（窗口销毁后）：策略切 Accessory。
pub(crate) fn on_closed<R: Runtime>(app: &AppHandle<R>) {
    crate::deep_link::mark_closed();
    OPEN.store(false, Ordering::SeqCst);
    set_policy(app, Policy::Accessory);
}

/// 测试共享：`OPEN` / `POLICY` / deep link 收件箱是进程级 static → 用例间串行 + 每次重置。
#[cfg(test)]
pub(crate) fn test_setup() -> (
    std::sync::MutexGuard<'static, ()>,
    tauri::App<tauri::test::MockRuntime>,
) {
    use tauri::test::{mock_builder, mock_context, noop_assets};
    static SERIAL: std::sync::Mutex<()> = std::sync::Mutex::new(());
    let guard = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    OPEN.store(false, Ordering::SeqCst);
    POLICY.store(1, Ordering::SeqCst);
    crate::deep_link::reset_for_test();
    let app = mock_builder()
        .build(mock_context(noop_assets()))
        .expect("build mock app");
    (guard, app)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tauri::test::MockRuntime;

    fn setup() -> (std::sync::MutexGuard<'static, ()>, tauri::App<MockRuntime>) {
        test_setup()
    }

    #[test]
    fn ui_open_creates_window_and_sets_regular() {
        let (_g, app) = setup();
        assert!(!exists());
        open(app.handle());
        assert!(exists());
        assert_eq!(policy(), Policy::Regular);
        assert!(app.get_webview_window(LABEL).is_some());
    }

    #[test]
    fn ui_closed_means_absent_and_accessory() {
        let (_g, app) = setup();
        open(app.handle());
        on_closed(app.handle());
        assert!(!exists());
        assert_eq!(policy(), Policy::Accessory);
    }

    #[test]
    fn ui_open_twice_does_not_duplicate_window() {
        let (_g, app) = setup();
        open(app.handle());
        open(app.handle());
        assert_eq!(app.webview_windows().len(), 1);
        assert!(exists());
    }

    #[test]
    fn ui_reopen_after_close_cycles() {
        let (_g, app) = setup();
        for _ in 0..3 {
            open(app.handle());
            assert!(exists());
            assert_eq!(policy(), Policy::Regular);
            // MockRuntime 不派发 Destroyed，窗口表不摘除：手动 destroy 后走 on_closed，
            // 再 open 时窗口仍可解析 → 走 show+focus 分支，同样须恢复 Regular。
            if let Some(w) = app.get_webview_window(LABEL) {
                let _ = w.destroy();
            }
            on_closed(app.handle());
            assert!(!exists());
            assert_eq!(policy(), Policy::Accessory);
        }
    }

    #[test]
    fn start_silent_creates_no_window_and_is_accessory() {
        let (_g, app) = setup();
        start(app.handle(), true);
        assert!(!exists());
        assert!(app.get_webview_window(LABEL).is_none());
        assert_eq!(policy(), Policy::Accessory);
    }

    #[test]
    fn start_normal_opens_ui() {
        let (_g, app) = setup();
        start(app.handle(), false);
        assert!(exists());
        assert_eq!(policy(), Policy::Regular);
    }

    #[test]
    fn reopen_and_second_instance_open_ui_from_closed() {
        let (_g, app) = setup();
        on_reopen(app.handle());
        assert!(exists());
        on_closed(app.handle());
        on_second_instance(app.handle());
        assert!(exists());
        assert_eq!(policy(), Policy::Regular);
    }
}
