# Research: 全仓重复代码清单（复用消重依据）

- **Query**: 全量扫 src/ 找跨文件重复 / 近似重复（函数 / 常量 / 组件）
- **Scope**: internal (src/ 只读)
- **Date**: 2026-07-01

## 评级

- **完全相同**: 逐字一致或仅类型注解 / 空白差异
- **微小差异**: 同语义、不同实现细节，可统一参数化合并
- **coincidental**: 仅命名相似，逻辑不同（不合）

按合并收益（消费点数 × 实现体量）降序。

---

## Findings

### 高收益（必合）

#### D1. `routingModeLabel` —— 完全相同 (×2)

| 位置 | 实现 |
|---|---|
| `components/settings/SchedulingSettings.tsx:26` | `function routingModeLabel(t, mode)` 11 行，map: failover/load_balance/health_aware/least_latency/sticky |
| `pages/Groups.tsx:46` | 同上，**逐字相同** |

- **是否完全相同**: 是。两个 map 的 key + i18n key + fallback 全一致（已 line-by-line 对比）
- **额外发现**: Groups.tsx 紧邻还定义了 `routingModeDesc` (58 行起) 的描述版本，SchedulingSettings 没有对应物
- **合并建议**: 提取到 `@aidog/groups/routing`（或 `src/domains/groups/routingLabels.ts`），SchedulingSettings 与 Groups 共用
- **消费点**: 5 (SchedulingSettings ×1 + Groups ×3 + 间接被 Groups 列表/编辑器调用)

#### D2. `F` (字号 token) —— 微小差异 (×7 独立定义)

| 位置 | 定义 |
|---|---|
| `components/settings/editors.tsx:38` | `F = { xs, sm, md, lg, xl, h2, h3 } as const`（键最全，7 键） |
| `pages/Home.tsx:37` | `F = { title: 20, kpi: 30, label: 15, body: 14, hint: 13, small: 12 }` |
| `pages/Logs.tsx:18` | `F = { title: 20, label: 15, body: 15, hint: 13, small: 12 }` |
| `pages/Stats.tsx:24` | `F = { title: 20, label: 15, body: 15, hint: 13, small: 12 }` |
| `pages/PricingTab.tsx:13` | `F = { title: 15, body: 14, hint: 13, small: 12 }` |
| `pages/Groups.tsx:436` | `F = { title: 20, label: 15, body: 15, hint: 13, small: 12 }` |

- **是否完全相同**: **Logs / Stats / Groups 三处完全相同**；Home 多 `kpi`、少 `body`；PricingTab 少 `label`；editors 是完全不同的命名空间 (`xs/sm/md/lg/xl/h2/h3`)
- **差异列**:
  - Logs/Stats/Groups: `{title:20,label:15,body:15,hint:13,small:12}` — 可直接合并为 1 份
  - Home: 加 `kpi:30`、`body:14`（与上面 body:15 差 1）
  - PricingTab: 无 `label`，`title:15`（其他 20）
  - editors: 命名空间不同，应独立保留（settings 域专用）
- **合并建议**: 
  1. Logs/Stats/Groups → `@aidog/shared` 暴露 `F = { title: 20, label: 15, body: 15, hint: 13, small: 12 }`
  2. Home 的 `kpi: 30` 加入 shared F（kpi 是合理通用 token）
  3. PricingTab 用 shared F 即可（少 label 不影响，TS 允许子集消费）
  4. editors.tsx 的 F 因命名空间不同（xs/sm 系）保留在 settings 域
- **消费点**: F 在 src/ 共出现 75+ 次（21 Logs + 21 editors + 17 PricingTab + 9 Home + 9 Groups + 4 Stats）

#### D3. `allModelValues` —— 微小差异 (×2，跨文件)

| 位置 | 实现 |
|---|---|
| `pages/Platforms.tsx:778` | `for (const slot of MODEL_SLOTS) { const v = models[slot.key]; ... }` —— MODEL_SLOTS 是 `{key:ModelSlot, labelKey}[]` |
| `pages/Groups.tsx:371` | `for (const slot of MODEL_SLOTS) { const v = models[slot]; ... }` —— MODEL_SLOTS 是 `ModelSlot[]` |

- **是否完全相同**: **逻辑完全相同**（去重收集非空 model 名），仅因 MODEL_SLOTS 形状不同导致 `.key` vs 直接取
- **合并建议**: 统一 MODEL_SLOTS 形状（见 D4），合并为单一 `allModelValues`，放 `@aidog/platforms/models.ts`
- **消费点**: PlatformCard.tsx:104,107 + Groups.tsx:1702,1862 + Platforms.tsx 内部多处

#### D4. `MODEL_SLOTS` —— 微小差异 (×2，跨文件)

| 位置 | 定义 |
|---|---|
| `pages/Platforms.tsx:769` | `MODEL_SLOTS: { key: ModelSlot; labelKey: string }[]` —— 富结构（带 i18n key），共 5 项 |
| `pages/Groups.tsx:26` | `MODEL_SLOTS: ModelSlot[] = ["default","sonnet","opus","haiku","gpt"]` —— 裸枚举 |

- **是否完全相同**: 否。Groups 是子集（只有 key），Platforms 是超集（key + labelKey）
- **差异**: Platforms 多 `labelKey` 字段用于 UI 展示；Groups 仅遍历枚举
- **合并建议**: 保留 Platforms 的富结构作 single source of truth，Groups 改用 `MODEL_SLOTS.map(s => s.key)` 或 `export const MODEL_SLOT_KEYS: ModelSlot[] = MODEL_SLOTS.map(s => s.key)` 派生
- **消费点**: Platforms.tsx (2250, 2262, 2901) + Groups.tsx (374) + PlatformCard 隐式

#### D5. `stableStringify` —— 完全相同 (×2)

| 位置 | 实现 |
|---|---|
| `pages/Settings.tsx:37` | 5 行，递归 stable JSON stringify |
| `pages/CodexSettings.tsx:17` | 同上，**逐字相同**（仅参数注解 `any` vs `unknown`） |

- **是否完全相同**: 是（除类型注解）
- **合并建议**: 提取到 `@aidog/shared/serialize.ts`（或 `utils/stableStringify.ts`），两边 import
- **消费点**: Settings ×3 + CodexSettings ×3 = 6

#### D6. `CopyButton` —— 微小差异 (×2)

| 位置 | 实现 |
|---|---|
| `pages/Home.tsx:42` | `{ text, title, label, size = 14 }` —— 无 icon 参数 |
| `pages/Groups.tsx:440` | `{ text, title, label, icon, size = 14 }` —— 多 `icon?: ReactNode` |

- **是否完全相同**: 否。Groups 版多一个 `icon` 参数（用于在按钮前显示 claude/codex 图标）
- **差异**: Home 版无 icon；其余逻辑（clipboard 复制 + title 提示）应一致
- **合并建议**: Groups 版为超集，提取到 `@aidog/shared/CopyButton.tsx`，Home 改 import（不传 icon 即可）
- **消费点**: Home ×2 + Groups ×7 = 9

#### D7. `isPlainObject` —— 微小差异 (×2)

| 位置 | 实现 |
|---|---|
| `components/settings/editors.tsx:2513` | `typeof v === "object" && v !== null && !Array.isArray(v)` |
| `utils/deepMerge.ts:12` | 多行展开，**不排除 Array**（仅检查 typeof + !== null） |

- **是否完全相同**: 否。语义不同：deepMerge 版接受 Array（因 override 是数组替换语义），editors 版排除 Array（diff 树仅对纯对象递归）
- **差异**: Array 处理策略相反，**不可简单合并**
- **合并建议**: **保留两份**，但在各自文件顶部加注释说明语义差异。或抽 `isPlainObjectStrict`（排 Array）与 `isNonNullObject`（不排）两个具名函数到 shared，避免命名碰撞。需 main 确认。
- **消费点**: editors.tsx 内部 + Settings.tsx (via export) + deepMerge 内部

### 中收益

#### D8. `S` (间距 token) —— 完全相同 (×2)

| 位置 | 定义 |
|---|---|
| `components/settings/editors.tsx:46` | `S = { ... } as const` |
| `pages/Groups.tsx:437` | `S = { gap: 18, pad: 28, inputPad: "10px 14px", btnPad: "8px 18px", btnIcon: 34 } as const` |

- **是否完全相同**: 需对比 editors 的 S 字段（未在本审计展开）。Groups 的 S 是 page 间距通用 token
- **合并建议**: 同 D2，移到 `@aidog/shared`
- **消费点**: S 共 7 文件消费

#### D9. `routingModeDesc` —— 单点定义但耦合 D1

- 仅 `pages/Groups.tsx:58`，无重复。但与 D1 的 `routingModeLabel` 是同簇，应一起迁移到 `@aidog/groups/routing`

### 低收益 / 需确认

#### D10. `platformMatchesQuery` —— coincidental? (×1 + 1 概念相似)

- `pages/Groups.tsx:29`: `pinyinMatch(q, p.name) || pinyinMatch(q, p.base_url) || pinyinMatch(q, p.platform_type)`
- `pages/Platforms.tsx`: 无同名函数，但有 inline filter 逻辑（`standalonePlatforms` 概念，注释「与 Groups platformMatchesQuery 同口径」见 Groups.tsx:27）

- **是否完全相同**: Groups 的注释声称"与 Platforms standalonePlatforms filter 同口径"，但 Platforms 中是 inline filter 而非独立函数
- **合并建议**: **需 main 确认**。疑点：Groups 注释暗示两处口径应同步，但 Platforms 没抽出函数。建议抽到 `@aidog/platforms/query.ts`，两边共用
- **消费点**: Groups 内 3 处 + Platforms inline

#### D11. `shellSquote` / `buildClaudeCommand` / `buildCodexCommand` —— **非重复（main 猜测排除）**

- `/usr/bin/grep -rn` 全仓仅 `pages/Groups.tsx` 一处定义，无任何其它文件 import 或重复定义
- 不需要合并。保留在 Groups（或迁 `@aidog/groups/commands.ts` 仅做包内聚簇）

#### D12. `GroupIcon` —— 单点 (×1)

- 仅 `pages/Groups.tsx:70`，无重复。归 `@aidog/groups`

---

## 合并优先级总表

| ID | 项目 | 收益 | 消费点 | 合并目标 |
|---|---|---|---|---|
| D2 | F token (Logs/Stats/Groups/Home/PricingTab) | 高 | 75+ | `@aidog/shared` (新建 typography tokens) |
| D1 | routingModeLabel | 高 | 5 | `@aidog/groups/routing` 或 shared |
| D3 | allModelValues | 高 | 5+ | `@aidog/platforms/models` |
| D4 | MODEL_SLOTS | 高 | 5+ | `@aidog/platforms/models` (与 D3 同簇) |
| D5 | stableStringify | 中 | 6 | `@aidog/shared/serialize` 或 `utils/` |
| D6 | CopyButton | 中 | 9 | `@aidog/shared/CopyButton` |
| D8 | S token | 中 | 7 | `@aidog/shared` |
| D7 | isPlainObject | 低/不合 | 3 | 保留两份（语义不同），加注释 |
| D10 | platformMatchesQuery | 待确认 | 3+ | `需要: main` 确认是否抽公共 |

## Caveats / Not Found

- **需要: main 决策点**: D7 (isPlainObject) 是否合 / D10 (platformMatchesQuery) Platforms 是否真的有 inline 同口径逻辑需对齐
- 本审计基于 top-level 函数/常量定义的 grep，**未覆盖组件内部局部 helper 的重复**（如多个 useEffect 内的同样计算）。如需深度重复检测需用 jscpd / ts-prune 类工具，超出只读 grep 范围
- F token 的 editors 版（xs/sm/md）与 page 版（title/label/body）命名空间差异较大，**不应强合**，建议分两套：shared 通用 + settings 专用
- `DEFAULT_NAMES` / `autoCategorize` 仅在 Platforms.tsx 内部，无跨文件重复，归 platforms 域即可
