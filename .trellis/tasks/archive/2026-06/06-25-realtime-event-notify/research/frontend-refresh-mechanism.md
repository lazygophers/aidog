# Research: aidog 前端实时数据更新机制 & 事件通知可行性

- **Query**: 浮窗/首页/弹窗/日志/统计等各页是否可改「后端推送事件 → 前端监听更新」取代「轮询 / mount fetch / 手动刷新」
- **Scope**: internal（纯项目代码考古）
- **Date**: 2026-06-25

---

## 0. 一句话结论

**aidog 已经走事件通知这条路了**：Tauri 后端在 `proxy_log` 写库成功后 `app.emit("proxy-log-updated", platform_id)`，前端 Home / Platforms / Stats / Groups 四页通过 `onProxyLogUpdated()` 全部监听并 debounce 500ms 触发刷新（`src/services/api.ts:1574-1590`）。

**剩余非事件化的场景**只有 3 类：
1. **popover 浮窗（独立窗口）**——Tauri `app.emit` 是全局广播，跨 webview 窗口本应可达，但 popover.tsx **未** import `onProxyLogUpdated`，仅 mount fetch。**最高 ROI 改造点**。
2. **Logs 页**——仍用 `usePolling(3000ms)` + `usePolling(2000ms)`（详情）固定轮询，**没有** listen `proxy-log-updated`。改造可去掉固定轮询，改事件驱动 + 长滚动兜底。
3. **PopoverConfigTab / TrayConfigTab 预览统计**——30s 轮询 todayStats；这两个是配置预览页（非运行时数据展示），30s 轮询合理。

**前端已有自建事件总线**（`window.dispatchEvent` + `CustomEvent`），用于跨页单卡刷新测试徽章（`aidog-platform-test-completed`）和分组变更广播（`aidog-groups-changed`）——可推广但不解决「后端→前端」方向问题。

**推荐**：方案 (c) 混合——补全 popover 与 Logs 的事件订阅（沿 `onProxyLogUpdated` 范式），CRUD 类低频操作保持现状（写操作前端已主动刷新或事件广播），不动后端 emit 节流（已有 debounce）。

---

## 1. 各页数据更新机制盘点

| 页 / 场景 | 当前机制 | 数据源 | 实时性需求 | file:line |
|---|---|---|---|---|
| **Home 首页** | mount fetch + `onProxyLogUpdated` 事件订阅 debounce 500ms | todayStats / platformToday / groups / platforms / 24h trend buckets | 中（每请求后秒级） | `src/pages/Home.tsx:122-124` |
| **Platforms** | mount `load()` + `onProxyLogUpdated` → `refreshStats()`（局部 merge est_balance / est_coding_plan 字段，不整列表替换） | platformApi.list / usageStatsAll / quota(IntersectionObserver) | 中（每请求后秒级） | `src/pages/Platforms.tsx:1874, 1930, 1839-1872` |
| **Groups** | mount `load()` + `onProxyLogUpdated` → `refreshStats()` + `aidog-groups-changed` DOM 事件 → `load()` | groupDetailApi.list / groupUsageApi.stats | 中 | `src/pages/Groups.tsx:1502, 1505-1509` |
| **Stats 统计** | mount `load()` + `onProxyLogUpdated` → `load()` + `loadFilterOptions()` | statsApi.query（含上一等周期 + 自动降级粒度） | 中 | `src/pages/Stats.tsx:165, 168, 179` |
| **Logs 日志（列表）** | **`usePolling(refreshList, 3000ms, !detail)`**（仅可见时跑） | proxyLogApi.list / listFiltered | 高（请求持续写入） | `src/pages/Logs.tsx:193` |
| **Logs 日志（详情）** | **`usePolling(refreshDetail, 2000ms, !!detail)`** | proxyLogApi.get | 高（流式 in-flight） | `src/pages/Logs.tsx:236` |
| **popover 浮窗（独立窗口）** | **mount fetch 一次 + 失焦即销毁**（无定时 / 无事件订阅） | popover_data + statsApi.queryBatch + groupApi.list | 高（用户期望开窗即最新） | `src/popover.tsx:97-119` |
| **PopoverConfigTab 配置预览** | mount fetch + `usePolling(refreshStats, 30_000ms)` | todayStats / platformToday / statsApi.queryBatch | 低（配置预览，30s 够） | `src/pages/PopoverConfigTab.tsx:223` |
| **TrayConfigTab 配置预览** | mount fetch + `usePolling(refreshStats, 30_000ms)` | trayConfigApi.todayStats | 低（同上） | `src/pages/TrayConfigTab.tsx:178` |
| **平台编辑表单 modal** | 打开时按需 fetch（无定时 / 无事件） | 各路 invoke | 无（编辑期数据冻结需求） | `src/pages/Platforms.tsx` handleEdit 等 |
| **Skills** | mount fetch + `focus` / `visibilitychange` revalidate（10s 节流） | skills CLI（npx）| 低（CLI 改动才需刷新） | `src/pages/Skills.tsx:167-168` |
| **Mcp / Notifications / About / PricingTab / ModelTestPanel / CodexSettings / AppSettings** | mount fetch / 手动刷新 / 测试时 `aidog-platform-test-completed` 事件 | 各自 invoke | 低 | 见 `src/pages/*` |

### 轮询 vs 事件统计
- **事件订阅（Tauri `proxy-log-updated`）**：4 页（Home / Platforms / Groups / Stats）
- **固定 setInterval 轮询**：Logs 列表 3s + Logs 详情 2s + PopoverConfigTab 30s + TrayConfigTab 30s = 4 处
- **DOM 事件总线**：3 处派发 + 多处监听（`aidog-platform-test-completed`、`aidog-groups-changed`、`aidog-platforms-changed` 推测存在但未确认）
- **focus/visibility 触发**：Skills 页（仿轮询节流）
- **mount 一次性 fetch**：popover、modal 编辑、Skills/Mcp/About 等

---

## 2. 已有事件机制

### 2.1 后端 → 前端（Tauri event，**关键基础设施已就绪**）

**emit 总表**（`src-tauri`，去除测试）：

| 事件名 | payload | emit 位置 | 频率 | file:line |
|---|---|---|---|---|
| `proxy-log-updated` | `platform_id: u64` | `upsert_log` 写库成功后 | **高频**（每条请求 1+ 次，流式多节点多次） | `src-tauri/src/gateway/proxy/log.rs:153` |
| `tray-refresh` | `()` | `upsert_log` / `spawn_estimate` 完成后 / 冷启动真查后 | 高 | `src-tauri/src/gateway/proxy/log.rs:154, 253`、`src-tauri/src/commands/quota.rs:96` |
| `NOTIF_SPEAK` | `String`（文本） | TTS dispatch | 按通知 | `src-tauri/src/gateway/notification/tts.rs:37` |

**前端 listen 表**（`src`，全量）：

| 监听者 | 监听事件 | debounce | file:line |
|---|---|---|---|
| Home / Platforms / Stats / Groups | `proxy-log-updated`（经 `onProxyLogUpdated` 封装） | 500ms | `src/services/api.ts:1580-1590` |
| `app_setup.rs` 主进程 | `tray-refresh`（同步触发 `refresh_tray_menu`） | 无（同步 block_on） | `src-tauri/src/app_setup.rs:267` |

**前端 `onProxyLogUpdated` 封装**（`src/services/api.ts:1574-1590`）：
```ts
export const PROXY_LOG_UPDATED = "proxy-log-updated";
export function onProxyLogUpdated(callback: () => void, debounceMs = 500): () => void {
  let timer: ReturnType<typeof setTimeout> | null = null;
  const unlistenPromise = listen(PROXY_LOG_UPDATED, () => {
    if (timer) clearTimeout(timer);
    timer = setTimeout(() => { callback(); }, debounceMs);
  });
  return () => {
    if (timer) clearTimeout(timer);
    unlistenPromise.then((un) => un()).catch(...);
  };
}
```
- 关键设计：**默认 500ms debounce**，把高频 emit 聚合成一次 callback 触发。
- 单元测试覆盖：`src/services/api.test.ts:106-122`。

### 2.2 前端 → 前端（DOM `CustomEvent` 总线）

| 事件名 | payload | 派发点 | 监听点 |
|---|---|---|---|
| `aidog-platform-test-completed` | `{ platformId, success }` | `usePlatformCards.handleQuickTest`（Platforms 编辑器 + Groups 卡片 + ModelTestPanel） | `usePlatformCards.ts:159-171`（单卡刷新 `lastTestMap` + `testResults`）；Platforms 同名 handler `Platforms.tsx:2409` |
| `aidog-groups-changed` | 无 | Platforms 改分组归属 3 处（保存/删除/拖拽后） | `Groups.tsx:1507`（reload + refreshStats） |

**派发与监听代码**：
```ts
// 派发
window.dispatchEvent(new CustomEvent("aidog-platform-test-completed",
  { detail: { platformId: p.id, success } }));
window.dispatchEvent(new Event("aidog-groups-changed"));

// 监听（usePlatformCards.ts:159-171）
useEffect(() => {
  const handler = (e: Event) => {
    const ce = e as CustomEvent<{ platformId: number; success?: boolean }>;
    const pid = ce.detail?.platformId;
    if (pid == null) return;
    refreshLastTest(pid);  // 单卡拉 lastTestResult，不整列表
    if (ce.detail.success != null) setTestResults(...);
  };
  window.addEventListener("aidog-platform-test-completed", handler);
  return () => window.removeEventListener(...);
}, [refreshLastTest]);
```
- 这是**已有的事件通知范式**，memory [[platform-last-test-badge]] 落档。

### 2.3 关键观察
- **后端 emit 已经在做**（log.rs 是热路径，每请求触发）。
- **CRUD 类写操作（platform_create/update/delete、group_*）目前不 emit**——`src-tauri/src/commands/{platform,group}.rs` 全文 grep 无 `emit`。前端靠写操作返回值 / DOM 事件广播 / 父组件主动 `load()` 同步。

---

## 3. 后端数据变更源盘点（前端依赖的数据）

| 写源 | 频率 | 影响前端页/组件 | 当前是否 emit | file:line |
|---|---|---|---|---|
| **`upsert_log` 写 proxy_log**（每请求 / count_tokens / 中间件 block / 流式终态 / finish spawn_estimate） | **高频**（每次代理请求 N 次，N=流式节点数） | Logs 列表、Logs 详情、Home 今日统计、Platforms 局部 est_balance、Stats 维度/趋势、Groups 局部 stats、popover today_platform_stats | **是**（`proxy-log-updated`） | `src-tauri/src/gateway/proxy/log.rs:153`；调用方 `proxy/{handler,forward,finish,count_tokens}.rs` 16+ 处 |
| `spawn_estimate` 后台 quota 预估 | 高频（每有 token 请求） | Platforms quota / tray 标题 | **是**（`tray-refresh`，无前端订阅） | `log.rs:253` |
| `calibrate_from_quota`（冷启动 / quota 真查） | 低（冷启动 + 阈值触发） | Platforms est_balance / tray | **是**（`tray-refresh`） | `commands/quota.rs:96` |
| `platform_create / update / delete` | **低**（用户操作） | Home / Platforms / Groups / popover 平台列表 | **否**（前端写后自己 reload） | `commands/platform.rs:43, 169, 190` |
| `group_create / update / delete / set_platforms` | **低** | Home / Platforms / Groups / Settings 同步 | **否**（DOM 广播 + 父组件刷新） | `commands/group.rs:20, 52, 63, 75` |
| `manual_budget` / `scheduling` 配置变更 | **低** | Platforms 卡片 / popover | **否** | `commands/{manual_budget,scheduling}.rs` |
| `skills / mcp / notifications` CRUD | **低** | Skills / Mcp / Notifications 页 | **否** | — |

---

## 4. 可行性 + 收益/风险评估

### 4.1 适合改事件通知（高收益）

#### (A) **Logs 页列表 / 详情** ⭐⭐⭐⭐⭐
- **现状**：固定 3s + 2s 轮询，**没用** `proxy-log-updated`。
- **问题**：空闲期（无请求）也每 3s 查一次 DB（页面可见时）；高峰期 3s 延迟才看到新行。
- **改造**：subscribe `onProxyLogUpdated(() => refreshList())`；详情页改成只对**当前 detail.id** 的 platform_id 匹配才刷新（payload 已带 platform_id，但单条 log 写多次，应直接全量重拉详情，反正流式结束就稳定）。
- **收益**：消除空闲轮询 + 高峰期实时；空闲页面 0 IPC。
- **风险**：无（debounce 已聚合高频）。

#### (B) **popover 浮窗** ⭐⭐⭐⭐
- **现状**：mount fetch 一次，开窗期间数据冻结。
- **问题**：浮窗常驻（用户开着看），请求进来不会更新。
- **改造**：在 `popover.tsx` 顶层 useEffect 加 `onProxyLogUpdated(() => reloadStats())`，类似 Platforms `refreshStats`。
- **收益**：开窗期间实时；用户高频场景体感强。
- **风险**：Tauri `app.emit` 是否能跨 webview 窗口到达 popover 独立 webview？**需验证**（推测：可以，Tauri 2.x `emit` 广播所有 webview，`emit_to` 才限定；但 popover 窗口可能因创建时机错过早期 listen 注册）。
- **降级**：若跨窗口不可达，加 visibilitychange + 5s 兜底轮询。

### 4.2 保持现状（合理）

#### (C) **PopoverConfigTab / TrayConfigTab 30s 轮询** ⭐
- 这是**配置编辑预览页**，不是运行时数据展示。30s 够用，且用户编辑配置时不应被外界更新打断。**保持**。

#### (D) **平台/分组 CRUD** ⭐
- 低频用户操作，写操作前端已主动 reload 或 DOM 事件广播。**不需要**后端 emit（增加复杂度无收益）。

#### (E) **modal 编辑 / Skills / Mcp 等** ⭐
- 编辑期数据应冻结；Skills 跨 CLI 改动用 focus/visibility 触发已足够。

### 4.3 风险点

1. **Tauri event 高频推送风险**：`upsert_log` 每条请求多次触发（流式多节点写库），高频代理场景下 `proxy-log-updated` emit 频率可达每秒数十次。
   - **已缓解**：`onProxyLogUpdated` 默认 500ms debounce（`api.ts:1580`）。
   - **进一步优化（如需）**：后端再聚合一层（如改 emit 频率为「最多每 500ms 一次」用 `tokio::time::interval` 节流），但当前 4 页前端各自 debounce 已足够。

2. ** popover 跨窗口事件可达性**：Tauri 2.x `app.emit(event, payload)` 默认广播到所有 webview window，但需要 popover.tsx 在 webview 启动后**主动 listen**（启动后到 listen 注册之间的事件会丢——这对浮窗无所谓，因为浮窗每次开都新 mount）。

3. **Logs 详情页改事件驱动的语义偏差**：详情页轮询是为了等流式日志聚合完成（`is_stream=true` 的 log 终态写完）；payload 只带 platform_id，单条 log 可能多次 emit，直接 listen 全量 reload 详情即可，逻辑等价。

4. **DOM `CustomEvent` 总线 vs Tauri event 不互通**：DOM 事件仅在当前 webview 内传播，popover 独立窗口收不到 `aidog-platform-test-completed`。若要跨窗口同步，必须走 Tauri event。

---

## 5. 架构选项 + 推荐

### 选项 (a) 全链路事件化
所有数据变更都后端 emit，前端只 listen。
- **优点**：最实时、零轮询。
- **缺点**：CRUD 低频场景过度设计；需要给 platform/group CRUD 加 emit + 前端监听，复杂度↑；后端 emit 频率难控（如 block_log 高频）。
- **不推荐**。

### 选项 (b) 前端自建 pub-sub 总线
推广 `aidog-platform-test-completed` 范式，所有页共享状态。
- **优点**：不动后端，纯前端改造。
- **缺点**：DOM 事件**跨不了 popover 独立窗口**；且后端写源（proxy_log）触发不到前端 DOM 事件——必须仍依赖 Tauri event 桥接到前端。
- **不推荐**（已部分存在，继续推广有限）。

### 选项 (c) 混合（推荐）⭐⭐⭐⭐⭐
- **高频数据走 Tauri event + 前端 listen**（已落地 4 页）：补 Logs 页 + popover 浮窗订阅 `proxy-log-updated`。
- **低频 CRUD 保持 fetch + 写后主动 reload / DOM 广播**（不动后端）。
- **30s 轮询保留**（配置预览页）。
- **DOM 事件总线继续用于同窗口跨页单卡刷新**（`aidog-platform-test-completed` 等）。

#### 具体改造点（按 ROI 排序）
1. **Logs.tsx:193** 加 `useEffect(() => onProxyLogUpdated(() => refreshList()), [])`；保留 `usePolling` 但拉长到 30s（兜底 + 流式详情收敛）。
2. **Logs.tsx:236** 详情页改 `onProxyLogUpdated(() => refreshDetail(), 1000)`（更长 debounce 避免流式高频 reload）；保留 5s 兜底轮询防事件丢失。
3. **popover.tsx** 在 mount useEffect 内加 `onProxyLogUpdated(() => {/* 重拉 popover_data + collectStatsQueries */}, 1000)`；先做 Tauri 跨 webview 可达性验证。
4. **后端 emit 节流（可选 / 后置）**：若实测高频场景前端 re-render 压力大，在 `log.rs:153` 加 `tokio::time::interval(500ms)` 节流 emit（但 4 页前端 debounce 已够，**先不做**）。

---

## 6. 不确定点

1. **Tauri 2.x `app.emit` 跨 webview 可达性**：popover 是独立 window + 独立 webview。需代码或文档证实 `emit("proxy-log-updated")` 是否广播到所有 webview，还是仅主窗口。若不广播，popover 改造需用 `emit_to(window_label, ...)` 或前端用 `getCurrentWindow().listen(...)`。
   - **验证方式**：在 popover 临时加 `listen("proxy-log-updated", console.log)`，发请求观察是否触发。

2. **popover.tsx 是否能 import `services/api`**：popover 是独立 entry（`vite.config.ts` 多入口），需确认 `@tauri-apps/api/event` 的 `listen` 在 popover webview context 可用（推测可用，与 main webview 同 runtime）。

3. **`aidog-platforms-changed` 事件是否存在**：grep 仅命中 `aidog-platform-test-completed` 和 `aidog-groups-changed`，但 Platforms.tsx 保存/删除平台后是否有等价广播未确认（ Platforms 写后自己 `load()`，Groups 页靠 `aidog-groups-changed` 间接感知——可能足够）。

4. **Stats 页 `loadFilterOptions()` 也订阅了 `proxy-log-updated`**（`Stats.tsx:179`）：每请求 debounce 后重拉 groups + platforms 列表。这是**潜在过度刷新**（platform/group CRUD 才改这俩列表，proxy_log 写入不改），可考虑改为只听 CRUD 事件（但当前 CRUD 不 emit，改造成本 vs 收益不划算，保持现状）。

---

## 7. 关键文件清单（速查）

### 前端
- `src/services/api.ts:1569-1590` — `PROXY_LOG_UPDATED` + `onProxyLogUpdated` 封装
- `src/hooks/usePolling.ts` — 可见性感知轮询 hook
- `src/popover.tsx:91-120` — popover 独立窗口 mount fetch（**未订阅事件**）
- `src/pages/Home.tsx:122-124` — 首页 mount + 事件订阅
- `src/pages/Platforms.tsx:1874, 1930, 1839-1872` — Platforms mount + 事件 → refreshStats 局部 merge
- `src/pages/Groups.tsx:1502, 1505-1509` — Groups 事件订阅 + `aidog-groups-changed` 监听
- `src/pages/Stats.tsx:165, 168, 179` — Stats 事件订阅（含 filterOptions）
- `src/pages/Logs.tsx:193, 236` — Logs **固定轮询**（**未订阅事件**）
- `src/pages/PopoverConfigTab.tsx:223` / `TrayConfigTab.tsx:178` — 配置预览 30s 轮询
- `src/components/platforms/usePlatformCards.ts:142, 159-171` — DOM 事件总线范式
- `src/services/api.test.ts:106-122` — `onProxyLogUpdated` 单元测试

### 后端
- `src-tauri/src/gateway/proxy/log.rs:147-156, 250-255` — `upsert_log` 写库后 emit 双事件
- `src-tauri/src/app_setup.rs:263-304` — `tray-refresh` 监听 + 5min 兜底定时器（跨日重算）
- `src-tauri/src/commands/quota.rs:65-99` — 冷启动真查后 emit `tray-refresh`
- `src-tauri/src/commands/platform.rs` / `group.rs` — **CRUD 无 emit**
