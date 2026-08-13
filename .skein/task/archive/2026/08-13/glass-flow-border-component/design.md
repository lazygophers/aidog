# 流光描边 opt-in 化 — 详细设计

## 背景：原立项理由已被证伪

登记时理由是「`@property --flow-ang` 逐帧动画覆盖全仓 `.glass`/`.glass-surface`，是全局 CPU 底噪」。
`frontend-compositing-purge` 的 s2-flow-border 实测推翻：`globals.css` 里 `animation: flowBorder`
**只出现在 `.glass:hover::after` / `.glass-surface:hover::after` 规则内**，空闲态 `opacity: 0` 且无
animation 声明 —— 零 tick；同一时刻至多一个元素 hover。原「两层 DOM 替代 @property」方案作废。

用户拍板保留本 task，**改目标为纯视觉重构**：不以性能为由。

## 现状

```css
/* globals.css（流光段） */
@property --flow-ang { syntax: "<angle>"; inherits: false; initial-value: 0deg; }
@keyframes flowBorder { to { --flow-ang: 360deg; } }

.glass, .glass-surface { position: relative; }

.glass::after, .glass-surface::after {
  content: ""; position: absolute; inset: 0; border-radius: inherit; padding: 1px;
  background: conic-gradient(from var(--flow-ang), ...金色...);
  opacity: 0; transition: opacity 250ms ease; pointer-events: none;
}

.glass:hover::after, .glass-surface:hover::after {
  mask: ... ; mask-composite: exclude;        /* 已收在 hover 内（s2 的既有优化） */
  opacity: 0.9;
  animation: flowBorder 3s linear infinite;
}
```

调用面：`glass-surface` 122 处 + `glass` 125 处 = **247 处**，即 247 个常驻 `::after` 伪元素。

## 方案（当前方案 = 精简守现状）

### 改动一：选择器换名，规则体逐字照搬

`::after` 两组规则的选择器 `.glass, .glass-surface` → 单一 opt-in 类 **`.flow-border`**，
**规则体一行不动**（conic 色值 / 3s 周期 / hover 触发时机 / mask 收在 `:hover` 内的既有优化全保留）：

```css
.flow-border { position: relative; }   /* ::after 定位所需 */
.flow-border::after { /* 原 .glass::after 规则体 */ }
.flow-border:hover::after { /* 原 .glass:hover::after 规则体 */ }
```

`@property --flow-ang` 与 `@keyframes flowBorder` **保留**（本身不产生 tick，仍被 `.flow-border` 用）。

🔴 **`.glass` / `.glass-surface` 原有的 `position: relative` 不能整条删** —— 该属性被其他布局逻辑
依赖（绝对定位子元素、层叠上下文）。只摘 `::after` 相关规则，`position: relative` 留在原处。
这是本 task 最容易踩的回归点。

### 改动二：opt-in 点位（用户拍板「仅顶层主卡片」）

已 grep 定位完毕，**共 6 处**（grill 后由 7 收窄为 6，见下）：

| # | 位置 | 覆盖范围 |
|---|---|---|
| 1 | `src/components/shared/CompactCard.tsx:67` (`className={\`glass-surface hover-lift...\`}`) | **一处覆盖两页全部主卡片** —— PlatformCard（`PlatformCard.tsx:161` 用 CompactCard）与 GroupListItem（`Groups/GroupListItem.tsx:337` 用 CompactCard）都以它为根 |
| 2-4 | `src/pages/Home.tsx:181` / `:202` / `:262` | 状态栏卡 / KPI 卡 / 趋势卡 |
| 5-6 | `src/pages/Home.tsx:526` / `:557` | 分组平台速览卡 / 今日平台用量 Top5 卡 |

**明确排除**（虽 grep 命中但不属「顶层主卡片」）：
- `src/pages/Home.tsx:581` —— 快捷操作卡：**按钮面板，非独立内容卡**（grill 时用户拍板剔除，契约 7）
- `PlatformCard.tsx:943` —— 卡片**内部**小容器（`marginTop: 4, padding: "6px 8px"`）
- `GroupTestPanel.tsx:58` —— 浮层面板
- `components/settings/**` 全部表单区块、`ImportExport/**` 面板、所有 Dialog / Modal

### 改动三：reduced-motion 块同步改名（grill 挖出，契约 6）

`globals.css:949-950` 的 `@media (prefers-reduced-motion)` 块内列了
`.glass:hover::after` / `.glass-surface:hover::after` 设 `animation: none`。
选择器改名后这两行必须同步为 `.flow-border:hover::after`，**漏改 = reduced-motion
用户流光不停，无障碍回归**。

⚠️ `globals.css:842-846` 的 `.glass, .glass-surface { position: relative }` 是**独立规则块**
（只有这一条属性），本次**整块保留不动** —— 它不是 `::after` 规则的一部分。

判据一句话：**是不是「页面上一块独立的、用户会整块看待的内容卡」**。是 → 加；
是它的内部结构、是浮层、是表单块 → 不加。

247 处 → **7 处**。

## 为什么不选别的

| 备选 | 否决理由 |
|---|---|
| 保持全局挂载不动 | 用户已拍板 opt-in 化；247 个常驻 `::after` 本身是无谓 DOM |
| 彻底删流光（含 @property / @keyframes） | 用户未选此项；金色流光是产品既有视觉签名 |
| 包成 React 组件（原「重构为独立组件」标题字面） | 纯 CSS 换选择器就够，加组件层是无谓抽象（YAGNI）。task 标题的「组件」字面不构成必须建组件的约束 |
| `.glass:not(.no-flow)` 反向 opt-out | 247 处里只有 7 处要留，反向标注要改 240 处，方向反了 |

## 数据流（验证链路）

```
改 globals.css 选择器 → 7 个点位加 .flow-border 类
  → yarn build / yarn test / check-i18n 全绿
  → 视觉核对：7 处 hover 流光与改前逐帧一致
  → 视觉核对：设置页 / 弹窗 hover 除「无流光」外无其他变化（重点看 position:relative 是否还在）
```

## 可能性分支（不进当前方案，仅留痕）

- **流光做成用户可关的设置项** — 触发条件：用户后续要求「能关掉流光」。opt-in 类已天然支持
  不加即无，做运行时开关需 CSS 变量 + 设置项 + i18n，YAGNI。
- **流光色值跟随主题** — 触发条件：某主题下金色与配色冲突。当前无冲突报告，不做。
