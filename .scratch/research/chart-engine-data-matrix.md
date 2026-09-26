# 九种图表 × proxy_log 可聚合维度矩阵（issue #23）

调研范围：`statsApi.query` / `proxy_log_list*` / `get_group_usage_stats` 三条数据链 + `proxy_log` / `stats_agg_hourly` 两张表。
全部结论出自本仓库源码（可信源第 4 类），file:line 均为当前 master 实测。

> 口径说明：票面「九种」枚举了 8 个名词，本文把「饼图 / 环形」按两种图型拆开凑足九行（两者数据形状完全相同，仅视觉差异）。

## 0. 数据面现状（矩阵的前置事实）

| 事实 | 出处 |
|---|---|
| `StatsQuery` 参数：`start/end`(ms) + `granularity`(minute/5min/hourly/daily) + `group_by`(platform/model/group) + `filter_group/filter_model/filter_platform/filter_coding_plan` | `src-tauri/crates/aidog_db/src/models/stats.rs:53-72` |
| `StatsResult` 返回四块：`overview`（窗口汇总）/ `buckets`（纯时间序列）/ `dimension_data`（**整窗单维度** top50）/ `available_models` | `src-tauri/crates/aidog_db/src/models/stats.rs:126-131` |
| 粒度路由：minute/5min 走 log.db 裸扫 `proxy_log` 内存聚合；hourly/daily（含缺省）走主库聚合表 `stats_agg_hourly` | `src-tauri/crates/aidog_stats/src/query_stats.rs:518`（match 分发）、`query_stats.rs:28-31`（注释） |
| `stats_agg_hourly` 物理粒度 = (time_hour, model, group_key, platform_id) 四元组，度量列 request/success/error count + in/out/cache tokens + est_cost + duration 的 SUM | `src-tauri/crates/aidog_db/src/lib.rs:59-76` |
| `proxy_log` 可聚合列：created_at / group_key / platform_id / model / actual_model / status_code / duration_ms / input_tokens / output_tokens / cache_tokens / est_cost / is_stream / retry_count / blocked_by / blocked_reason / done / field_trace | 建表 `src-tauri/crates/aidog_db/src/schema_early.rs:99-128` + ALTER `schema_early.rs:140,145,150,154,159,163` + `schema_late.rs:485,504` |
| 日志行级 API：`proxyLogApi.list / listFiltered`（limit/offset 分页）返回 `ProxyLogSummary`，**不含 est_cost** | `src/services/api/proxy.ts:41-45`、字段清单 `src/services/api/types/generated/ProxyLogSummary.ts` |
| 组级汇总：`get_group_usage_stats`（单组累计+今日）、`get_all_group_usage_stats`（批量）、`platform_usage_stats_all`（批量） | `src-tauri/crates/aidog_stats/src/usage_stats.rs:183 / 241 / 312` |
| 配额（仪表盘数据源）：`platform_query_quota` command 返回 `PlatformQuota`；platform 表持久列 `est_balance_remaining / est_coding_plan / last_real_query_at / estimate_count`；手动预算 `ManualBudget{amount,consumed,window_*}` | command 注册 `src-tauri/src/startup.rs:241-243`、前端 `src/services/api/platforms.ts:591-595`、类型 `src/services/api/types/manual.ts:300-327`、列清单 `src-tauri/crates/aidog_db/src/platform.rs:5`、余额逐请求扣减 `platform.rs:494-500`、`src/services/api/types/generated/ManualBudget.ts` |

## 1. 矩阵

| # | 图表 | 所需数据形状（维度×度量×粒度） | 现有 API 可否喂 | 缺口 | 落点建议 |
|---|---|---|---|---|---|
| 1 | 折线（趋势） | 时间 × 任选度量（cost/requests/tokens/成功率/均延迟）× minute~daily | ✅ **完全可喂**：`statsApi.query` 的 `buckets[]` 每桶自带全部度量（`StatsBucket`，models/stats.rs:92-108） | 无 | 已有实现：`src/components/shared/CostTrendChart.tsx:20`（浮窗 cost 曲线）、`src/components/PopoverCards.tsx:185-198`（today→hourly / 7d/30d→daily 选粒度）。新折线图直接复用同一 query，零后端改动 |
| 2 | 柱状 | 同折线（时间 × 单度量） | ✅ **完全可喂**（同上 `buckets[]`） | 无 | 已有实现：`src/pages/Home.tsx:126`（24h hourly 三曲线/柱）+ `Home.tsx:259`（今日 hourly 主图）。新柱状图零后端改动 |
| 3 | 堆叠面积 | 时间 × N 系列（platform/model/group）× 度量，即 **时间×维度交叉分组** | ❌ **喂不了**：`buckets[]` 只有整窗合计，`dimension_data[]` 只有整窗单维度 top50（query_stats.rs:357/413 `LIMIT 50`），两者拼不出每桶每系列 | 需要后端新聚合：`GROUP BY time_bucket, dim`。**无需新表新列**——`stats_agg_hourly` 本身就是 (time_hour×model×group×platform) 行（lib.rs:59-76），SQL 加一维 GROUP BY 即得 hourly/daily 堆叠；minute 级复用现有 minute 路径的内存聚合改按 桶+维度 双键累计（query_stats.rs:670-677 已逐行取全维度字段） | `aidog_stats/src/query_stats.rs` 加 `group_by` + 时间桶联合分组分支（建议新返回字段 `series: [{name, buckets}]`），聚合表路径一条 SQL；分批第一优先 |
| 4 | 饼图（占比） | 单窗口 × 维度 × 单度量合计 | ✅ 可喂：`dimension_data[]`（group_by=platform/model/group，含 total_cost/tokens/requests 等 8 度量，models/stats.rs:109-124） | 小缺口：维度硬编码 `LIMIT 50`（query_stats.rs:357/413），>50 个系列时饼图份额失真（当前 65 协议、模型上千，model 维度可能触顶） | 落点建议：`LIMIT` 参数化（StatsQuery 加可选 `limit`，缺省 50 兼容）；前端 top-N + 其他聚合兜底 |
| 5 | 环形 | 同饼图（数据形状相同） | ✅ 同上 | 同上 | 同上，纯前端视觉变体 |
| 6 | 堆叠柱 | 同堆叠面积（时间 × 维度 × 度量） | ❌ 同 #3 | 同 #3（同一个后端缺口，一次补齐喂两张图） | 同 #3 |
| 7 | 热力图 | ① 时刻×日期矩阵（hour-of-day × day）② 时刻×平台/模型 | ① ✅ 可喂：hourly `buckets[].time_bucket` = `"YYYY-MM-DD HH:00:00"`（本地时区，query_stats.rs:316-319 取 `time_hour`），前端纯 pivot 即得 24×N 矩阵，零后端改动 ② ❌ 同 #3 需时间×维度交叉 | ② 同 #3；① 无缺口。另注意：分钟级热力图受 `proxy_log` retention 影响长窗口可能缺行（retention 删整行，CLAUDE.md「Proxy 日志」节），hourly 聚合表不受影响 | ① 先行（纯前端）；② 并入 #3 的交叉聚合票 |
| 8 | 散点 | 请求级点集：x=duration_ms/tokens，y=cost/tokens，按 status/model 着色 | ⚠️ **半可喂**：`proxyLogApi.listFiltered`（proxy.ts:44-45）返回请求级 `ProxyLogSummary`（duration_ms/tokens/status_code/created_at/model/platform_id 全有），但 ① **缺 est_cost**（ProxyLogSummary.ts 字段清单无此列）→ cost 轴画不了 ② 分页 50 条/页，无大批量拉取路径 | ① 给 summary 加 `est_cost` 列（Rust SELECT 加一列 + ts-rs 重新导出）；② 散点需要千行级数据，要么放大 limit 上限要么加专用聚合（如 duration 分桶直方图） | 落点建议：`gateway/db/proxy_log.rs` 的 list SELECT 补 est_cost；散点图服务端预分桶（直方图/密度）比裸拉全行更省 IPC，建议直方图聚合 command |
| 9 | 仪表盘（配额） | 快照值：余量 % / utilization（0-100）+ resets_at；数据源 = 上游 quota 查询 或 本地估算 | ✅ 可喂：`PlatformQuota.coding_plan.tiers[].utilization/remaining/limit/resets_at` + `balance{remaining,total,used}`（manual.ts:300-327）按需实时查（platforms.ts:591）；无脚本平台回落 `platform.est_balance_remaining` 本地估算（platform.rs:5，逐请求扣减 platform.rs:494-500）；手动预算 `ManualBudget.consumed/amount`（generated/ManualBudget.ts） | ① **无历史序列**：quota 查询结果不落库存档（`queried_at` 只活在当次响应），仪表盘只能是快照，画不出配额趋势线 ② `est_balance_remaining` 是单调估算（扣减制），不是真实历史 | 快照仪表盘零后端改动即可做（复用 `usePlatformQuota`，src/pages/platforms/usePlatformQuota.ts:9）。配额趋势需要新采样表（quota result 定期落库），属独立票，建议后置 |

## 2. 缺口清单（按堵图程度排序）

1. **时间×维度交叉聚合**（堵 #3 堆叠面积、#6 堆叠柱、#7② 维度热力图）：`statsApi.query` 只出整窗合计或整窗单维度。修法 = `query_stats.rs` 聚合表路径 SQL 加一维 GROUP BY（`stats_agg_hourly` 已含全部维度列，无 schema 改动）+ minute 路径双键内存聚合 + `StatsResult` 加 series 形状。一次改动喂三张图。
2. **ProxyLogSummary 缺 est_cost + 无批量路径**（堵 #8 散点 cost 轴）：list SELECT 加列 + limit 放宽或改服务端直方图。
3. **配额无历史存档**（堵 #9 仪表盘趋势）：quota 结果不落库，快照仪表盘不受影响，趋势需新表 + 定时采样，建议独立票。
4. **dimension LIMIT 50 硬编码**（劣化 #4/#5 饼图占比精度）：参数化即可，低风险。
5. **status_code 细分丢失**：聚合表只存 success/error 两计数（lib.rs:66-67），429/5xx 分解需回 `proxy_log`（blocked_by/blocked_reason 列在，schema_early.rs:159-163）。九图之外的扩展需求，记一笔。

## 3. 落点/分批建议

- **批次一（零后端）**：折线、柱状、饼图、环形、时刻×日期热力图、快照配额仪表盘 —— 全部现有 API 可喂，纯前端图表组件工作。
- **批次二（一个后端票）**：堆叠面积、堆叠柱、维度热力图 —— 共用缺口 1，`query_stats.rs` 交叉聚合 + `StatsResult.series`。
- **批次三（独立小票）**：散点（缺口 2）、配额趋势（缺口 3）、LIMIT 参数化（缺口 4）。
