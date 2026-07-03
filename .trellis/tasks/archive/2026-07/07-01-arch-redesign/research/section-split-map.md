# Research: 4 巨型文件拆分映射

- **Query**: editors.tsx / Platforms.tsx / Groups.tsx / api.ts 各拆到哪些子文件，拆后无 >800 行
- **Scope**: internal (src/ 只读, 实测行号)
- **Date**: 2026-07-01

## 拆分原则

- 按域聚簇（非按 section 1:1）—— 同簇 export 合并到 1 子文件
- 公共 SDK 优先抽（被跨目录消费的先独立）
- 每子文件目标 ≤ 500 行，硬上限 ≤ 800 行
- barrel `index.ts` 重导出，保持现有 import 路径兼容（最小外部 churn）

---

## 1. `components/settings/editors.tsx` (4609 行 → 9 子文件)

### 1.1 每个 export 实测行数（基于 `^export` 边界）

| 行号 | Export | 行数 | 性质 |
|---|---|---|---|
| 38 | `F` (字号 token) | 8 | **shared token**（应迁 @aidog/shared，见 duplication-audit D2） |
| 46 | `S` (间距 token) | 14 | **shared token**（同上 D8） |
| 60 | `SvgIcon` | 39 | 通用图标 |
| 99 | `SectionIcon` | 424 | 大组件，内置图标 path 表 |
| 523 | `EnvEditor` | 668 | 大组件（环境变量 KV 编辑器） |
| 1191 | `PermissionsSection` | 19 | 薄包装 |
| 1210 | `PermissionsSectionInline` | 404 | 实体（权限矩阵 UI） |
| 1614 | `SandboxSection` | 17 | 薄包装 |
| 1631 | `SandboxSectionInline` | 768 | **最大单体**（沙箱设置 UI） |
| 2399 | `StatusLineSection` | 106 | 中 |
| 2505 | `DiffNode` (interface) | 8 | type |
| 2513 | `isPlainObject` | 22 | util（**D7 重复**, 与 utils/deepMerge 同名） |
| 2535 | `readManagedPaths` | 78 | util |
| 2613 | `buildImportDiffTree` | 54 | util |
| 2667 | `ImportDiffModal` | 575 | 大组件（diff 树渲染 + 冲突解决） |
| 3242 | `PluginsSection` | 17 | 薄包装 |
| 3259 | `PluginsSectionInline` | 66 | 实体 |
| 3325 | `HooksConfig` (type) | 151 | type + matcher 配置 schema |
| 3476 | `HooksSection` | 464 | 大组件 |
| 3940 | `HooksSectionInline` | 525 | 大组件 |
| 4465 | `FieldRenderer` | ~145 (4465→4609) | 字段通用渲染器 |

### 1.2 拆分映射（按域聚簇）

| 目标子文件 | 含 export | 行数估算 | 备注 |
|---|---|---|---|
| `tokens.ts` | F + S | ~25 | 迁 @aidog/shared 更佳 |
| `icons.tsx` | SvgIcon + SectionIcon | ~470 | 通用图标，可独立 |
| `EnvEditor.tsx` | EnvEditor | 668 | 独立大组件 |
| `PermissionsSection.tsx` | PermissionsSection + PermissionsSectionInline | ~420 | 权限域 |
| `SandboxSection.tsx` | SandboxSection + SandboxSectionInline | ~785 | 沙箱域（接近上限，需内部进一步抽小组件） |
| `StatusLineSection.tsx` | StatusLineSection | ~106 | statusline 域 |
| `ImportDiff.tsx` | DiffNode + isPlainObject + readManagedPaths + buildImportDiffTree + ImportDiffModal | ~735 | diff 域（util + 组件同簇） |
| `PluginsSection.tsx` | PluginsSection + PluginsSectionInline | ~85 | 插件域 |
| `HooksSection.tsx` | HooksConfig + HooksSection + HooksSectionInline | ~1140 | **超 800**，需二次拆：`HooksSection.tsx`(464) + `HooksSectionInline.tsx`(525) + `hooks-types.ts`(151) |
| `FieldRenderer.tsx` | FieldRenderer | ~145 | 通用渲染器 |
| `index.ts` (barrel) | re-export all | ~30 | |

### 1.3 验证拆后无 >800

- SandboxSection.tsx ~785 ✅ (临界，建议内部抽 `<SandboxPathInput>` 子组件到 ~600)
- HooksSection 域拆 3 个 (max 525) ✅
- 其余均 < 700 ✅
- **总拆后最大子文件 ≈ 785**（SandboxSection 警戒线）

---

## 2. `pages/Platforms.tsx` (3583 行 → 8 子文件)

### 2.1 实测块边界（含两个巨无霸：getDefaultModels @ 167 起 276 行 / SearchableProtocolSelect @ 846 起 221 行 / MockConfigEditor @ 1079 起 98 行 / Platforms 组件本体 @ 1342 起 **2119 行**）

| 行号 | Decl | 行数 |
|---|---|---|
| 19 | ProtocolOption (type) | 2 |
| 21 | PROTOCOLS | 78 |
| 99 | ENDPOINT_PROTOCOLS | 9 |
| 108 | CLIENT_TYPES | 20 |
| 128 | defaultClientForProtocol | 10 |
| 138 | toDatetimeLocal | 10 |
| 148 | HealthStatus (type) | 1 |
| 149 | HEALTH_COLORS | 10 |
| 159 | healthStatus | 8 |
| 167 | **getDefaultEndpoints** | **276** |
| 443 | getDefaultModels | 40 |
| 483 | getDefaultModelList | 135 |
| 618 | PROTOCOL_LABELS | 74 |
| 692 | DEFAULT_NAMES | 3 |
| 695 | QUOTA_CONCURRENCY | 2 |
| 697 | PROTOCOL_COLORS | 72 |
| 769 | MODEL_SLOTS | 9 |
| 778 | allModelValues | 14 |
| 792 | EstCodingTier/Plan + parseEstCodingPlan + autoCategorize | 54 |
| 846 | **SearchableProtocolSelect** | **221** |
| 1067 | MockConfigEditorProps | 5 |
| 1072 | MOCK_ERROR_MODES | 7 |
| 1079 | MockConfigEditor | 98 |
| 1177 | QuotaDisplay + computeQuotaDisplay | 67 |
| 1244 | tierLabel | 8 |
| 1252 | formatResetCountdown | 17 |
| 1269 | newManualBudget | 8 |
| 1277 | ManualBudgetDisplay + computeManualBudgetDisplay | 24 |
| 1314 | FormSectionProps + FormSection | 28 |
| 1342 | **Platforms 主组件** | **2119** (1342→3583) |

### 2.2 拆分映射

| 目标子文件 | 含 export | 行数估算 | 备注 |
|---|---|---|---|
| `domains/platforms/constants.ts` | PROTOCOLS + ENDPOINT_PROTOCOLS + CLIENT_TYPES + PROTOCOL_LABELS + PROTOCOL_COLORS + HEALTH_COLORS + MODEL_SLOTS + DEFAULT_NAMES + QUOTA_CONCURRENCY + MOCK_ERROR_MODES + ProtocolOption + HealthStatus | ~280 | **公共 SDK**（被 7+ 文件消费，dependency-graph §3.2） |
| `domains/platforms/defaults.ts` | getDefaultEndpoints + getDefaultModels + getDefaultModelList + defaultClientForProtocol | ~470 | 公共 SDK |
| `domains/platforms/health.ts` | healthStatus + allModelValues + computeQuotaDisplay + tierLabel + formatResetCountdown + computeManualBudgetDisplay + QuotaDisplay + ManualBudgetDisplay + newManualBudget | ~140 | 公共 SDK（D3/D4 同簇） |
| `domains/platforms/autoCategorize.ts` | EstCodingTier/Plan + parseEstCodingPlan + autoCategorize | ~54 | 公共 SDK |
| `domains/platforms/SearchableProtocolSelect.tsx` | SearchableProtocolSelect | 221 | UI 组件 |
| `domains/platforms/MockConfigEditor.tsx` | MockConfigEditor + MockConfigEditorProps | ~105 | UI 组件 |
| `pages/Platforms/FormSection.tsx` | FormSection + FormSectionProps + toDatetimeLocal | ~40 | 页面私有 UI helper |
| `pages/Platforms/Platforms.tsx` | Platforms 主组件 | ~2119 | **超 800，需进一步按 UI 区块拆** |
| `pages/Platforms/index.ts` | barrel re-export Platforms | 5 | 保持 App.tsx import 路径 |

#### 2.2.1 Platforms 主组件二次拆（2119 → 多个）

Platforms 主组件 1342-3583 是单文件单函数，需按 UI 区块抽子组件（推测，需 main 确认区块边界）：
- `<PlatformsHeader>` 搜索/筛选/视图切换
- `<PlatformList>` 列表 + 卡片挂载
- `<PlatformEditForm>` 编辑面板（含 1342-2200 的表单 state + JSX）
- `<PlatformCreateModal>` 新建
- `<PlatformDuplicateFlow>` 复制
- 各 `useState/useEffect` hook 抽 `usePlatformForm` / `usePlatformQuota` / `usePlatformCreate`

目标：每个 < 600 行。

### 2.3 验证拆后无 >800

- defaults.ts ~470 ✅
- Platforms 主组件二次拆后视 main 决策（**需要: main** 提供 UI 区块切分点，或同意按 hook 抽）

---

## 3. `pages/Groups.tsx` (2250 行 → 9 子文件)

### 3.1 实测块边界

| 行号 | Decl | 行数 |
|---|---|---|
| 26 | MODEL_SLOTS | 3 |
| 29 | platformMatchesQuery | 6 |
| 35 | groupMatchesQuery | 5 |
| 40 | BATCH_TEST_CONCURRENCY | 3 |
| 43 | ROUTING_MODES | 3 |
| 46 | routingModeLabel | 12 |
| 58 | routingModeDesc | 12 |
| 70 | GroupIcon | 31 |
| 101 | SortablePlatform (interface) | 6 |
| 107 | PICKER_F | 6 |
| 113 | PlatformPicker | 101 |
| 214 | GroupRow | 6 |
| 220 | GroupTestStatus + GroupTestRow | 14 |
| 234 | GroupTestPanel | 80 |
| 314 | EditState | 12 |
| 326 | EMPTY_EDIT | 12 |
| 338 | EditAction (type) | 5 |
| 343 | editReducer | 28 |
| 371 | allModelValues (重复 D3) | 14 |
| 385 | upsertPlatformInto | 9 |
| 394 | buildClaudeCommand | 11 |
| 405 | shellSquote | 13 |
| 418 | buildCodexCommand | 18 |
| 436 | F (重复 D2) | 1 |
| 437 | S (重复 D8) | 3 |
| 440 | CopyButton (重复 D6) | 69 |
| 509 | CardsSnapshot | 13 |
| 522 | GroupListItemProps | 55 |
| 577 | **GroupListItem** | **309** |
| 886 | **GroupsEmbedded** | **1364** (886→2250) |

### 3.2 拆分映射

| 目标子文件 | 含 export | 行数估算 | 备注 |
|---|---|---|---|
| `domains/groups/routing.ts` | ROUTING_MODES + routingModeLabel + routingModeDesc | ~30 | **D1 重复源**，与 SchedulingSettings 共用 |
| `domains/groups/query.ts` | platformMatchesQuery + groupMatchesQuery | ~12 | D10 待 main 确认是否合 |
| `domains/groups/models.ts` | (合并到 @aidog/platforms/models，删除本地 allModelValues + MODEL_SLOTS) | -14 | **D3/D4 消重** |
| `domains/groups/commands.ts` | buildClaudeCommand + buildCodexCommand + shellSquote | ~45 | Groups 私有 |
| `domains/groups/editReducer.ts` | EditState + EMPTY_EDIT + EditAction + editReducer + upsertPlatformInto | ~65 | 状态机 |
| `domains/groups/GroupIcon.tsx` | GroupIcon | 31 | UI |
| `domains/groups/PlatformPicker.tsx` | SortablePlatform + PICKER_F + PlatformPicker | ~115 | UI |
| `domains/groups/GroupTestPanel.tsx` | GroupRow + GroupTestStatus + GroupTestRow + GroupTestPanel + BATCH_TEST_CONCURRENCY | ~100 | UI |
| `domains/groups/CopyButton.tsx` | (合并到 @aidog/shared/CopyButton，删除本地) | -69 | **D6 消重** |
| `pages/Groups/GroupListItem.tsx` | CardsSnapshot + GroupListItemProps + GroupListItem | ~377 | UI |
| `pages/Groups/GroupsEmbedded.tsx` | GroupsEmbedded | ~1364 | **超 800，需二次拆** |
| `pages/Groups/index.ts` | barrel re-export GroupsEmbedded | 5 | 保持 Platforms.tsx import 路径 |

#### 3.2.1 GroupsEmbedded 二次拆（1364 → 多个）

GroupsEmbedded 886-2250 单组件，按 UI 区块抽（推测，需 main 确认）：
- `<GroupsEmbeddedHeader>` 搜索 + 视图切换
- `<GroupsList>` 分组列表
- `<GroupCreateModal>` 新建分组
- `<GroupEditPanel>` 编辑面板（含 1700-1860 的 platform picker + model 显示）
- `<GroupTestRunner>` 批量测试编排
- 抽 hooks: `useGroupEdit` / `useGroupTest`

目标：每个 < 600 行。

### 3.3 验证拆后无 >800

- GroupListItem.tsx ~377 ✅
- GroupsEmbedded 二次拆后视 main 决策

---

## 4. `services/api.ts` (2072 行 → 11 子文件)

### 4.1 34 个 Api 对象 + 解析函数，按域分组

实测每个 `export const xxxApi` 块行数（基于 awk delta）：

| 域 | Api 对象（行号:行数） | 合计行数 |
|---|---|---|
| **platforms** | platformApi(435:92) + quotaApi(1503:57) + modelTestApi(1463:40) + modelPriceApi(1560:17) | ~206 |
| **groups** | groupApi(714:57) + groupDetailApi(771:34) + groupUsageApi(544:67) | ~158 |
| **proxy** | proxyApi(805:21) + proxyLogApi(941:19) + proxyTimeoutApi(960:103) + onProxyLogUpdated + PROXY_LOG_UPDATED(1589) | ~150 |
| **settings** | settingsApi(1266:16) + statuslineApi(1282:16) + codexApi(1313:12) + claudeSettingsImportApi(1325:8) + codingToolsSettingsApi(1358:62) + configApi(826:115) + NOTIF_SPEAK(1262) | ~230 |
| **tray/popover** | trayApi(527:17) + trayConfigApi(611:92) + popoverConfigApi(703:11) | ~120 |
| **scheduling/middleware** | schedulingApi(1099:107) + middlewareApi(1063:36) | ~143 |
| **notification** | notificationApi(1206:60) | ~60 |
| **log/db** | appLogApi(1333:13) + dbApi(1346:12) + scriptExecutorApi(1298:15) | ~40 |
| **stats** | statsApi(1420:17) + statsSettingsApi(1437:26) | ~43 |
| **price** | priceSyncApi(1577:125) | ~125 |
| **skills** | skillsApi(1702:121) | ~121 |
| **mcp** | mcpApi(1823:92) | ~92 |
| **import-export/backup** | importExportApi(1915:62) + backupApi(2043:26) + ccswitchApi(1977:66) | ~154 |
| **about** | aboutApi(2069:~3) | ~3 |

### 4.2 拆分映射

| 目标子文件 | 含 | 行数估算 |
|---|---|---|
| `services/api/types.ts` | Protocol / RoutingMode / PlatformStatus / ClientType / ModelSlot / Platform* / Group* / SharePlatform / MockConfig / NewApiConfig / ManualBudget 等 type/interface | ~430 (1-434) |
| `services/api/platforms.ts` | platformApi + quotaApi + modelTestApi + modelPriceApi + DEFAULT_MOCK_CONFIG + DEFAULT_NEWAPI_CONFIG + parse/serializeMockConfig/NewApiConfig/PlatformBreaker | ~330 |
| `services/api/groups.ts` | groupApi + groupDetailApi + groupUsageApi | ~160 |
| `services/api/proxy.ts` | proxyApi + proxyLogApi + proxyTimeoutApi + onProxyLogUpdated + PROXY_LOG_UPDATED | ~150 |
| `services/api/settings.ts` | settingsApi + statuslineApi + codexApi + claudeSettingsImportApi + codingToolsSettingsApi + configApi + NOTIF_SPEAK | ~230 |
| `services/api/tray.ts` | trayApi + trayConfigApi + popoverConfigApi | ~120 |
| `services/api/scheduling.ts` | schedulingApi + middlewareApi | ~143 |
| `services/api/notification.ts` | notificationApi | ~60 |
| `services/api/system.ts` | appLogApi + dbApi + scriptExecutorApi + aboutApi | ~45 |
| `services/api/stats.ts` | statsApi + statsSettingsApi | ~45 |
| `services/api/pricing.ts` | priceSyncApi | ~125 |
| `services/api/skills.ts` | skillsApi | ~121 |
| `services/api/mcp.ts` | mcpApi | ~92 |
| `services/api/exchange.ts` | importExportApi + backupApi + ccswitchApi | ~155 |
| `services/api/index.ts` | barrel re-export all（保持 `from "./services/api"` 路径零 churn） | ~80 |

### 4.3 验证拆后无 >800

- 最大子文件 types.ts ~430 ✅
- 其余均 < 350 ✅
- 全部 11 个域文件 + types + barrel = 13 个文件，平均 ~160 行

---

## 拆分全局校验

| 文件 | 拆前 | 拆后最大子文件 | 是否 ≤800 |
|---|---|---|---|
| editors.tsx | 4609 | SandboxSection.tsx ~785（临界） | ✅ 警戒 |
| Platforms.tsx | 3583 | defaults.ts ~470 / 主组件需二次拆 | ⚠️ 主组件二次拆需 main 决策 |
| Groups.tsx | 2250 | GroupListItem.tsx ~377 / GroupsEmbedded 需二次拆 | ⚠️ 二次拆需 main 决策 |
| api.ts | 2072 | types.ts ~430 | ✅ |

**满足 PRD 验收 1（拆后无 >800）需配合 2 个二次拆分决策**：
1. Platforms 主组件（2119 行单函数）按 UI 区块拆
2. GroupsEmbedded（1364 行单函数）按 UI 区块拆

## Caveats / Not Found

- **需要: main 决策点**:
  1. Platforms 主组件 1342-3583 (2119 行) 的 UI 区块切分点（哪些 JSX 段抽成独立子组件）
  2. GroupsEmbedded 886-2250 (1364 行) 同上
  3. SandboxSectionInline 1631-2399 (768 行) 是否进一步抽 `<SandboxPathInput>` 子组件降到 ~600
- 单函数巨型组件的拆分无法纯靠 `^export` 边界判定，需 main 读 JSX 结构后定切点，本审计只能给出"需拆"信号 + 当前行数
- barrel index 设计假设"保持现有 import 路径零 churn"，若 main 接受破坏性 import 路径变更，可去掉部分 barrel
- 推荐执行顺序：先抽公共 SDK (api.ts 拆 + platforms/groups 域 SDK 抽) → 再消重 (D1-D8) → 最后拆 editors/巨型组件（依赖关系最简）
