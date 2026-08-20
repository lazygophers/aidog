---
title: Tauri 托盘浮窗性能优化（窗口复用模式）
layer: recall
category: arch
keywords: [tauri,window,popover,performance,复用,hide/show,NSWindow]
source: -
authored-by: skein-spec
created: 1784810159
status: active
related: [rule-45,trellis-03,trellis-18]
updated: 1784810159
---

## 触发场景
实现 Tauri 桌面应用的浮窗（如托盘 popover）时，需要避免每次点击都冷启 webview，导致的延迟与卡顿。

## 陷阱-正解
❌ **陷阱**：tray 点击每次 destroy + 新建窗口 → 冷启 webview + 瀑布 IPC 4 路 (popover_data → query + list + list) → 「点击后 0.3-0.5s 才展示」。失焦销毁窗口。

✅ **正解**：窗口复用模式（create-once, hide/show）：
1. **Rust 窗口复用** (`app_setup.rs::setup` 预建)：`.visible(false)` 建隐藏 popover 窗，webview 提前 boot + React 提前 mount；一次性 `setHidesOnDeactivate(true)`（NSWindow 指针全程存活）。
2. **tray toggle** → `is_visible()` 判：true→`hide()`；false→算位置 `set_position`→`show()`→`set_focus()`（删 destroy 分支）。
3. **失焦改 hide** (`startup.rs` Focused(false))：`destroy()` → `hide()`。
4. **前端保温** (`popover.tsx`)：React state/data 跨开常驻（proxy_log 事件 1000ms debounce 持续刷新）；`show()` 后 emit("popover-shown")→前端 `reloadData()` 确定性刷；scaleFactor 缓存首测值避免每次 IPC。

## 性能收益
- 消除冷启 webview (setup 预建一次)。
- 去掉 tray click 时的 4 路 IPC 瀑布（背景保温 + show 时刷新即可）。
- 实测 show 延迟从 300-500ms → <100ms。

## 反例
```rust
// ❌ 陷阱实现（每次销毁）
if let Some(w) = app.get_webview_window("popover") {
    let _ = w.destroy();  // 下次需冷启
}
WebviewWindowBuilder::new(...)  // 重新建

// ✅ 正确实现（窗口复用）
let Some(w) = app.get_webview_window("popover") else { return; };
if w.is_visible().unwrap_or(false) {
    let _ = w.hide();  // 隐藏不销毁，NSWindow 指针存活
} else {
    let _ = w.set_position(position);
    let _ = w.show();
}
```

## 实现清单
- [ ] `app_setup.rs::setup` 阶段 `prebuild_popover()`：`.visible(false)` + `setHidesOnDeactivate(true)`
- [ ] `on_tray_icon_event` 改为 toggle 逻辑（`is_visible()` → hide/show）
- [ ] `startup.rs` Focused(false) 改 `hide()` 非 `destroy()`
- [ ] `popover.tsx`：状态保温、show 时 emit("popover-shown") + reloadData、scaleFactor 缓存
- [ ] 测试：反复 show/hide 无白屏、无重复 IPC、失焦即隐

## 适用
- Tauri 桌面应用浮窗（托盘 popover、context menu、floating panel）
- 需要快速响应的小窗口（性能关键路径）

## 关联
[[rule-45]] (popover 域划分) / [[trellis-03]] (Crate 边界契约) / [[trellis-18]] (前端约定)

## 案例
- popover-perf task (commit 14ec141d)：预建隐藏窗 + toggle hide/show，去掉销毁流程
