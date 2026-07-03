# Research: 5 历史大文件拆分映射（arch 扩 scope 补审）

- **Query**: Skills / Mcp / Logs / PopoverConfigTab / AppSettings 5 文件各拆到哪些子文件，拆后无 >800 行
- **Scope**: internal（worktree 只读 src/pages/，实测行号）
- **Date**: 2026-07-03

## 拆分原则（引 section-split-map.md，不重复）

- 决策锁 #3：**UI 区块 + hook 混合** —— 先抽 hook 收 state（`useXxxState`/`useXxxData`/`useXxxForm`），再按 JSX 区块抽子组件
- 每子文件目标 ≤ 600，硬上限 ≤ 800
- **包边界沿用 Groups/platforms 既成先例**（实测确认）：
  - 保留 `pages/Xxx.tsx` 作 facade（外部 import 路径 `from "./pages/Skills"` 等**零 churn**，App.tsx 6 处 import 全不动）
  - 子目录 `pages/Xxx/` 装抽出的 hooks + 子组件
  - **禁造 `pages/Xxx/index.ts` barrel** —— Groups 先例无此文件（`src/pages/Groups/index.ts` 不存在），App.tsx 直接 `from "./pages/Groups"` 命中 `Groups.tsx`。Skills/Mcp/Logs/PopoverConfigTab 同此规。
- 不改业务逻辑 / i18n key / Tauri command 签名

---

## 5. `pages/Skills.tsx` (1307 行 → facade + 7 子文件)

### 5.1 实测块边界（基于 `^export` / `^function` / `^const` + Read 交叉确认）

| 行号 | Decl | 行数 | 性质 |
|---|---|---|---|
| 29 | AGENTS (const) | 4 | 常量 |
| 30 | AGENT_ICONS | 2 | 常量 |
| 33 | `skillCatalogId` | 3 | util |
| 38 | `SkillSharePayload` (interface) | 3 | type |
| 44 | `decodeSkillShare` | 27 | util（base64/JSON 解码） |
| 72 | **`Skills` 主组件** (export) | **1235** (72→1307) | 单函数巨无霸，需二次拆 |
| — | (无其他顶层 export) | — | — |

主组件内部实测（Read 全文确认）：
- L72-128：state 声明（15 个 useState + 2 useRef + 1 useMemo）
- L139-234：5 个 useEffect（checkEnv / refresh / deep-link / message 自动消失）
- L145-205：`refreshInstalled` + `loadInstalled`（数据加载）
- L236-528：10 个 handler（pickProjectDir / handleToggle / handleUpdate / handleUninstallAll / handleUninstallSingle / handleAlign / handleEnableAll / handleShare / openImportConfirm / handleImport / handlePasteImport）
- L516-543：`openDeepLinkImport`（useCallback）+ deep-link mount 副作用
- **L545-958**：JSX 主列表视图（list subview）：Header 545-609 / 统计卡 677-752 / 搜索框 754-763 / 已装列表 765-953
- L961-969：install subview（直接挂 `<SkillInstallView>`，9 行）
- L973-1015：confirmUninstall modal（43 行）
- L1016-1058：uninstallTarget modal（43 行）
- L1059-1128：alignOpen modal（70 行）
- L1130-1134：detailTarget modal（5 行，挂 `<SkillDetailView>`）
- L1136-1148：shareData modal（13 行，挂 `<ShareModal>`）
- L1150-1198：pasteOpen modal（49 行）
- L1199-1304：importIds modal（106 行）

### 5.2 拆分映射

| 目标子文件 | 含 export | 行数估算 | 备注 |
|---|---|---|---|
| `pages/Skills.tsx` (facade) | `Skills`（精简版，仅 mount `<SkillsView>` + 转发） | ~30 | 保持 App.tsx import 路径 |
| `pages/Skills/share.ts` | `skillCatalogId` + `SkillSharePayload` + `decodeSkillShare` | ~40 | 分享编解码 |
| `pages/Skills/constants.ts` | AGENTS + AGENT_ICONS | ~10 | 复用 token（与 Mcp AGENTS 同形，**D-候选重复**见 §6.4） |
| `pages/Skills/useSkillsState.ts` | 收 15 useState + 2 useRef（env/installed/busyKey/message/confirmUninstall/uninstallTarget/shareData/pasteOpen/importIds 等）+ 5 useEffect（checkEnv/refresh/deepLink/messageTimeout） | ~140 | hook 抽 state |
| `pages/Skills/useSkillsActions.ts` | `refreshInstalled` + `loadInstalled` + 10 个 handler + `openDeepLinkImport` | ~310 | hook 抽 actions（依赖 useSkillsState） |
| `pages/Skills/SkillsView.tsx` | 主 JSX（Header + 统计卡 + 搜索框 + 已装列表 + install subview） | ~430 | UI 区块（L545-969） |
| `pages/Skills/SkillModals.tsx` | confirmUninstall + uninstallTarget + alignOpen + detailTarget + shareData + pasteOpen + importIds 共 7 个 modal | ~330 | UI 区块（L973-1304，全部 createPortal 弹窗聚簇） |

### 5.3 单函数巨型组件二次拆

Skills 主组件 1235 行（L72-1307）拆分切点：

**Step 1 — hook 抽（消除 545 行 state/handler/effect）**：
- `useSkillsState()` 收 L75-128 的 15 useState + 2 useRef + L127 的 `filteredInstalled` useMemo → ~80 行
- `useSkillsActions(state, setters)` 收 L145-528 的 2 loader + 10 handler + openDeepLinkImport + L526-543 deep-link mount effect → ~310 行

**Step 2 — JSX 区块切点（按 createPortal 边界天然分簇）**：
- 切点 1（**L969 后**，install subview 结束）：主视图 `SkillsView` 与 modals 分离
- 切点 2（**L973**）：modals 整体抽 `SkillModals`，内部 7 个 modal 各为内部小组件（`ConfirmUninstallModal` L973-1015 / `UninstallSingleModal` L1016-1058 / `AlignModal` L1059-1128 / `DetailModal` L1130-1134 / `ShareModal` 段 L1136-1148 / `PasteImportModal` L1150-1198 / `ImportConfirmModal` L1199-1304）

facade `Skills.tsx` 仅：`const state = useSkillsState(); const actions = useSkillsActions(state); return <><SkillsView {...}/><SkillModals {...}/></>`

### 5.4 验证拆后无 >800

- useSkillsActions.ts ~310 ✅
- SkillsView.tsx ~430 ✅
- SkillModals.tsx ~330 ✅
- facade ~30 ✅
- **最大子文件 ≈ 430（SkillsView）** ✅ 远低于 800

---

## 6. `pages/Mcp.tsx` (1169 行 → facade + 8 子文件)

### 6.1 实测块边界

| 行号 | Decl | 行数 | 性质 |
|---|---|---|---|
| 26 | AGENTS (const) | 7 | 常量（**与 Skills AGENTS 形近，D-候选**） |
| 27 | AGENT_ICONS | 6 | 常量 |
| 33 | `agentSupported` | 6 | util |
| 39 | `transportStyle` | 12 | util |
| 51 | `summaryOf` | 8 | util |
| 59 | **`Mcp` 主组件** (export) | **767** (59→825) | 单函数巨无霸 |
| 826 | `McpRow` | 195 (826→1020) | 子组件（行级） |
| 1021 | `TransportBadge` | 23 | 子组件（badge） |
| 1044 | `KVRows` | 55 (1044→1098) | 子组件（KV 表） |
| 1099 | `btnPrimary` | 11 | style 常量 |
| 1110 | `btnGhost` | 10 | style 常量 |
| 1120 | `btnDanger` | 11 | style 常量 |
| 1131 | `modalOverlay` | 10 | style 常量 |
| 1141 | `modalBody` | 12 | style 常量 |
| 1153 | `fieldLabel` | 8 | style 常量 |
| 1161 | `inputStyle` | 8 (1161→1169) | style 常量 |

主组件内部实测：
- L61-91：11 useState（servers/loading/busyKey/message/scanOpen/scanItems/scanning/selected/importing/pasteOpen/pasteText/pasteBusy/deleteTarget/editTarget/editOpen/shareData/editForm）
- L105-300：`refresh` + 9 handler（handleToggle/handleScan/handleImport/handlePasteImport/handleDelete/handleResync/handleShare/handleToggleAll/openAdd/openEdit/handleEditSave/openDeepLinkImport）
- L353-369：2 useEffect（mount refresh + deep-link）
- **L446-825**：JSX（顶栏 446-503 / 消息条 488-503 / 列表 504-537 + 5 modal：scan 539-657 / paste 659-692 / delete 693-719 / edit 720-806 / share 808-825）

### 6.2 拆分映射

| 目标子文件 | 含 export | 行数估算 | 备注 |
|---|---|---|---|
| `pages/Mcp.tsx` (facade) | `Mcp`（精简，mount `<McpView>` + 转发） | ~30 | App.tsx import 路径不变 |
| `pages/Mcp/constants.ts` | AGENTS + AGENT_ICONS | ~15 | |
| `pages/Mcp/styles.ts` | btnPrimary + btnGhost + btnDanger + modalOverlay + modalBody + fieldLabel + inputStyle | ~70 | 7 个 style 常量聚簇 |
| `pages/Mcp/transport.ts` | `agentSupported` + `transportStyle` + `summaryOf` | ~30 | util |
| `pages/Mcp/useMcpState.ts` | 11 useState + mount refresh effect + deep-link effect | ~80 | hook 收 state |
| `pages/Mcp/useMcpActions.ts` | `refresh` + 9 handler | ~210 | hook 收 actions |
| `pages/Mcp/McpRow.tsx` | `McpRow` + `TransportBadge` + `KVRows` | ~280 | 行级组件 + 其依赖 badge/KV |
| `pages/Mcp/McpView.tsx` | 主 JSX（顶栏 + 消息条 + 列表挂 McpRow） | ~100 | UI 区块（L446-537） |
| `pages/Mcp/McpModals.tsx` | scan + paste + delete + edit + share 共 5 modal | ~290 | UI 区块（L539-825） |

### 6.3 单函数巨型组件二次拆

Mcp 主组件 767 行（L59-825）—— **实测 767 < 800 硬门，但 > 600 软目标**，仍建议拆。切点：

**Step 1 — hook 抽**：
- `useMcpState()` 收 L61-91 state + L116/L353 两个 effect → ~80 行
- `useMcpActions(state)` 收 L105-300 的 `refresh` + 9 handler → ~210 行

**Step 2 — JSX 切点**：
- 切点 1（**L537 后**，列表段结束）：`McpView`（L446-537）与 `McpModals`（L539-825）分离
- 切点 2（L538 注释行 `{/* 扫描导入 modal */}`）：5 个 modal 各为内部小组件

### 6.4 验证拆后无 >800

- McpRow.tsx ~280 ✅
- McpModals.tsx ~290 ✅
- useMcpActions.ts ~210 ✅
- **最大子文件 ≈ 290（McpModals）** ✅

### 6.5 Caveats

- **需要: main 决策**：Skills AGENTS（L29 `["claude","codex"]`）与 Mcp AGENTS（L26 `["claude-code","codex"]`）形近但 slug 不同（claude vs claude-code），**不可盲目合并**。建议各留本地常量，标 `// ponytail: slug 与 Skills 不同，不合`。
- Mcp 主组件 767 行**卡在硬门 800 内**，二次拆为软目标优化，非硬性阻塞。

---

## 7. `pages/Logs.tsx` (1061 行 → facade + 8 子文件)

### 7.1 实测块边界

| 行号 | Decl | 行数 | 性质 |
|---|---|---|---|
| 20 | PAGE_SIZE_OPTIONS / DEFAULT_PAGE_SIZE | 4 | 常量 |
| 24-31 | ROW_STYLE / INLINE_FLEX_STYLE / PLATFORM_NAME_STYLE / RETRY_BADGE_STYLE / MODEL_NAME_STYLE / SSE_BADGE_STYLE / ACTION_BTN_STYLE / GROUP_BADGE_STYLE | 8 | style 常量聚簇（8 个） |
| 34 | `TimePreset` (type) | 2 | type |
| 36 | `timePresetToRange` | 7 | util |
| 43 | **`Logs` 主组件** (export) | **600** (43→642) | 单函数，**实测 600 < 600 临界**（含 detail panel + list view 两个内联分支） |
| 643 | `safeParseJson` | 8 | util |
| 651 | `LogRowProps` (interface) | 9 | type |
| 660 | `LogRow` (memo) | 55 (660→714) | 行级组件 |
| 715 | `Pagination` | 87 (715→801) | 分页组件 |
| 802 | `FilterSelect` | 36 (802→837) | 筛选器组件 |
| 838 | COPY_ICON_STYLE | 8 | style 常量 |
| 846 | `CopyButton` | 28 (846→873) | 复制按钮 |
| 874 | `MetaItem` | 17 (874→890) | 元信息项 |
| 891 | `RequestTabs` | 56 (891→946) | 请求 tab 组件 |
| 947 | `RequestSectionContent` | 85 (947→1031) | 请求段内容 |
| 1032 | `ThCell` | 16 (1032→1047) | 表头单元格 |
| 1048 | `TdCell` | 14 (1048→1061) | 表体单元格 |

主组件内部实测：
- L45-63：11 useState + platforms/groups state
- L66-109：4 useEffect（mount load platforms/groups / activeFilter memo / modelOptions / copyDetail）
- L110-171：`copyDetail`（62 行 JSON 序列化）
- L172-203：`load` + refresh effect + offset reset effect
- L205-249：handleClear / openDetail / copyRow / refreshDetail + 2 polling effect
- L251-283：platformMap / groupNameMap memo
- **L285-455**：detail panel JSX（分支 `if (detail)` 内联，170 行）—— 含 userReq/upstream tab 调用
- **L459-642**：list view JSX（183 行）—— Header / FilterSelect×4 / model filter / LogRow 列表 / Pagination

### 7.2 拆分映射

| 目标子文件 | 含 export | 行数估算 | 备注 |
|---|---|---|---|
| `pages/Logs.tsx` (facade) | `Logs`（精简，mount `<LogsView>` + 转发） | ~30 | App.tsx import 路径不变 |
| `pages/Logs/constants.ts` | PAGE_SIZE_OPTIONS + DEFAULT_PAGE_SIZE + 8 个模块级 style 常量 + TimePreset type + timePresetToRange | ~50 | |
| `pages/Logs/useLogsState.ts` | 11 useState（logs/total/offset/pageSize/loading/detail/copied/copiedId/platforms/groups/filter×7）+ 4 useEffect + platformMap/groupNameMap memo | ~140 | hook 收 state |
| `pages/Logs/useLogsData.ts` | `load` + `copyDetail`（62 行 JSON 序列化）+ `openDetail` + `copyRow` + `refreshDetail` + `handleClear` + 2 polling effect | ~240 | hook 收 data fetch |
| `pages/Logs/LogDetailView.tsx` | detail panel JSX（L285-455 分支抽成 `<LogDetailView detail={detail} .../>`） | ~180 | UI 区块 |
| `pages/Logs/LogsView.tsx` | list view JSX（Header + FilterSelect×4 + model filter + LogRow 列表 + Pagination） | ~190 | UI 区块（L459-642） |
| `pages/Logs/LogRow.tsx` | `LogRowProps` + `LogRow`（memo）+ COPY_ICON_STYLE + `CopyButton` | ~95 | 行级组件 + 复制按钮同簇 |
| `pages/Logs/RequestDetail.tsx` | `RequestTabs` + `RequestSectionContent` + `MetaItem` + `safeParseJson` | ~170 | 请求详情渲染簇（LogDetailView 依赖） |
| `pages/Logs/FilterSelect.tsx` | `FilterSelect` + `Pagination` + `ThCell` + `TdCell` | ~155 | 通用 UI（列表 + 分页 + 单元格） |

### 7.3 单函数巨型组件二次拆

Logs 主组件 600 行（L43-642）—— **实测恰等于软目标 600**，且含两个内联 JSX 大分支（detail panel 170 + list view 183）。切点：

**Step 1 — hook 抽**：
- `useLogsState()` 收 L45-63 state + L66-109 4 effect + L251-283 platform/group memo → ~140 行
- `useLogsData(state)` 收 `load`/`copyDetail`/`openDetail`/`copyRow`/`refreshDetail`/`handleClear` + 2 polling effect → ~240 行

**Step 2 — JSX 切点**：
- 切点 1（**L284 后**，`if (detail)` 前的 platformMap memo 结束）：`LogDetailView` 抽出，签名 `<LogDetailView detail={detail} platformMap={...} t={t} onBack={() => setDetail(null)} onCopy={copyDetail}/>`
- 切点 2（**L458 `}` 后**，detail 分支 `}` 结束，L456 注释 `// ── List view ──`）：`LogsView` 抽出 list view

facade 仅：`if (detail) return <LogDetailView/>; return <LogsView/>`

### 7.4 验证拆后无 >800

- useLogsData.ts ~240 ✅
- LogsView.tsx ~190 ✅
- LogDetailView.tsx ~180 ✅
- RequestDetail.tsx ~170 ✅
- FilterSelect.tsx ~155 ✅
- **最大子文件 ≈ 240（useLogsData）** ✅

---

## 8. `pages/PopoverConfigTab.tsx` (908 行 → facade + 7 子文件)

### 8.1 实测块边界

| 行号 | Decl | 行数 | 性质 |
|---|---|---|---|
| 56 | ALL_ITEM_TYPES | 16 | 常量 |
| 72 | MULTI_INSTANCE_TYPES | 5 | 常量 |
| 77 | GROUP_TYPES | 5 | 常量 |
| 82 | TYPE_LABELS | 15 | 常量 |
| 97 | TREND_WINDOWS / SIZE_OPTIONS / MAX_COLS | 4 | 常量 |
| 103 | COLOR_PRESETS | 7 | 常量 |
| 110 | `defaultColor` | 5 | util |
| 115 | `isValidHex` | 4 | util |
| 119 | `makeItem` | 32 | util（建新 item） |
| 151 | `effRow` | 6 | util |
| 157 | `normalizeConfig` | 21 | util |
| 178 | **`PopoverConfigTab` 主组件** (export) | **401** (178→578) | 单函数（DnD 编排） |
| 579 | `RowContainer` | 54 (579→632) | 行 droppable |
| 633 | `gripSvg` | 8 | svg 常量 |
| 641 | `SortableCard` | 28 (641→668) | 可拖卡片壳 |
| 669 | `CardEditor` | 103 (669→771) | 卡片编辑器 |
| 772 | `CustomHexInput` | 30 (772→801) | hex 输入 |
| 802 | `ScopeConfig` | 59 (802→860) | scope 配置 |
| 861 | `GroupSelect` | 16 (861→876) | group 选择 |
| 877 | `PlatformSelect` | 16 (877→892) | platform 选择 |
| 893 | `WindowSelect` | 16 (893→908) | window 选择 |

主组件内部实测：
- L180-189：9 useState（config/todayStats/platformToday/groups/groupDetails/platforms/loading/message/showAddMenu/activeId）+ L226-247 statsMap/statsLoaded + 2 effect（L191 mount / L229 stats load）
- L213-225：`refreshStats`
- L250-365：DnD handlers（addItem/updateItem/setRowCols/handleDragStart/handleDragOver/handleDragEnd）
- **L446-577**：JSX（说明卡 446-458 / 展示项布局编辑器：DndContext 494-542 + RowContainer×SortableCard×CardEditor 嵌套 + add menu 469-490）

### 8.2 拆分映射

| 目标子文件 | 含 export | 行数估算 | 备注 |
|---|---|---|---|
| `pages/PopoverConfigTab.tsx` (facade) | `PopoverConfigTab`（精简，mount `<PopoverView>` + 转发） | ~30 | AppSettings.tsx import 路径不变 |
| `pages/PopoverConfigTab/constants.ts` | ALL_ITEM_TYPES + MULTI_INSTANCE_TYPES + GROUP_TYPES + TYPE_LABELS + TREND_WINDOWS + SIZE_OPTIONS + MAX_COLS + COLOR_PRESETS | ~65 | 7 常量聚簇 |
| `pages/PopoverConfigTab/popoverUtils.ts` | `defaultColor` + `isValidHex` + `makeItem` + `effRow` + `normalizeConfig` | ~75 | util |
| `pages/PopoverConfigTab/usePopoverState.ts` | 9 useState + statsMap + 2 effect + `refreshStats` | ~110 | hook 收 state |
| `pages/PopoverConfigTab/usePopoverDnd.ts` | addItem + updateItem + setRowCols + handleDragStart + handleDragOver + handleDragEnd | ~120 | hook 收 DnD（依赖 usePopoverState） |
| `pages/PopoverConfigTab/PopoverView.tsx` | 主 JSX（说明卡 + 展示项布局编辑器 + DndContext + add menu） | ~140 | UI 区块（L446-577） |
| `pages/PopoverConfigTab/PopoverCards.tsx` | `RowContainer` + `gripSvg` + `SortableCard` + `CardEditor` + `CustomHexInput` + `ScopeConfig` + `GroupSelect` + `PlatformSelect` + `WindowSelect` | ~340 | 卡片渲染簇（9 子组件同簇，CardEditor 调用其余） |

### 8.3 单函数巨型组件二次拆

PopoverConfigTab 主组件 401 行（L178-578）—— **实测 401 远低于 600 软目标，且 < 800 硬门**。

**结论：单函数本身不需要拆**（401 行可接受）。但为对齐全局"hook 抽 state"决策锁 #3 的一致性，**建议仍抽 usePopoverState + usePopoverDnd**，使 facade 降到 ~140 行（PopoverView）。若 main 倾向最小改动，**可整文件仅做"子组件外迁"（PopoverCards.tsx 抽 9 个行级组件），facade 保留主组件 ~401 行 + constants/utils 外迁**，仍满足 ≤800。

切点（若执行 hook 抽）：
- 切点 1（**L189 后**，9 useState 结束）：state 抽 hook
- 切点 2（**L365 后**，handleDragEnd 结束）：DnD handlers 抽 hook
- JSX（L446-577）保留为 PopoverView

### 8.4 验证拆后无 >800

- PopoverCards.tsx ~340 ✅
- usePopoverDnd.ts ~120 ✅
- usePopoverState.ts ~110 ✅
- PopoverView.tsx ~140 ✅
- **最大子文件 ≈ 340（PopoverCards）** ✅

### 8.5 Caveats

- **需要: main 决策**：PopoverConfigTab 主组件 401 行**已满足硬门**，是否仍强制 hook 抽（决策锁 #3 一致性 vs 最小改动）。两方案均 ≤800。
- 注意已有 `src/components/PopoverCards.tsx`（不同文件，浮窗运行时卡片），本拆分子文件建议命名 `pages/PopoverConfigTab/PopoverCardEditors.tsx` 避免与现有组件重名。

---

## 9. `pages/AppSettings.tsx` (852 行 → facade + 5 子文件)

### 9.1 实测块边界

| 行号 | Decl | 行数 | 性质 |
|---|---|---|---|
| 17 | `Tab` (type, export) | 1 | type |
| 19 | **`AppSettings` 主组件** (export) | **833** (19→852) | 单函数巨无霸（编排容器） |

主组件内部实测（无任何顶层子函数，全部内联在 AppSettings 内）：
- L21-46：23 useState（running/proxyPort/autostart/bindLan/autolaunch/silentLaunch/logEnabled/logRetention/logUserReq/logUpstreamReq/userReqRetention/upstreamReqRetention/reqTimeout/connTimeout/logFileEnabled/logLevel/logRetHours/message/appVersion/dbCompacting/statsRetention/statsRebuilding/proxyClient）
- L48-103：3 useEffect（getVersion / mount 加载所有 settings）
- L105-240：13 handler（handleProxyStart/Stop/handleAutostartChange/handleBindLanChange/handleAutolaunchChange/handleSilentLaunchChange/handleProxyClientChange/handleLogSettingsChange/handleDbCompact/handleStatsRetentionChange/handleStatsRebuild/handleTimeoutChange/updateLogSettings）
- **L242-852**：return —— 单一巨型 ternary 链（tab 路由 + system tab 内联 566 行）
  - L242-256 + L839-852：tab 路由（pricing/tray/popover/middleware/scheduling/notifications 委派子组件 + codex/coding_tools/importexport/mitm 委派 + fallback `<Settings/>`）
  - **L256-838**：`tab === "system"` 分支内联，582 行，12 个 section：
    - L258-315：Proxy Status（58 行）
    - L316-337：Autostart（22 行）
    - L338-359：Bind LAN（22 行）
    - L360-381：Autolaunch（22 行）
    - L382-405：Silent Launch（24 行）
    - L406-518：Upstream Proxy（113 行）
    - L519-561：Timeout（43 行）
    - L562-679：Log recording（118 行，含 sub-toggles + retention）
    - L680-707：DB Maintenance（28 行）
    - L708-756：Aggregate Stats（49 行）
    - L757-821：Application Logging（65 行）
    - L822-838：App version（17 行）

### 9.2 拆分映射

| 目标子文件 | 含 export | 行数估算 | 备注 |
|---|---|---|---|
| `pages/AppSettings.tsx` (facade) | `AppSettings` + `Tab` type（精简，纯 tab 路由 + mount `<SystemTab>`） | ~50 | App.tsx import `{ AppSettings, type Tab }` 路径不变 |
| `pages/AppSettings/useSystemSettings.ts` | 23 useState + 3 useEffect + 13 handler（全 state + actions） | ~330 | hook 收 system tab 全部 state/actions |
| `pages/AppSettings/ProxyStatusSection.tsx` | Proxy Status + Upstream Proxy（L258-518，含 proxyClient 子段） | ~260 | 2 section 合簇（均涉 proxy） |
| `pages/AppSettings/StartupSection.tsx` | Autostart + Bind LAN + Autolaunch + Silent Launch（L316-405） | ~90 | 4 section 合簇（启动相关） |
| `pages/AppSettings/LogSettingsSection.tsx` | Log recording + Application Logging（L562-679 + L757-821） | ~185 | 2 section 合簇（日志相关） |
| `pages/AppSettings/SystemMiscSection.tsx` | Timeout + DB Maintenance + Aggregate Stats + App version（L519-561 + L680-756 + L822-838） | ~140 | 4 小 section 合簇 |

### 9.3 单函数巨型组件二次拆

AppSettings 主组件 833 行（L19-852）—— **超 800 硬门**，**必须拆**。最大内联块是 system tab 的 582 行（L256-838）。

切点（**全部按 section 注释行天然分簇**，无需 main 决策 UI 边界 —— 注释 `{/* Proxy Status */}` / `{/* Autostart */}` 等已明确切分）：

**Step 1 — hook 抽（消除 330 行 state/handler）**：
- `useSystemSettings()` 收 L21-46 的 23 useState + L48-103 的 3 useEffect + L105-240 的 13 handler → ~330 行

**Step 2 — JSX section 切点（按注释行）**：
- 切点 1（**L256**，`tab === "system"` 开）：SystemTab 分支整体抽出
- 切点 2（**L316**，`{/* Autostart */}` 前）：ProxyStatusSection 与 StartupSection 分离
- 切点 3（**L406**，`{/* Upstream Proxy */}` 前）：StartupSection 结束（Upstream 并入 ProxyStatusSection，因为操作 proxyClient 同簇）
- 切点 4（**L519**，`{/* Timeout */}` 前）：LogSettingsSection 起（ProxyStatus 含 Upstream Proxy 结束）
- 切点 5（**L562**，`{/* Log recording */}` 前）：Timeout 移入 SystemMiscSection
- 切点 6（**L680**，`{/* DB Maintenance */}` 前）：LogSettingsSection 结束
- 切点 7（**L757**，`{/* Application Logging */}` 前）：SystemMisc 中段（DB/Stats 结束）
- 切点 8（**L822**，`{/* App version */}` 前）：LogSettingsSection 收尾

facade `AppSettings.tsx`：
```tsx
export function AppSettings({tab, ...}) {
  if (tab !== "system") return <TabRouter tab={tab} .../>;  // L242-256 + L839-852
  return <SystemTab/>;
}
```

### 9.4 验证拆后无 >800

- useSystemSettings.ts ~330 ✅
- ProxyStatusSection.tsx ~260 ✅
- LogSettingsSection.tsx ~185 ✅
- SystemMiscSection.tsx ~140 ✅
- StartupSection.tsx ~90 ✅
- facade ~50 ✅
- **最大子文件 ≈ 330（useSystemSettings）** ✅

---

## 全局校验

| 文件 | 拆前 | 拆后最大子文件 | 是否 ≤800 | 是否 <600（软目标） |
|---|---|---|---|---|
| Skills.tsx | 1307 | SkillsView.tsx ~430 | ✅ | ✅ |
| Mcp.tsx | 1169 | McpModals.tsx ~290 | ✅ | ✅ |
| Logs.tsx | 1061 | useLogsData.ts ~240 | ✅ | ✅ |
| PopoverConfigTab.tsx | 908 | PopoverCards.tsx ~340 | ✅ | ✅ |
| AppSettings.tsx | 852 | useSystemSettings.ts ~330 | ✅ | ✅ |

**5 文件全部满足硬门 ≤800 + 软目标 <600**，无需任何二次 main 决策（所有切点均有 `^export` 边界或 `{/* section */}` 注释天然分簇）。

## Barrel 设计（实测先例确认）

- **不造 `index.ts`**：`src/pages/Groups/index.ts` 实测**不存在**，App.tsx 直接 `from "./pages/Groups"` 命中 `Groups.tsx`。Skills/Mcp/Logs/PopoverConfigTab/AppSettings 沿用此规 —— facade 保留原文件名 `pages/Xxx.tsx`，子目录 `pages/Xxx/` 装 hooks + 子组件，facade 直接 `import { useXxxState } from "./Xxx/useXxxState"`。
- 外部 import 路径**零 churn**（App.tsx 6 处 import 全不动 + AppSettings.tsx import PopoverConfigTab 不动）。

## 推荐执行顺序

1. **AppSettings（§9）** — 优先拆。理由：(a) 超硬门 833 行必须拆；(b) 切点全为注释行，零 UI 判断；(c) 抽出 useSystemSettings 是其他 4 文件 hook 抽的模板。
2. **Logs（§7）** — 次拆。切点清晰（detail panel vs list view 内联分支），抽出 LogDetailView/LogsView 双视图 + RequestDetail 详情簇，复用模式与 AppSettings 一致。
3. **Skills（§5）** 与 **Mcp（§6）** — **可并行**。两文件结构高度同构（state/actions/modals/list 同簇），hook 抽 + 7-8 子文件拆分模式一致，无共享文件冲突。
4. **PopoverConfigTab（§8）** — 最后。401 行主组件本身不强制拆，待 main 决策（§8.5），最小方案仅外迁 PopoverCards 子组件。

依赖关系：5 文件**两两无 import 依赖**（仅 AppSettings → PopoverConfigTab 一条），可全并行，但 §1→§2→§3-4 的顺序便于"先验证 hook 抽模式再批量套用"。

## Caveats / 需 main 决策点

1. **PopoverConfigTab 主组件 401 行已满足硬门** —— 是否仍强制 hook 抽（决策锁 #3 一致性）vs 最小改动（仅外迁子组件）。两方案均 ≤800。当前推荐 hook 抽方案以保持 5 文件模式一致。
2. **Skills AGENTS vs Mcp AGENTS**：slug 不同（claude vs claude-code），**不可合并**，各留本地常量。
3. **命名冲突预警**：`pages/PopoverConfigTab/PopoverCards.tsx` 会与现有 `src/components/PopoverCards.tsx`（浮窗运行时）重名 —— 建议子文件命名 `PopoverCardEditors.tsx` 或 `CardEditors.tsx`。
4. 所有行数估算基于 export 边界 + handler/effect 聚簇的粗算（含 import 头 ~15 行 + export 间空白），实际拆分时 ±10% 浮动正常，但全部留有 >200 行安全余量到 800 硬门。
