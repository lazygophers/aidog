# PRD: 分组使用情况按分组标识聚合

## 背景

请求记录 proxy_log **已有 `group_name` 列**(migrations/001_init.sql 建表起就有 + `idx_proxy_log_group` 索引)。proxy.rs:609 在 resolve_group 后写入 group.name; 成功请求全填充, 400/404 早退行留空 `''`。

但 Groups 页卡片使用情况现状 (research/02): Groups.tsx:251-292 前端对关联 platforms 的 `platformApi.usageStats` **求和 = 平台级聚合**。问题: 一个平台被多个 group 引用时, 其全量请求被**重复计入每个 group**, 非"本分组的请求"。

**关键发现**: 后端按 group 维度聚合**全链路已就绪** — `get_group_usage_stats`(db.rs:1320, `WHERE group_name=?1 AND deleted_at=0`) + command `group_usage_stats`(lib.rs:1155) + api `groupApi.usageStats`(api.ts:362)。Groups.tsx 只是没用它。

## 目标

Groups 卡片使用情况改为**只含本分组的请求数据**(按 proxy_log.group_name 过滤), 而非关联平台的全部数据。

## 决策

- 后端**无需新增** query/command/索引(已就绪)。改造**纯前端**: Groups.tsx 切换数据源。
- group_name 空值('', 400/404 早退请求)天然不计入任何 group, 符合"只含本 group"语义, 不特殊处理。
- balance / 余额链路维持现状(平台级, 不按 group 拆)。

## 范围

- `src/pages/Groups.tsx`: load + refreshStats 两处(251-292)把"对 `g.platforms` 的 PlatformUsageStats 求和"改为调 `groupApi.usageStats(g.group.name)`。移除 `_platformStats`(202) 求和逻辑(先确认无其他消费者)。
- 文档: 更新 CLAUDE.md「Group 统计」段(从"关联 platforms 聚合"改为"按 group_name 查 proxy_log") + memory `group-stats-aggregation`。

## 非目标

- 不改后端聚合 query(已就绪)
- 不改 balance / 余额展示(维持平台求和)
- 不改 proxy_log group_name 写入逻辑(已正确填充)
- 不为 400/404 空 group_name 请求补归类

## 风险 (research/03)

- **与重试 task 同改 Groups.tsx**: 重试 task 加 max_retries 字段, 本 task 改 stats 数据源, 区段不同但需 worktree 基线协调(本 task 实施前 worktree 同步含重试 task 改动)。
- group 级 recent 健康度: `get_group_usage_stats` helper 现恒返回 recent=0(db.rs:1285-1286), 卡片成功率展示需确认不退化(或维持现状标注)。
- `_platformStats`(Groups.tsx:202) 移除前确认无其他消费者。

## 验收标准

- Groups 卡片 tokens/cost/请求数 = 仅该 group 的 proxy_log 聚合(被多 group 共享的平台不再重复计入)
- 切换数据源后无残留平台求和逻辑(除非 balance 仍需)
- group_name 空请求不计入任何 group(符合语义)
- CLAUDE.md + memory group-stats-aggregation 更新准确
- yarn tsc 0 error 无新增 warning
- 与重试 task 的 Groups.tsx 改动不冲突(实施时 worktree 已含重试改动)

## 编排

单一交付(纯前端切数据源 + 文档), 单 worktree, main 在 worktree 内 inline 实施(轻量)。**依赖**: 与重试 task 同改 Groups.tsx → **必须等重试 task merge 后再 start**, worktree 基线同步含重试改动, 避免冲突。串行排在重试 task 之后。
