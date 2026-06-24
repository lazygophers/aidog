# PRD — DB 层去 JOIN / 子查询重构

- **Task**: 06-24-db-dejoin-queries
- **Source**: session:claude_47944c72-3a07-498d-9e9b-db2e961f3db4
- **Date**: 2026-06-24
- **盘点依据**: [research/join-subquery-inventory.md](./research/join-subquery-inventory.md)（11 条 J1–J6 + S1–S6，file:line 以 master 5927c02 为准）

## 目标

尽量消除 `src-tauri/src/gateway/db/**` production SQL 里的联表查询（JOIN）、子查询、`IN(SELECT)`、`EXISTS`。替代手段二选一按场景：
- **后端多条 SQL + 内存拼接**（仿 J6 `today_platform_stats` 范例：分步取数 → HashMap/Vec 内存 join）。
- **前端分别获取**（J3 改前端分页触底加载）。

## 用户已裁定边界（AskUserQuestion 2026-06-24）

| 决策点 | 选择 |
|---|---|
| 覆盖难度档 | **全部含 hard**（easy+medium+hard 全做，含 eff_pid 回溯 S2/S3/S5/S6） |
| 落地方式 | **两者混用按场景**：展示型 → 前端分取；写时/聚合 → 后端多 SQL 内存拼 |
| J3 处理 | **前端分页获取，自动触底加载**（反转上个任务的 H6 单 JOIN，改前端无限滚动） |

## 核心设计：eff_pid 去子查询（贯穿 S2/S3/S5/S6）

eff_pid = `platform_id=0`（自动分组日志）时按 `proxy_log.group_key` 在 `"group".auto_from_platform` 回溯源平台 id（记忆 stats-platform-dimension-effpid）。当前是 SQL 标量相关子查询，单一源 `eff_pid_case(col_prefix)`（mod.rs:227）+ stats_agg.rs 两份逐字副本。

**去子查询方案**：把回溯从 SQL 子查询前移到 **Rust 内存解析**。

1. 新增 Rust 解析器（mod.rs，替代 `eff_pid_case`）：
   - `load_auto_from_map(conn) -> HashMap<String, i64>`：一次 `SELECT group_key, CAST(auto_from_platform AS INTEGER) FROM "group" WHERE auto_from_platform != '' AND deleted_at = 0`，建 `group_key → eff_pid` 映射（无 JOIN/子查询，单表全量，"group" 仅 ~15 行）。
   - `resolve_eff_pid(platform_id: i64, group_key: &str, map: &HashMap<String,i64>) -> i64`：`platform_id != 0 → platform_id`；否则 `map.get(group_key).copied().unwrap_or(0)`。纯内存。
2. **写时物化（S5，每请求热路径）**：`upsert_stats_agg` 当前在 SQL VALUES 内联子查询算 eff_pid。改为调用方传入已解析的 `platform_id`（在 Rust 侧用 map 解析），SQL 只存字面值。`proxy/log.rs` 终态写入路径取 map（map 可随 group 写操作失效缓存，或每次写时轻量查——"group" 表小，单表查 sub-ms）。→ stats_agg_hourly.platform_id 仍是 eff_pid，**读路径彻底无回溯**。
3. **读路径（S3 minute/5min 原 proxy_log 路径）**：SQL 保留 WHERE 过滤，去掉 eff_pid 子查询与 LEFT JOIN platform；改为 `SELECT platform_id, group_key, <metrics...> FROM proxy_log WHERE ...`，Rust 侧 `resolve_eff_pid` 逐行算 + 内存 `GROUP BY eff_pid` 聚合 + 内存补平台名（仿 J6）。
4. **回填/rebuild（S6）**：同 map，批量读 proxy_log → 内存算 eff_pid → 批量写。`NOT EXISTS` 空表守卫改 Rust 侧 `SELECT 1 FROM stats_agg_hourly LIMIT 1` 判空。
5. 删除 `eff_pid_case` 及 stats_agg.rs 两份字符串副本（三处副本同步风险一并消除）。

> ⚠️ 风险：S3 把 SQL 聚合搬内存，细粒度大时间窗下 proxy_log 行数可能大。缓解：WHERE 过滤仍在 SQL，仅 eff_pid 维度 GROUP BY 移内存；保留原 LIMIT/范围约束。check 阶段须验语义等价（同输入聚合结果逐字段一致）。

## 逐条改造清单

### 后端多 SQL 内存拼（easy）

- **J1** `group_platform.rs:76` sync_platform_manual_groups：先查 `group_platform`（group_id），再批量 `"group".auto_from_platform`，内存配对。
- **J4** `platform_lifecycle.rs:130` purge auto_disabled：先查本组 `group_platform.platform_id`，再 `SELECT id FROM platform WHERE id IN(...) AND status='auto_disabled' AND deleted_at=0`。
- **J5** `settings.rs:141` list_all_group_platform_pairs：全量 `group_platform` + group/platform 各一条 `id→name` map，内存映射，按 name 排序。
- **S4** `query_stats.rs:216` dimension=platform：去 LEFT JOIN，`GROUP BY platform_id` 聚合 + `SELECT id,name FROM platform` 内存补名（同 J6）。
- **S6-NOTEXISTS** `mod.rs:91`：`NOT EXISTS` 守卫改 Rust 判空。

### 后端多 SQL 内存拼（medium，共用 parse 列序）

- **J2** `group_platform.rs:192` get_group_platforms：拆「group_platform 行（保 ORDER BY priority）」+「按 platform_id 批量取 platform」，内存按 priority 重组。**必须保持 `parse_group_platform_row` 列序（platform 23 列 + priority/weight/level_priority）一致**。
- **S1** `usage_stats.rs:21` recent_health_single：子查询先 `SELECT group_key FROM "group" WHERE auto_from_platform=? AND deleted_at=0` 取列表，再 `proxy_log WHERE platform_id=?1 OR group_key IN(动态?,...) ... LIMIT 5`。必须裸查 proxy_log（聚合表无法重建近 5 条）。

### eff_pid 核心（hard，见上「核心设计」）

- **S2** mod.rs:227 `eff_pid_case` → 删，替换为 `load_auto_from_map` + `resolve_eff_pid`。
- **S3** query_stats.rs:319/334/401/450 minute/5min：去子查询 + LEFT JOIN，内存算 eff_pid + 内存聚合 + 补名。
- **S5** stats_agg.rs:7/91 `upsert_stats_agg`：写时 Rust 解析 eff_pid，SQL 存字面值。
- **S6-effpid** stats_agg.rs:34 / mod.rs:61 回填 rebuild：内存算 eff_pid 批量写。

### 前端分页（J3，反转 H6）

- **J3** `group_platform.rs:236` list_all_group_platforms：
  - 后端新增分页命令（lib.rs + api.ts）：`list_group_platforms_paged(offset, limit)` 或按 group 分页，**无 JOIN** —— 后端单表取 group_platform 页 + 内存补 platform（仿 J6），或前端拿 group 列表后逐页 invoke。
  - 前端 `Groups.tsx`：分组列表改**触底加载**（IntersectionObserver / 滚动到底拉下一页），替代一次性全量。
  - 保留 priority 排序。注意记忆 groups-load-n-plus-1-batch（前端已有 batch group_detail_list）——分页须与现有缓存/失效点协调，不退回逐组 N+1。

## 验收标准

- `cargo build` 通过（block v0.1.6 future-incompat 第三方可忽略，记忆 block-future-incompat-accepted）。
- `cargo clippy` 零新 warning，禁 `#[allow]` 掩盖。
- `cargo test` db 相关全绿：`cargo test usage_stats query_stats stats_agg group_platform stats_today` + eff_pid 相关单测全 pass，断言不削。
- **语义等价**：去 JOIN/子查询前后，stats / 分组详情 / 平台用量 / eff_pid 平台维度聚合结果逐字段一致。eff_pid 写时物化后，读 stats_agg_hourly.platform_id 与改前一致。
- `grep` 自证：production `db/**` 无残留 `JOIN ` / `IN (SELECT` / `EXISTS (` / eff_pid 标量子查询（前端分取/内存拼替代）。豁免项须在 notes 标注理由。
- 前端 J3：`yarn build` 通过，分组列表触底加载可用，无 N+1 回退。
- 主工作区零改动，全部落 worktree。

## 失败处理

- 某项编译/测试失败且 2 次内修不好 → 跳过该项，notes 标 `需要: <问题>`，继续其余项。
- eff_pid 语义不等价 → 优先回退该读路径到原 SQL（保 S5 写时物化），notes 标记。
- clippy 新 warning 必须真修不得 allow。

## 执行编排

文件高度共享（group_platform.rs 被 J1/J2/J3、mod.rs+stats_agg.rs 被 S2/S5/S6、query_stats.rs 被 S3/S4），**串行单 worktree**：
1. **implement-backend**（1 agent，1 worktree）：全后端去 JOIN/子查询（J1/J2/J4/J5/S1 + eff_pid 核心 S2/S3/S5/S6 + S4）。
2. **implement-frontend**（1 agent，同 worktree，backend 后）：J3 前端触底加载 + 后端分页命令。
3. **verify**（1 check agent，同 worktree）：复跑门禁 + 语义等价抽查 + grep 自证。

worktree 隔离走手动绕过（记忆 workflow-worktree-hook-conflict）：main 先 `git worktree add`，agent 无 isolation、prompt 钉死绝对路径。
</content>
</invoke>
