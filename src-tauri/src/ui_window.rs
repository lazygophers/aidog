//! 主界面（主窗口 webview）生命周期：关窗即销毁，托盘常驻；需要时按配置重建。
//!
//! 单一真值：`OPEN` 标志而非 `get_webview_window`——最小化驻托盘时窗口隐藏但 `OPEN` 保持
//! true（存活），关闭销毁后才置 false；两者恢复/重建都走 `open()`。
//! `destroy()` 后窗口从表里摘除要等
//! `RunEvent::WindowEvent::Destroyed`（同 `popover_window.rs` 模块注释），MockRuntime 还从不派发它。
//! macOS Dock 图标与顶部菜单由同一个激活策略开关控制：界面开 = Regular，关 = Accessory；
//! `POLICY` 记录当前策略，MockRuntime 下真实调用不可观测，测试断言该记录。
//! Windows / Linux 任务栏图标随窗口销毁自然消失，策略记录仅为跨平台统一状态。

use std::sync::atomic::{AtomicBool, AtomicU64, AtomicU8, Ordering};
#[cfg(test)]
use std::sync::atomic::AtomicUsize;
use tauri::Listener;
use tauri::{AppHandle, Manager, Runtime};

pub(crate) const LABEL: &str = "main";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Policy {
    Regular,
    Accessory,
}

static OPEN: AtomicBool = AtomicBool::new(false);
static POLICY: AtomicU8 = AtomicU8::new(0);

/// 进程正在退出（ExitRequested / Exit）：此时窗口被销毁不是「进托盘常驻」，不发首次关窗提示。
static EXITING: AtomicBool = AtomicBool::new(false);

pub(crate) fn mark_exiting() {
    EXITING.store(true, Ordering::SeqCst);
}

/// `ExitRequested` 是否该拦：`code=None` 是「最后一个窗口销毁」，界面已不在 = 托盘常驻，要拦；
/// 显式退出码（托盘/菜单退出）或界面还在时的 Cmd+Q 照常退出。
pub(crate) fn should_prevent_exit(code: Option<i32>) -> bool {
    code.is_none() && !exists()
}

/// 界面是否存在（`AppCtx::ui_exists` 经 `tauri_ctx::set_ui_probe` 读它）。
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

/// 系统通知被点击（tauri-plugin-notification 2.8 `on_action`，点击本体的 action 为 `"tap"`）：打开界面。
/// 更新通知的挂起版本已在 settings，界面就绪时 `flush_pending` 自会补弹更新提示，无需来源标记。
pub(crate) fn on_notification_action<R: Runtime>(app: &AppHandle<R>, action_id: &str) {
    if action_id != "tap" {
        return;
    }
    // 回调在插件的 spawn_blocking 线程；建窗须主线程。
    let a = app.clone();
    if let Err(e) = app.run_on_main_thread(move || open(&a)) {
        tracing::warn!(error = %e, "notification tap: dispatch to main thread failed");
    }
}

/// 在任何通知发出前注册点击回调（setup 调一次）。
pub(crate) fn register_notification_tap<R: Runtime>(app: &AppHandle<R>) {
    use tauri_plugin_notification::NotificationExt;
    let a = app.clone();
    if let Err(e) = app
        .notification()
        .on_action(move |p| on_notification_action(&a, p.action_id()))
    {
        tracing::warn!(error = %e, "notification on_action register failed");
    }
}

/// 前端 `closeGuard` 在 close-requested 里同步 emit 的存活确认（见 `utils/closeGuard.ts`）。
pub(crate) const CLOSE_ACK_EVENT: &str = "aidog-close-ack";
/// 前端放行关窗、即将 `destroy()` 前 emit：此刻起旧 webview 不再接收 deep link。
pub(crate) const UI_CLOSING_EVENT: &str = "aidog-ui-closing";

/// 关窗兜底超时：有 JS close-requested 监听时 Rust 自动 prevent_close（tauri-2.12.5
/// `manager/window.rs:177-181`），前端挂起则永远关不掉，超时没收到 ack 就强制销毁。
pub(crate) const CLOSE_ACK_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(3);

/// 失焦后等最小化动画/状态落定再查 `is_minimized`，避免正常失焦误判。
pub(crate) const MINIMIZE_CHECK_DELAY: std::time::Duration = std::time::Duration::from_millis(200);

/// 主窗口 CloseRequested 计数 / 最近一次已 ack 的计数；定时器只认自己那一次请求。
static CLOSE_GEN: AtomicU64 = AtomicU64::new(0);
static ACK_GEN: AtomicU64 = AtomicU64::new(0);

#[cfg(test)]
static FORCED_DESTROYS: AtomicUsize = AtomicUsize::new(0);

/// 主窗口收到 CloseRequested：起超时计时，前端没在时限内 ack 就强制销毁。
pub(crate) fn on_close_requested<R: Runtime>(app: &AppHandle<R>, timeout: std::time::Duration) {
    let seq = CLOSE_GEN.fetch_add(1, Ordering::SeqCst) + 1;
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(timeout).await;
        if ACK_GEN.load(Ordering::SeqCst) >= seq {
            return;
        }
        tracing::warn!("close ack timed out, destroying main window");
        if let Some(w) = app.get_webview_window(LABEL) {
            #[cfg(test)]
            FORCED_DESTROYS.fetch_add(1, Ordering::SeqCst);
            let _ = w.destroy();
        }
    });
}

/// 前端 ack：当前这次关窗请求已被前端接住。
pub(crate) fn on_close_ack() {
    ACK_GEN.store(CLOSE_GEN.load(Ordering::SeqCst), Ordering::SeqCst);
}

/// 挂关窗相关的前端事件监听（setup 阶段调一次）。
pub(crate) fn setup<R: Runtime>(app: &AppHandle<R>) {
    app.listen(CLOSE_ACK_EVENT, |_| on_close_ack());
    app.listen(UI_CLOSING_EVENT, |_| crate::deep_link::mark_closed());
}

/// 界面已关闭（窗口销毁后）：策略切 Accessory。
pub(crate) fn on_closed<R: Runtime>(app: &AppHandle<R>) {
    crate::deep_link::mark_closed();
    OPEN.store(false, Ordering::SeqCst);
    set_policy(app, Policy::Accessory);
    if !EXITING.load(Ordering::SeqCst) {
        // 放行关窗到销毁之间到达的链接排在收件箱里：重建界面让它们有去处。
        if crate::deep_link::has_pending() {
            open(app);
            return;
        }
        tauri::async_runtime::spawn(async {
            if let Some(c) = aidog_ctx::try_ctx() {
                aidog_core::ui_notice::notify_first_tray_close(c).await;
            }
        });
    }
}

/// 主窗口失焦：延迟后查 `is_minimized`，真最小化才转托盘驻留（正常失焦不动）。
pub(crate) fn on_focus_lost<R: Runtime>(app: &AppHandle<R>) {
    let a = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(MINIMIZE_CHECK_DELAY).await;
        let Some(w) = a.get_webview_window(LABEL) else {
            return;
        };
        match w.is_minimized() {
            Ok(true) => minimize_to_tray(&a),
            Ok(false) => {}
            Err(e) => tracing::warn!(error = %e, "is_minimized check failed"),
        }
    });
}

/// 最小化转托盘：窗口隐藏但存活（OPEN 保持 true，前端状态保留），策略切 Accessory，
/// 恢复走既有 `open()`（托盘 show / Dock Reopen → show + focus + Regular）。
pub(crate) fn minimize_to_tray<R: Runtime>(app: &AppHandle<R>) {
    set_policy(app, Policy::Accessory);
    if let Some(w) = app.get_webview_window(LABEL) {
        // 先退出最小化态再 hide：macOS Dock 不留 miniaturized 残影，恢复时 open() 直接 show 正常态。
        let _ = w.unminimize();
        let _ = w.hide();
    }
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
    use std::sync::Arc;
    use tauri::test::MockRuntime;
    use tauri::Emitter;

    fn setup_events(app: &tauri::App<MockRuntime>) {
        super::setup(app.handle());
    }

    fn setup() -> (std::sync::MutexGuard<'static, ()>, tauri::App<MockRuntime>) {
        test_setup()
    }

    fn forced() -> usize {
        FORCED_DESTROYS.load(Ordering::SeqCst)
    }

    #[test]
    fn notification_tap_opens_ui_other_actions_do_not() {
        let (_g, app) = setup();
        on_notification_action(app.handle(), "other");
        assert!(!exists(), "非 tap 动作不开界面");
        on_notification_action(app.handle(), "tap");
        assert!(exists(), "tap 打开界面");
    }

    #[test]
    fn close_without_ack_force_destroys_after_timeout() {
        let (_g, app) = setup();
        setup_events(&app);
        open(app.handle());
        let before = forced();
        on_close_requested(app.handle(), std::time::Duration::from_millis(50));
        std::thread::sleep(std::time::Duration::from_millis(400));
        assert_eq!(forced(), before + 1, "无 ack → 超时销毁");
    }

    #[test]
    fn close_with_ack_is_not_destroyed_by_timer() {
        let (_g, app) = setup();
        setup_events(&app);
        open(app.handle());
        let before = forced();
        on_close_requested(app.handle(), std::time::Duration::from_millis(150));
        app.emit(CLOSE_ACK_EVENT, ()).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(500));
        assert_eq!(forced(), before, "有 ack → 不由计时器销毁");
    }

    #[test]
    fn closing_event_resets_deep_link_ready_and_destroy_replays_to_new_ui() {
        let (_g, app) = setup();
        setup_events(&app);
        let n = Arc::new(AtomicUsize::new(0));
        let n2 = n.clone();
        app.listen("aidog-deep-link", move |_| {
            n2.fetch_add(1, Ordering::SeqCst);
        });
        open(app.handle());
        crate::deep_link::mark_ready(app.handle());
        app.emit(UI_CLOSING_EVENT, ()).unwrap();
        crate::deep_link::deliver(
            app.handle(),
            crate::deep_link::DeepLinkPayload { entity: "platform".into(), action: "import".into(), data: "x".into() },
        );
        assert_eq!(n.load(Ordering::SeqCst), 0, "放行后到达的链接不得 emit 给旧窗口");
        on_closed(app.handle());
        assert!(exists(), "有排队链接：旧窗口销毁后重建界面");
        crate::deep_link::mark_ready(app.handle());
        assert_eq!(n.load(Ordering::SeqCst), 1, "新界面就绪后补发");
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
    fn exit_prevented_only_when_last_window_gone_and_no_code() {
        let (_g, app) = setup();
        open(app.handle());
        assert!(!should_prevent_exit(None), "界面还在：Cmd+Q 照常退出");
        on_closed(app.handle());
        assert!(should_prevent_exit(None), "最后窗口销毁：留托盘");
        assert!(!should_prevent_exit(Some(0)), "显式退出码照常退出");
    }

    #[test]
    fn start_normal_opens_ui() {
        let (_g, app) = setup();
        start(app.handle(), false);
        assert!(exists());
        assert_eq!(policy(), Policy::Regular);
    }

    #[test]
    fn minimize_to_tray_hides_but_keeps_ui_alive_and_open_restores() {
        let (_g, app) = setup();
        open(app.handle());
        minimize_to_tray(app.handle());
        assert!(exists(), "最小化驻托盘：界面仍存活（前端状态保留）");
        assert_eq!(policy(), Policy::Accessory, "最小化驻托盘：策略切 Accessory");
        assert!(
            app.get_webview_window(LABEL).is_some(),
            "窗口不销毁，托盘 show / Dock Reopen 走 show+focus 分支"
        );
        open(app.handle());
        assert_eq!(policy(), Policy::Regular, "恢复后策略回 Regular");
    }

    #[test]
    fn focus_lost_without_minimize_keeps_ui_visible() {
        let (_g, app) = setup();
        open(app.handle());
        on_focus_lost(app.handle());
        std::thread::sleep(MINIMIZE_CHECK_DELAY + std::time::Duration::from_millis(400));
        assert!(exists());
        assert_eq!(policy(), Policy::Regular, "正常失焦不得触发托盘驻留");
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
