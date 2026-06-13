# Research: 改造点 + 风险

- **Query**: 实现"Groups 卡片只含本 group 请求"需改哪、风险、衔接
- **Scope**: internal
- **Date**: 2026-06-13

## 结论速览

- **后端 query / command / api 全已就绪**（`get_group_usage_stats` / `group_usage_stats` / `groupApi.usageStats`），改造重心在 **Groups.tsx 前端切换数据源**。
- 索引 `idx_proxy_log_group` 已现成，按 group_name 聚合不缺索引。
- 主要风险：与重试 task（同改 Groups.tsx）冲突、balance 链路是否一并改、空 group_name 归类、auto 分组的 platform_id=0 行覆盖。

## 改造点

### 1. 前端切换数据源（核心改动）

`src/pages/Groups.tsx:251-292`（`load`）+ `299-344`（`refreshStats`）两处求和块改为按 group 查：
- 现：`platformApi.usageStats(plat.id)` → 前端对 `g.platforms` 求和。
- 改：对每个 group 调 `groupApi.usageStats(g.group.name)`（`src/services/api.ts:362` 已存在），直接拿按 group_name 过滤的 stats，去掉前端求和循环。
- `_platformStats`（Groups.tsx:202, 252-259）若仅用于求和，可同步移除；需确认无其他消费者。

注意：`load` 和 `refreshStats` 两处逻辑重复（Groups.tsx:260-292 与 309-342 几乎相同），改造时两处都要改，建议抽公共函数。

### 2. 后端（基本无需新增）

- `get_group_usage_stats` 已实现（`db.rs:1320-1328`），命令 `group_usage_stats` 已注册（`lib.rs:1155-1157, 2490`）。
- 若卡片需要 `recent_failures/recent_total`（健康度），注意 `get_group_usage_stats` 走的是 `usage_stats` helper，该 helper 返回的 recent_* 恒为 0（`db.rs:1285-1286`）；只有 `get_platform_usage_stats` 在 `db.rs:1292-1304` 额外补了 recent 健康度。如卡片需 group 级健康度，需给 group 查询补同款"最近 5 次"子查询。当前 Groups 卡片求和也把 recent 置 0（Groups.tsx:287），故大概率不需要。

### 3. 空 group_name 请求归类

400/404 早退行 group_name=''（见 01）。`get_group_usage_stats` 用 `group_name = ?1` 精确匹配，空行天然不计入任何真实 group → 符合"只含本 group"语义，无需额外处理。除非产品要单独展示"未匹配请求"。

## 风险 / 衔接

### 与重试 task 冲突（同改 Groups.tsx）
- 另一 task "平台重试改 db.rs/Groups.tsx + Groups.tsx max_retries"。两者**都动 Groups.tsx**，存在合并冲突风险。建议串行或明确文件区段：重试 task 改 proxy_log attempts / max_retries 字段渲染；本 task 改 usage stats 数据源（`load`/`refreshStats` 的求和块）。区段不同但同文件，需协调先后。

### 与 group-stats-aggregation memory 衔接
- memory `group-stats-aggregation.md` + CLAUDE.md 记"Group stats 从关联 platforms 聚合（前端求和）"。本改造**推翻**该结论 → 改用按 group_name 查 proxy_log。完成后需更新该 memory 与 CLAUDE.md 关键约束段，否则文档与实现漂移。

### balance 链路
- 余额聚合（Groups.tsx:270-271, 290）仍按平台 `est_balance_remaining` 求和，与 usage stats 独立。balance 本质是平台属性、无 per-group 概念，**不应**也无法按 group_name 拆。本 task 只改 usage stats，balance 维持平台求和。

### 索引 / 数据量
- `idx_proxy_log_group ON proxy_log(group_name) WHERE deleted_at = 0`（`migrations/001_init.sql:97`）已现成，且与 `get_group_usage_stats` 的 `group_name = ?1 AND deleted_at = 0` 完全匹配。**无需新增索引。**
- 大数据量下按 group_name 聚合走该部分索引，性能可。

### auto 分组 platform_id=0 行
- auto 分组日志可能 platform_id=0（`db.rs:1310-1311` 注释）。`get_group_usage_stats` 纯按 group_name 过滤，不依赖 platform_id，故 auto 分组行只要 group_name 正确就能正确计入，反而比平台维度更干净。

## Caveats / Not Found

- 未确认 `_platformStats`（Groups.tsx:202）除求和外是否还有别的渲染消费者；移除前需 grep 全文件确认。
- 未读 `resolve_group` 实现，无法 100% 确认"直连/默认 group"场景 group_name 是否非空（影响这些请求是否计入某 group）。
- 卡片是否需要 group 级 recent 健康度，取决于 UI 需求，当前求和实现是置 0，未展示。
