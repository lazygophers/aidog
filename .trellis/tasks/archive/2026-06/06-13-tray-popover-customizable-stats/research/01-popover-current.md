# Research: 现有 popover 实现

- **Query**: tray 左击 popover 浮窗当前实现：展示什么、数据从哪来、窗口怎么创建/定位
- **Scope**: internal
- **Date**: 2026-06-13

## Findings

### Files Found

| File Path | Description |
|---|---|
| `src/popover.tsx` | popover 浮窗 React 组件（独立入口） |
| `popover.html` | popover webview HTML 入口（透明全屏 root） |
| `src/styles/popover.css` | popover 样式 |
| `vite.config.ts:33-37` | rollup 多入口：`main: index.html` + `popover: popover.html` |
| `src-tauri/src/lib.rs:301-320` | `popover_data` Tauri command（数据源） |
| `src-tauri/src/lib.rs:2295-2343` | tray 左击事件 → 创建/销毁 popover 窗口 |

### 当前 popover 展示什么

`src/popover.tsx:93-144` 渲染三块：

1. **Header — 代理状态** (`:99-104`)：状态点 + `Running :{port}` / `Stopped`。
2. **Platform entries** (`:107-119`)：遍历 `data.entries`（来自 tray config 的列），每条显示 `dot + name + value`。即「当前 tray 配置的展示项」原样搬到 popover，不是「所有平台当日使用」。
3. **Today stats 四格** (`:122-142`)：固定四项硬编码
   - `formatNumber(today_stats.tokens)` — tokens
   - `formatCostUsd(today_stats.cost)` — cost
   - `formatPercent(today_stats.cache_rate, 0)` — cache
   - `formatNumber(today_stats.total_requests)` — reqs

数据结构 `PopoverData`（`src/popover.tsx:24-29`）：
```ts
interface PopoverData {
  entries: PopoverEntry[];        // { name, value, color } —— 来自 tray 列
  today_stats: TodayStats;        // { tokens, cache_rate, cost, total_requests }
  proxy_running: boolean;
  proxy_port: number;
}
```

### 数据从哪个 command 来

唯一数据源 `popover_data`（`src-tauri/src/lib.rs:301-320`）：
```rust
async fn popover_data(db, app) -> Result<PopoverData, String> {
    let layout = tray_layout(&app).await;                 // tray 配置列
    let entries = layout.columns.map(|c| PopoverEntry { name, value, color });
    let today_stats = db::today_stats(&db).await?;        // 今日聚合（见 02）
    let proxy_running = ...;                               // ProxyHandle 锁
    let settings = load_proxy_settings(&app)...;           // port
    Ok(PopoverData { entries, today_stats, proxy_running, proxy_port })
}
```
关键：`entries` 不是独立查询，而是复用 `tray_layout(&app)`（`lib.rs:1787`）——即 **menubar tray 当前显示的那几列**。所以 popover 的平台条目 = tray 配置项，并非「所有当日已用平台」。

前端调用：`src/popover.tsx:75` `invoke<PopoverData>("popover_data")`，组件挂载时拉一次，无轮询。

### 窗口怎么创建/定位

tray 左击处理 `src-tauri/src/lib.rs:2295-2343`：

- `.show_menu_on_left_click(false)`（`:2294`）—— 左击不弹原生菜单，改走自定义 popover。右键菜单走 `on_menu_event`（`:2344`）。
- 只响应 `MouseButton::Left` + `MouseButtonState::Down`（`:2299`），Up 忽略（注释：否则 Down 创建 → Up 立刻销毁）。
- **Toggle**：已有 `popover` 窗口则 `w.destroy()` 返回（`:2303-2306`）。
- **定位**（`:2307-2323`）：从 tray `rect` 取图标位置，居中于图标正下方。`pw=300, ph=420`，`x = rx + rw/2 - pw/2`，`y = ry + rh`。
- **创建**（`:2325-2341`）：`WebviewWindowBuilder::new(app, "popover", WebviewUrl::App("popover.html"))`，属性 `inner_size(300,420)` / `decorations(false)` / `transparent(true)` / `always_on_top(true)` / `skip_taskbar(true)` / `focused(true)`。
- **失焦关闭**：在前端 `src/popover.tsx:81-87`，`onFocusChanged` 监听到失焦即 `current.destroy()`。

### 未提交 WIP（主工作区 `git diff src-tauri/src/lib.rs`）

WIP 仅改 tray 左击块（`lib.rs:2293-2343`），3 处：
1. 加 `MouseButtonState`，从「只判 Left」改为「Left + Down」toggle（避免 Down 建 Up 销）。
2. **定位修复 scale factor**：rect 是 Physical 像素，`position()` 收 Logical，故 `p.x / scale`（scale 取 main 窗口 `scale_factor()`，回退 2.0）。
3. 加 `ph=420.0` 变量 + tracing 日志（`tray click → toggle popover` / `popover position` / `popover window created successfully`）。

**WIP 不涉及 popover 展示内容/数据结构，纯窗口定位与 toggle 修复。** 本需求扩展 popover.tsx 渲染 + popover_data 数据 + 设置 UI，与 WIP 几乎不重叠；唯一交集是同一 `on_tray_icon_event` 闭包所在文件，注意改 popover_data/PopoverData 时不要回退 WIP 的定位代码。

## Caveats / Not Found

- popover 无主题响应轮询，theme 从 `localStorage("aidog-settings")` 读一次（`src/popover.tsx:38-48`）。
- popover 窗口 id 固定 `"popover"`，单例。
- WIP 是未提交本地改动，另一 task（group-multi-platform-retry-failover，见 `.trellis/worktrees/`）也在改 lib.rs，但本主工作区 diff 只含 popover 定位 WIP。
