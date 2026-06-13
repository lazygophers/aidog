# Research: 设置自定义机制

- **Query**: 设置页结构 / 现有 tray 配置数据结构 / 持久化 / 怎么加 "popover 展示项自定义"
- **Scope**: internal
- **Date**: 2026-06-13

## Findings

### 设置页结构

`src/pages/AppSettings.tsx` — tab 容器，5 个 tab（`AppSettings.tsx:10,177`）：
```ts
type Tab = "system" | "claude" | "codex" | "pricing" | "tray";
```
`tab === "tray"` 渲染 `<TrayConfigTab />`（`AppSettings.tsx:206-207`，import `:7`）。
（注：`Settings.tsx` 是**平台/分组**设置编排容器，与 tray 无关；tray 配置在 AppSettings 的独立 tab。）

`src/pages/TrayConfigTab.tsx`（742 行）= 现有 tray 自定义 UI，已是成熟的「展示项列表 + 拖拽排序 + 每项配置 popover + 实时预览」。

### 现有 tray 配置数据结构

后端 `src-tauri/src/gateway/models.rs`：

- **`TrayColor`**（`:570-584`）：`{ mode: "follow"|"preset"|"custom", value }`。
- **`TrayItem`**（`:591-625`）：
  ```rust
  item_type: "platform" | "today_usage" | "separator"  // :592,627
  platform_id: Option<u64>      // platform 项指定平台
  display: String               // platform: "balance"|"coding"；separator: 分隔符文本
  metric: Option<String>        // today_usage: "tokens"|"cache_rate"|"cost"|"requests"
  label: Option<String>         // 自定义标签
  decimals: Option<u32>
  color: TrayColor
  font_size: f64
  line_mode: "single" | "two"
  align / align_row2 / enabled / order
  ```
- **`TrayConfig`**（`:636-642`）：`{ separator: String, items: Vec<TrayItem> }`。

前端镜像类型 `src/services/api.ts:373-411`（`TrayColor` / `TrayItem` / `TrayConfig`，snake_case 与 serde 对齐，注释 `api.ts:366`）。

### 持久化（setting 表 scope/key/value）

`TrayConfig` 存 settings 表：**scope=`"tray"`, key=`"config"`, value=JSON**。

- 写 `db::set_tray_config`（`db.rs:476-484`）→ `set_setting(SetSettingInput { scope:"tray", key:"config", value })`。
- 读 `db::get_tray_config`（`db.rs:421-446`）→ `get_setting("tray","config")`，含旧配置迁移容错。
- 通用 `set_setting`（`db.rs:955`）/ `get_setting`（`db.rs:931`），settings 表按 (scope,key) 存 JSON value。

Command：`tray_config_get`（`lib.rs:254`）/ `tray_config_set`（`lib.rs:262`，set 后 `refresh_tray_menu`）。前端 `trayConfigApi`（`api.ts:420-427`）。

### TrayConfigTab 现有 UI 能力（可复用的范式）

`src/pages/TrayConfigTab.tsx`：
- 实时预览模拟 menubar（`:294-398`），行数预算 2/2 提示（`:298-304`）。
- **拖拽排序**：`SortableList` + dnd-kit（`:325,359`，`reorderItems`/`reorderColumns` `:233-247`）。
- **每项配置 popover**：点预览列弹出设置面板（`:400+`，本文件 426 行后未读全，含 label/color/font_size/align/display/metric 编辑）。
- **添加项菜单**：`addPlatform`（`:185`）/ `addTodayUsage`（`:191`）/ separator；`availablePlatforms` 排除已用平台（`:260-261`）。
- 每次改动 `persist()`（`:167-170`）即时 `trayConfigApi.set` 写库。
- `makePlatformItem`/`makeTodayUsageItem`/`makeSeparatorItem`（`:56-78`）= item 工厂，含默认值。
- today_usage metric 选项 `TODAY_METRICS`（`:45-50`）：tokens/cache_rate/cost/requests。

### 怎么加 "popover 展示项自定义"

两条路线（决策点，见 04）：

**路线 A — 复用 TrayConfig，加 popover 标志位**：在 `TrayItem` 加字段如 `show_in_popover: bool`（serde default true），让同一套 items 同时驱动 menubar tray 与 popover。改动最小，但语义耦合（tray 列 vs popover 项是两个展示面，列约束 2 行不适用 popover）。

**路线 B — 新增独立 popover 配置**：新 scope/key，如 `set_setting(scope="popover", key="config", value)`，独立 `PopoverConfig { items: Vec<PopoverItem> }`。`PopoverItem` 可复用 `today_usage`/`platform`/「all_platforms_today」等类型 + enabled/order。设置 UI 在 tray tab 加子区或新 tab。语义清晰，但需新 command + 新前端 UI（可复用 TrayConfigTab 的 SortableList/工厂范式）。

持久化机制 (settings scope/key/value) 对两路线都现成可用，无 schema 改动。

## Caveats / Not Found

- TrayConfigTab.tsx 仅读到 425/742 行；426 行后是每项配置 popover 的具体编辑控件（label/color/decimals/align 等），结构已从前 425 行的 `computeItemText`/工厂函数/类型推断清楚，不影响结论。
- i18n：tray 相关 key 走 `t("tray.*", "默认中文")`（`TrayConfigTab.tsx` 多处），新增 popover 配置项需补 7 语言 key。
- 「自定义粒度」（仅勾选显隐 vs 可排序可加自定义项）= 需 main 向用户确认的最大不确定点，见 04。
