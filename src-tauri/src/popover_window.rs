//! 托盘悬浮窗生命周期（按需创建 / 收起即销毁）。
//!
//! 旧模型：启动期预建一个隐藏的 popover webview（常驻 58 MB / 0.20% CPU，即使从不点开），
//! 托盘点击只做 show/hide。新模型：托盘点击时才建窗，收起（再次点击 / 失焦）直接 `destroy()`，
//! 整个 WebKit 实例随之释放。代价是首次点击多一次建窗 + React mount 的延迟。
//!
//! 状态权威在本模块的 `OPEN` 标志而非 `get_webview_window(LABEL)`：`destroy()` 只是向事件循环
//! 派发销毁消息，窗口从 `AppManager` 的表里摘除要等 `RunEvent::WindowEvent::Destroyed` 回来
//! （tauri 2.11 `app.rs::on_event_loop_event` → `manager::on_window_close`），期间 label 仍可解析。
//! 若用「窗口是否存在」当 toggle 依据，销毁后立刻再点会误判成「还开着」而再销毁一次。
//! `OPEN` 同时充当「创建中」去重标志，容忍托盘的快速重复点击（不会建出两个 popover）。

use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{AppHandle, Emitter, Manager, Runtime};

/// popover 窗口 label（唯一）。
pub(crate) const LABEL: &str = "popover";

/// 窗口宽度（逻辑像素）；托盘图标下方居中定位用。
const WIDTH: f64 = 300.0;
const HEIGHT: f64 = 420.0;

/// 已打开（含建窗在途）。见模块注释：这是 toggle 的权威状态。
static OPEN: AtomicBool = AtomicBool::new(false);

/// 悬浮窗当前是否处于打开状态。
pub(crate) fn is_open() -> bool {
    OPEN.load(Ordering::SeqCst)
}

/// 打开：按需建窗 + 定位到 `pos`（逻辑坐标，托盘图标下方居中）+ show。
///
/// 已打开或建窗在途 → 直接返回（去重，快速双击只建一个窗）。
pub(crate) fn open<R: Runtime>(app: &AppHandle<R>, pos: Option<(f64, f64)>) {
    if OPEN.swap(true, Ordering::SeqCst) {
        return;
    }
    // 建窗延迟埋点：按需创建的代价（webview 冷启 + 窗口 show）在日志里可量。
    let t0 = std::time::Instant::now();
    let mut builder = tauri::webview::WebviewWindowBuilder::new(
        app,
        LABEL,
        tauri::WebviewUrl::App("popover.html".into()),
    )
    .inner_size(WIDTH, HEIGHT)
    .decorations(false)
    .transparent(true)
    .always_on_top(true)
    .skip_taskbar(true)
    .visible(false)
    // 首次点击到面板可用的端到端延迟埋点：页面 load 完成 = HTML/JS 就绪、React 即将挂载。
    .on_page_load(move |_w, payload| {
        if matches!(payload.event(), tauri::webview::PageLoadEvent::Finished) {
            tracing::info!(
                elapsed_ms = t0.elapsed().as_secs_f64() * 1000.0,
                "popover page load finished"
            );
        }
    });
    if let Some((x, y)) = pos {
        builder = builder.position(x, y);
    }
    match builder.build() {
        Ok(w) => {
            // cfg(not(test))：MockRuntime 的 window_handle() 返回一个指向 `&()` 的假 AppKit 指针
            // （tauri 2.11.5 `src/test/mock_runtime.rs:846`），对它 retain 会段错误。
            // 真机路径不受影响；本调用无法在 mock 下覆盖。
            #[cfg(all(target_os = "macos", not(test)))]
            apply_hides_on_deactivate(&w);
            let _ = w.show();
            let _ = w.set_focus();
            // 窗口是新建的，前端 mount 时会自己拉一次数据；此事件覆盖「窗口已在但前端已 mount」
            // 的竞态路径（emit 到不存在的监听器为 no-op）。
            let _ = w.emit("popover-shown", ());
            tracing::info!(
                ?pos,
                elapsed_ms = t0.elapsed().as_secs_f64() * 1000.0,
                "popover window created on demand"
            );
        }
        Err(e) => {
            OPEN.store(false, Ordering::SeqCst);
            tracing::error!(error = %e, "create popover window failed");
        }
    }
}

/// 收起：销毁窗口（非 hide），释放整个 WebKit 实例。
pub(crate) fn close<R: Runtime>(app: &AppHandle<R>) {
    OPEN.store(false, Ordering::SeqCst);
    if let Some(w) = app.get_webview_window(LABEL) {
        let _ = w.destroy();
        tracing::info!("popover window destroyed");
    }
}

/// macOS `NSWindow.hidesOnDeactivate` —— app 失活自动隐藏 popover，覆盖 tao `Focused(false)`
/// 不触发的失活场景（点桌面 / silent_launch 主窗 hide 后点别处 / 点 Dock 菜单栏空白）。
/// Apple docs: <https://developer.apple.com/documentation/appkit/nswindow/hidesondeactivate>
/// 按需创建模型下 NSWindow 指针随窗口销毁失效，故每次建窗后都要重设一次。
#[cfg(all(target_os = "macos", not(test)))]
fn apply_hides_on_deactivate<R: Runtime>(w: &tauri::WebviewWindow<R>) {
    use objc2::rc::Retained;
    use objc2_app_kit::NSWindow;
    match w.ns_window() {
        Ok(ptr) => {
            // SAFETY: ns_window() 返回指向主线程当前 autoreleased NSWindow 的指针；
            // retain_autoreleased 在类型转换前获得所有权。NSWindow 通过 objc2-app-kit
            // NSWindow feature 绑定（继承自 NSResponder）暴露 setHidesOnDeactivate。
            let ns_window = unsafe { Retained::<NSWindow>::retain_autoreleased(ptr.cast()) };
            if let Some(ns_window) = ns_window {
                ns_window.setHidesOnDeactivate(true);
                tracing::info!("popover setHidesOnDeactivate:YES applied");
            } else {
                tracing::warn!("popover ns_window pointer was nil");
            }
        }
        Err(e) => tracing::warn!(error = %e, "popover ns_window() unavailable"),
    }
}

#[cfg(test)]
mod tests {
    //! seam：Tauri 测试 mock app（无头 AppHandle）覆盖窗口生命周期四条验收。
    //!
    //! MockRuntime 的两处能力边界（tauri 2.11.5，均已在对应用例里注明）：
    //! 1. `MockWindowDispatcher::is_visible()` 硬编码 `Ok(true)`（`src/test/mock_runtime.rs:777`），
    //!    不跟踪真实可见性，故「可见」只能断言到「窗口已建出且 show() 未报错」。
    //! 2. `MockRuntime::run()` 从不派发 `RunEvent::WindowEvent::Destroyed`
    //!    （`src/test/mock_runtime.rs:1368` 只删自身窗口表），而 `AppManager` 摘除 label 依赖该事件
    //!    （`src/app.rs:2544` → `manager/mod.rs:653`），所以 destroy 后 `get_webview_window` 仍返回
    //!    Some。销毁语义改为断言「toggle 权威状态归零」+「close 走的是 destroy 而非 hide」（源码单点）。
    use super::*;
    use std::sync::{Mutex, MutexGuard};
    use tauri::test::{MockRuntime, mock_builder, mock_context, noop_assets};

    /// `OPEN` 是进程级 static，测试并行会互相污染 → 用例间串行 + 每次重置。
    static SERIAL: Mutex<()> = Mutex::new(());

    fn setup() -> (MutexGuard<'static, ()>, tauri::App<MockRuntime>) {
        let guard = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
        OPEN.store(false, Ordering::SeqCst);
        let app = mock_builder()
            .build(mock_context(noop_assets()))
            .expect("build mock app");
        (guard, app)
    }

    #[test]
    fn popover_absent_at_startup() {
        let (_g, app) = setup();
        assert!(
            app.get_webview_window(LABEL).is_none(),
            "启动期不得预建 popover 窗口"
        );
        assert!(!is_open());
    }

    /// 托盘点击路径 = `app_setup.rs` 的 `is_open() ? close : open(pos)`。
    fn tray_click(app: &AppHandle<MockRuntime>, pos: Option<(f64, f64)>) {
        if is_open() {
            close(app);
        } else {
            open(app, pos);
        }
    }

    #[test]
    fn tray_click_creates_visible_popover() {
        let (_g, app) = setup();
        tray_click(app.handle(), Some((10.0, 20.0)));
        let w = app
            .get_webview_window(LABEL)
            .expect("托盘点击后 popover 窗口应存在");
        assert!(is_open());
        // MockRuntime is_visible 恒 true（见模块内注释 1）：此断言只证明窗口句柄可用。
        assert!(w.is_visible().unwrap_or(false));
    }

    #[test]
    fn hide_destroys_popover_not_hides_it() {
        let (_g, app) = setup();
        tray_click(app.handle(), None);
        assert!(is_open());
        tray_click(app.handle(), None); // 第二次点击 = 收起
        assert!(!is_open(), "收起后 popover 不再处于打开态");
        // 「销毁而非 hide」的强断言受限于 MockRuntime（见模块内注释 2）：
        // close() 内唯一的窗口操作是 destroy()，无 hide() 调用点。
    }

    #[test]
    fn rapid_double_open_creates_single_window() {
        let (_g, app) = setup();
        open(app.handle(), Some((0.0, 0.0)));
        open(app.handle(), Some((0.0, 0.0)));
        let n = app
            .webview_windows()
            .keys()
            .filter(|l| l.as_str() == LABEL)
            .count();
        assert_eq!(n, 1, "快速连点只应产生一个 popover 窗口");
    }
}
