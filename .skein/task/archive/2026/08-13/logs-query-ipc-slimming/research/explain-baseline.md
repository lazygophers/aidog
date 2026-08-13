# EXPLAIN QUERY PLAN 基线 — logs-query-ipc-slimming s1

> 采集时间：2026-07-29。全程只读连接（`sqlite3 "file:~/.aidog/{log,aidog,platform}.db?mode=ro"`），未写用户库。
> log.db 8.7GB / 18174 行 proxy_log；aidog.db 1.7MB / 703 行 stats_agg_hourly；platform.db 96KB（platform 8 行 / group 4 行 / group_platform 20 行）。
> 本文档只做基线勘察与 SCAN 标注，**不改任何查询、不加索引**。

## 1. 查询清单

### 1) Logs 分页查询（log.db）
- **Rust**：`src-tauri/crates/aidog_core/src/gateway/db/proxy_log.rs:344`（`filtered_list_proxy_logs`，取行）+ `:369`（`filtered_count_proxy_logs`，取总数）
- 命令层：`src-tauri/crates/aidog_core/src/proxy_cmd/proxy_log.rs:22`（`filteredListProxyLogs`）/`:32`（`filteredCountProxyLogs`）
- 前端调用点：`src/pages/Logs/useLogsList.ts:25-26` —— `Promise.all([proxyLogApi.listFiltered(...), proxyLogApi.countFiltered(...)])`，`onProxyLogUpdated` 事件驱动，转发期节流窗口 500ms（`useLogsList.ts:36`）→ **COUNT 查询在转发期每 500ms 重跑一次**（与 PRD 目标 1 描述一致）
- 默认 filter：`activeFilter = { exclude_sources: ["test", "quota"] }`（`useLogsFilters.ts:39`）
- 实际 SQL（`build_filter_where` 展开，`proxy_log.rs:493-580`）：
  ```sql
  -- list（取 20 行首屏）
  SELECT id, group_key, model, actual_model, source_protocol, target_protocol, platform_id,
         status_code, duration_ms, input_tokens, output_tokens, cache_tokens, is_stream, retry_count, created_at
  FROM proxy_log
  WHERE deleted_at = 0 AND (source_protocol NOT IN ('test','quota') OR source_protocol IS NULL)
  ORDER BY created_at DESC LIMIT 20 OFFSET 0;

  -- count（每 500ms 轮询）
  SELECT COUNT(*) FROM proxy_log
  WHERE deleted_at = 0 AND (source_protocol NOT IN ('test','quota') OR source_protocol IS NULL);
  ```
  另有 `list_proxy_logs`（`proxy_log.rs:305`，无 filter 变体，供部分测试/e2e 用）：
  ```sql
  SELECT ... FROM proxy_log WHERE deleted_at = 0 ORDER BY created_at DESC LIMIT ?1 OFFSET ?2;
  ```

### 2) Stats 聚合查询（aidog.db 为主，log.db 附带）
- **Rust**：`src-tauri/crates/aidog_core/src/gateway/db/query_stats.rs:167`（`query_stats_inner_agg`，hourly/daily/None 粒度默认路径，命中 `stats_agg_hourly`）
- 命令层：`src-tauri/crates/aidog_core/src/platform_cmd/stats.rs` → `stats_query` / `stats_query_batch`
- 前端调用点：`src/pages/Stats.tsx:169`（`statsApi.query`，默认 range=7d，走 daily/hourly 聚合表路径，非逐行 proxy_log）
- 实际 SQL（`query_stats.rs:209-316`，4 段）：
  ```sql
  -- overview
  SELECT COALESCE(SUM(request_count),0), COALESCE(SUM(success_count),0),
         COALESCE(SUM(sum_input_tokens),0), COALESCE(SUM(sum_output_tokens),0), COALESCE(SUM(sum_cache_tokens),0),
         COALESCE(SUM(sum_duration_ms),0), COALESCE(SUM(sum_est_cost),0.0)
  FROM stats_agg_hourly WHERE deleted_at = 0 AND time_hour >= ?1 AND time_hour <= ?2;

  -- time buckets (daily)
  SELECT substr(time_hour,1,10) AS b, COALESCE(SUM(request_count),0), ...
  FROM stats_agg_hourly WHERE deleted_at = 0 AND time_hour >= ?1 AND time_hour <= ?2 GROUP BY b ORDER BY b;

  -- dimension by-platform / by-group（GROUP BY group_by 维度 LIMIT 50）
  SELECT platform_id AS pid, COALESCE(SUM(request_count),0), ...
  FROM stats_agg_hourly WHERE deleted_at = 0 AND time_hour >= ?1 AND time_hour <= ?2
  GROUP BY platform_id ORDER BY 2 DESC LIMIT 50;
  ```
- **关联发现（附带标注，非本次三条查询之一，但与 Stats/Platforms 卡片健康度共用同一聚合体系）**：
  `usage_stats.rs:364-367`（`platform_usage_stats_all` 的 recent-5 健康度回填）：
  ```sql
  SELECT platform_id, group_key, status_code FROM proxy_log
  WHERE deleted_at = 0 ORDER BY created_at DESC;   -- 无 LIMIT，Rust 侧逐行内存截前 5
  ```
  该查询由 `platform_usage_stats_all`（`usage_stats.rs:295`，命令 `all_platform_usage_stats`，`proxy_cmd/proxy_log.rs:96`）调用，供 Platforms/Home 卡片健康度用，非 Stats 页本体，但同属「聚合查询」族且直接对 log.db 全表排序，一并标注供下游取舍。

### 3) 平台余额聚合（platform.db）
- **Rust**：`src-tauri/crates/aidog_core/src/gateway/db/platform.rs:209`（`list_platforms`）+ `src-tauri/crates/aidog_core/src/gateway/db/group_platform.rs:339`（`list_group_details`，内部 `list_groups` + `list_group_platforms_for_groups` 批量 IN，无 JOIN）
- 命令层：`platform_cmd`/`group` 相关 command（`platformApi.list` / `groupDetailApi.list`）
- 前端调用点：`src/pages/Groups/useGroupData.ts:33-40` —— balance 完全是 **前端内存 reduce**（`platById.get(gp.platform.id)?.est_balance_remaining` 累加），无专用聚合 SQL，只依赖 `list_platforms` + `list_group_details` 两次已有 IPC 的返回数据，无额外 HTTP/SQL。
- 实际 SQL：
  ```sql
  -- list_platforms
  SELECT id, name, ..., est_balance_remaining, ... FROM platform
  WHERE deleted_at = 0 ORDER BY sort_order, created_at;

  -- list_groups
  SELECT ... FROM "group" WHERE deleted_at = 0 ORDER BY sort_order, created_at;

  -- list_group_platforms_for_groups（批量 IN，group_platform.rs:260-266）
  SELECT group_id, platform_id, priority, weight, level_priority
  FROM group_platform WHERE deleted_at = 0 AND group_id IN (?,?,...) ORDER BY group_id, priority;
  ```

## 2. EXPLAIN QUERY PLAN 原文

```
### [1] Logs 分页查询 (log.db, proxy_log 18174 行) ###
-- 1a. filtered_list_proxy_logs（默认 filter exclude_sources=[test,quota]） --
QUERY PLAN
`--SCAN proxy_log USING INDEX idx_proxy_log_stats

-- 1b. filtered_count_proxy_logs（同 filter，转发期每 500ms 轮询一次） --
QUERY PLAN
`--SCAN proxy_log

-- 1c. list_proxy_logs（无 filter 变体） --
QUERY PLAN
`--SCAN proxy_log USING INDEX idx_proxy_log_stats

### [2] Stats 聚合查询 (aidog.db, stats_agg_hourly 703 行；默认 7d daily 粒度) ###
-- 2a. overview (SUM 聚合) --
QUERY PLAN
`--SEARCH stats_agg_hourly USING INDEX idx_stats_agg_time (time_hour>? AND time_hour<?)

-- 2b. time buckets (daily) --
QUERY PLAN
|--SEARCH stats_agg_hourly USING INDEX idx_stats_agg_time (time_hour>? AND time_hour<?)
`--USE TEMP B-TREE FOR GROUP BY

-- 2c. dimension by-platform (GROUP BY platform_id LIMIT 50) --
QUERY PLAN
|--SEARCH stats_agg_hourly USING INDEX idx_stats_agg_time (time_hour>? AND time_hour<?)
|--USE TEMP B-TREE FOR GROUP BY
`--USE TEMP B-TREE FOR ORDER BY

-- 2d. dimension by-group (GROUP BY group_key LIMIT 50) --
QUERY PLAN
|--SEARCH stats_agg_hourly USING INDEX idx_stats_agg_time (time_hour>? AND time_hour<?)
|--USE TEMP B-TREE FOR GROUP BY
`--USE TEMP B-TREE FOR ORDER BY

-- 2e. [附带/关联] platform_usage_stats_all 近5条健康度裸查 proxy_log（无 LIMIT，log.db） --
QUERY PLAN
`--SCAN proxy_log USING INDEX idx_proxy_log_stats

### [3] 平台余额聚合 (platform.db, platform 8 行 / group 4 行 / group_platform 20 行) ###
-- 3a. list_platforms --
QUERY PLAN
|--SCAN platform
`--USE TEMP B-TREE FOR ORDER BY

-- 3b. list_groups --
QUERY PLAN
|--SCAN group
`--USE TEMP B-TREE FOR ORDER BY

-- 3c. list_group_platforms_for_groups（批量 IN） --
QUERY PLAN
|--SEARCH group_platform USING INDEX sqlite_autoindex_group_platform_1 (group_id=?)
`--USE TEMP B-TREE FOR LAST TERM OF ORDER BY
```

### 附：三库现有索引清单

```sql
-- log.db
sqlite_autoindex_proxy_log_1
sqlite_autoindex_stats_agg_hourly_1
idx_proxy_log_model            ON proxy_log(model) WHERE deleted_at = 0
idx_proxy_log_actual_model     ON proxy_log(actual_model) WHERE deleted_at = 0
idx_proxy_log_stats            ON proxy_log(created_at, est_cost, input_tokens, output_tokens, cache_tokens, status_code) WHERE deleted_at = 0
idx_proxy_log_group_key_stats  ON proxy_log(group_key, est_cost, input_tokens, output_tokens, cache_tokens, status_code) WHERE deleted_at = 0
idx_stats_agg_time             ON stats_agg_hourly(time_hour)
idx_stats_agg_platform         ON stats_agg_hourly(platform_id)
idx_proxy_log_status_created   ON proxy_log(status_code, created_at) WHERE deleted_at = 0
idx_proxy_log_platform_created ON proxy_log(platform_id, created_at) WHERE deleted_at = 0
idx_proxy_log_group_created    ON proxy_log(group_key, created_at) WHERE deleted_at = 0
idx_notification_created       ON notification(created_at)

-- aidog.db
sqlite_autoindex_setting_1 / sqlite_autoindex_model_price_1 / sqlite_autoindex_mcp_server_1 / sqlite_autoindex_stats_agg_hourly_1
idx_mw_rule_lookup   ON middleware_rule(enabled, rule_type, scope)
idx_stats_agg_time   ON stats_agg_hourly(time_hour)
idx_stats_agg_platform ON stats_agg_hourly(platform_id)

-- platform.db
sqlite_autoindex_group_1 / sqlite_autoindex_group_2 / sqlite_autoindex_group_platform_1
idx_cli_proxy_group  ON cli_proxy_provider(group_id) WHERE group_id IS NOT NULL
```

## 3. SCAN 标注表

| 查询 | 出现 SCAN 的表 | 该表行数/体积 | 是否有可用索引 | 初判影响 |
|---|---|---|---|---|
| 1a `filtered_list_proxy_logs`（默认 exclude_sources 首屏取 20 行） | `proxy_log` | 18174 行 / 8.7GB | 有 `idx_proxy_log_stats(created_at,...)` 被用上（`SCAN ... USING INDEX`），但因 `source_protocol NOT IN (...) OR IS NULL` 非 sargable，走的是**全索引扫描**而非按 `created_at` 的有界 seek——仍需遍历索引全部 18174 项才能凑够 LIMIT 20 | 每次翻页/刷新都全量扫一遍索引；索引本身不含 `source_protocol`，无法用该列缩小范围 |
| 1b `filtered_count_proxy_logs`（同 filter，转发期每 500ms 轮询） | `proxy_log` | 18174 行 / 8.7GB | **无**——连索引都未命中，落到裸表 SCAN（COUNT 不需要取列值，优化器选了体积更小的路径，但仍是全表遍历） | **PRD 明确点名的头号问题**：转发期每 500ms 一次全表 SCAN，是灌 page cache 的驱动源；且 `source_protocol` 上无索引，`NOT IN` 天然非 sargable |
| 1c `list_proxy_logs`（无 filter 变体，测试/e2e 用） | `proxy_log` | 18174 行 / 8.7GB | 同 1a，`idx_proxy_log_stats` 覆盖 `created_at` 排序，但仍是全索引 SCAN（无 WHERE 缩小范围，OFFSET/LIMIT 前必须扫完） | 生产 Logs 页不走这条（走 filtered 版本），影响面限于测试路径，优先级低 |
| 2a/2b/2c/2d Stats 聚合（`stats_agg_hourly`，overview/buckets/dimension） | 无 SCAN，全部 `SEARCH ... USING INDEX idx_stats_agg_time` | 703 行 / 1.7MB | 有，`idx_stats_agg_time(time_hour)` 命中 range seek | **健康**，无需优化；`GROUP BY`/`ORDER BY` 落 TEMP B-TREE 但源表极小，代价可忽略 |
| 2e `platform_usage_stats_all` 近 5 条健康度（附带发现，log.db） | `proxy_log` | 18174 行 / 8.7GB | 有 `idx_proxy_log_stats`，但**查询整表 `ORDER BY created_at DESC` 且无 LIMIT**，为了取每平台前 5 条，Rust 侧要把 18174 行全部经索引顺序读出再逐行内存截断 | 与 Stats 页无关（供 Platforms/Home 卡片健康度），但是唯一无 LIMIT 的全表遍历，代价可能高于 1a/1b；建议下游一并纳入排查范围（PRD 边界未明确覆盖，标记但不越权处理） |
| 3a `list_platforms` | `platform` | 8 行 / 96KB | 无索引也无妨——8 行全表扫是 SQLite 对小表的正常最优策略 | 可忽略，表本身极小 |
| 3b `list_groups` | `group` | 4 行 | 同上 | 可忽略 |
| 3c `list_group_platforms_for_groups`（批量 IN） | 无 SCAN，`SEARCH ... USING INDEX sqlite_autoindex_group_platform_1 (group_id=?)` | 20 行 | 有，唯一约束索引命中 | 健康，已是批量 IN + 索引 seek，无 JOIN |

## 4. 复跑方式

一条命令重跑全部三条 + 关联发现 + 索引清单：

```bash
.scratch/perf-200mb/assets/explain-baseline.sh
```

已实跑验证（本文档「EXPLAIN QUERY PLAN 原文」段落即该脚本本次输出，逐字一致）。脚本内部用 `date -u -v-7d` (macOS) / `date -u -d '-7 days'` (GNU) 双兼容取 Stats 默认 7 天窗口起点，全程 `?mode=ro` 只读连接，不修改任何库。

## 验收自检

- [x] 三条查询的 EXPLAIN QUERY PLAN 已落盘（本文件第 2 段）
- [x] 当前出现 SCAN 的位置已逐条标注，含所在表 + 行数（第 3 段表格）
- [x] 复跑脚本一条命令可重跑，已实跑一次验证，输出与本文档一致（第 4 段 + 上方脚本执行记录）
- [x] 全程 `mode=ro` 只读，未写用户库；实际连接串：`sqlite3 "file:$HOME/.aidog/log.db?mode=ro"` / `"file:$HOME/.aidog/aidog.db?mode=ro"` / `"file:$HOME/.aidog/platform.db?mode=ro"`
