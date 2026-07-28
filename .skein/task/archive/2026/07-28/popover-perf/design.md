# 浮窗性能优化 — 详细设计

架构 / 数据流 / 关键取舍 / 技术选型 (不含调度图, 调度归 task.json):

## 根因 (勘察确认)
- `app_setup.rs:320-391` tray 左键 Down: 有窗→`w.destroy()`；无窗→`WebviewWindowBuilder::new` 全新建 popover 窗口(冷启 webview)。
- `startup.rs:33-38` Focused(false) → `window.destroy()`。
- `popover.tsx:70-98` mount 后 4 路 IPC: `popover_data` → `statsApi.queryBatch` + `groupApi.list` + `groupDetailApi.list`。冷启 webview + 瀑布 = "没及时展示"。
- 渲染层 (`PopoverCards.tsx` 638 行) 轻量，非瓶颈。

## 方案 (全量，用户选定)

### 1. Rust 窗口复用 (create-once, hide/show) — 核心
- **setup 阶段预建隐藏窗口** (`app_setup.rs::setup`): 用现有 `WebviewWindowBuilder` 参数(decorations false/transparent/always_on_top/skip_taskbar)建 label="popover"，但 `.visible(false)` 不 focused。建后一次性 `setHidesOnDeactivate(true)` (指针全程有效，不再每次建窗设)。webview 提前 boot，React 提前 mount。
- **tray toggle 改 show/hide** (`on_tray_icon_event`): `get_webview_window("popover")` 必存在 → `is_visible()`: true→`hide()`；false→按 tray rect 算位置 `set_position`(Logical) → `show()` → `set_focus()`。删掉 `destroy()` + rebuild 分支。
- **失焦改 hide** (`startup.rs` Focused(false)): `window.destroy()` → `window.hide()`。窗口 + NSWindow 指针存活可复用。
- hidesOnDeactivate 仍生效(失活自动隐藏)；hide 后窗口保留，下次 show 秒显。

### 2. 前端数据保温 (`popover.tsx`)
- 窗口不再销毁 → React state/`data`/`statsMap` 跨开常驻，proxy-log 事件(1000ms debounce)持续后台刷。再次 show 时 `data` 非 null → 直接渲染上次内容，无空白 loading。
- **show 时刷新**: Rust `show()` 后 `window.emit("popover-shown")`；前端订阅 → `reloadData()`。保证隐藏期间累积的变化在 show 后立即拉新(背景 debounce 之外的确定性刷新)。ponytail: 若背景刷已足够可省，但 show 刷新成本低、保新鲜，保留。

### 3. 精简 resize/重定位循环 (`popover.tsx:117-156`)
- 现每次 applySize 调 `outerPosition()` + `scaleFactor()` 多次 await。窗口复用后 `scaleFactor` 恒定 → 缓存首测值，避免每次 IPC。`centerXRef` 逻辑保留但复用后由 Rust show 时定位主导，前端仅做尺寸自适应 + x 微调。减少 setSize/setPosition/outerPosition 往返次数。

### 4. backdrop-filter 复核 (`popover.css`)
- `.popover-root` 现 `backdrop-filter: blur(glass-blur+10px) saturate(...)` 在透明置顶窗每帧合成。复核: 若为 jank 来源则降 blur radius 或去 +10px 增量；否则保留(60% 不透明度上一轮已定，视觉不回退)。以实际观感为准，不盲降。

## 不变量
- 仍居中于 tray 图标正下方；失活即隐；视觉(60% 不透明+卡片布局)不回退。
- macOS NSWindow 指针在 hide 生命周期下存活(不 destroy)。
- `cargo build`+`clippy` 零 warning；`yarn build` 过。

## 风险
- 预建隐藏窗口增启动开销(1 webview) — 可接受，换取点即显。
- hide 后 always_on_top/skip_taskbar 状态保持 — 复用无需重设。
- 首次 show 定位: setup 预建时无 tray rect，位置在首次 tray click 时 set_position 补齐。
