# Research: est_cost 日聚合可用性

- **Query**: proxy_log est_cost 字段 + 现有 usage stats 查询；是否已有"最近 N 天每天用量"；schema 能否支持按天 GROUP BY；Platform/BalanceInfo 相关字段
- **Scope**: internal
- **Date**: 2026-06-13

## Findings

### proxy_log.est_cost schema（按天/小时聚合可行）

- 列定义 `PROXY_LOG_COLUMNS`（`src-tauri/src/gateway/db.rs:998`）含 `est_cost`(第 24 列, REAL) + `created_at`(第 25 列) + `deleted_at`。
- `est_cost` 列由 migration 加（db.rs:101 `ALTER TABLE proxy_log ADD COLUMN est_cost REAL NOT NULL DEFAULT 0`）。
- **`created_at` 是 unix 毫秒**（写入 `chrono::Utc::now().timestamp_millis()`，参见 `get_group_spent_since` db.rs:1330 用 `timestamp_millis()` 比较；聚合查询里 `created_at/1000` 转秒喂 strftime，db.rs:1444）。
- est_cost 持久化保证：`upsert_log` 在 tokens>0 且 est_cost==0 时用 `calc_est_cost` 回填（proxy.rs:323-339），`calc_est_cost`（db.rs:584+）走 `resolve_price` 回退链保证非 0（pricing-resolve-single-source 记忆）。
- retention：retention_days 删整行（db.rs:1202），user/upstream retention 只清字段（db.rs:1216/1232）—— **est_cost/created_at 不被字段清理清空，只随整行 90 天删除**，7 天聚合不受影响。

### 现有 usage stats 查询（都是窗口 SUM，非每日 GROUP BY）

| 函数 | 文件:行 | 聚合方式 |
|---|---|---|
| `today_stats` | db.rs:520-571 | 本地午夜起 `SUM(est_cost)` 单值（db.rs:553），**今日累计，非每日序列** |
| `get_group_usage_stats` | db.rs:1313-1321 | group 全量 `usage_stats()` 单值（含 total_cost） |
| `get_platform_usage_stats` | db.rs:1300-1311 | platform 全量单值 |
| `get_group_spent_since(db,group,N)` | db.rs:1327-1344 | **近 N 天 `SUM(est_cost)` 单值**（一个数，非按天） |
| `usage_stats()` (helper) | db.rs:~1261 | `SUM(est_cost)` 单值 |

`get_group_spent_since`（db.rs:1334-1339）：
```sql
SELECT COALESCE(SUM(est_cost), 0.0) FROM proxy_log
 WHERE group_name = ?1 AND created_at >= ?2 AND deleted_at = 0
```
注释明确用途："statusline group-info 端点推算「余额可用天数」：日均花费 = spent / N"（db.rs:1323-1325）。**即当前"日用量"= 7天总和/7，固定除 7，无"无7天回退近1天"、无小时精度。**

### 已有"按天 GROUP BY"查询 —— `query_stats` 时间桶（关键）

`query_stats_inner`（db.rs:1378-1469）支持时间分桶聚合，**这是唯一现成的"每天/每小时用量"查询**：

- 时间桶 SQL（db.rs:1443-1450）：
  ```sql
  SELECT strftime('{time_fmt}', created_at/1000, 'unixepoch'), COUNT(*), ...,
         COALESCE(SUM(est_cost), 0.0)
   FROM proxy_log WHERE deleted_at = 0 AND {where_sql} GROUP BY 1 ORDER BY 1
  ```
- 粒度 `time_fmt`（db.rs:1407-1410）：`granularity="hourly"` → `%Y-%m-%d %H:00`（**小时桶**），否则 `%Y-%m-%d`（**天桶**）。
- 过滤支持 `filter_group`（db.rs:1394-1395，绑定 `group_name`）、filter_model、filter_protocol，时间窗 `created_at>=start AND <=end`（db.rs:1393）。
- 每桶 `total_cost = SUM(est_cost)`（`StatsBucket`，db.rs:1465）。

→ **schema + 现成查询完全支持按天/按小时 GROUP BY 算近 7 天每日 est_cost**。可直接用 `query_stats(StatsQuery{ filter_group, start=now-7d, granularity:"daily" })` 拿 7 个天桶；或 `granularity:"hourly"` 拿小时桶做"精确到小时"。

### Platform 结构相关字段
`Platform`（`src-tauri/src/gateway/models.rs:325+`）/ TS `Platform`（`src/services/api.ts:154`）:
- `est_balance_remaining: f64`（models.rs:347 / api.ts:169）—— 预估余额，group-info `balance` 主来源（proxy.rs:263）。
- `est_coding_plan: String`（models.rs:350 / api.ts:171）—— EstCodingPlan JSON。
- `last_real_query_at: i64`（models.rs:353 / api.ts:173）—— 上次真查时间戳。
- `estimate_count: i64`（models.rs:356）。
- `manual_budgets`（models.rs，parse db.rs:128 列）—— total 类计入 balance，窗口类压成 coding tier。

### quota.rs BalanceInfo 字段（`src-tauri/src/gateway/quota.rs:33-44`）
`remaining: f64` / `total: Option<f64>` / `used: Option<f64>` / `currency: String` / `is_valid: bool`。多数平台 `total: None`（如 quota.rs:153/170/194/235），仅少数返回 total（如 OpenRouter total_credits，quota.rs:215）。

## Caveats / 结论

- **"近 7 天每日用量"查询：现有 `get_group_spent_since` 只给单值（总和/7），但 `query_stats` 时间桶 SQL 已能按天/按小时 GROUP BY est_cost —— 无需新增 SQL，可复用 `query_stats(filter_group, granularity)`。** 唯一需新增的是"日用量算法包装"：近 7 天有数据按 7 天均值、不足 7 天用近 1 天、精确到小时——这是上层逻辑，底层桶查询已具备。
- 若不想引入 query_stats（它带 overview+dimension 较重），最小改造可在 db.rs 新增一个轻量"按天/小时分桶 SUM(est_cost) by group"查询，schema 完全支持。
- 余额"剩余天数 = balance / 日用量"链路后端已有雏形（group-info），但日用量算法（7天均值/回退1天/小时精度）需重写。
- 多数平台 `BalanceInfo.total=None` → 前端 BalanceBar 无分母 → 现有占比色失效，正好支撑改"按速率/剩余天数"上色的需求。
