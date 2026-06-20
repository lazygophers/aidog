# Research: Platforms 页增删改刷新机制现状

- **Query**: 摸清 Platforms.tsx 增删改后刷新机制，定位所有全量刷新/全页 reload/整页 load() 触点，为「改局部刷新」备料
- **Scope**: internal
- **Date**: 2026-06-20

## 1. 数据模型与 state 结构

主文件 `src/pages/Platforms.tsx`（3109 行，主组件从 ~1303 起）。关键 state：

| State | 声明 | 含义 |
|---|---|---|
| `platforms: Platform[]` | `Platforms.tsx:1303` | 平台列表主数据源（含 `status`/`enabled`/`est_balance`/`est_coding_plan` 等派生字段） |
| `groupDetails: GroupDetail[]` | `Platforms.tsx:1456` | 分组详情列表（含每组 platforms 成员），驱动「已分组/未分组」归属 |
| `platformMembership: Map<number, string[]>` | `Platforms.tsx:1460` | 平台→所属分组名映射，**由 `groupDetails` effect 派生重建**（`:1765`） |
| `usageMap: Record<number, PlatformUsageStats>` | `Platforms.tsx:1374` | 用量统计（批量 `usageStatsAll`） |
| `quotaMap / quotaRealIds / quotaRefreshing / quotaPending` | `Platforms.tsx:1375-1381` | 余额/coding plan 配额（外部 HTTP，渐进填充） |
| `lastTestMap: Record<number, LastTestResult>` | `Platforms.tsx:1386` | 每平台「最近测试」徽章数据 |
| `testResults: Record<number,'ok'\|'fail'>` | `Platforms.tsx:1384` | 测试即时结果（驱动 health 点） |
| `loading: boolean` | `Platforms.tsx:1406` | **整页 loading 标志，`load()` 置 true → 触发整页 loading 态** |
| quota 调度 ref 群 | `Platforms.tsx:1389-1393` | `quotaQueueRef`/`quotaScheduledRef`/`quotaWantMapRef` 等，IntersectionObserver + 有界并发池 |

**关键派生链**：`groupDetails` 变 → effect (`:1765`) 重建 `platformMembership` → 决定平台显示在哪个分组/未分配区。所以**任何改分组归属的操作必须刷 `groupDetails`**，仅刷 `platforms` 不够（platforms-groupdetails-refresh-gap 坑现场已证实）。

### 核心刷新函数

- `load()` — `Platforms.tsx:1668-1712`：**全量重拉**。`setLoading(true)` → `platformApi.list()` → 重置 quota 调度状态 → `setPlatforms(list)` → `setLoading(false)` → 后台批量 `usageStatsAll` + 每平台 `lastTestResult`（`Promise.all`，N 次调用）。**最重的刷新路径**。
- `refreshStats()` — `Platforms.tsx:1715-1722`：**轻量全量**。`platformApi.list()` 全列表替换 + `usageStatsAll` 批量，**不置 loading、不拉 quota HTTP**。由 `onProxyLogUpdated` 订阅触发（`:1779`）。
- `handleGroupsChanged()` — `Platforms.tsx:1561-1565`：只 `setGroupDetails(await groupDetailApi.list())`，effect 自动重建 membership。**局部、专用于分组归属刷新**。

## 2. 每个写操作的当前刷新方式（逐个 file:line）

| # | 操作 | 入口 file:line | 当前刷新方式 | 全量? |
|---|---|---|---|---|
| 1 | 新建/编辑保存 | `handleSave` `:1963`，刷新在 `:2025-2029` | `resetForm(); load();` + `handleGroupsChanged()` + 广播 `aidog-groups-changed` | **全量 `load()`** + groupDetails |
| 2 | 删除平台 | `handleDelete` `:2037`，刷新 `:2042-2044` | `load(); handleGroupsChanged();` + 广播 | **全量 `load()`** + groupDetails |
| 3 | 启用/禁用切换 | `handleToggle` `:2048`，`:2053-2062` | **乐观单项 setState** → `platformApi.update` → 用返回值校正单 item；失败回滚单 item | 否（已局部） |
| 4 | 拖拽排序（未分组子集内） | `handlePlatPointerUp` `:1352`，`:1366-1367` | **乐观 setPlatforms 重排** → `platformApi.reorder` fire-and-forget（不重拉） | 否（已局部） |
| 5 | 拖拽平台到分组 | `onStandaloneGroupPointerUp` `:1602`，`:1609-1613` | `movePlatform` → `.then` 内 `load(); handleGroupsChanged();` + 广播 | **全量 `load()`** + groupDetails |
| 6 | 快速测试 | `handleQuickTest` `:2068`，`:2075/2081/2087` | **局部**：`setTestResults` 单项 + 广播 `aidog-platform-test-completed`（监听器 `:2104` 单卡刷 `lastTestMap`） | 否（已局部） |
| 7 | 刷新单平台配额 | `refreshQuota` `:1782`，`:1798-1799/1813` | **局部**：`setQuotaMap`/`setQuotaRealIds`/`setQuotaRefreshing` 单项 | 否（已局部） |
| 8 | 一键清理失效平台 | onClick `:2951`，`:2961-2963` | `load(); handleGroupsChanged(); groupsReloadRef.current?.()` | **全量 `load()`** + groupDetails |
| 9 | 一键获取模型（fetchModels） | `handleFetchModels` `:1915` | 仅改表单内 state（`setAvailableModels`/`setModels`），不触列表刷新 | 否（表单内） |
| 10 | proxy log 更新订阅 | effect `:1779` → `refreshStats` | **轻量全量列表替换**（`platformApi.list()`），高频被动触发 | 半全量 |

> `est_balance_remaining` / `est_coding_plan` 等派生字段随 `platformApi.list()` 返回，操作 1/2/5/8 走 `load()` 时一并刷新；操作 3 用 `update` 返回的单 Platform 校正，不刷其他平台余额。

## 3. 局部 vs 全量分类

**已是局部更新（乐观/单项 setState，无需改）**：
- 启用/禁用 `handleToggle`（`:2053`）— 乐观 + 返回值校正 + 失败回滚，已是范本
- 拖拽排序 `handlePlatPointerUp`（`:1366`）— 乐观重排 + fire-and-forget reorder
- 快速测试 `handleQuickTest`（`:2075`）— 单项 + 事件总线单卡刷新
- 刷新配额 `refreshQuota`（`:1798`）— 单项 quotaMap

**全量刷新（改造目标）**：
- 操作 1 保存（`:2025` `load()`）
- 操作 2 删除（`:2042` `load()`）
- 操作 5 拖到分组（`:1612` `load()`）
- 操作 8 清理失效（`:2961` `load()`）
- 操作 10 proxy log 订阅（`:1779`→`refreshStats` 全列表替换，高频）

## 4. 改局部刷新触点清单 + 建议策略

> API 事实：`platformApi.create`/`update` **均返回完整 `Platform`**（`api.ts:410` / `:434`）；`delete` 返回 void（`:436`）；`purgeDisabled` 返回 `{deletedIds, unassignedIds}`（`:442`）；`reorder`/`movePlatform` 返回 void。可据返回值做单项 setState。

| 触点 | 当前 | 建议策略 | 须刷 groupDetails? |
|---|---|---|---|
| 操作 1 保存（`:2025`） | `load()` | 编辑：用 `update` 返回值 `setPlatforms(prev=>map 替换单 item)`；新建：`setPlatforms(prev=>[...prev, created])`（注意 sort_order/位置）。仍后台补 quota/usage | **是**（join_group_ids/auto_group 改归属，保留 `handleGroupsChanged`） |
| 操作 2 删除（`:2042`） | `load()` | `setPlatforms(prev=>prev.filter(x=>x.id!==id))` 乐观删；失败回滚 | **是**（后端清 group_platform + 可能删孤儿 auto 组，保留 `handleGroupsChanged`，delete-platform-group-cleanup 坑） |
| 操作 5 拖到分组（`:1612`） | `load()` | 平台行本身不变，**只需 `handleGroupsChanged()`** 重建 membership 即可让卡片移动到目标组；可去掉 `load()` | **是**（核心就是改归属） |
| 操作 8 清理失效（`:2961`） | `load()` | 用 `r.deletedIds` `setPlatforms(prev=>prev.filter(x=>!deletedIds.includes(x.id)))` | **是**（删平台连带组清理 + `groupsReloadRef`） |
| 操作 10 proxy log（`:1779`） | `refreshStats` 全列表替换 | 可保留（轻量、无 loading），或改为只批量刷 `usageMap` + 派生余额字段，避免整列表对象替换打断 memo/拖拽态 | 否 |

**新建插入位置注意**：操作 1 新建时直接 push 到列表尾，需核对后端 `platform_list` 的排序（sort_order）是否与 push 尾部一致，否则乐观位置会与下次 list 不符；保守可对新建仍走轻量 `list()` 重排或读 `created.sort_order` 插入。

## 5. 风险点

1. **StrictMode 双跑 + 慢后端覆盖回弹**（mount-fetch-late-resolve-overwrites-optimistic）：`load()` 是 async 全量 `setPlatforms`。若改局部后仍有并发 `load()`/`refreshStats` 在途，其晚到 resolve 会用旧列表覆盖乐观删除/编辑 → 「操作后几秒回弹」。改造须加 in-flight 守卫（cancelled flag / dirtyRef），尤其操作 2 删除与操作 10 高频订阅竞争。
2. **groupDetails 同步缺失**（platforms-groupdetails-refresh-gap）：操作 1/2/5/8 改归属后若漏 `handleGroupsChanged()`，`platformMembership`（`:1765` 派生）不更新 → 已分组平台误现未分配区 / chips 陈旧。改造不得删 `handleGroupsChanged()` 调用，只去 `load()`。
3. **跨组件状态同步**：操作 1/2/5/8 均广播 `window.dispatchEvent("aidog-groups-changed")`（`:2029/2044/1613`）+ 操作 8 调 `groupsReloadRef.current?.()`（`:2963`）。`GroupsEmbedded`（`:2977`）依赖这些刷新；改局部时**必须保留事件广播**，否则嵌入的分组视图不更新。
4. **quota 调度状态重置耦合 load()**：`load()` 内 `:1678-1686` 重置 `quotaQueueRef`/`quotaScheduledRef`/`quotaWantMapRef`/`quotaPending`，并依赖 `setPlatforms` 后 IntersectionObserver 初次回调（`:1729` effect）触发首屏 quota。改局部 setState 不走 `load()` 时，新建/编辑平台的 quota 不会自动进入 wantMap → 新平台余额永不查。需对单项变更同步补 quota 调度（enqueue 或更新 wantMapRef）。
5. **memo 击穿**：`cardActions` 用 latest-ref 保稳定引用以让 `PlatformCard` memo 生效（`:2118-2134`）。操作 10 若每次全列表对象替换会让所有卡片 props 变化重渲；局部单项替换可保留未变卡片的引用，利于性能。

---

## Caveats / 未定位

- 未发现独立的「批量编辑」「导入回挂」写操作在本文件内触发列表刷新（导入走 Settings/导入导出模块，不在 Platforms.tsx）。
- 操作 1 新建乐观插入的确切 sort_order 排序规则未深入后端 `platform_list`/`platform_create` 验证（属 Rust 侧，本次只读前端范围）。
