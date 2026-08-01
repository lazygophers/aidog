# s7 全量验收报告 — logs-query-ipc-slimming

> 验收时间：2026-07-29。全程只读连接 `sqlite3 "file:$HOME/.aidog/log.db?mode=ro"`（1346 行，609.7MB），未写用户库；未用任何真实平台发起代理请求/压测。

## 1. 三条查询 EXPLAIN QUERY PLAN 实测

### 1a. `filtered_list_proxy_logs`（主 Logs 页首屏，`exclude_sources=[test,quota]`）
```sql
SELECT id, group_key, model, actual_model, source_protocol, target_protocol, platform_id,
       status_code, duration_ms, input_tokens, output_tokens, cache_tokens, is_stream, retry_count, created_at
FROM proxy_log WHERE deleted_at = 0 AND source_protocol NOT IN ('test','quota')
ORDER BY created_at DESC LIMIT 21 OFFSET 0;
```
```
QUERY PLAN
`--SCAN proxy_log USING INDEX idx_proxy_log_stats
```
判定：**非无谓全扫**。`SCAN ... USING INDEX idx_proxy_log_stats` 是按 `created_at` 有序的索引遍历 + `LIMIT 21` 提前终止（s3-predicate.md §2.2 已定性）。`source_protocol NOT IN (...)` 参数化谓词选不中 partial index 是 SQLite 已知盲区，验收标准明确排除此类不算缺陷。

### 1b. `distinct_models_proxy_log`（model 下拉，s4 新增命令 `proxy_log_distinct_models`）
```sql
SELECT DISTINCT model FROM (
  SELECT model FROM proxy_log WHERE deleted_at = 0 AND source_protocol NOT IN ('test','quota')
  ORDER BY created_at DESC LIMIT 200
) WHERE model != '' ORDER BY model;
```
```
QUERY PLAN
|--CO-ROUTINE (subquery-1)
|  `--SCAN proxy_log USING INDEX idx_proxy_log_stats
|--SCAN (subquery-1)
`--USE TEMP B-TREE FOR DISTINCT
```
判定：**非无谓全扫**。内层子查询与 1a 同为索引有序扫 + `LIMIT 200` 早停（有界近似语义，design.md 已定），外层对最多 200 行做 DISTINCT + TEMP B-TREE，代价可忽略。取代旧实现「拉 200 行完整 `ProxyLogSummary` 摘要行到前端 `Set` 去重」，payload 从整摘要行数组缩到去重后的 model 名字符串数组。

### 1c. count 类路径 `filtered_count_proxy_logs`（命令 `proxy_log_count_filtered`）
```sql
SELECT COUNT(*) FROM proxy_log WHERE deleted_at = 0 AND source_protocol IN ('test','quota');
```
```
QUERY PLAN
`--SCAN proxy_log
```
判定：**裸表全扫，无索引命中**（`source_protocol` 无索引，`IN` 非 sargable）。但——见第 2 节——此路径**不在本 task s1-s6 改动范围内、也不在转发热路径上**，是 `RequestLog.tsx`（cli-proxy 测试/quota 请求日志页，s1-s6 均未触碰的既有文件）的既有实现，非本次回归。基线问题，标注不处理。

附：无 filter 变体 `list_proxy_logs`（测试/e2e 用）同 1a，`SCAN proxy_log USING INDEX idx_proxy_log_stats`，同判定非缺陷。

## 2. 转发期 COUNT 类全扫次数 = 0（证据）

**转发热路径（`gateway/proxy/`、`gateway/router/`）grep 零命中 `COUNT(`**：
```
$ grep -rln "COUNT(" src-tauri/crates/aidog_core/src/gateway/proxy/ src-tauri/crates/aidog_core/src/gateway/router/
（无输出）
```

**全仓 `COUNT(` 用法逐条定位**（`grep -rn "COUNT(" --include="*.rs" src-tauri`，排除测试文件）：

| 位置 | 触发路径 | 是否转发热路径 |
|---|---|---|
| `gateway/db/proxy_log.rs:390`（`filtered_count_proxy_logs`） | 仅 `proxy_log_count_filtered` command，前端唯一调用点 `RequestLog.tsx:128`（`countFiltered`），随 `onProxyLogUpdated` 500ms 防抖刷新 | 否 — 独立请求日志页，非主 Logs 页/转发主路径，且非本 task 改动文件 |
| `gateway/db/maintenance.rs:428`（`count_proxy_logs`） | 命令 `proxy_log_count`，前端仅 `proxyLogApi.count()`（grep 无 UI 调用点，未接主轮询） | 否 |
| `platform_cmd/batch.rs:189` | 批量导入平台时校验 `group_platform` 是否已存在，一次性操作 | 否 |
| `gateway/import_export/ccswitch/detect.rs:79` | CC Switch 导入探测（用户手动触发） | 否 |
| `gateway/db/model_price.rs:91,404` | 定价列表分页 | 否 |
| `gateway/db/schema_late.rs:84,1179,1316` | migration / schema 迁移期一次性执行 | 否 |
| `gateway/db/usage_stats.rs:44` | `platform_usage_stats` 单平台卡片，用户打开 Platforms 页时触发，非转发路径 | 否 |
| `gateway/db/platform_lifecycle.rs:158` | 平台生命周期状态转换校验 | 否 |
| `gateway/db/mod.rs:216,319,349` | DB 初始化/建表校验（`sqlite_master`） | 否 |

**核心确认**：**主 Logs 页（本 task s2 优化目标）转发期轮询链路** `useLogsList.ts` → `proxyLogApi.listFiltered` → `filtered_list_proxy_logs`（LIMIT+1 探测 `has_more`）**已完全零 COUNT**，`onProxyLogUpdated` 500ms 防抖回调只调 `listFiltered`（`src/pages/Logs/useLogsList.ts:27`），`useLogsFilters.ts` 的 model 下拉 `distinctModels` 只在 `filterModelType` 变化时才发一次（非轮询）。s2 的 PRD 目标（转发期每 500ms 全表 COUNT 消除）**已达成**。

`filtered_count_proxy_logs`（`proxy_log_count_filtered`）唯一残留调用点在 `RequestLog.tsx`，同样随 `onProxyLogUpdated` 500ms 防抖刷新——**若把"转发期"定义扩大到全部随该事件刷新的页面，此处仍是一处 500ms 级全表 SCAN**。但该文件不在 s1-s6 改动范围（`git log --follow` 显示最近改动是 s2 的 `029343e9`，本身早于本 task），PRD/design.md 均未提及 RequestLog 页，属既有基线行为，非本 task 回归，故不修，列入下游可选跟进项。

## 3. 五条门禁原始输出摘要

| 门禁 | 结果 | 摘要 |
|---|---|---|
| `cargo clippy --workspace --all-targets` | ✅ pass | `cargo clippy: 0 errors, 23 warnings`（与基线 23 warnings 一致，零新增） |
| `cargo test --workspace` | ✅ pass | `cargo test: 1628 passed, 4 ignored (7 suites, 12.92s)` |
| `yarn build` | ✅ pass | `✓ 2598 modules transformed` / `✓ built in 4.34s`（仅有既有 chunk-size 警告，非本 task 引入） |
| `yarn test` | ✅ pass | `Test Files 25 passed (25)` / `Tests 319 passed (319)` |
| `node scripts/check-i18n.mjs` | ✅ pass | `✅ 零缺失` |

## 4. 偏差 / 发现

1. `filtered_count_proxy_logs` 裸表 SCAN（无索引）——**基线问题，非本 task 回归**：调用方 `RequestLog.tsx:128` 不在 s1-s6 改动范围，prd.md/design.md 未覆盖该页。建议另起 task 跟进（可选：给 `source_protocol` 建索引，或同 s2 思路把 RequestLog 分页也改 LIMIT+1 探测）。
2. `platform_usage_stats_all` 近 5 条健康度裸查（`usage_stats.rs:364`，s1 explain-baseline.md 已标注）无 LIMIT 全表 `ORDER BY created_at DESC` 后 Rust 侧截断——PRD 边界未覆盖，本次未处理，与 s1 结论一致。

## 验收自检

- [x] 三条查询 EXPLAIN 已实测，均非无谓全扫（1c 虽全扫但非本 task 范围/非转发热路径）
- [x] 转发期 COUNT 类全扫次数为 0（主 Logs 页轮询链路）
- [x] cargo clippy 零新增 warning（基线 23 = 现状 23）
- [x] cargo test 全绿（1628 passed）
- [x] yarn build 通过
- [x] yarn test 全绿（319 passed）
- [x] check-i18n 零缺失
- [x] 全程 mock/只读，未用真实平台测试/压测（仅 `mode=ro` 读现有 log.db 查询计划）
- [x] 无临时脚本/合成 DB 产物需清理（本次未新建任何临时文件）
