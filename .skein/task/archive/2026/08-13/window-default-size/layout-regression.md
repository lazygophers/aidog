# s4 — 默认窗口 1026×759 布局回归审计（静态审计）

## 口径声明

本次为**静态代码审计**（读 `src/` 源码 + grep，无桌面 GUI，未做像素级实渲染验证）。判定基于：
- 硬编码尺寸（`width`/`minWidth`/CSS px 值）是否可能超出窗口 1026×759 下的真实可用区域
- 多列布局（grid/flex 并排固定宽元素）总宽是否可能超出可用区域
- `overflow-x` 策略（`hidden` 静默截断 vs `auto`/`scroll` 可视化）
- Settings 锚点导航（AnchorNav）与内容区布局关系
- Modal 是否遵守 `createPortal(document.body)` 规则（防 liquid glass 祖先 `transform` 使 `position:fixed` 退化）

真实实渲染（不同 DPI / 字体度量 / OS 差异导致的像素级换行、组件库内部测量）需要人工用 `yarn tauri dev` 在 1026×759 窗口下过一遍，本次未覆盖。

## 可用区域计算

- 窗口：1026 × 759
- 外层 `App.tsx:173-178`：`padding: 12` + `gap: 12`
- 侧栏 `Sidebar.tsx:249-250`：`width: 200, minWidth: 200`（固定宽）
- `main` 区域 `App.tsx:188-194`：`padding: "24px 32px"`
- **内容区可用宽度** ≈ 1026 − 12×2(外padding) − 12(gap) − 200(侧栏) − 32×2(main padding) ≈ **726px**
- **内容区可用高度** ≈ 759 − 12×2(外padding) − 24×2(main padding) − titlebar(未知，macOS 原生标题栏另占若干 px，未纳入) ≈ **~687px**（保守估计，实际更小）

727px / 687px 是判定「是否可能截断」的基准阈值。

## 18 页判定表

| 页面 | 判定 | 证据 file:line | 说明 |
|---|---|---|---|
| Home | 通过 | `src/pages/Home.tsx:207,523` 用 `grid-template-columns: repeat(auto-fit, minmax(120px/300px, 1fr))`；`:81,414` 的 `minWidth:110/140` 是卡片内子项软下限，非页面级硬宽 | 响应式网格，窄窗口自动折行，无固定总宽超限 |
| AppSettings | 通过 | `src/pages/AppSettings.tsx` 本身无硬编码 px 宽度（grep 全 0 命中） | 编排容器，实际布局由子组件（见下方 settings/* 系列）决定 |
| CodexSettings | 通过 | `src/pages/CodexSettings.tsx` 无硬编码 px 宽度命中 | 同上，走通用设置组件 |
| Groups | 通过 | `src/pages/Groups.tsx` 页面级无固定宽；子组件 `GroupEditPanel.tsx:171,190` 的 `width:140` 为行内单个 Input/Select，同行有 `flex:1` 填充项分摊剩余空间 | 需留意 `GroupListItem.tsx:513,520,532,541`（编辑态行：`minWidth:100`×2 + `width:140` + `minWidth:100`≈480px），726px 内安全 |
| Logs | 通过 | `src/pages/Logs.tsx` 无固定宽；`Logs/DetailPanel.tsx:165` 用 `repeat(auto-fill, minmax(160px,1fr))` 响应式；`Logs/ListView.tsx:161` 单个筛选框 `maxWidth:180` | 详情面板字段网格自动折行 |
| Mcp | 通过 | `src/pages/Mcp.tsx` 无固定宽；`Mcp/McpModals.tsx` 走 `AlertDialog`（Radix Portal） | — |
| ModelTestPanel | 通过 | `src/pages/ModelTestPanel.tsx:140-143` 本身是 `Dialog`（Radix Portal, 见下方 modal 项），`width:560, maxWidth:560` 远小于窗口宽度 1026，且 Portal 挂 `document.body` 不受内容区宽度约束 | 名为「Panel」实为 modal |
| Notifications | 通过 | `src/pages/Notifications.tsx` 无固定宽命中；`components/settings/NotificationEventList.tsx:211` 的 `minWidth:150` 是单个 `<code>` 标签 | — |
| Platforms | 通过 | `src/pages/Platforms.tsx` 无固定宽；子表单 `platforms/formSections.tsx` `formSectionsEndpoints.tsx` 行内固定宽 Select(110~140) 均配 `flex:1` Input 填充剩余、`flexShrink:0` 防止被压碎，行总宽远低于 726 | endpoint 行示例：`formSectionsEndpoints.tsx:79,107`（120+140=260，+flex:1 URL 输入框自适应） |
| PopoverConfigTab | 通过 | `PopoverConfigTab/ScopeConfig.tsx:39,83,102,121`（4 个 Select，各 100~120，`width:"auto"`）；`PopoverConfigTab/PopoverLayout.tsx:94` `minWidth:160` 是弹出菜单非页面主体；`PopoverCards.tsx:631` 用 `repeat(${cols}, minmax(0,1fr))` 响应式 | Popover 本身是独立浮层，非嵌在 726px 内容区内 |
| PricingTab | 通过 | `src/pages/PricingTab.tsx:296` 价格表外层 `overflow:"auto"`（自滚动，非静默截断）；`:269` 单个 Select `width:110` | 表格列多时靠自身滚动条，不会撑破/截断整页 |
| Settings | 通过（含专项检查见下） | `src/pages/Settings.tsx` 无固定宽；容器编排走 `AnchorNav` + 内容区（详见「Settings 锚点导航」专项） | — |
| SkillDetailView | 通过 | `src/pages/SkillDetailView.tsx:190` 单处 `width:220`（详情侧栏内一个信息块），未见并排多固定宽列 | — |
| SkillInstallView | 通过 | `src/pages/SkillInstallView.tsx` 无固定宽命中 | — |
| Skills | 通过 | `src/pages/Skills.tsx` 无固定宽命中；卡片列表未见 grep 出 grid 硬列数 | 未见风险模式，视为通过 |
| Stats | 通过 | `src/pages/Stats.tsx:469` 图表 `<svg viewBox>` + `width:"100%"`（响应式）；`:591` 表格单元格 `maxWidth:200` + `overflow:hidden`+`textOverflow:ellipsis`（有意截断单列文本，非静默截全容器）；`:313,698` 单个 Select/子项固定宽 | ellipsis 截断是标准 UX 模式，不属于「容器级静默截断」缺陷 |
| TrayConfigTab | 通过 | `src/pages/TrayConfigTab.tsx:462,750` 的 `minWidth:220/280` 均为 `PopoverContent`（独立浮层菜单），非页面主体列宽 | — |

## 四项专项检查

### 1. 内容截断（overflow-x: hidden 静默截断）

```
grep -rn "overflowX\|overflow-x" src/pages src/components
```
命中：`ModelsMatrixSection.tsx:326`、`About.tsx:464`、`MultiKeyPreview.tsx:58`、`SectionAnchorNav.tsx:33` 均为 `overflowX: "auto"`（可视滚动条，非静默截断）。仅 `components/ui/select.tsx:82`、`dropdown-menu.tsx:67` 用 `overflow-x-hidden`，但这是 Radix Select/DropdownMenu **内部下拉菜单**的横向裁切（配合 `overflow-y-auto` 纵向滚动是设计意图，非页面级容器）。

**结论：无内容截断风险** — 未发现页面级容器用 `overflow-x: hidden` 静默裁切内容。

### 2. 非预期横向滚动

顶层 `App.tsx` 的 `main` 用 `overflow: "auto"`（`App.tsx:190`），子页面未见强制 `min-width` 超过 726px 的容器（见上表逐页 grep）。唯一横向滚动是**有意为之**的场景：`ModelsMatrixSection.tsx:326`（模型矩阵列多时）、`PricingTab.tsx:296`（价格表列多时）、`MultiKeyPreview.tsx:58`（多密钥预览）——均为数据量驱动的表格类组件，横向滚动是预期交互而非布局回归。

**结论：无非预期横向滚动**（预期内的表格自滚动除外）。

### 3. Settings 锚点导航

`components/settings/SectionAnchorNav.tsx:22-37`：`position:"sticky"` + `display:"flex"` + `flexWrap:"nowrap"` + `overflowX:"auto"` —— 锚点栏是**内容区顶部的水平胶囊条**（chip 超出自身滚动），**不与内容区并排**（不占用内容区宽度维度）。窄窗口下锚点条自身水平滚动，不挤压下方内容区宽度。

**结论：AnchorNav 布局关系正常，无并排挤压问题。**

### 4. 弹窗居中（createPortal 规则）

全仓 grep `Modal` 相关文件（18 个候选），实际排查：
- 无一处手写 `position:"fixed"` 自制弹窗
- 全部弹窗组件（`WindowsEditModal` / `SmartPasteModal` / `ShareModal` / `BatchOverrideModelsModal` / `BatchSetStatusModal` / `BatchDeleteModal` / `BatchMoveGroupModal` / `SegmentEditModal` / `ModelsMatrixSection` 内嵌 import 弹窗 / `ModelTestPanel` / `McpModals`）均基于 `components/ui/dialog.tsx` 或 `alert-dialog.tsx`，两者内部 `DialogPortal`/`AlertDialogPortal` = Radix `*.Portal`（挂载到 `document.body`，等价于本项目规则要求的 `createPortal(document.body)`）
- `StatusLinePanel.tsx` 本身不含弹窗，仅 import `SegmentEditModal`（已确认走 Dialog）
- 弹窗宽度扫描（`maxWidth` grep）：380~560px 区间，均远小于窗口 1026px 宽度；Portal 挂载 body 不受 726px 内容区约束

**结论：Modal 全部合规，无祖先 `transform`/`backdrop-filter` 退化风险；弹窗宽度在任何窗口尺寸下均居中且不超界。**

## 缺陷清单

**空** —— 本次静态审计（宽度计算 + grep 全量硬编码尺寸 + grid/flex 多列布局 + overflow 策略 + AnchorNav + Modal portal）未发现确认缺陷或疑似缺陷。已扫描：18 个页面文件本体 + `components/settings/`、`components/platforms/`、`components/ui/dialog.tsx`/`alert-dialog.tsx`、`Sidebar.tsx`、`App.tsx`。

**待人工验证清单**（静态不可判，需实渲染，非「缺陷」）：
- 8 种语言（尤其德语/俄语/阿拉伯语 RTL）文案长度是否在 726px 内容区内挤爆固定宽 label（如 `minWidth:120` 的 settings label 列，`AppSettings/LogSettingsSection.tsx:127,148,168`、`SystemMiscSection.tsx:121`）—— 静态审计只能确认容器结构合理，无法确认真实字符串渲染宽度
- 系统字体渲染差异（不同 OS 字号度量）导致的临界换行
- macOS 原生标题栏实际占高（本次高度可用区间为估算，未读 Tauri titlebar 配置精确值）

## 附：本次扫描范围

- `src/pages/*.tsx`（18 个页面本体）
- `src/pages/{platforms,Groups,Logs,AppSettings,PopoverConfigTab,CliProxy}/*.tsx`（拆分子组件）
- `src/components/{settings,platforms,shared,ui}/*.tsx`
- `src/App.tsx`、`src/components/Sidebar.tsx`
- grep 维度：`minWidth`/`width:`（px 值）、`gridTemplateColumns`、`overflowX`/`overflow-x`、`createPortal`/`Dialog`/`Portal`、`position:"fixed"`
