# exclude_sources NOT IN 谓词恒真判定 — logs-query-ipc-slimming s3

> 基底：`029343e9`（s2，COUNT→LIMIT+1 探测）已在 HEAD。本文档基于该基底复核。

## 1. 恒真性判定：**不恒真，谓词是真实过滤，不能跳过**

design.md §2「恒发谓词 → 可绕过」的前提站不住：

- 前端默认值：`src/pages/Logs/useLogsFilters.ts:39` `activeFilter = { exclude_sources: ["test", "quota"] }`（无条件常量，模型下拉分支 `:62` 同值）。
- 后端处理：`gateway/db/proxy_log.rs:564-580`（`build_filter_where`）对非空 `exclude_sources` 拼 `AND source_protocol NOT IN (...)`，是真实生效的参数化过滤，非空判恒真。
- **关键事实**：`source_protocol='test'` / `'quota'` 会被真实写入 `proxy_log` 表：
  - `src-tauri/crates/aidog_core/src/ai_tools_cmd/model_test.rs:157` `source_protocol: "test".into()`
  - `src-tauri/crates/aidog_core/src/gateway/quota/http.rs:187` `source_protocol: "quota".into()`（该文件 `:129` 注释：「所有 quota 出站 HTTP 经此单点，落 proxy_log」）
  - 两者是日常操作触发的正常路径（模型测试面板、quota 探测轮询），不是边角场景。

  跳过该谓词会让这两类请求泄漏进 Logs 主页列表——`useLogsFilters.ts:37-38` 的注释已明说「Logs 主页默认排除 test/quota 两类（已迁到 RequestLog 新页）」，这是**故意的产品行为**，不是可优化掉的冗余判断。

- 两侧一致性：前端恒发 `["test","quota"]`（`useLogsFilters.ts:39`），后端恒对非空 exclude_sources 生效过滤（`proxy_log.rs:564`）——两侧行为一致，**无需对齐改动**。

**结论**：不删/不跳过该谓词。design.md 里「恒发谓词→可绕过」这条方案条目本身基于错误前提（把「恒发」误判为「恒真/可跳过」），已在本 subtask 范围内否决并留痕于此，未改 design.md 本体（`.skein/task/.../design.md` exec 阶段禁动，按约束原样保留，仅在此研究文档标注结论）。

## 2. 改法：保留谓词 + 死分支简化，**不加新索引**（EXPLAIN 证明无收益）

### 2.1 保留但简化：删 `OR source_protocol IS NULL`

`source_protocol` 列在 `schema_early.rs:119` 声明为 `TEXT NOT NULL DEFAULT ''`（log.db 建表的唯一真值源，`CREATE TABLE IF NOT EXISTS`），DB 级约束保证该列永不为 NULL。原代码里 `OR source_protocol IS NULL` 分支是防御性但**可证伪的永假分支**（`NULL NOT IN (...)` 返 NULL 被 WHERE 过滤掉的场景在此表上不可能触发）。删除该分支不改变任何结果集，纯简化。

改动：`src-tauri/crates/aidog_core/src/gateway/db/proxy_log.rs:564-580`
```sql
-- 改前
AND (source_protocol NOT IN (?1, ?2) OR source_protocol IS NULL)
-- 改后
AND source_protocol NOT IN (?1, ?2)
```
占位符数量、`idx` 递增逻辑不变（`idx += srcs.len()` 仍在同一 if 块内，未受影响）。

### 2.2 索引：EXPLAIN 证明现状已够用，新加索引无收益

**关键发现（本仓 SQLite 行为验证）**：`NOT IN (?1, ?2)` 用**绑定参数**表达时，SQLite 查询规划器**无法**用「字面量匹配」的方式证明 partial index 谓词蕴含关系——只有 SQL 文本里的**字面量常量**（如 `NOT IN ('test','quota')`）才能被 partial index 选中；一旦是 `?` 占位符，即使建了完全对应的 `CREATE INDEX ... WHERE deleted_at=0 AND source_protocol NOT IN ('test','quota')`，规划器也不会选中它（本地空库验证：字面量查询走 `SCAN ... USING INDEX idx_pv`，占位符查询退化为 `SCAN proxy_log` + `TEMP B-TREE FOR ORDER BY`，见下方复现记录）。而本仓 `build_filter_where` 出于安全考虑（禁字符串拼接用户输入）全程走 `rusqlite` 参数绑定，不可能改成字面量拼接。

于是候选的「给 source_protocol 建索引」方案实测无收益：

- 现有 `idx_proxy_log_stats(created_at, est_cost, input_tokens, output_tokens, cache_tokens, status_code) WHERE deleted_at=0`（`schema_early.rs`）因 `deleted_at = 0` 是 SQL 文本里的**字面量**（`proxy_log.rs:364` `"...WHERE deleted_at = 0{where_sql}..."`），已被 partial index 匹配命中，`ORDER BY created_at DESC` 直接吃索引序，配合 `LIMIT` 提前终止——这就是 s1 baseline 里看到的 `SCAN proxy_log USING INDEX idx_proxy_log_stats`（"SCAN" 字样不代表全表读完，是索引有序遍历 + 提前止损）。
- 合成库验证（5万行，2% 穿插 test/quota，模拟真实稀疏分布）：现有索引下 `filtered_list_proxy_logs` 查询 **0ms**（`Run Time: real 0.000`），提前终止生效。
- 极端场景（最新 5000 行连续全是 test/quota，人为制造最坏情况——真实场景不会出现，quota 轮询是周期性穿插而非连续批量写入）：仍仅 **1ms**（`Run Time: real 0.001`），可忽略。
- 新建 `CREATE INDEX ... ON proxy_log(source_protocol, created_at) WHERE deleted_at=0` 后，`EXPLAIN QUERY PLAN`（占位符查询）**仍走** `idx_proxy_log_stats`，新索引未被选中——验证了上一段的参数化盲区结论，新索引对本查询是纯写放大成本、零收益。

**决策：不加索引**（design.md §3「先用 EXPLAIN 证明确实被选中再决定」的判据已跑出「不被选中」的结果，对应否决）。

## 3. idx 占位符自证

未引入新的「跳过分支」——`exclude_sources`/`sources` 的跳过路径本就已存在（空 Vec/None → 不进 if 块 → 不拼占位符 → `idx` 不递增，天然对齐），本次改动只是删掉非空分支内的 `OR IS NULL` 子句，`idx += srcs.len()` 位置未动。

测试覆盖：
- 既有 `empty_sources_is_noop_binds_ok`（`proxy_log.rs` test 区）：验证空 Vec 跳过不发 `IN ()`。
- 既有 `all_filters_plus_sources_binds_ok`：穷举全标量 + sources + exclude_sources + cli_proxy_provider_id 链路。
- **新增** `empty_exclude_sources_then_cli_proxy_provider_id_binds_ok`：显式覆盖「跳过分支（空 exclude_sources/sources）+ 后续参数化分支（cli_proxy_provider_id）」组合，锁死本次改动可能触碰的占位符对齐风险。

三者均通过真实 sqlite `Connection` 执行校验占位符数与 bind 数一致（`assert_binds_ok` helper，`query_row` 失败即 panic）。

## 4. EXPLAIN 对照

全程用合成库（`/tmp/logs_explain_synth.db`，测试完已删除，未碰用户库；user 库当时仅 501 行不足以代表真实分布，故按 team-lead 要求造合成库）。

```
-- 改前 = 改后（谓词逻辑等价，仅去掉恒假的 OR IS NULL 子句，SCAN/SEARCH 选择不变）：
EXPLAIN QUERY PLAN SELECT id FROM proxy_log
WHERE deleted_at = 0 AND source_protocol NOT IN (:a,:b)
ORDER BY created_at DESC LIMIT 21 OFFSET 0;
→ SCAN proxy_log USING INDEX idx_proxy_log_stats

-- 加候选新索引 idx_try(source_protocol, created_at) WHERE deleted_at=0 后，
-- 同一占位符化查询：
→ SCAN proxy_log USING INDEX idx_proxy_log_stats   -- 新索引未被选中，无变化
```

即「改前」「改后」EXPLAIN 计划完全一致（删除死分支不改变 SQL 的可优化结构），且尝试新增索引对该计划无影响——两份对照均已给出。

## 5. 门禁结果

```
cd src-tauri && cargo clippy --workspace --all-targets   → 0 errors, 23 warnings（既有，未新增）
cd src-tauri && cargo test --workspace                    → 1626 passed, 4 ignored
yarn build                                                 → 通过
yarn test                                                  → 319 passed (25 files)
node scripts/check-i18n.mjs                                → ✅ 零缺失
```

commit: 见 git log（本 subtask 提交）。

## 验收自检

- [x] 默认过滤下 SQL **仍含** `NOT IN` 段——已证明它不恒真（§1），保留是正确决策，非遗漏
- [x] 未引入新跳过分支，既有跳过路径 idx 对齐——新增测试 `empty_exclude_sources_then_cli_proxy_provider_id_binds_ok` 覆盖
- [x] `useLogsFilters.ts` 默认值（`:39`/`:62`）与后端 `build_filter_where`（`proxy_log.rs:564`）行为一致，两侧均已给出 file:line，无需改动对齐
- [x] 索引加/不加均有 EXPLAIN 依据：现有索引已提前终止（0ms），候选新索引验证不被参数化查询选中（1ms 极端场景亦可忽略）→ 不加
