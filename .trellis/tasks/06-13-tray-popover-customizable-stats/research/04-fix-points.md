# Research: 改造点 + 风险

- **Query**: 实现新需求（popover 默认4项 + 各平台当日 + 设置自定义）的改造点、数据结构、风险
- **Scope**: internal
- **Date**: 2026-06-13

## Findings

### 改造点清单

| # | 层 | 文件:位置 | 改动 |
|---|---|---|---|
| 1 | 后端数据 | `db.rs`（新增，仿 `today_stats` `:524`） | 新增 `today_platform_stats()`：`WHERE created_at>=今日 GROUP BY platform_id`，返回各平台当日 tokens/cost/requests/cache。复用 platform_id=0 自动分组回溯（`db.rs:1310`）。**这是需求④唯一硬缺口**。 |
| 2 | 后端 command | `lib.rs:301 popover_data` + `PopoverData` `:294` | 扩展 `PopoverData` 加各平台当日列表 + 按 popover 配置裁剪展示项。默认①②③已在 `today_stats`，④走新查询。 |
| 3 | 后端配置 | 路线 A：`models.rs:591 TrayItem` 加 `show_in_popover`；路线 B：新 `PopoverConfig` + scope="popover" 持久化 | 见 03，决策点。 |
| 4 | 后端 command | `lib.rs`（新增，仿 `tray_config_get/set` `:254/:262`） | 路线 B 需 `popover_config_get/set`。 |
| 5 | 前端渲染 | `src/popover.tsx:122-142` | 当前硬编码 4 格 + entries；改为按配置驱动渲染，加「各平台当日」分区（只列已用）。 |
| 6 | 前端类型 | `src/services/api.ts`（`PopoverData` 镜像 + 新配置类型） | 与 serde 对齐。 |
| 7 | 设置 UI | `src/pages/TrayConfigTab.tsx` 或新组件 | 加 popover 展示项配置入口；可复用 SortableList/工厂/预览范式（`TrayConfigTab.tsx:325,56-78`）。 |
| 8 | i18n | 7 语言 | 新增 popover 配置项 key（`tray.*` / 新 `popover.*`）。 |

### popover 展示项配置数据结构（建议）

默认 4 项：①cost ②cache_rate ③tokens ④各平台当日（聚合块）。前三项可直接映射现有 `today_usage` metric（`models.rs:599` metric ∈ tokens/cache_rate/cost/requests）。第④项是新的「平台当日列表」块类型。

推荐独立 `PopoverConfig`（路线 B，语义清晰）：
```rust
struct PopoverItem { kind: "today_metric"|"platforms_today", metric: Option<String>, enabled: bool, order: i32, label: Option<String> }
struct PopoverConfig { items: Vec<PopoverItem> }  // scope="popover", key="config"
```
持久化复用 settings scope/key/value（`db.rs:955 set_setting`），零 schema 改动。

### 数据一致性

- popover 的全局今日①②③ 与主窗口/Logs 页一致：都走 `db::today_stats`（`db.rs:524`），同一 SQL，天然一致。
- 各平台当日④ 与平台页：平台页 `get_platform_usage_stats` 是**累计全时段**（`db.rs:1307`，无日期过滤），popover④是**当日**——**语义不同，数值会不一致是预期的**，UI 文案需标「今日」避免误解。
- popover 数据是挂载拉一次（`popover.tsx:75`，无轮询），窗口每次左击重建（`lib.rs:2303-2341`），所以每次打开都是新鲜数据，无陈旧问题。

### 与未提交 tray WIP 的衔接

主工作区 `git diff src-tauri/src/lib.rs`（+16 -6）只改 `on_tray_icon_event` 闭包（`lib.rs:2293-2343`）：scale-factor 定位修复 + Down-only toggle + 日志。**与本需求改的 `popover_data`/`PopoverData`/`today_platform_stats` 不重叠**。风险点：
- 本任务改 `popover_data`（`lib.rs:301`）与 WIP 块（`lib.rs:2295`）在同文件不同函数，git 不冲突。
- 若调整 popover 窗口尺寸（展示项变多→ ph 可能需变），会碰 WIP 的 `ph=420.0`（`lib.rs:2321`）——改前先确认 WIP 已提交或保留其 scale 修复，**别覆盖 `/ scale` 那几行**（`lib.rs:2313,2317`）。
- 另一 task（group-multi-platform-retry-failover，`.trellis/worktrees/`）也动 lib.rs，但在独立 worktree；本主工作区只读不改其内容，按任务说明只读即可。

### 风险 / 不确定

1. **平台名归属**：④需平台名，proxy_log 只有 platform_id；platform_id=0 自动分组日志需回溯（`db.rs:1310-1311`），否则归一团。平台名由前端 `platformApi.list()` 映射（同 `TrayConfigTab.tsx:217`）。
2. **窗口高度自适应**：展示项可配 → 内容高度变化，固定 `ph=420` 可能不够/留白，可能要动态算高或滚动。
3. **自定义粒度未定**（最大不确定点）：见下。

## Caveats / Not Found（需 main 向用户确认）

**最大不确定点 — popover 自定义粒度**，三档，需用户拍板：
- (A) 仅勾选显隐 4 个默认项（最简）；
- (B) 显隐 + 排序（复用 SortableList）；
- (C) 显隐 + 排序 + 可增删自定义项（如指定某平台、加 today_usage metric，完全对齐 tray config 能力）。

档位决定数据结构（A=4 个 bool 即可，C=需完整 `PopoverConfig` items 列表 + 设置 UI）与工作量。建议 main 用专门提问工具向用户确认后再定路线 A/B（03）。
