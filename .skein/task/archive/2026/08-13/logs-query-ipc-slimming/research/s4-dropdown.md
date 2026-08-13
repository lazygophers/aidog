# s4 model 下拉改后端 DISTINCT

## 1. 改前调用 + 等价性

`src/pages/Logs/useLogsFilters.ts:57-69`（改前行号）：

```ts
const { items } = await proxyLogApi.listFiltered({ exclude_sources: ["test", "quota"] }, 200, 0);
const col = filterModelType === "actual" ? "actual_model" : "model";
const set = new Set<string>();
(items || []).forEach(l => { if ((l as any)[col]) set.add((l as any)[col]); });
setModelOptions(Array.from(set).sort());
```

关键点：filter 对象**只带 `exclude_sources: ["test","quota"]`**，不携带 `filterPlatform` / `filterGroup` /
`filterTime` / `filterPath` 等当前生效筛选（下拉与主列表筛选条件独立，一直如此，非本次改动引入）。
列选择来自局部 state `filterModelType`（"actual" → actual_model，否则 → model），不是 `ProxyLogFilter.model_type`
字段。取最近 200 行（`limit=200, offset=0`，`ORDER BY created_at DESC`），前端 Set 去重 + sort。

## 2. 选了 ①（严格等价）

新查询保持「最近 200 行内的 distinct」有界近似，子查询语义 1:1 对应旧实现：

```sql
SELECT DISTINCT {col} FROM (
  SELECT {col} FROM proxy_log WHERE deleted_at = 0{where_sql} ORDER BY created_at DESC LIMIT ?
) WHERE {col} != '' ORDER BY {col}
```

`where_sql` 复用 `build_filter_where(&filter)`，`filter` 与旧调用一致仅带 `exclude_sources`。
`{col}` 由新增 `actual: bool` 参数选择（true→actual_model，对应旧 `filterModelType === "actual"`），
不经 `filter.model_type`（旧调用本就未设置该字段，无需引入）。`ORDER BY {col}` 由 SQL 完成排序，
替代前端 `Array.from(set).sort()`。选 ② 会把早已滚出主列表（>200 行外）的历史 model 也列出，
是可见行为变化，未选。

## 3. 新 command + 注册点 + 前端 invoke

- Rust DB 层：`gateway/db/proxy_log.rs::distinct_models_proxy_log(db, filter, actual, limit)`
  （紧跟 `filtered_count_proxy_logs` 之后，`get_proxy_log` 之前）
- Tauri command：`proxy_cmd/proxy_log.rs::proxy_log_distinct_models`（宏 `tauri_command!` 包装，薄转发）
- 注册：`src-tauri/src/startup.rs`（`proxy_log_list_filtered` / `proxy_log_count_filtered` 同批次追加
  `aidog_core::proxy_cmd::proxy_log::proxy_log_distinct_models`）
- 前端封装：`src/services/api/proxy.ts::proxyLogApi.distinctModels(filter, actual, limit=200)`
- 调用点：`src/pages/Logs/useLogsFilters.ts`（`modelOptions` 的 `useEffect`，依赖 `[filterModelType]` 不变）

## 4. EXPLAIN QUERY PLAN（合成库，20000 行随机 5 model × 4 source，`/tmp/s4_bench.db`，只读连接，测完已删）

旧查询（`filtered_list_proxy_logs` 等价 SQL，`exclude_sources=[test,quota]`，`LIMIT 201 OFFSET 0`）：
```
QUERY PLAN
|--SCAN proxy_log
`--USE TEMP B-TREE FOR ORDER BY
```

新查询（`distinct_models_proxy_log`，`actual=false`，`LIMIT 200`）：
```
QUERY PLAN
|--CO-ROUTINE (subquery-1)
|  |--SCAN proxy_log
|  `--USE TEMP B-TREE FOR ORDER BY
|--SCAN (subquery-1)
`--USE TEMP B-TREE FOR DISTINCT
```

内层子查询计划与旧查询完全一致（同样全表 SCAN + TEMP B-TREE ORDER BY，`source_protocol NOT IN`
参数化谓词选不中 partial index，与 s3 已证实的 `EXPLAIN` 结论一致，未新增索引）；外层仅对**已被
LIMIT 200 截断的行集**多一次 TEMP B-TREE DISTINCT（成本从 200 行摊，可忽略）。无回归。

## 5. Payload 前后对比（估算）

- 改前：200 行 `ProxyLogSummary`（15 字段：id/group_key/model/actual_model/source_protocol/
  target_protocol/platform_id/status_code/duration_ms/input_tokens/output_tokens/cache_tokens/
  is_stream/retry_count/created_at），单行 JSON 约 250~350 字节 → 200 行约 **50~70 KB**。
- 改后：`Vec<String>`，去重后典型 model 数量个位数到二十几个，单值约 10~30 字节 → 约 **0.1~1 KB**。
- 降幅约两个数量级，且改后字段窄到与用途完全对齐（不再夹带 status/tokens 等下拉用不到的列）。

## 6. 门禁结果

```
cargo clippy --workspace --all-targets   → 0 errors, 23 warnings（与改前基线持平，未新增）
cargo test --workspace                   → 1628 passed, 4 ignored, 0 failed
yarn build                               → tsc + vite build 成功
yarn test                                → 25 files / 319 tests passed
node scripts/check-i18n.mjs              → ✅ 零缺失（本次改动无新增 i18n key）
```

commit: 见 git log（`refactor(logs): model 下拉改后端 DISTINCT 查询，替代拉 200 行完整日志前端去重`）
