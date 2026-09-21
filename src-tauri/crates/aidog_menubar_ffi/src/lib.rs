//! 菜单栏宿主的 C ABI 外壳（票 I20）。Swift 侧声明见
//! `flutter/macos/Runner/AidogMenuBar.h`，用法见 `flutter/macos/Runner/MenuBar.swift`。
//!
//! 只做三件事：转 C 函数指针 → Rust 闭包、转 JSON → [`MenuBarState`]、把线程约束讲清楚。
//! 菜单栏的排版本体一行都不在这里（在 `aidog_core::tray_render`，单一实现）。
//!
//! **全部函数都必须在 macOS 主线程调用**。底下的 `install` / `render` 自己拿
//! `MainThreadMarker` 复核，拿不到就只记一条 error 然后什么也不做 —— 不 panic，
//! 因为 panic 跨 FFI 边界是 UB。
//!
//! 非 macOS 上整个 crate 编出一个空壳（`aidog_core::menubar` 那时不存在）；
//! 它只被 macOS 的 Xcode 构建阶段链，别的平台压根不会走到。

#![cfg(target_os = "macos")]

use aidog_core::menubar::{MenuBarActions, MenuBarState, install, render};
use std::ffi::{CStr, c_char};

/// 宿主注入的 4 个动作。全是**无捕获**的 C 函数指针 —— Swift 侧传的是全局函数，
/// 不是闭包（带捕获的 Swift 闭包转不成 `@convention(c)`）。
///
/// 字段顺序 = `AidogMenuBar.h` 里结构体的字段顺序，改一个就要改两个。
#[repr(C)]
pub struct AidogMenuBarActions {
    /// 左键点击图标。参数 = 图标在屏幕上的矩形 `(left, top, width, height)`，
    /// logical 像素、**左上角原点**（`menubar.rs` 已从 AppKit 的左下原点换算好）。
    pub on_click: extern "C" fn(f64, f64, f64, f64),
    /// 菜单里的启停项。参数 = 点击时代理是否正在运行（true → 该停）。
    pub on_toggle_proxy: extern "C" fn(bool),
    pub on_show: extern "C" fn(),
    pub on_quit: extern "C" fn(),
}

/// 安装菜单栏条目并接管点击。重复调用只覆盖动作，不重复建条目。
///
/// # Safety
/// 必须在主线程调用。四个函数指针必须在进程余生内一直有效（Swift 侧的全局函数天然满足）。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn aidog_menubar_install(actions: AidogMenuBarActions) {
    let AidogMenuBarActions {
        on_click,
        on_toggle_proxy,
        on_show,
        on_quit,
    } = actions;
    install(MenuBarActions {
        on_click: Box::new(move |x, y, w, h| on_click(x, y, w, h)),
        on_toggle_proxy: Box::new(move |running| on_toggle_proxy(running)),
        on_show: Box::new(move || on_show()),
        on_quit: Box::new(move || on_quit()),
    });
}

/// 把一帧 [`MenuBarState`]（JSON，来自内核的 `menu_bar_state` 命令）画到菜单栏。
///
/// JSON 解不开 = 上游给错了形状，记一条 warn 就返回：菜单栏保持上一帧，
/// 比画成空白强。这是**边界**上的校验，不是内部兜底。
///
/// # Safety
/// 必须在主线程调用。`state_json` 必须是非空、NUL 结尾、在本次调用期间有效的 C 字符串。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn aidog_menubar_render(state_json: *const c_char) {
    if state_json.is_null() {
        tracing::warn!("aidog_menubar_render: null json");
        return;
    }
    // SAFETY: 调用方保证非空 + NUL 结尾 + 调用期间有效（见函数文档）。
    let raw = unsafe { CStr::from_ptr(state_json) };
    let text = match raw.to_str() {
        Ok(s) => s,
        Err(e) => {
            tracing::warn!(error = %e, "aidog_menubar_render: json not utf-8");
            return;
        }
    };
    match serde_json::from_str::<MenuBarState>(text) {
        Ok(state) => render(state),
        Err(e) => tracing::warn!(error = %e, "aidog_menubar_render: bad json, frame skipped"),
    }
}
