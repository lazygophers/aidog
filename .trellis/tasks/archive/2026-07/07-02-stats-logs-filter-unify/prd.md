# PRD — Stats/Logs 筛选完全对齐(平台/模型/分组)

> parent: `07-01-proxy-http-relay`(proxy P1 前置 child)

## 背景
Stats 与 Logs 的【平台/模型/分组】筛选是两套独立组件:Stats 用本地 `SearchableFilter`(`Stats.tsx:703`,带搜索下拉 + glass-elevated + zIndex 1000),Logs 用本地 `FilterSelect`(原生 `<select>` 无搜索)。UI/交互/主题不一致。用户要求 3 维筛选**完全对齐**(UI/主题/内容)。

## 决策(用户已锁)
| 维度 | 决策 |
|---|---|
| 归属 | 并入 proxy parent 作前置 child(proxy P1 依赖本 child 抽好的公共组件) |
| 对齐基准 | 以 Stats SearchableFilter(带搜索)为统一标准 |
| 内容对齐 | UI/主题/交互对齐,数据源各保留(Logs=platformApi.list 全平台 / Stats=有数据平台派生) |

## 交付
1. **抽公共组件** `src/components/shared/FilterDropdown.tsx` — 从 Stats `SearchableFilter` + `FilterOption` 提炼,props 同现有 `SearchableFilterProps`(width/value/onChange/allLabel/searchPlaceholder/options/emptyLabel)。保留 glass-elevated + zIndex 1000 + search 输入。
2. **shared/index.ts** 加 `export { FilterDropdown, type FilterDropdownProps }`。
3. **Stats.tsx** 删本地 `SearchableFilter` + `FilterOption`,改 import shared `FilterDropdown`,3 处调用(group/model/platform)替换。
4. **Logs.tsx** 的【平台/模型/分组】3 处 `FilterSelect` 替换为 `FilterDropdown`;**status/time/path 筛选保留原样**(Logs 独有,不在对齐范围)。
5. 数据源各保留(Logs:platforms/groups/modelOptions;Stats:allPlatforms/allModels/groups),仅 UI 组件统一。

## 验收
- 两页【平台/模型/分组】筛选视觉 + 交互完全一致(同一 FilterDropdown)
- Stats 原筛选行为零回归(选值/搜索/clear)
- Logs 平台/模型/分组筛选获搜索能力(原 FilterSelect 无),status/time/path 不受影响
- light/dark 两主题两页筛选外观一致
- i18n 复用现有 key(stats.allPlatforms/logs.filterPlatform 等),无新 key
- `yarn build` + `scripts/check-i18n.mjs` 全绿;Stats/Logs 现有测试(若有)不回归

## 非目标(YAGNI)
- status/time/path 筛选对齐(Logs 独有)
- groupBy/granularity 对齐(Stats 独有聚合维度)
- 数据源统一(决策=各保留)
- 无平台/无分组筛选选项(归 proxy P1,本 child 是其前置)

## 调度
proxy P1 前置(P1 要在筛选加「无平台/无分组」选项,依赖本 child 抽好的 FilterDropdown)。
```mermaid
graph LR
  P0[P0 筛选对齐 本child] --> P1[P1 隧道+元数据+加无平台选项]
```

## 风险
- Logs `FilterSelect` 若被其他页复用,替换需同步全调用点(grep 确认仅 Logs 内部)
- FilterDropdown 抽出后 Stats/Logs 主题变量(--bg-floating/glass-elevated 等)须两页都在 scope(CLAUDE.md themes 架构,应天然对齐)
