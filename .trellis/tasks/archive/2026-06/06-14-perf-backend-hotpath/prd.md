# perf 后端热路径 + 数据层（问题 1/2/3/6）

> parent: `06-14-deep-perf-optimization`。文件集与 child-B(前端) 不相交，可并行。

## Goal

优化 aidog 后端每请求热路径 + 数据层。只变快、不变行为。worktree 内改，全程串行（共享 `proxy.rs`/`db.rs`）。

## 范围（4 项，按依赖顺序）

### 问题 2 — settings 缓存 + resolve_group 内存 map（先做，风险最低）
- settings(log_settings/lang/sync_settings) 进程内缓存（`ArcSwap` 或 `RwLock<Cache>`），**写时失效**。
- `resolve_group`(`proxy.rs:2175`) 用 group_name→Group 内存 map 替代每请求 `list_groups` 全表查。
- 关键：先 grep 全部 settings/group 写入点，确保失效挂钩无遗漏（遗漏 = 改了不生效）。
- 位置：`db.rs:13`（单连接）、`proxy.rs:429/22/2175`、`calc_est_cost db.rs:1093`。

### 问题 3 — 覆盖索引（零代码风险）
- 新 migration：`CREATE INDEX idx_proxy_log_stats ON proxy_log(created_at, est_cost, input_tokens, output_tokens, cache_tokens, status_code) WHERE deleted_at=0;`
- 放 `src-tauri/migrations/` 下递增编号文件。

### 问题 1 — 渐进式日志激进重构（最大收益，中风险）
- 写入次数 15→3 关键节点：①建立(INSERT 建行) ②上游完成 ③最终。后续节点 UPDATE 增量字段，非 INSERT OR REPLACE 全列重写(`proxy.rs:1844`)。
- `upsert_log` 改接 `&ProxyLog`，消除 `proxy.rs:454` 每次全量 clone；仅 strip 字段时 clone 受影响字段。
- 关键：保证「行已存在才 UPDATE」；验证重构后 proxy_log 各字段仍完整（崩溃留痕语义已与用户确认可牺牲）。

### 问题 6 — 批量 group stats 端点
- 新增后端命令 `get_all_group_usage_stats`：单查 `... GROUP BY group_name` 返回所有 group 聚合 map。
- `services/api.ts` 加 `groupUsageApi.statsAll`。
- `Groups.tsx:242` 改：`Promise.all(N 次)` → 一次 invoke。
- 约束：CLAUDE.md「共享平台不重复计入」——`GROUP BY group_name` 天然满足；balance 仍走平台级。

## Acceptance Criteria

- [ ] `cd src-tauri && cargo build && cargo clippy`（无新 warning）`&& cargo test` 全过。
- [ ] `yarn build` 过（动了 api.ts/Groups.tsx）。
- [ ] before/after 量化：每请求 settings 重查次数（4-6→0 命中缓存）、upsert_log 写入次数(15→3)、clone 计数、Groups invoke 次数(7→1)。
- [ ] 无回归：发一条代理请求后查 proxy_log 该行字段完整；Stats/Groups 统计数值与改前一致。

## Technical Notes

- 全部改动落 worktree `.trellis/worktrees/06-14-perf-backend-hotpath`。
- 顺序：问题 2 → 3 → 1 → 6（缓存先行降风险，日志重构最后做隔离影响）。
- 缓存失效点全集是问题 2 成败关键，先 grep 列全。
