//! macOS 菜单栏原生宿主（票 I10）。
//!
//! 自己向 `NSStatusBar::systemStatusBar()` 要一个 `NSStatusItem`，不再经 Tauri 的
//! `TrayIcon` 透传。菜单栏那行多列 / 多字号 / 带颜色的 `NSAttributedString` 标题没有任何
//! GUI 框架能画，所以这块无条件留在 AppKit；渲染主体仍是
//! [`crate::tray_render::set_tray_attributed_title`]（逐字保留），本文件只负责**宿主**：
//! 建 status item、建原生 `NSMenu`、派发点击。
//!
//! **零 tauri 依赖**：主线程跳转由调用方负责（`install` / `render` 断言 `MainThreadMarker`），
//! 4 个动作（左键点击 / 开关代理 / 显示主窗口 / 退出）由宿主经 [`MenuBarActions`] 注入。
//! 共存期宿主是 Tauri `app_setup`，换壳后只换注入方，本文件不动。
//!
//! **与 Tauri 不抢 status item**：macOS 上 `TrayIconBuilder` 整块不再构建
//! （`app_setup.rs` 里 `cfg(not(target_os = "macos"))`），菜单栏恒只有本模块这一个条目。

use crate::tray_render::{
    TrayLayout, set_tray_attributed_title, tray_layout, tray_quota_text, tray_separator,
};
use aidog_db::Db;
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, NSObject};
use objc2::{AnyThread, MainThreadOnly, define_class, msg_send, sel};
use objc2_app_kit::{
    NSApplication, NSEventMask, NSEventModifierFlags, NSEventType, NSImage, NSMenu, NSMenuItem,
    NSScreen, NSStatusBar, NSStatusItem,
};
use objc2_foundation::{MainThreadMarker, NSData, NSSize, NSString};
use std::cell::RefCell;
use std::rc::Rc;

/// 菜单栏图标（无数据列时展示）。与 tauri.conf.json 的 bundle icon 同一份文件。
const ICON_PNG: &[u8] = include_bytes!("../../../icons/32x32.png");
/// 菜单栏图标边长（pt）。系统菜单栏高约 22pt，留上下余量。
const ICON_SIDE: f64 = 18.0;
/// `NSVariableStatusItemLength`：宽度随内容自适应。
const VARIABLE_LENGTH: f64 = -1.0;

const TAG_TOGGLE_PROXY: isize = 1;
const TAG_SHOW: isize = 2;
const TAG_QUIT: isize = 3;

/// 宿主注入的 4 个动作，均在主线程调用。
pub struct MenuBarActions {
    /// 左键点击图标。参数 = 图标在屏幕上的矩形 `(left, top, width, height)`，
    /// logical 像素、**左上角原点**（已从 AppKit 的左下原点换算），与 Tauri 的
    /// `Position::Logical` 同坐标系，调用方可直接拿去定位浮窗。
    pub on_click: Box<dyn Fn(f64, f64, f64, f64)>,
    /// 菜单里的启停项。参数 = 点击时代理是否正在运行（true → 该停）。
    pub on_toggle_proxy: Box<dyn Fn(bool)>,
    pub on_show: Box<dyn Fn()>,
    pub on_quit: Box<dyn Fn()>,
}

/// 一次渲染所需的全部数据。DB 取数在 [`collect_state`]（任意线程 await），
/// 取完再跳主线程交给 [`render`]——AppKit 调用里不做 IO。
pub struct MenuBarState {
    pub layout: TrayLayout,
    pub separator: String,
    /// 菜单首行状态文字，如 `"● Proxy Running :9890"`。
    pub status_text: String,
    /// 菜单第二行的余额 / 配额概要；None = 不加该项。
    pub quota_text: Option<String>,
    pub proxy_running: bool,
}

// 全部状态只在主线程触碰，故用 thread_local 而非 static + unsafe Sync。
// 取值一律 clone 出来再用（`Retained` / `Rc` 都是引用计数），绝不把 borrow 跨进
// AppKit 回调——否则 performClick 同步弹菜单会重入 borrow 而 panic。
thread_local! {
    static ITEM: RefCell<Option<Retained<NSStatusItem>>> = const { RefCell::new(None) };
    static MENU: RefCell<Option<Retained<NSMenu>>> = const { RefCell::new(None) };
    static ACTIONS: RefCell<Option<Rc<MenuBarActions>>> = const { RefCell::new(None) };
    static TARGET: RefCell<Option<Retained<Target>>> = const { RefCell::new(None) };
}

fn item() -> Option<Retained<NSStatusItem>> {
    ITEM.with(|i| i.borrow().clone())
}

fn actions() -> Option<Rc<MenuBarActions>> {
    ACTIONS.with(|a| a.borrow().clone())
}

define_class!(
    // SAFETY: 超类 NSObject 无子类化要求；本类型不实现 Drop，无 ivars。
    #[unsafe(super(NSObject))]
    #[name = "AidogMenuBarTarget"]
    struct Target;

    impl Target {
        #[unsafe(method(statusItemClicked:))]
        fn status_item_clicked(&self, _sender: *mut AnyObject) {
            on_button_click();
        }

        #[unsafe(method(menuItemClicked:))]
        fn menu_item_clicked(&self, sender: *mut NSMenuItem) {
            // SAFETY: 该 selector 只挂在本模块自建的 NSMenuItem 上，sender 非空且类型正确。
            let tag = unsafe { sender.as_ref() }.map(|s| s.tag()).unwrap_or(0);
            on_menu_click(tag);
        }
    }
);

/// 唯一的 action target（单例，进程内长活）。
fn target() -> Retained<Target> {
    if let Some(t) = TARGET.with(|t| t.borrow().clone()) {
        return t;
    }
    let t: Retained<Target> = unsafe { msg_send![Target::alloc(), init] };
    TARGET.with(|slot| *slot.borrow_mut() = Some(t.clone()));
    t
}

/// 建（或取）唯一的 NSStatusItem。首次调用时向系统状态栏申请。
fn ensure_item(mtm: MainThreadMarker) -> Option<Retained<NSStatusItem>> {
    let _ = mtm;
    if let Some(it) = item() {
        return Some(it);
    }
    let it = NSStatusBar::systemStatusBar().statusItemWithLength(VARIABLE_LENGTH);
    ITEM.with(|slot| *slot.borrow_mut() = Some(it.clone()));
    Some(it)
}

fn icon() -> Option<Retained<NSImage>> {
    let data = NSData::with_bytes(ICON_PNG);
    let img = NSImage::initWithData(NSImage::alloc(), &data)?;
    img.setSize(NSSize::new(ICON_SIDE, ICON_SIDE));
    // template = 系统按明暗主题自动反色，与菜单栏其它图标一致。
    img.setTemplate(true);
    Some(img)
}

/// 安装菜单栏条目并接管点击。必须在主线程调用；重复调用只覆盖动作，不重复建条目。
pub fn install(actions: MenuBarActions) {
    let Some(mtm) = MainThreadMarker::new() else {
        tracing::error!("menubar::install off main thread, skipped");
        return;
    };
    ACTIONS.with(|slot| *slot.borrow_mut() = Some(Rc::new(actions)));
    let Some(button) = ensure_item(mtm).and_then(|it| it.button(mtm)) else {
        tracing::error!("menubar: status item has no button");
        return;
    };
    let tgt = target();
    // SAFETY: target 为本模块自建类，实现了 statusItemClicked:。
    unsafe {
        button.setTarget(Some(&tgt));
        button.setAction(Some(sel!(statusItemClicked:)));
    }
    // 左右键都走同一个 action，在 handler 里按 currentEvent 区分（右键弹菜单、左键开浮窗）。
    button.sendActionOn(NSEventMask::LeftMouseDown | NSEventMask::RightMouseDown);
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
pub(crate) fn status_text(running: bool, port: u16) -> String {
    if running {
        format!("● Proxy Running :{port}")
    } else {
        "○ Proxy Stopped".to_string()
    }
}

/// 兜底纯文本标题：富文本渲染失败时仍有可读内容（各列 "名 值"，间隙用 separator）。
pub(crate) fn fallback_title(layout: &TrayLayout, separator: &str) -> String {
    layout
        .columns
        .iter()
        .map(|c| format!("{} {}", c.name, c.value))
        .collect::<Vec<_>>()
        .join(separator)
}

/// 把 [`MenuBarState`] 画到菜单栏。必须在主线程调用（调用方负责跳转）。
pub fn render(state: MenuBarState) {
    let Some(mtm) = MainThreadMarker::new() else {
        tracing::error!("menubar::render off main thread, skipped");
        return;
    };
    let Some(button) = ensure_item(mtm).and_then(|it| it.button(mtm)) else {
        tracing::error!("menubar: status item has no button");
        return;
    };

    // 有数据列 → 隐藏 logo，只显富文本标题；无数据列 → 恢复 logo 并清标题。
    if state.layout.columns.is_empty() {
        button.setImage(icon().as_deref());
        button.setTitle(&NSString::from_str(""));
    } else {
        button.setImage(None);
        button.setTitle(&NSString::from_str(&fallback_title(
            &state.layout,
            &state.separator,
        )));
        if let Err(e) = set_tray_attributed_title(
            &button,
            state.layout.columns,
            state.layout.gaps,
            state.separator,
        ) {
            tracing::warn!("menubar attributed title failed, fallback to plain text: {e}");
        }
    }

    let menu = build_menu(
        mtm,
        &state.status_text,
        state.quota_text.as_deref(),
        state.proxy_running,
    );
    MENU.with(|slot| *slot.borrow_mut() = Some(menu));
    // 注意：**不**调 setMenu —— 一旦常挂菜单，左键点击就只弹菜单、拿不到 action，浮窗打不开。
    // 菜单只在右键时临时挂一下，见 show_menu。
}

fn build_menu(
    mtm: MainThreadMarker,
    status_text: &str,
    quota_text: Option<&str>,
    proxy_running: bool,
) -> Retained<NSMenu> {
    let menu = NSMenu::new(mtm);
    let tgt = target();
    let push = |title: &str, tag: Option<isize>| {
        // SAFETY: action selector 由本模块的 target 实现；keyEquivalent 空串合法。
        let item = unsafe {
            NSMenuItem::initWithTitle_action_keyEquivalent(
                NSMenuItem::alloc(mtm),
                &NSString::from_str(title),
                tag.map(|_| sel!(menuItemClicked:)),
                &NSString::from_str(""),
            )
        };
        match tag {
            Some(t) => {
                item.setTag(t);
                // SAFETY: tgt 实现了 menuItemClicked:。
                unsafe { item.setTarget(Some(&tgt)) };
            }
            // 纯信息行：置灰不可点（与原 Tauri 菜单的 .enabled(false) 一致）。
            None => item.setEnabled(false),
        }
        menu.addItem(&item);
    };

    push(status_text, None);
    if let Some(q) = quota_text {
        push(q, None);
    }
    menu.addItem(&NSMenuItem::separatorItem(mtm));
    push(
        if proxy_running {
            "Stop Proxy"
        } else {
            "Start Proxy"
        },
        Some(TAG_TOGGLE_PROXY),
    );
    menu.addItem(&NSMenuItem::separatorItem(mtm));
    push("Show Window", Some(TAG_SHOW));
    push("Quit", Some(TAG_QUIT));
    menu
}

/// 右键（或按住 Control 左键）→ 弹菜单；否则 → 交给 `on_click`。
fn on_button_click() {
    let Some(mtm) = MainThreadMarker::new() else {
        return;
    };
    if current_event_is_secondary(mtm) {
        show_menu(mtm);
        return;
    }
    let Some((x, y, w, h)) = button_screen_rect(mtm) else {
        return;
    };
    if let Some(a) = actions() {
        (a.on_click)(x, y, w, h);
    }
}

fn current_event_is_secondary(mtm: MainThreadMarker) -> bool {
    let Some(ev) = NSApplication::sharedApplication(mtm).currentEvent() else {
        return false;
    };
    ev.r#type() == NSEventType::RightMouseDown
        || ev.modifierFlags().contains(NSEventModifierFlags::Control)
}

/// 临时挂上菜单 → performClick 弹出 → 立刻摘掉，保证左键仍走 action。
/// AppKit 侧常规做法：NSStatusItem 挂了 menu 就不再发 action。
fn show_menu(mtm: MainThreadMarker) {
    let (Some(it), Some(menu)) = (item(), MENU.with(|m| m.borrow().clone())) else {
        return;
    };
    let Some(button) = it.button(mtm) else {
        return;
    };
    it.setMenu(Some(&menu));
    // SAFETY: performClick 在主线程同步弹菜单；此处未持有任何 thread_local borrow。
    unsafe { button.performClick(None) };
    it.setMenu(None);
}

/// 图标在屏幕上的矩形，换算成「左上角原点」的 logical 坐标。
/// AppKit 屏幕坐标是左下原点，用主屏高度翻转。
///
/// ponytail: 多显示器下取 `screens[0]`（带菜单栏那块）做翻转基准，副屏菜单栏上弹窗会偏；
/// 要精确改用 `button.window().screen()`，等真有人用副屏菜单栏再说。
fn button_screen_rect(mtm: MainThreadMarker) -> Option<(f64, f64, f64, f64)> {
    let button = item()?.button(mtm)?;
    let f = button.window()?.frame();
    let screen_h = NSScreen::screens(mtm).firstObject()?.frame().size.height;
    Some((
        f.origin.x,
        flip_y(screen_h, f.origin.y, f.size.height),
        f.size.width,
        f.size.height,
    ))
}

/// 左下原点 → 左上原点的纵坐标翻转。
pub(crate) fn flip_y(screen_height: f64, origin_y: f64, height: f64) -> f64 {
    screen_height - (origin_y + height)
}

fn on_menu_click(tag: isize) {
    let Some(a) = actions() else { return };
    match tag {
        TAG_TOGGLE_PROXY => (a.on_toggle_proxy)(aidog_ctx::ctx().proxy_handle().is_running()),
        TAG_SHOW => (a.on_show)(),
        TAG_QUIT => (a.on_quit)(),
        _ => {}
    }
}

#[cfg(test)]
#[path = "test_menubar.rs"]
mod test_menubar;
