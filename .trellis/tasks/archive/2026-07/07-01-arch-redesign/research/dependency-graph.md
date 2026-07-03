# Research: Cross-Directory Dependency Graph (包边界重划依据)

- **Query**: api.ts 真实消费方 / editors.tsx 消费方 / 跨目录依赖矩阵 / 包边界候选
- **Scope**: internal (src/ 只读审计)
- **Date**: 2026-07-01

## Findings

### 1. api.ts 真实消费方清单（47 处，非 0）

`services/api.ts` 共 34 个 `xxxApi` 对象 + 4 个 `DEFAULT_*` / 事件常量 + 8 个解析函数 + 大量 type/interface。被 47 个文件直接 import（不含 api.test.ts）。前缀统一为 `services/api`，main 之前 grep "from.*services/api'" 得 0 是因 import 语句带空格 + 单/双引号混用，需要 `from ['\"].*services/api['\"]` 才命中。

#### 1.1 按目录分组（消费方文件清单）

| 消费目录 | 文件数 | 文件 |
|---|---|---|
| `pages/` | 19 | App.tsx (实为根)、Settings / CodexSettings / Home / Platforms / Logs / PopoverConfigTab / TrayConfigTab / SkillInstallView / Notifications / AppSettings / Stats / ModelTestPanel / PricingTab / Mcp / Skills / SkillDetailView / Groups / About |
| `components/settings/` | 9 | SchedulingSettings / MiddlewareRules / CodingToolsSettings / CcSwitchImport / NotificationSettings / ImportExport / editors / Sub2ApiImport / NotificationEventList |
| `components/platforms/` | 4 | SmartPasteModal / usePlatformCards / PlatformCard / ShareModal |
| `components/shared/` | 2 | CostTrendChart(.test) |
| `utils/` | 4 | ccswitchMatch(.test) / sub2apiMatch(.test) |
| `context/` | 1 | AppContext |
| `popover.tsx` (root) | 1 | tray 浮窗入口 |
| `assets/platforms/index.ts` | 1 | 仅 `type Protocol, Platform` |
| `components/PopoverCards.tsx` | 1 | tray 浮窗卡片 |

#### 1.2 按域聚合的 Api 消费热度（采样前 5）

- **settings 域**（10+ 文件）: `settingsApi` / `configApi` / `statuslineApi` / `claudeSettingsImportApi` / `codexApi` / `codingToolsSettingsApi`
- **platforms 域**（4 文件 + Groups 反向）: `platformApi` / `quotaApi` / `modelTestApi` / `schedulingApi`
- **groups 域**（3 文件 + popover）: `groupApi` / `groupDetailApi` / `groupUsageApi`
- **proxy/log 域**（5 文件）: `proxyApi` / `proxyLogApi` / `proxyTimeoutApi` / `onProxyLogUpdated` / `configApi`
- **stats 域**（4 文件）: `statsApi` / `statsSettingsApi`

完整 34 Api 对象清单（file:line）见 `services/api.ts`：
`platformApi:435 trayApi:527 groupUsageApi:544 trayConfigApi:611 popoverConfigApi:703 groupApi:714 groupDetailApi:771 proxyApi:805 configApi:826 proxyLogApi:941 proxyTimeoutApi:960 middlewareApi:1063 schedulingApi:1099 notificationApi:1206 settingsApi:1266 statuslineApi:1282 scriptExecutorApi:1298 codexApi:1313 claudeSettingsImportApi:1325 appLogApi:1333 dbApi:1346 codingToolsSettingsApi:1358 statsApi:1420 statsSettingsApi:1437 modelTestApi:1463 quotaApi:1503 modelPriceApi:1560 priceSyncApi:1577 skillsApi:1702 mcpApi:1823 importExportApi:1915 ccswitchApi:1977 backupApi:2043 aboutApi:2069`

### 2. editors.tsx 消费方（symbol 级精确清单）

`components/settings/editors.tsx` 共 24 个 export。消费方 9 个文件，symbol 分布：

| Symbol | 消费文件（除 editors 自身） |
|---|---|
| `F` (字号 token) | pages/{Settings, CodexSettings, Home, Logs, PopoverConfigTab, TrayConfigTab, Stats, PricingTab, Groups} + components/settings/{MiddlewareRules, SectionAnchorNav, SettingsHeader, UnsavedChangesModal, statusline-gen} + utils/platformPaste (15 文件) |
| `S` (间距 token) | pages/{Settings, CodexSettings, Groups} + components/settings/{MiddlewareRules, SettingsHeader, UnsavedChangesModal} + utils/platformPaste (7 文件) |
| `SectionIcon` | components/settings/{CcSwitchImport, SectionAnchorNav, ImportExport, SettingsHeader, Sub2ApiImport} + pages/{Settings, CodexSettings} (7) |
| `SvgIcon` | components/settings/SettingsHeader (1) |
| `FieldRenderer` | pages/{Settings, CodexSettings} + services/{codex-settings-schema, claude-settings-schema} (4) |
| `EnvEditor` / `PermissionsSectionInline` / `SandboxSectionInline` / `StatusLineSection` / `PluginsSectionInline` / `HooksSectionInline` / `ImportDiffModal` / `buildImportDiffTree` / `DiffNode` / `HooksConfig` | **仅 pages/Settings.tsx** (各 1) |
| `isPlainObject` | pages/Settings + **utils/deepMerge.ts (重复定义, 见 duplication-audit)** |

**Settings.tsx 消费 14 个 editors export**（`pages/Settings.tsx:9-24`），是 editors.tsx 的主消费方。
**CodexSettings.tsx 消费 4 个**（`pages/CodexSettings.tsx:9-13`：F/S/SectionIcon/FieldRenderer）。

### 3. 跨目录依赖矩阵（标出"反向 / 跨域"引用）

#### 3.1 标准方向（合理）

```
pages/ → services/         (全部 page 依赖 api.ts)
pages/ → utils/            (formatters/pinyin/navGuard/deepMerge/platformPaste/ccswitchMatch/sub2apiMatch)
pages/ → components/{settings,platforms,shared}
pages/ → context/AppContext
components/platforms/ → services/api
components/settings/ → services/api + services/*-settings-schema
context/ → services/api
utils/ → services/api      (ccswitchMatch/sub2apiMatch 仅取 type)
```

#### 3.2 反向 / 跨域引用（包边界重划焦点）

| 引用 | 方向问题 | 说明 |
|---|---|---|
| `components/platforms/PlatformCard.tsx:11-13` ← `pages/Platforms.tsx` | **components 反向 import pages** | 消费 11+ export (allModelValues/PROTOCOL_LABELS/PROTOCOL_COLORS/HEALTH_COLORS/healthStatus/getDefaultModels/tierLabel/formatResetCountdown/computeQuotaDisplay/computeManualBudgetDisplay + type QuotaDisplay/HealthStatus)。说明 `pages/Platforms.tsx` 实际充当"平台域公共库"，组件反向依赖页面入口文件 |
| `components/platforms/usePlatformCards.ts:8` ← `pages/Platforms.tsx` | 同上 | 消费 computeQuotaDisplay/computeManualBudgetDisplay/getDefaultModels/allModelValues/healthStatus 等 |
| `utils/ccswitchMatch.ts:16` ← `pages/Platforms.tsx` | **utils 反向 import pages** | 消费 PROTOCOLS/getDefaultEndpoints/ProtocolOption |
| `utils/sub2apiMatch.ts:12` ← `pages/Platforms.tsx` | 同上 | 消费 getDefaultEndpoints |
| `components/settings/Sub2ApiImport.tsx:19` ← `pages/Platforms.tsx` | **settings 域 import pages/platforms 域** | 消费 PROTOCOLS（跨域） |
| `pages/Platforms.tsx:10` ← `pages/Groups.tsx` (`GroupsEmbedded`) | **pages 之间互引** | Platforms 嵌入 Groups 视图（Groups 视为独立子模块） |
| `pages/Groups.tsx:21-23` ← `components/platforms/{PlatformCard,usePlatformCards,ShareModal}` | 合理方向 | Groups 复用平台域组件 |

#### 3.3 关键结论

**`pages/Platforms.tsx` 不是页面，是"平台域 SDK"**。它有 29 个 top-level 声明，其中 11 个 export 被 `components/platforms/`、`utils/`、`components/settings/Sub2ApiImport`、`pages/Groups.tsx`、`utils/platformPaste.test` 共 7+ 文件跨目录消费。物理位置 `pages/` 是历史遗留，导致 4 处依赖方向倒置。**这是包边界重划的第一优先级。**

### 4. 包边界候选（基于依赖图）

| 候选包 | 物理落点建议 | 对外 barrel 应暴露 | 当前物理位置问题 |
|---|---|---|---|
| **`@aidog/platforms`** | `src/domains/platforms/` | PROTOCOLS / PROTOCOL_LABELS / PROTOCOL_COLORS / HEALTH_COLORS / healthStatus / getDefaultEndpoints / getDefaultModels / MODEL_SLOTS / allModelValues / computeQuotaDisplay / computeManualBudgetDisplay / tierLabel / formatResetCountdown / ProtocolOption / QuotaDisplay / HealthStatus / autoCategorize / 平台域 type | Platforms.tsx 当前混了"页面组件 (Platforms fn @ 1342)" + "域 SDK (exports @ 19-1340)"，需拆 `pages/Platforms.tsx` 仅留页面组件，SDK 抽到 `domains/platforms/` |
| **`@aidog/groups`** | `src/domains/groups/` | GroupsEmbedded / GroupListItem / PlatformPicker / GroupTestPanel / GroupIcon / editReducer / EditState / routingModeLabel / routingModeDesc / buildClaudeCommand / buildCodexCommand / shellSquote / ROUTING_MODES | routingModeLabel 与 SchedulingSettings 重复（见 duplication-audit） |
| **`@aidog/settings`** | `src/domains/settings/` (现状 `components/settings/` + 部分 pages/) | F / S / SectionIcon / SvgIcon / FieldRenderer / EnvEditor / 各 *Section(Inline) / ImportDiffModal / buildImportDiffTree / statusline-gen / statusline-runtime / claude/codex-settings-schema | editors.tsx 4609 行需按 section 拆（见 section-split-map） |
| **`@aidog/shared`** | `src/components/shared/` (现状) + F/S token 上移 | CompactCard / StatChip / BalanceBar / CostTrendChart / TestResultBody / colorScale / usageColor / formatters / **F / S token（当前散落 6 文件）** | F/S token 应从 editors.tsx 移到 shared，避免 shared 反向 import settings |
| **`@aidog/api`** | `src/services/api/` | 维持 barrel `index.ts` 重导出，内拆按域 (见 section-split-map) | api.ts 2072 行需按域拆 |

#### 4.1 依赖图（域间，箭头 = import）

```
        ┌──────────────────────────────────────┐
        │            @aidog/api                │  (所有人依赖，无环)
        └──────────────────────────────────────┘
                         ▲ ▲ ▲
        ┌────────────────┘ │ └───────────────┐
        │                  │                 │
  ┌─────┴──────┐    ┌──────┴───────┐   ┌──────┴──────┐
  │ @aidog/    │    │ @aidog/      │   │ @aidog/     │
  │ platforms  │◄───┤ groups       │   │ settings    │
  └─────┬──────┘    └──────┬───────┘   └──────┬──────┘
        │                  │                  │
        │     ┌────────────┘                  │
        ▼     ▼                               ▼
  ┌─────────────────┐              ┌──────────────────┐
  │ @aidog/shared   │◄─────────────┤  pages/ (orch)   │
  │  (F/S, formatters│              │  Settings/Groups │
  │   usageColor)   │              │  /Platforms      │
  └─────────────────┘              └──────────────────┘
```

无环。`groups → platforms` 单向（Groups 复用平台域组件 + SDK）；`settings` 不依赖 platforms/groups（独立）。

## Caveats / Not Found

- `assets/platforms/index.ts` 用了 `Protocol`/`Platform` type，应纳入 `@aidog/platforms` 的对外接口（图标资源 + type 同包）
- `popover.tsx` (root) 直接 import services/api + context/AppContext，是 tray 浮窗入口，不归任何业务域，可保留 root 或归 `@aidog/tray` 包
- `test/render.tsx` 是测试 util，不影响生产包边界
- 没发现任何 page 直接 import 另一个 page 的内部 helper（pages 之间唯一互引是 Platforms ← Groups 的 `GroupsEmbedded`，是组件级合理引用）
