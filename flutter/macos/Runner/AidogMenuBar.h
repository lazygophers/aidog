// 票 I20：菜单栏宿主的 C ABI 声明（Swift 的 bridging header）。
//
// 实现在 Rust：`src-tauri/crates/aidog_menubar_ffi/src/lib.rs`，编成 staticlib 由
// Xcode 的「Build aidog-menubar staticlib」构建阶段产出、经 AppInfo.xcconfig 里的
// `-laidog_menubar_ffi` 链进 Runner。
//
// 为什么菜单栏要走 Rust：那行多列 / 多字号 / 带颜色的 `NSAttributedString` 标题由
// `aidog_core::tray_render` 排版，340 行逻辑只有一份，抄到 Swift 里就会漂移。
//
// 两条硬约束：
// 1. 两个函数都**必须在主线程调用**（AppKit 要求，Rust 侧会复核，不满足就静默跳过）。
// 2. 回调是 `@convention(c)` 函数指针，不能带捕获 —— Swift 侧传全局函数。

#ifndef AIDOG_MENUBAR_H
#define AIDOG_MENUBAR_H

#include <stdbool.h>

/// 宿主注入的 4 个动作。字段顺序与 Rust 侧 `AidogMenuBarActions` 一一对应。
typedef struct {
  /// 左键点击图标。参数 = 图标在屏幕上的矩形 (left, top, width, height)，
  /// logical 像素、**左上角原点**（Rust 侧已从 AppKit 的左下原点换算好）。
  void (*on_click)(double x, double y, double w, double h);
  /// 菜单里的启停项。参数 = 点击时代理是否正在运行（true → 该停）。
  void (*on_toggle_proxy)(bool running);
  void (*on_show)(void);
  void (*on_quit)(void);
} AidogMenuBarActions;

/// 建 NSStatusItem 并接管点击。重复调用只覆盖动作，不重复建条目。
void aidog_menubar_install(AidogMenuBarActions actions);

/// 画一帧。`state_json` = 内核 `menu_bar_state` 命令返回的 JSON（UTF-8，NUL 结尾）。
/// JSON 不合法时保持上一帧不动。
void aidog_menubar_render(const char *state_json);

#endif /* AIDOG_MENUBAR_H */
