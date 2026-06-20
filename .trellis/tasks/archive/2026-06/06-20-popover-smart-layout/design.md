# Design · 浮窗智能布局

## 模块与执行层

| 模块 | 文件 | Subtask | 执行层 | 资源边界 |
| --- | --- | --- | --- | --- |
| 数据模型 | `models.rs` `db.rs` `api.ts` | S1 | sub-agent(worktree) | 独占三文件 |
| 渲染层 | `popover.tsx` `styles/popover.css` | S2 | sub-agent(worktree) | 独占两文件 |
| 配置 UI | `PopoverConfigTab.tsx` `SortableList.tsx` | S3 | sub-agent(worktree) | 独占两文件 |
| 实时预览 | `PopoverConfigTab.tsx` + 抽 `components/PopoverCards.tsx` | S4 | sub-agent(worktree) | 改 ConfigTab + 新建共享卡片组件 |

> 全部在**同一 worktree** `.worktrees/06-20-popover-smart-layout` 内串行执行，每个 subtask 基于前一个的 commit。不用 per-agent isolation(串行共享文件会冲突)。

## 数据契约 (S1 定，下游消费)

### Rust (models.rs)

```rust
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct RowMeta {
    #[serde(default = "default_cols")]
    pub cols: i32,            // 1 | 2 | 3
}
fn default_cols() -> i32 { 1 }

// PopoverItem 新增字段
#[serde(default = "default_row")]   pub row: i32,        // 默认沿用 order 行为见下
#[serde(default = "default_size")]  pub size: String,    // "s" | "m" | "l"
#[serde(default)]                   pub color: TrayColor, // 默认 follow

// PopoverConfig 新增
#[serde(default)] pub rows: Vec<RowMeta>,  // 按 row 索引; 缺省项视为 cols=1
```

> `TrayColor` 必须 `impl Default`(mode="follow")；若现无 Default，S1 补 `#[derive(Default)]` 或手写。
> 老配置无 `row` → serde default 给 0。**迁移策略**：`get_popover_config` 读出后若所有 item.row 相同(全 0=老配置)则按 `order` 重写 row(各占一行)，保证老用户外观不变。或前端渲染时 fallback：row 缺省按 order 当行号。**采纳前端 fallback 更简**：S2 渲染时 `effectiveRow = item.row ?? item.order`，rows[r].cols 缺省 1。S1 只加字段不做 DB 迁移脚本。

### TS (api.ts)

```ts
export interface RowMeta { cols: 1 | 2 | 3; }
export interface PopoverItem {
  // ...existing
  row?: number; size?: "s" | "m" | "l"; color?: TrayColor;
}
export interface PopoverConfig { items: PopoverItem[]; rows?: RowMeta[]; }
```

## 渲染算法 (S2)

```
1. visible = items.filter(visible)
2. 按 effectiveRow 分组(Map<row, items[]>)；effectiveRow = item.row ?? item.order
3. 行号升序遍历：每行 cols = config.rows?.[row]?.cols ?? 1
4. <div class="popover-grid-row" style="grid-template-columns:repeat(cols,1fr)">
     行内 items 按 order 升序 → renderItem(item, size)
5. renderItem 读 item.size 选密度变体；read item.color → resolveColor() 给数值上色
```

CSS：`.popover-grid-row{display:grid;gap:6px}` + size 变体类 `.pc-s/.pc-m/.pc-l`(font-size/padding/min-height 阶梯) + 密度：组件内按 size 条件渲染富信息块。

## 密度变体约定 (S3 标准, S2 实现)

| 卡片类 | s (核心) | m (现状) | l (富信息) |
| --- | --- | --- | --- |
| MetricRow(today_*) | 仅大数值 | 标签+值 | 标签+值+副标(如同比/单位说明) |
| PlatformToday/MetricCard | 平台名+金额 | +token | +token 拆分(in/out/cache) |
| CostTrendCard | 迷你 sparkline | 现曲线 | 曲线+坐标/汇总数值 |
| Group* | 数值 | 现状 | +趋势/明细 |

> l 卡片信息密度高，配 3 列时空间不足：S2 可对 `cols>=3 && size=="l"` 给紧凑回退或文档提示用户(不强制锁)。

## 拖拽方案 (S3)

- 复用 @dnd-kit，PointerSensor(不依赖 WKWebView 失效的 HTML5 DnD，记忆 wkwebview-html5-dnd-drop-fails)。
- 二维：每行一个 SortableContext(行内排序) + 跨行用多容器 DnD(onDragOver 检测目标行) **或** 单 flat SortableContext + rectSortingStrategy 后按落点反推 row/order。
- 优先尝试 rectSortingStrategy flat 方案(改动小)；若跨行落点判定不稳，退多容器。S3 实测选定，结论落 cortex。
- 新增「加一行 / 删空行」操作 → 调整 rows[] + items.row。

## 颜色编辑器 (S3)

复用 Tray 已有颜色编辑 UI 模式(TrayConfigTab 有 follow/preset/custom)：抽成可复用控件或就地实现。custom 模式提供 hex input(6 位校验，复用 resolveColor 解析规则)。

## 预览 (S4)

- 抽 `popover.tsx` 内卡片渲染为共享组件 `src/components/PopoverCards.tsx`(或 popover.tsx export renderItem)，配置页与真实浮窗共用 → 单一事实源，避免预览与实际不一致。
- 预览区用配置页**本地 draft state**(未保存的 items/rows) 渲染，套 `.popover-root` 同款容器，改配置即时重渲。
- 预览数据：复用 PopoverConfigTab 已轮询的 stats(usePolling 30s)，无需真实浮窗数据通道。

## 资源边界与串行理由

S1→S2→S3→S4 全串行：①共享 `PopoverItem` 模型(S1 定义，S2/S3/S4 消费)②S2 与 S4 都碰卡片渲染(S4 抽取 S2 的渲染)③S3 与 S4 同改 `PopoverConfigTab.tsx`。并发会写冲突 + 契约漂移。

## 回滚

- 单 worktree commit 链：任一 subtask 失败 `git reset` 回上一 subtask commit。
- 整体回滚：worktree 不合并即可，master 不受影响。
- DB 兼容：新字段被旧代码 serde 忽略，配置不会损坏。
