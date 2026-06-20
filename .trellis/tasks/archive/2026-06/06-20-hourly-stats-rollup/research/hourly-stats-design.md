# Research: 小时维度预聚合统计表（hourly stats rollup）设计

- **Query**: 设计「小时 rollup 表加速统计渲染」方案：摸清当前统计聚合、定位渲染慢瓶颈、设计 schema/回填/增量维护/查询改写/测试方案
- **Scope**: internal（含本机实测）
- **Date**: 2026-06-20

## TL;DR（核心结论）

**瓶颈不在 DB 聚合。** 本机实测（247MB 库、11,411 行 live proxy_log）：
- 所有计费聚合（today_stats / 30d overview / today_platform_stats / 日桶曲线）均 **sub-millisecond**。
- 关键原因：Migration 019 的覆盖索引 `idx_proxy_log_stats(created_at, est_cost, input_tokens, output_tokens, cache_tokens, status_code) WHERE deleted_at=0` 让计费聚合走 **COVERING INDEX**（`EXPLAIN QUERY PLAN` 确认），只扫 ~380KB 索引页，不碰 255MB 的 body 列（body 平均 ~19KB/行，占满整个表体积）。
- 即使 `today_platform_stats` 的 EFF_PID 相关子查询、`group_by` 维度（走二级索引 + temp B-tree），实测仍 < 2ms。

**真正可感知的「渲染慢」（如有）来自前端，而非单条 SQL**：浮窗 `PopoverCards.tsx` 每张卡片各自 `useEffect` 独立 `statsApi.query`（N 卡 = N 次往返），且 `query_stats_inner` 一次调用内串行跑 **4 个查询**（overview / 时间桶 / dimension / available_models DISTINCT）。这是 **N×4 query fan-out + 串行 IPC 往返**，不是单查询扫全表慢。

**因此小时 rollup 表对「当前规模」收益有限，且不对症**：它能把单查询扫描行数降一两个数量级，但单查询本就 <2ms；它不能消除前端 N 卡 fan-out / 串行 IPC / 4-查询串行 / `unixepoch` vs `localtime` 时区错位等真实问题。

**建议**（详见 §2 失败处理）：把 rollup 当作「为未来 10×~100× 行数增长做的容量预案 + 顺带修时区/批量化」来设计，而非「修当前慢」。若用户感知「慢」，应先按 §2 定位是否前端 fan-out，优先批量化 popover 查询。本文档仍按要求给出完整 rollup 设计与测试方案，并标注「现在做 vs 推迟」的判断。

---

## 1. 当前统计查询清单

所有统计查询都实时 `SUM/GROUP BY` 扫 `proxy_log`（`WHERE deleted_at=0` + `created_at` 范围）。无任何统计结果缓存（`DbCache` 只缓存 settings/groups，见 `db.rs:68-181`）。

| 函数 | 位置 file:line | 聚合内容 | 时间窗 | 维度 | 扫描方式（实测 plan） |
|---|---|---|---|---|---|
| `today_token_total` (test only) | `db.rs:1384` | SUM(in+out tok) | 本地 00:00 起 | 无 | COVERING idx_proxy_log_stats |
| `today_stats` | `db.rs:1420` | SUM(tok)/SUM(cache)/SUM(in)/COUNT + 单独 SUM(est_cost) | 本地 00:00 起 | 无 | COVERING（2 次查询） |
| `today_platform_stats` | `db.rs:1493` | SUM(tok)/SUM(cost)/COUNT GROUP BY eff_pid | 本地 00:00 起 | 平台(EFF_PID 回溯) | idx_proxy_log_created + 相关子查询 + temp B-tree GROUP BY，<2ms |
| `hourly_rate_inner` | `db.rs:3424` | MIN(created_at)+SUM(est_cost) | now-7d | group/platform 可选 | 计费聚合（usage_color 速率用） |
| `usage_stats` (helper) | `db.rs:3132` | COUNT/成功数/SUM(tok×3)/SUM(cost) | by where_clause | 单平台/单组 | COVERING |
| `get_platform_usage_stats` | `db.rs:3180` | 同上 + 最近 5 错误 | 全量(deleted_at=0) | 单平台 | 二级索引 |
| `get_group_usage_stats` | `db.rs:3239` | 同上 | 全量 | 单组 | 二级索引 |
| `get_all_group_usage_stats` | `db.rs:3254` | COUNT/成功/SUM GROUP BY group_key（批量，N+1 已消除） | 全量 | 全部组 | idx_proxy_log_group_key |
| `platform_usage_stats_all` | `db.rs:3313` | GROUP BY eff_pid（批量） | 全量 | 全部平台 | 二级索引 + 相关子查询 |
| `query_stats_inner` | `db.rs:3540` | **4 查询串行**：overview / 时间桶(strftime 分桶) / dimension(group_by) / available_models(DISTINCT) | start~end（默认 7d） | 时间桶 × group/model/platform 筛选 + group_by 维度 | overview=COVERING；时间桶/dimension/DISTINCT 走 strftime 表达式或二级索引 + temp B-tree |

### 消费者（谁调用）

- **Stats 页** `Stats.tsx:130-169`：`statsApi.query` 一次 load 触发 **2~4 次** `query_stats`（当前窗 + 上一窗对比 + 可能的 minute/5min 自动降级重查）。`onProxyLogUpdated` 事件触发重 load（`Stats.tsx:174`）。
- **浮窗** `PopoverCards.tsx`：`popover_data`（一次拿 today_stats + platform_today）+ **每张 cost_trend / platform_metric / group_metric 卡片各自独立** `statsApi.query`（`PopoverCards.tsx:234-248, 316, 379`）。卡片数 = 用户配置的 popover items 数，每卡一次 query → **N×4 子查询**。
- **Group 卡** `Groups.tsx fetchGroupStats`：调 `all_group_usage_stats`（已批量化，1 查询）。
- **Platform 列表**：`all_platform_usage_stats`（已批量化）。
- **托盘标题**：`tray_today_stats` → `today_stats`，事件驱动 + 00:00 定时器（`lib.rs:4136`）。

---

## 2. 渲染慢瓶颈定位（DB vs 前端）——决定 rollup 是否对症

### 证据（本机实测，2026-06-20）

库 `~/.aidog/aidog.db` = **247.5MB**，但 live 行仅 **11,411**。`dbstat`：`proxy_log` 表体 **255MB**（body 列 avg ~19KB/行），`idx_proxy_log_stats` 仅 **380KB**。

实测 `.timer` + `EXPLAIN QUERY PLAN`：

| 查询 | plan | 实测耗时 |
|---|---|---|
| 30d overview `COUNT/SUM(est_cost)` | `SEARCH ... USING COVERING INDEX idx_proxy_log_stats` | < 1ms |
| 30d 日桶曲线 `strftime GROUP BY` | 二级索引 + temp B-tree | < 2ms |
| today_platform_stats（EFF_PID 相关子查询 30d） | `idx_proxy_log_created` + `CORRELATED SCALAR SUBQUERY`(group_key 索引) | < 2ms |
| available_models DISTINCT 30d | 二级索引 | < 2ms |
| dimension group_by model | `SCAN USING idx_proxy_log_actual_model` + temp B-tree ORDER | < 2ms |

**结论：DB 聚合不是瓶颈。** 覆盖索引（Migration 019）已经把计费聚合变成 index-only scan，扫描量与表体积（body 列）解耦。255MB 体积是 body 列造成的（见记忆 `db-volume-soft-delete-no-vacuum`），与统计聚合无关。

### 前端侧的真实成本（推测：若用户感知慢，源头在此）

1. **浮窗 N 卡 fan-out**：每卡独立 `statsApi.query`，每次 `query_stats_inner` 内部串行 4 个查询 → N 卡 × 4 = 大量串行 SQL + N 次 Tauri IPC 往返。单查询快，但 IPC 序列化 + React 各卡 `useEffect` 独立 resolve 的累积延迟可感知。**rollup 表不能消除此 fan-out**（仍是 N 次 query）。
2. **Stats 页一次 load 2~4 次 query**（对比窗 + 自动降级重查），每次 4 子查询。
3. **available_models DISTINCT 全表**：随历史模型数增长而变重（但当前快）。

### 何时 rollup 才真正对症（DB 聚合成为瓶颈的条件）

- proxy_log live 行数增长 **10×~100×**（→ 10万~100万行）后，覆盖索引扫描虽仍 index-only，但 `strftime` 分桶 GROUP BY + temp B-tree + 相关子查询的常数因子会线性放大到几十~几百 ms，届时 rollup（预聚合到小时桶，行数降 ~×60）才显著。
- 或 retention 放宽（当前 body 90d / 行不删整行），proxy_log 行数长期累积时。

**判断**：当前 11K 行下 rollup 是**容量预案**而非「修慢」。建议同步把 §3.6「顺带收益」（修 unixepoch 时区、popover 批量化）做掉——后者才是当前可感知收益。

---

## 3. 小时 rollup 表设计

### 3.1 Schema

按「最小够用维度」设计。当前统计需要的维度键：时间桶（小时）、平台（eff_pid）、分组（group_key）、模型（actual_model）、状态成功/失败。维度全交叉会爆炸，故拆为**一张事实表 + 维度列**，而非每维度一表。

```sql
CREATE TABLE IF NOT EXISTS proxy_log_hourly (
    hour_bucket   INTEGER NOT NULL,  -- 小时桶起点 epoch ms（UTC 整点，= floor(created_at/3600000)*3600000）
    eff_pid       INTEGER NOT NULL,  -- 有效平台 id（auto 分组已回溯；0=未知）
    group_key     TEXT    NOT NULL DEFAULT '',
    actual_model  TEXT    NOT NULL DEFAULT '',  -- actual_model 优先，回退 model
    -- 度量列
    req_count       INTEGER NOT NULL DEFAULT 0,
    success_count   INTEGER NOT NULL DEFAULT 0,
    input_tokens    INTEGER NOT NULL DEFAULT 0,
    output_tokens   INTEGER NOT NULL DEFAULT 0,
    cache_tokens    INTEGER NOT NULL DEFAULT 0,
    est_cost        REAL    NOT NULL DEFAULT 0.0,
    duration_ms_sum INTEGER NOT NULL DEFAULT 0,  -- 存 SUM 不存 AVG，AVG=sum/count 查询时算（AVG 不可二次聚合）
    PRIMARY KEY (hour_bucket, eff_pid, group_key, actual_model)
) WITHOUT ROWID;

CREATE INDEX IF NOT EXISTS idx_plh_bucket ON proxy_log_hourly(hour_bucket);
```

**设计要点**：
- **唯一键 = (hour_bucket, eff_pid, group_key, actual_model)**：4 维交叉，upsert 幂等的依据。`WITHOUT ROWID` 省空间（主键即聚簇）。
- **存 SUM 不存 AVG**：`avg_duration_ms` 必须由 `SUM(duration_ms_sum)/SUM(req_count)` 在查询时算——AVG 不可对预聚合结果二次平均（数学错误）。同理 `cache_rate`/`success_rate` 也是查询时由 SUM 比值算，不预存比率。
- **hour_bucket 用 UTC 整点 epoch ms**：避免存本地时区字符串。本地日界由查询时 `created_at` 反算（见 §3.5 时区）。
- **不含 status_code 细分**：只存 success_count（2xx）；失败数 = req_count - success_count。Stats 页若需「错误数」用此推导。
- **eff_pid 在写入 rollup 时就固化**：避免查询时再跑 EFF_PID 相关子查询（把回溯成本前移到 rollup 维护时）。

**维度爆炸评估**：当前一天约 5000 请求，平台~10 × 模型~20 × 组~10，实际非空交叉远小于笛卡尔积（稀疏），一天小时桶行数预计几百~几千，30 天 ~数万行——仍远小于明细。

### 3.2 增量维护策略（三选一，推荐方案 B）

| 方案 | 机制 | 优点 | 缺点 |
|---|---|---|---|
| A. 写时 upsert | 每条 proxy_log 落库后同步 upsert 对应小时桶 | 实时一致 | 热路径每请求 +1 写（违背 `perf-hotpath-optimization` 零分配精神）；当前桶频繁更新 |
| **B. 定时增量 rollup（推荐）** | 维护 `rollup_watermark`（已聚合到的 created_at）；定时器（如每 5min + 00:00）把 `(watermark, now-当前小时起点)` 的明细聚合 upsert，watermark 推进到「当前小时起点」 | 不碰热路径；当前未闭合小时永不写 rollup（查询时用实时尾部补，见 §4） | 非实时（当前小时靠实时查询补）；需 watermark 状态 |
| C. SQLite 触发器 | AFTER INSERT ON proxy_log 触发 upsert | DB 层自动 | 触发器内跑 EFF_PID 子查询代价高；难处理 retry-update（est_cost 后填）；调试难 |

**推荐 B**，理由：
- proxy_log 的 `est_cost` / token 是请求完成后 **diff-UPDATE 回填**的（`db.rs:2700 changed_since` + `ProxyLogColumns`），写时 upsert（A）/触发器（C）会在 est_cost 还没填时就聚合，导致 rollup 漏计 cost。方案 B 只聚合「已闭合小时」的明细（此时 est_cost 必已回填），天然规避。
- watermark = 已聚合到的「整点」。只聚合 `hour_bucket < floor(now/3600000)*3600000` 的明细（当前小时永远走实时尾部），保证幂等且不重复聚合可变数据。

watermark 存 `setting` 表（scope='rollup', key='hourly_watermark', value=epoch_ms）。

### 3.3 增量 rollup SQL（方案 B 核心）

```sql
-- 1. 算闭合边界
-- closed_end = floor(now/3600000)*3600000   （当前小时起点，未闭合不算）
-- 聚合区间 = [watermark, closed_end)
INSERT INTO proxy_log_hourly (hour_bucket, eff_pid, group_key, actual_model,
    req_count, success_count, input_tokens, output_tokens, cache_tokens, est_cost, duration_ms_sum)
SELECT
    (created_at/3600000)*3600000 AS hb,
    <EFF_PID 表达式> AS eff_pid,
    group_key,
    CASE WHEN actual_model!='' THEN actual_model ELSE model END AS am,
    COUNT(*),
    SUM(CASE WHEN status_code>=200 AND status_code<300 THEN 1 ELSE 0 END),
    SUM(input_tokens), SUM(output_tokens), SUM(cache_tokens),
    COALESCE(SUM(est_cost),0.0), SUM(duration_ms)
FROM proxy_log
WHERE deleted_at=0 AND created_at>=?watermark AND created_at<?closed_end
GROUP BY hb, eff_pid, group_key, am
ON CONFLICT(hour_bucket, eff_pid, group_key, actual_model) DO UPDATE SET
    req_count = req_count + excluded.req_count,
    success_count = success_count + excluded.success_count,
    input_tokens = input_tokens + excluded.input_tokens,
    output_tokens = output_tokens + excluded.output_tokens,
    cache_tokens = cache_tokens + excluded.cache_tokens,
    est_cost = est_cost + excluded.est_cost,
    duration_ms_sum = duration_ms_sum + excluded.duration_ms_sum;
-- 2. UPDATE setting SET value=?closed_end WHERE key='hourly_watermark'
```

**关键约束**：watermark 必须严格按「整点边界」推进（不能停在小时中间），否则 `ON CONFLICT ... +=` 会重复累加同一小时已聚合部分。区间用半开 `[watermark, closed_end)`，watermark 从 0（或最早日志）起，每次推进到 closed_end。

**软删/retention 处理**：proxy_log 的 retention 是「清空 body 列（UPDATE SET=''）+ retention_days 删整行」（见 `db.rs:2956` + CLAUDE.md）。清 body 不影响 rollup（rollup 不含 body）。**删整行会让明细消失但 rollup 已聚合保留——这是优点**（rollup 成为历史长存的聚合，明细可删）。但需注意：**rollup 维护必须先于 retention 删行**完成对该时段的聚合（watermark 已过 = 已聚合，删行无影响；watermark 未过就删行 = 漏计）。retention_days(90d) 远大于 rollup 定时间隔（5min），实践中不冲突，但测试需覆盖。

### 3.4 回填历史方案

一次性脚本（migration 内或独立 command），按小时分批避免长事务锁：

```text
for hb in [floor(MIN(created_at)/3600000)*3600000 .. closed_end) step 3600000:
    跑 §3.3 的 INSERT...SELECT，区间 = [hb, hb+3600000)
    （回填用 INSERT OR REPLACE 或先 DELETE 该桶再 INSERT，比 += 更安全——回填是全量重算，不该累加）
设 watermark = closed_end
```

**回填用「替换」而非「累加」**：回填是从零重算，若用 `+=` 且脚本重跑会翻倍。增量（§3.3）才用 `+=`。两者语义不同，代码须区分。建议回填用 `INSERT ... ON CONFLICT DO UPDATE SET req_count=excluded.req_count, ...`（覆盖）配合「回填前 `DELETE FROM proxy_log_hourly`」清空，确保幂等。

回填触发时机：Migration 建表后立即跑一次（同步，11K 行 < 100ms）；未来大库可做后台异步 + 进度。

### 3.5 时区处理（当前隐患，rollup 须显式解决）

**现状不一致**：
- `today_stats`/`today_platform_stats` 用 `chrono::Local` 算本地 00:00（`db.rs:1422-1428`）→ 本地日界正确。
- `query_stats` 的 `bucket_time_expr` 用 `strftime(..., 'unixepoch')` = **UTC**（`db.rs:3517-3527`，无 `'localtime'`）→ 日桶/小时桶按 UTC 切，与「今日」本地语义错位（东八区差 8 小时）。

**rollup 设计决策**：hour_bucket 用 **UTC 整点**存（中立）。查询时：
- 「今日」= 本地 00:00 → 本地午夜对应的 UTC 时刻可能落在某 UTC 小时中间。东八区本地 00:00 = UTC 16:00（整点），多数整数时区本地午夜对齐 UTC 整点，故按 hour_bucket >= 本地午夜的 UTC ms 过滤即可精确（半小时时区如印度 +5:30 例外，见测试）。
- 小时桶展示给前端时，前端按本地时区格式化 hour_bucket（已是 epoch ms，前端 `new Date(hb).toLocaleString`）——比当前后端 strftime UTC 字符串更正确，顺带修 §2 的 unixepoch bug。

### 3.6 顺带收益（当前可感知，建议同 task 一起做）

1. **修 `bucket_time_expr` 时区**：现在 Stats 曲线按 UTC 切桶，跨日错位。rollup 用 epoch ms + 前端本地格式化可根治。
2. **popover 批量化**：把 N 卡的 N 次 `statsApi.query` 合并为 1 个批量 command（一次返回所有卡数据），消除 fan-out——这是当前真正的渲染延迟来源。

---

## 4. 查询改写

### 4.1 哪些切到 rollup

| 查询 | 切 rollup? | 改写方式 |
|---|---|---|
| `today_stats` | 部分 | 闭合小时从 rollup SUM，当前未闭合小时从 proxy_log 实时 SUM，相加 |
| `today_platform_stats` | 部分 | rollup GROUP BY eff_pid + 实时尾部 GROUP BY eff_pid，merge |
| `query_stats` overview/时间桶（粒度 hourly/daily） | 是 | rollup 按 hour_bucket 聚合；daily 桶 = `floor(hb 本地日)`；筛选 group/platform/model 走 rollup 维度列 |
| `query_stats` 粒度 minute/5min | **否** | 小时 rollup 无法降到分钟，仍走 proxy_log 明细（Stats 页自动降级到 minute 时） |
| `query_stats` available_models DISTINCT | 是 | `SELECT DISTINCT actual_model FROM proxy_log_hourly WHERE hour_bucket in range`（rollup 含此维度） |
| `get_all_group_usage_stats` / `platform_usage_stats_all`（全量，无时间窗） | 是 | 全量 SUM rollup + 当前小时实时尾部 |
| `hourly_rate_inner`（速率，需 MIN(created_at)） | **否/谨慎** | 依赖精确 MIN(created_at)，rollup 小时粒度丢失分钟精度——保留走明细，或接受小时精度 |

### 4.2 跨小时边界 / 部分小时（当前未闭合小时）混合

核心模式 **rollup + 实时尾部**：

```text
closed_end = floor(now/3600000)*3600000   -- 当前小时起点
result = AGG(rollup WHERE hour_bucket >= range_start AND hour_bucket < closed_end)
       + AGG(proxy_log WHERE created_at >= max(range_start, closed_end) AND created_at <= now AND deleted_at=0)
```

- rollup 覆盖 `[range_start, closed_end)` 的整点小时；
- 实时尾部覆盖 `[closed_end, now]` 当前未闭合小时；
- 若 watermark 落后于 closed_end（定时器还没跑到），尾部区间需从 watermark 起：`max(range_start, watermark)`——**用 watermark 而非 closed_end 作为分界更安全**（保证不漏不重）：rollup 覆盖 `[range_start, watermark)`，实时覆盖 `[max(range_start, watermark), now]`。

**这是设计的正确性核心**：分界点统一用 watermark，rollup 区间 `[_, watermark)` + 实时区间 `[watermark, now]`，半开/半闭不重叠不遗漏。所有改写查询共用此分界逻辑（抽成一个 helper）。

---

## 5. 完善测试方案（必须，用户硬要求）

测试放 `db.rs` 的 `#[cfg(test)]`（现有 stats 测试在 `db.rs:4331+`，可复用 seed helper）。

### 5.1 回填正确性

- `T1_backfill_matches_realtime`：seed 跨多小时多平台多组多模型的 proxy_log → 回填 rollup → 对每个维度组合，`SUM(rollup)` == 对应明细 `SUM(proxy_log)`（token/cost/count/success 逐项对账）。
- `T2_backfill_idempotent`：回填跑 2 次（重跑），结果不翻倍（验证回填用「替换」语义而非累加）。
- `T3_backfill_empty`：空库回填不报错，rollup 0 行。

### 5.2 增量一致性

- `T4_incremental_equals_backfill`：同一份数据，路径 A=全量回填 vs 路径 B=分多次增量 rollup（watermark 逐步推进）→ 两表逐行相等。
- `T5_incremental_idempotent`：增量在同一 watermark 重跑不重复累加（watermark 严格整点推进验证）。
- `T6_est_cost_late_fill`：先插 proxy_log（est_cost=0），rollup 不应聚合当前小时；待 est_cost diff-UPDATE 回填 + 小时闭合后再 rollup → cost 正确计入（验证方案 B 规避「est_cost 未填就聚合」）。

### 5.3 边界（跨日 / 时区 / 空桶）

- `T7_cross_day_boundary`：日志跨本地 00:00（如 23:30 和 00:30 各一条）→ `today_stats`(rollup 改写版) 只含 00:30 那条，与现行 Local 午夜语义一致。
- `T8_utc_localtime_divergence`：东八区下，UTC 整点桶与本地午夜对齐验证；**印度 +5:30 半小时时区**用例（本地午夜落在 UTC 小时中间）→ 验证「今日」过滤精度，记录已知误差或用明细补半小时。
- `T9_empty_hour_gaps`：中间有无请求的空小时 → rollup 无该桶行，曲线渲染缺口符合预期（不应补 0 行还是补，明确语义并测）。
- `T10_unclosed_hour_tail`：当前小时有新请求 → rollup+实时尾部 == 纯实时全量（分界用 watermark）。

### 5.4 rollup 与实时查询对账（核心）

- `T11_rollup_vs_realtime_overview`：随机 seed → `query_stats`(rollup 路径) overview == `query_stats`(纯 proxy_log 路径) overview，token/cost/count/success_rate 全等（success_rate/cache_rate 由 SUM 比值算，验证不预存比率的正确性）。
- `T12_rollup_vs_realtime_buckets`：hourly/daily 时间桶逐桶对账。
- `T13_rollup_vs_realtime_dimension`：group_by platform/model/group 维度逐项对账（验证 eff_pid 固化回溯 == 实时 EFF_PID 子查询）。
- `T14_avg_duration_correctness`：`avg_duration` 由 `SUM(duration_ms_sum)/SUM(req_count)` 算 == 明细 `AVG(duration_ms)`（验证不能存 AVG 二次平均）。
- `T15_filter_combinations`：group+platform+model 组合筛选下 rollup vs 实时一致。

### 5.5 retention 交互

- `T16_retention_delete_preserves_rollup`：proxy_log 删整行（retention_days 过期）后，rollup 已聚合数据仍在，历史曲线不丢。
- `T17_rollup_before_retention`：watermark 已过的时段删明细不影响 rollup；watermark 未过就删 → 测试断言此场景被防护（rollup 须先于该时段 retention）。

---

## 6. 触点清单（file:line）与风险

### 改动触点

| 文件 | 位置 file:line | 改动 |
|---|---|---|
| `src-tauri/src/gateway/db.rs` | `db.rs:495` 之后（Migration 031） | 新增 `CREATE TABLE proxy_log_hourly` + 索引 + 回填（当前最新 = Migration 030 @ `db.rs:495`） |
| `db.rs` | 新增 fn | `rollup_incremental(db)` + `backfill_hourly(db)` + watermark 读写（用 `setting` 表 helper） |
| `db.rs:1420` `today_stats` | 改写 | rollup + 实时尾部（分界 watermark） |
| `db.rs:1493` `today_platform_stats` | 改写 | 同上 |
| `db.rs:3540` `query_stats_inner` | 改写 | overview/时间桶/dimension/available_models 切 rollup（minute/5min 保留明细）；**bucket 改 epoch ms 修时区** |
| `db.rs:3254` `get_all_group_usage_stats` / `db.rs:3313` `platform_usage_stats_all` | 改写 | 全量走 rollup + 尾部 |
| `src-tauri/src/lib.rs` | `lib.rs:4136` 附近 setup 定时器 | 新增 rollup 定时器（每 5min + 00:00），复用现有 00:00 定时器骨架 |
| `lib.rs` | 可选新 command | `rollup_now`（手动触发，测试/调试用）；popover 批量 command（§3.6.2） |
| 前端 `Stats.tsx`/`PopoverCards.tsx` | `PopoverCards.tsx:234,316,379` | 若做批量化：N 卡 → 1 批量 query；bucket 时间改本地格式化 |

### 风险

1. **收益与当前规模不匹配**（最大风险）：11K 行下 DB 聚合已 <2ms，rollup 修不了「慢」。若用户实际感知慢，根因在前端 fan-out（§2），rollup 做完仍慢 → 需先验证慢在哪。**强烈建议先做 §3.6.2 popover 批量化 + 实测前后端耗时拆分，再决定 rollup 优先级。**
2. **est_cost 延迟回填**：方案 A/C 会漏计 cost；必须用方案 B（只聚合闭合小时）。
3. **watermark 推进 bug**：非整点推进或重跑会导致 `+=` 重复累加 → 数据翻倍。测试 T2/T5/T10 必须覆盖。
4. **AVG 二次聚合错误**：存 AVG 会算错；必须存 SUM + count。T14 覆盖。
5. **时区**：现行 `unixepoch` UTC 桶本就有 bug；rollup 用 epoch ms 是修复机会，但半小时时区（印度 +5:30）边界需明确语义。T8 覆盖。
6. **回填 vs 增量语义混淆**：回填=替换，增量=累加，代码须严格区分，否则回填重跑翻倍。T2 覆盖。
7. **维护成本**：新增表 + 定时器 + 双路径（rollup+尾部）增加复杂度与新 bug 面，需权衡 vs 当前「单条 SQL <2ms」的简洁。

---

## Caveats / Not Found

- 未实测「浮窗实际渲染感知延迟」的端到端耗时（需运行 app + 计时各卡 resolve）；§2 前端 fan-out 为基于代码结构的**推测**，建议实现前用浏览器 devtools / Tauri 日志实测 N 卡并发耗时确认。
- `hourly_rate_inner`（usage_color 速率）依赖 MIN(created_at) 分钟精度，rollup 小时粒度会损失精度，本设计建议其**保留走明细**，未深入设计其 rollup 化。
- 半小时/45 分钟时区（印度 +5:30、尼泊尔 +5:45）下「本地今日」与 UTC 小时桶的精确对齐为已知边界难点，本设计建议这些时区当前小时用明细补，未给完整公式。
