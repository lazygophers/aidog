# Research: aidog 后端 DB 层 JOIN / 子查询盘点

- **Query**: 精确盘点 `src-tauri/src/gateway/db/**` production Rust 代码里所有真实 SQL JOIN（LEFT/INNER）、相关/标量子查询、`IN (SELECT)`、`EXISTS`，产出去 JOIN/子查询重构清单
- **Scope**: internal（只读）
- **Date**: 2026-06-24

## 方法与自证

多轮 grep（`JOIN ` / `(SELECT` / `IN (SELECT` / `EXISTS (` / `auto_from_platform`）覆盖 `db/**` + `gateway/*.rs` 顶层。逐一 Read 命中行的实际 SQL 字符串，剔除注释、Rust `.join(`、字段名命中、test_*.rs（test 文件仅复刻被测 SQL，不单列）。

**production SQL 命中文件**：group_platform.rs、usage_stats.rs、query_stats.rs、settings.rs、stats_today.rs、stats_agg.rs、mod.rs、platform_lifecycle.rs。其余文件（platform.rs 等）的命中均为注释或字段名，无真实 JOIN/子查询。

eff_pid 回溯标量子查询有**单一事实源** `eff_pid_case()`（mod.rs:227）与两份逐字副本字符串（stats_agg.rs `AGG_EFF_PID_EXPR`、回填 schema SQL、upsert SQL），下面按出现位置分别登记。

---

## 条目清单

### J1 — `[group_platform.rs:76-78]` `sync_platform_manual_groups` 取平台当前所在组及 auto 标记

- **SQL 类型**: INNER JOIN（`group_platform gp JOIN "group" g`）
- **涉及表 / 关联键**: `group_platform.group_id = "group".id`；WHERE `gp.platform_id=?1 AND gp.deleted_at=0 AND g.deleted_at=0`
- **调用频率**: 手动（platform_update 保存平台时）
- **去 JOIN 可行性**: **easy** — 先查 `group_platform` 拿 `(group_id)` 列表，再按 id 批量查 `"group".auto_from_platform`（或单条），内存配对。非热路径、行数极小。

### J2 — `[group_platform.rs:192-194]` `get_group_platforms` 单组的平台详情列表

- **SQL 类型**: INNER JOIN（`group_platform gp JOIN platform p`）
- **涉及表 / 关联键**: `gp.platform_id = p.id`；WHERE `gp.group_id=?1 AND gp.deleted_at=0 AND p.deleted_at=0 ORDER BY gp.priority`
- **调用频率**: 页面加载（分组详情 / get_group_detail 走它）
- **去 JOIN 可行性**: **medium** — 拆成「查 group_platform 行（priority/weight/level_priority/platform_id，保 ORDER BY priority）」+「按 platform_id 批量取 platform」两步，内存按 priority 重组。需保序与 23 列 platform 解析（`parse_group_platform_row` 列序 2..24）一致，是 medium 主因。

### J3 — `[group_platform.rs:236-238]` `list_all_group_platforms` 全组平台关联一次取（N+1 batch 版）

- **SQL 类型**: INNER JOIN（`group_platform gp JOIN platform p`）
- **涉及表 / 关联键**: `gp.platform_id = p.id`；WHERE `gp.deleted_at=0 AND p.deleted_at=0 ORDER BY gp.group_id, gp.priority`；末列 `gp.group_id` 用于内存分桶
- **调用频率**: 页面加载（分组列表 list_group_details 批量，见记忆 groups-load-n-plus-1-batch）
- **去 JOIN 可行性**: **medium** — 与 J2 同构，可拆「全量 group_platform 行」+「全量 platform map」内存 join 分桶。注意此 JOIN 本身就是为消除逐组 N+1 引入的；去 JOIN 改两条全表查 + HashMap join 语义等价、行数可控，但仍需复刻 26 列解析与双重排序，故 medium。

### J4 — `[platform_lifecycle.rs:130-133]` 分组级 purge：列本组内 auto_disabled 平台 id

- **SQL 类型**: INNER JOIN（`platform p JOIN group_platform gp`）
- **涉及表 / 关联键**: `gp.platform_id = p.id`；WHERE `gp.group_id=?1 AND gp.deleted_at=0 AND p.status='auto_disabled' AND p.deleted_at=0`
- **调用频率**: 手动（分组内清理被禁用平台）
- **去 JOIN 可行性**: **easy** — 只取 `p.id`。可先查本组 `group_platform.platform_id`（活跃），再 `SELECT id FROM platform WHERE id IN (...) AND status='auto_disabled' AND deleted_at=0`（两条等值/IN 查）。纯 id 过滤、非热路径。

### J5 — `[settings.rs:141-144]` `list_all_group_platform_pairs` 导入导出用 (组名,平台名) 全量

- **SQL 类型**: INNER JOIN ×2（`group_platform gp JOIN "group" g JOIN platform p`）
- **涉及表 / 关联键**: `g.id=gp.group_id`、`p.id=gp.platform_id`；WHERE `gp.deleted_at=0 ORDER BY g.name, p.name`
- **调用频率**: 手动（导入导出）
- **去 JOIN 可行性**: **easy** — 拆「全量 group_platform 行」+「id→name map（group / platform 各一条全表）」内存映射，按 name 排序。非热路径、纯展示名拼接。

### J6 — `[stats_today.rs:104-127]` `today_platform_stats` 当日各平台用量（聚合表 + 平台名 map）

- **SQL 类型**: 无 JOIN/子查询（**已经是去 JOIN 范例**）。两条独立 SQL：`SELECT ... FROM stats_agg_hourly GROUP BY platform_id` + `SELECT id,name FROM platform`，内存 HashMap 拼名。
- **涉及表**: stats_agg_hourly（platform_id 已是 eff_pid）、platform
- **调用频率**: 页面加载 / tray 浮窗（popover today stats）
- **去 JOIN 可行性**: **N/A（已拆）** — 登记作为目标形态参照，无需重构。

### S1 — `[usage_stats.rs:21]` `recent_health_single` 最近 5 条健康度 eff_pid 回溯

- **SQL 类型**: `IN (SELECT)` 相关-成员子查询（`group_key IN (SELECT group_key FROM "group" WHERE auto_from_platform=?2 ...)`），且外层再套标量子表 `FROM (SELECT status_code FROM proxy_log WHERE ... LIMIT 5)`
- **涉及表 / 关联键**: proxy_log 直查 + `"group".auto_from_platform=?2`（=平台 id 十进制串）→ group_key 集合
- **调用频率**: **热路径偏热** — `get_platform_usage_stats` 内每平台调一次（平台卡片用量刷新）；非每代理请求，但页面/卡片刷新批量触发
- **去 JOIN 可行性**: **medium** — 子查询可先 `SELECT group_key FROM "group" WHERE auto_from_platform=? AND deleted_at=0` 取 group_key 列表，再 `proxy_log WHERE platform_id=?1 OR group_key IN (?,?,...)`（参数化 IN）。语义等价，但 IN 列表需动态绑定 + 外层 LIMIT 5 子表保留，故 medium。注释明确「聚合表丢失请求级顺序无法重建近 5 条」——此处必须裸查 proxy_log。

### S2 — `[mod.rs:227-234]` `eff_pid_case()` eff_pid 回溯 CASE+标量子查询（**单一事实源**）

- **SQL 类型**: 标量相关子查询（`(SELECT CAST(g.auto_from_platform AS INTEGER) FROM "group" g WHERE g.group_key = proxy_log.group_key ... LIMIT 1)`），包在 `CASE WHEN platform_id=0 THEN COALESCE(...) ELSE platform_id END`
- **涉及表 / 关联键**: 相关子查询关联 `g.group_key = proxy_log.group_key`（关联引用恒用表名 proxy_log.，不可裸列）
- **调用频率**: 见各使用点（S3/S4/S5/S6）；本体是字符串生成器
- **去 JOIN 可行性**: **hard** — 这是 platform_id=0 自动分组日志回溯源平台的核心（记忆 stats-platform-dimension-effpid）。内联进 SELECT/GROUP BY/WHERE 表达式，去掉须在 SQL 外预取 `group_key → eff_pid` 映射表再在内存对每行回溯，并重写所有按 eff_pid 的 GROUP BY/过滤。语义可等价但牵动面大、易 N+1，hard。

### S3 — `[query_stats.rs:319,334,401-402,450]` `query_stats_inner`（proxy_log 原路径，minute/5min 粒度）按 eff_pid

- **SQL 类型**: 标量相关子查询（内联 `eff_pid_case("proxy_log.")`）+ dimension 分支 `LEFT JOIN platform p ON p.id = ({eff_pid})`
- **涉及表 / 关联键**: proxy_log 自关联 `"group"`（子查询）+ LEFT JOIN platform 取真名；GROUP BY `({eff_pid})`
- **调用频率**: 页面加载（Stats 页，仅 minute/5min 细粒度才走 proxy_log 原路径；hourly/daily 走聚合表 S 系列另算）
- **去 JOIN 可行性**: **hard** — 子查询 + LEFT JOIN 都服务于 eff_pid 维度聚合 + 平台真名。拆开需：内存预取 group_key→eff_pid map、按行算 eff_pid、再内存 GROUP BY 聚合、再补平台名。聚合依赖 eff_pid，拆开等于把 SQL 聚合搬到内存，语义难等价且行数（细粒度时间窗内全量 proxy_log）可能大。LEFT JOIN platform 取名部分可单独拆（见 J6 模式）但 GROUP BY ({eff_pid}) 子查询难拆。

### S4 — `[query_stats.rs:216]` `query_stats_inner_agg` dimension=platform 取真名

- **SQL 类型**: LEFT JOIN（`stats_agg_hourly s LEFT JOIN platform p ON p.id = s.platform_id`）
- **涉及表 / 关联键**: `p.id = s.platform_id`（s.platform_id 已是 eff_pid，无子查询回溯）；GROUP BY s.platform_id
- **调用频率**: 页面加载（Stats 页 hourly/daily 粒度，默认路径，比 S3 更常走）
- **去 JOIN 可行性**: **easy** — 纯为补平台名。可先 `GROUP BY platform_id` 聚合（无 JOIN），再 `SELECT id,name FROM platform` 内存补名（与 J6 today_platform_stats 完全同构）。聚合不依赖 JOIN，platform 只贡献展示名。

### S5 — `[stats_agg.rs:7-9 / 91-92]` stats_agg_hourly UPSERT 内 eff_pid 回溯

- **SQL 类型**: 标量相关子查询（`AGG_EFF_PID_EXPR` 常量 line 6-9，及 `upsert_stats_agg` INSERT VALUES 内 line 90-92 同语义副本，关联键 `g.group_key = ?3`）
- **涉及表 / 关联键**: 写入时把 `proxy_log.group_key`/参数 `?3` → `"group".auto_from_platform` 回溯为 eff_pid 写进 stats_agg_hourly.platform_id
- **调用频率**: **每请求热路径** — `upsert_stats_agg` 由 `proxy/log.rs`（代理终态路径）每条终态请求无条件调用；`AGG_EFF_PID_EXPR` 用于 rebuild/回填（手动/启动）
- **去 JOIN 可行性**: **hard** — 这是「写时把 eff_pid 物化进聚合表」的设计支点，正因为写时回溯，读路径（S4/J6/today）才不用 JOIN。改为去子查询须在 Rust 侧先查 group→eff_pid 再传字面 platform_id：可行（多一次读），但属每请求热路径，且与回填/rebuild 三处副本须同步改，语义敏感，hard。**注意：此条去掉子查询的收益是把回溯前移到写时一次内存查，反而可能简化读路径——是否纳入重构由 main 裁定。**

### S6 — `[stats_agg.rs:34] / [mod.rs:61-96]` 回填 / rebuild：从 proxy_log 重建 stats_agg_hourly

- **SQL 类型**: 标量相关子查询（eff_pid，同 S5 字符串）+ `mod.rs:91` `NOT EXISTS (SELECT 1 FROM stats_agg_hourly LIMIT 1)`（空表守卫，标量 EXISTS）
- **涉及表 / 关联键**: proxy_log → "group"（eff_pid 子查询）；EXISTS 守卫仅判 stats_agg_hourly 是否空
- **调用频率**: 手动 / 启动（首次回填一次性；rebuild 手动触发）
- **去 JOIN 可行性**: eff_pid 子查询部分 **hard**（同 S5，且这是批量 GROUP BY 写入，搬内存代价大）；`NOT EXISTS` 守卫 **easy**（可改 Rust 侧先 `SELECT COUNT(*)`/`SELECT 1 LIMIT 1` 判空再决定是否执行 INSERT，无需 SQL 内 EXISTS）。

---

## 汇总表

| ID | 文件:行 | 类型 | 热路径 | 可行性 |
|---|---|---|---|---|
| J1 | group_platform.rs:76 | INNER JOIN | 手动 | easy |
| J2 | group_platform.rs:192 | INNER JOIN | 页面加载 | medium |
| J3 | group_platform.rs:236 | INNER JOIN | 页面加载 | medium |
| J4 | platform_lifecycle.rs:130 | INNER JOIN | 手动 | easy |
| J5 | settings.rs:141 | INNER JOIN ×2 | 手动 | easy |
| J6 | stats_today.rs:104 | （已去 JOIN 参照） | 页面/tray | N/A |
| S1 | usage_stats.rs:21 | IN(SELECT) 相关子查询 | **偏热**(卡片刷新) | medium |
| S2 | mod.rs:227 | 标量相关子查询(单一源) | 见使用点 | hard |
| S3 | query_stats.rs:319/334/401/450 | 标量子查询 + LEFT JOIN | 页面(细粒度) | hard |
| S4 | query_stats.rs:216 | LEFT JOIN | 页面(默认粒度) | easy |
| S5 | stats_agg.rs:7/91 | 标量相关子查询 | **每请求热路径** | hard |
| S6 | stats_agg.rs:34 / mod.rs:61(+91 NOT EXISTS) | 标量子查询 + EXISTS守卫 | 手动/启动 | eff_pid:hard / EXISTS:easy |

### 分桶计数

- **总条目**: 11 条（J1–J6 + S1–S6，其中 J6 为去 JOIN 参照不计重构；S6 含两个可分级片段）
- **easy**: J1, J4, J5, S4，加 S6 的 NOT EXISTS 片段 → **4.5**（纯展示补名 / 纯 id 过滤 / 空表守卫，可拆多查或前端分取）
- **medium**: J2, J3, S1 → **3**（需内存保序 join / 动态 IN 列表）
- **hard**: S2, S3, S5, S6-effpid → **4**（聚合依赖 eff_pid 回溯子查询，拆开搬内存聚合或前移到写时，语义敏感 / 三处副本同步）
- **N/A 参照**: J6 → 1

### 热路径高亮

- 🔥 **S5**（stats_agg.rs:7/91，`upsert_stats_agg`）= **每代理请求**终态写入路径调用，eff_pid 子查询 hard。改动须同步 S6 回填 / S2 单一源副本。
- 🔥 **S1**（usage_stats.rs:21）= 平台卡片用量刷新每平台一次，偏热，medium。
- S4（query_stats.rs:216）虽 easy，但是 Stats 页默认（hourly/daily）粒度路径，去 JOIN 收益直接（与 J6 同模式）。

## Caveats / 需要 main 裁定

- **S5 去子查询方向反转**：把 eff_pid 回溯从「写时 SQL 子查询」改为「写时 Rust 内存预查」反而能让读路径彻底无回溯。是否纳入本次「去 JOIN」范围，建议 main 裁定（这是写时 vs 读时的设计取舍，不只是机械去 JOIN）。
- **eff_pid 三处字符串副本**：S2(单一源 mod.rs:227) / S5(stats_agg.rs AGG_EFF_PID_EXPR + upsert VALUES) / S6(mod.rs 回填内联) 语义相同但**字符串各自独立**，任何去子查询改造须三处同步，否则读写 eff_pid 归属不一致（记忆 stats-platform-dimension-effpid 风险点）。
- **J2/J3 列序耦合**：两者共用 `parse_group_platform_row`（23 列 platform + priority/weight/level_priority），去 JOIN 改两步取数后内存拼装时必须保持同一解析列序，否则回归。
- 所有 file:line 以当前磁盘（master, commit 5927c02）为准。
