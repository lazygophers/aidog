# Logs/Stats 查询与 IPC 瘦身 — 详细设计

## 现状（静态盘点，非推测）

| 问题 | 位置 | 症状 |
|---|---|---|
| COUNT 全表扫 | `gateway/db/proxy_log.rs:383` | `SELECT COUNT(*)` 精确计数，7GB log.db 上每次分页都全扫 |
| 恒发谓词 | `gateway/db/proxy_log.rs:557-570` | `exclude_sources` 默认恒非空 → `NOT IN` 非 sargable，索引失效 |
| `source_protocol` 无索引 | `db/schema_early.rs:144-170`、`db/schema_late.rs:176-220` | 该列参与过滤但无索引 |
| 默认过滤无绕过 | `src/pages/Logs/useLogsFilters.ts:39` | 默认 `exclude_sources=["test","quota"]`，用户无法关掉 → 恒发 |
| model 下拉拉全字段 | `useLogsFilters.ts:58-69` | 为取 distinct model 拉 200 行完整日志 |
| Stats 全量重拉 | `src/pages/Stats.tsx:209` | `onProxyLogUpdated(() => loadFilterOptions())` 每条日志事件全量重拉筛选项 |
| minute/5min 绕过聚合表 | `gateway/db/query_stats.rs:385-460` | 不走 `stats_agg_hourly`，直接打原始表 |
| PlatformCard 重复 parse | `src/services/api/platforms.ts:206-216`、`components/platforms/PlatformCard.tsx:258,265,274,279-281` | 单卡单次渲染 `JSON.parse(p.extra)` 约 4 次 |

`DEFAULT_PAGE_SIZE=20`（`src/pages/Logs/useLogsList.ts:5`）。

## 为什么这是 08 的前置

`sqlite-page-cache-residency` 要测「page cache 降档对查询 p95 的影响」。当前 COUNT 与 `NOT IN` 每次分页都全表扫 7GB —— 它既是**灌满 cache 的源头**，也是「基线最慢 SQL」本身。不先掐掉，08 量的是一个病态基线，降档结论不可用。故 08 `deps=[logs-query-ipc-slimming]`（用户拍板）。

## 方案（当前方案 = 精简守现状）

### 1. COUNT → 有无下一页探测

分页 UI 只需要「还有没有下一页」，不需要精确总数。改 `LIMIT pageSize+1`：取回 21 行则第 21 行只用于置 `has_more`，不返给前端。

前端总页数展示随之退化为「第 N 页 / 有更多」。这是**用户可见变化**，grill 需用户拍板；若用户坚持要精确总数，退到「可能性分支」的近似计数方案。

### 2. 恒发谓词 → 可绕过

`exclude_sources` 为默认值时**不发谓词**，而非发一个恒真的 `NOT IN`。改动面在 `proxy_log.rs:557-570` 的 SQL 拼装分支：空/默认 → 跳过该段。

⚠️ 拼装时若跳过参数化分支，**必须同步 idx 递增**（memory `sql-in-placeholder-idx-increment`：漏 `idx += srcs.len()` 会让后续占位符错位，绑定错乱）。

### 3. 索引

给 `source_protocol` 补索引 —— 但**先用 `EXPLAIN QUERY PLAN` 证明它确实被选中**，再决定是否加（加索引有写放大成本，7GB 库上建索引本身也贵）。加在 `schema_late.rs`（late = 后置迁移，符合现有分层）。

### 4. model 下拉

改成后端 `SELECT DISTINCT model`（走已有索引），而非拉 200 行完整日志到前端 distinct。

### 5. Stats 事件重拉

`Stats.tsx:209` 的 `onProxyLogUpdated(() => loadFilterOptions())` 加节流（对照 `:198` 已有做法）。筛选项是慢变量，逐条日志重拉是纯浪费。

### 6. PlatformCard parse

`platforms.ts:206-216` 处一次 parse，结果挂到对象上；`PlatformCard.tsx` 四处消费改读已解析字段。**不引入 memo hook**（YAGNI —— 源头 parse 一次即可）。

### 7. minute/5min 聚合

`query_stats.rs:385-460` 是否可走 `stats_agg_hourly`：**先量再改**。小时粒度聚合表天然无法回答分钟粒度，可能确实必须打原始表。此项**先出 EXPLAIN 与耗时数据，再决定改不改** —— 若无法优化，显式记「该项已查，无阻断项」而非硬改。

## 数据流（验证链路）

```
mock 分组灌数据 → 打开 Logs 页翻页
  → sqlite3 mode=ro 跑 EXPLAIN QUERY PLAN 三条查询（用户已批准，纯只读）
  → 改前/改后对比 plan 是否仍出现 SCAN
  → 转发期抓 COUNT 类全扫次数（应为 0）
  → PlatformCard 渲染期 JSON.parse 计数（应为 1/卡）
```

**禁 COUNT 计时**（用户已裁定）：7GB 全扫会灌爆 page cache + 抢 I/O，污染运行实例，且 08 baseline 尚未采。

## 为什么不选别的

| 备选 | 否决理由 |
|---|---|
| 缓存 COUNT 结果 | 日志表持续写入，缓存立刻失效；多一条失效路径 |
| 给 `exclude_sources` 建覆盖索引 | 治标 —— 谓词本就不该恒发，删谓词比建索引便宜 |
| 前端虚拟化长列表 | 页大小才 20，非瓶颈 |
| 先治 log.db 体积（retention/VACUUM） | 独立问题，另开 task；且库缩小不解决 COUNT 全扫的算法性质 |

## 可能性分支（不进当前方案，仅留痕）

- **近似总数** — 触发条件：用户坚持精确页数。可用 `sqlite_stat1` 估算或「COUNT 只在首次进入时算一次并缓存到翻页结束」。代价是数字会不准，需 UI 标注。
- **minute/5min 专用聚合表** — 触发条件：若 §7 的量测证明原始表查询确实是 Stats 的主要延迟源。代价是多一张表 + 一条聚合写路径。
- **Logs 列表游标分页** — 触发条件：若深翻页（offset 很大）成为新瓶颈。当前 offset 分页够用。
