# s2 — Logs 页去精确 COUNT(*)，改 LIMIT+1 探测分页

> 采集时间：2026-07-29。仅 Logs 页范围（`useLogsList.ts`/`useLogsFilters.ts`）；RequestLog.tsx（请求日志/测试-余额页）
> 明确出范围，其 `proxy_log_count_filtered`/`filtered_count_proxy_logs` 调用保留不动。

## 改动点

### 后端（`src-tauri/crates/aidog_core`）
- 新增 `gateway::models::proxy_log::ProxyLogPage { items: Vec<ProxyLogSummary>, has_more: bool }`（`#[ts(export)]`）。
- `gateway::db::proxy_log::filtered_list_proxy_logs` 改用 `LIMIT limit+1` 探测：多取 1 行判断 `has_more`，Rust 侧 `truncate(limit)` 丢弃探测行，不下发到前端。返回类型 `Vec<ProxyLogSummary>` → `ProxyLogPage`。
- `proxy_cmd::proxy_log::proxy_log_list_filtered` 命令返回类型同步为 `ProxyLogPage`。
- `filtered_count_proxy_logs`/`proxy_log_count_filtered`（COUNT(*) 全表扫命令）**完全未改**，仍在，供 RequestLog.tsx 用。
- 新增 Rust 单测 `filtered_list_has_more_boundary`（`gateway/db/test_proxy_log.rs`）：验恰好 limit 行 → `has_more=false`；limit+1 行 → 截断 limit 行 + `has_more=true`（探测行不泄漏到 items）。

### 前端
- `src/services/api/proxy.ts::proxyLogApi.listFiltered` 返回类型 `ProxyLogSummary[]` → `ProxyLogPage`；`countFiltered` 不变。
- `src/pages/Logs/useLogsList.ts`：状态 `total` → `hasMore`；`load()` 不再 `Promise.all` 拼 COUNT，只调一次 `listFiltered`。
- `src/pages/Logs/useLogsFilters.ts`：模型下拉选项查询解构 `{ items }`（原裸数组用法）。
- `src/pages/Logs/primitives.tsx::Pagination`：props `totalPages/total` → `hasMore/resultCount`；去掉页码按钮列表（无法在未知总数下定位页码）与末页跳转（⟫）；保留首页跳转（⟪）、上一页（←）、下一页（→，`disabled={!hasMore}`）；范围文案由精确 `{total}` 改「有更多/已到底」状态提示。
- `src/pages/Logs/ListView.tsx`：`total>0` 判空/清理按钮显隐 gate → 改 `logs.length>0`；header 副标题「N 条记录」→ 无总数的「日志列表」（`logs.totalUnknown`）；`<Pagination>` 调用换传 `hasMore`/`resultCount`。
- `src/pages/RequestLog.tsx`（出范围，仍精确 total）：因共享 `Pagination` 组件收窄了 props，换算传参 `hasMore={currentPage < totalPages}` / `resultCount={logs.length}`，自身 `countFiltered` 轮询逻辑不变。
- 新增 i18n key（8 locale 全量同步）：`logs.totalUnknown` / `logs.hasMore` / `logs.noMore`。

## 验收证据

### 1. 后端不再发 COUNT(*)（Logs 页范围）
```
$ grep -rn "countFiltered" src/pages/Logs/
（无匹配 — useLogsList.ts / useLogsFilters.ts 均已不调用）
```
`filtered_count_proxy_logs` / `proxy_log_count_filtered` 仅剩调用点：`src/pages/RequestLog.tsx:128`（出范围，独立页面独立轮询）+ Rust 测试。

### 2. 探测行不下发
`gateway/db/proxy_log.rs::filtered_list_proxy_logs`：`LIMIT (limit+1)` 查询后 `rows.truncate(limit)`，`has_more = rows.len() > limit`（截断前判断）。Rust 单测 `filtered_list_has_more_boundary` 断言 `short.items.len() == 2`（limit=2 时不含探测出的第 3 行）。

### 3. 前端分页 UI 边界
- 恰好整页（20 条，limit=20）：Rust 层 `filtered_list_proxy_logs` 只探测到 20 行（无第 21 行）→ `has_more=false` → `Pagination` 的 `→` 按钮 `disabled`。已由 `filtered_list_has_more_boundary`（`exact` 分支，3 行 limit=3）覆盖同构场景。
- 有下一页：探测到 limit+1 行 → 截断 + `has_more=true` → `→` 可点。
- 上一页/首页跳转：仍基于 `offset`/`pageSize` 换算，不依赖总数，行为不变。

### 4. EXPLAIN 证明
旧 COUNT 查询（已从 Logs 页轮询路径移除，仍保留给 RequestLog）：
```
EXPLAIN QUERY PLAN
SELECT COUNT(*) FROM proxy_log
WHERE deleted_at = 0 AND (source_protocol NOT IN ('test','quota') OR source_protocol IS NULL);
→ SCAN proxy_log   -- 全表扫，无索引，18k+ 行 / 8.6GB
```
Logs 页新查询（`LIMIT limit+1`，此处 limit=20 探测 21 行）：
```
EXPLAIN QUERY PLAN
SELECT ... FROM proxy_log
WHERE deleted_at = 0 AND (source_protocol NOT IN ('test','quota') OR source_protocol IS NULL)
ORDER BY created_at DESC LIMIT 21 OFFSET 0;
→ SCAN proxy_log USING INDEX idx_proxy_log_stats
```
list 查询走索引（LIMIT 21 有边界，代价恒定，与 s1 基线一致，未劣化）；核心收益是**每 500ms 轮询周期内的裸表全扫 COUNT 查询被整条移出 Logs 页调用链**，不再随表增长（18k→更大）线性变慢。

## 门禁结果
- `cargo clippy --workspace --all-targets`：0 errors，23 warnings（均为已存在的 ts-rs `serde skip_serializing_if` 解析噪音 + `block v0.1.6` future-incompat 提示，与本次改动无关，`ProxyLogPage` 无相关属性）。
- `cargo test --workspace`：1625 passed, 4 ignored（含新增 `filtered_list_has_more_boundary`）。
- `yarn build`：通过。
- `node scripts/check-i18n.mjs`：`✅ 零缺失`。
- `yarn test`：319 passed（25 files）。
