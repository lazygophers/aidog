# PRD: 金额颜色按使用速率统一计算

## 背景

当前所有"金额/额度"颜色都是**静态阈值**, 与真实使用速率无关:
- `BalanceBar.remainingLevel` (src/components/shared/BalanceBar.tsx:24-28) — 按余额占比阈值; 多数预估余额平台 `balanceTotal=null` 退化中性色
- `utilColor` (src/pages/Platforms.tsx:1002-1006) — 按 coding plan 利用率阈值
- `costLevel` (src/components/shared/colorScale.ts:43-48) — 按绝对成本

statusline 侧后端**已部分实现**: group-info handler (proxy.rs:119-290) 已下发 `balance_days_remaining` + coding `pace`; bash 脚本 (editors.tsx 生成, lib.rs:1402 `generate_statusline_script`) 已用 ANSI 24-bit truecolor 三色。但其 pace 语义 = 利用率阈值 (estimate.rs `tier_pace` >=80 fast), **不是**本任务要的剩余时间速率。

## 目标

把全部金额颜色 (列表页 / 详情下拉 / statusline) 统一改为**按使用速率算红黄绿**:
- 速率过快 / 快不够用 → 红
- 临界 → 黄
- 充足 → 绿

红黄绿统一走 `var(--color-danger / --color-warning / --color-success)`。

## 算法规范 (唯一事实源)

### A. Coding Plan tier — 额度消耗速率 pace

```
周期时长 cycle (按 tier name 硬编码):
  five_hour     → 5h
  weekly_limit  → 7d (168h)
  mcp_monthly   → 30d
  其它未知 name → 回退利用率阈值 (无周期概念)

剩余时间 remain:
  真查路径   = resets_at - now
  预估路径   = (window_start + cycle) - now   (window_start 见 D)
elapsed_ratio = clamp((cycle - remain) / cycle, 0, 1)   # 时间已过比例
util_ratio    = utilization / 100                        # 额度已用比例

pace = util_ratio / elapsed_ratio        # elapsed_ratio→0 时 pace→∞
剩余可用时间% = clamp(100 / pace, 0, 100) # pace<1 (省着用) 视作 100% 充足

颜色:
  剩余可用时间% < 40   → 红 (pace > 2.5,  烧太快撑不到重置)
  40 ≤ 剩余 ≤ 60       → 黄 (pace 1.67~2.5)
  剩余 > 60            → 绿 (pace < 1.67)
```

校验用户例子: 剩余可用时间 50% ↔ pace=2 ↔ 黄区边缘, 自洽。

### B. 余额 — 预估剩余可用天数 days_remaining

```
日用量速率 (动态窗口):
  取最近有 est_cost 数据的最大窗口 span: 优先 7d, 数据不足逐步回退 (6d/5d/.../最小 5min)
  即 span = clamp(now - 最早有效数据时间, 5min, 7d)
  rate_per_hour = SUM(est_cost in span) / span_hours      # 与余额同币种金额
days_remaining = (余额 / rate_per_hour) / 24              # 小时精度, 再换算天

颜色 (示例 日用量=100 时即 <100红/<300黄):
  days_remaining < 1   → 红
  days_remaining < 3   → 黄
  否则                 → 绿
  rate_per_hour == 0 (无任何用量) → 中性/绿 (不报警)
```

`est_cost` 聚合: db.rs `query_stats` 时间桶 (1443-1450) 已支持 daily/hourly GROUP BY `SUM(est_cost)` + `filter_group`; `created_at` 为 unix 毫秒。底层桶查询够用, 需写上层"动态窗口日速率"包装 (现有 `get_group_spent_since` 总和/7 不满足)。

### C. 颜色阈值常量集中

红黄绿边界常量 (pace 2.5/1.67, days 1/3) 必须**前后端统一**定义, 禁前端 / statusline / 后端各写一套漂移。后端算出语义级字段 (剩余可用时间% / days_remaining / level) 下发, 前端与 statusline **只消费 level 不重算阈值** (最小漂移面)。

### D. Coding 预估侧窗口起点持久化

预估路径无 `resets_at` (proxy.rs:219 恒 None)。需在 `EstTier` (estimate.rs:35-56) 持久化 `window_start` (本周期起点 unix ms): 首次真查 / 上游 resets_at 推算时落地, 之后预估侧用 `window_start + cycle` 算 remain。无 window_start 时该 tier 退中性色, 不静默走旧利用率阈值。

## 范围

- 后端: estimate.rs (pace 算法 + EstTier.window_start) / db.rs (动态窗口日速率查询) / quota.rs / proxy.rs group-info handler (下发 level 字段) / models.rs (字段)
- 前端: BalanceBar.tsx / colorScale.ts / Platforms.tsx (utilColor / 余额色) — 改为消费后端 level
- statusline: editors.tsx 脚本生成 + group-info 字段对齐新语义 (现有 ANSI 三色复用, 阈值对齐)
- i18n: 若新增文案走 7 语言 key

## 非目标

- 不改余额 / 配额的数值展示与查询频率, 只改颜色
- 不新增上游 API 调用
- 不动 countdown 展示 (上个任务已完成)

## 验收标准

- Coding tier 颜色按 pace 算: 构造 pace=2.0/3.0/1.2 三组数据, 分别得黄/红/绿
- 余额颜色按 days_remaining 算: 日用量固定时 余额<1日红 / <3日黄 / 否则绿; 无 7 天数据时用现有最大窗口
- 三处前端定色 (BalanceBar / utilColor / costLevel 相关) + statusline 颜色语义一致, 无阈值漂移
- 预估侧无 window_start 的 coding tier 显示中性色, 不误报
- `cargo build` + `yarn tsc` 0 error, 无新增 warning
- i18n 7 语言齐全 (若有新文案)

## 编排

单一交付 (颜色语义统一), 单 worktree。改动跨后端算法 + 前端消费 + statusline, 但强耦合共享阈值常量 + 前端依赖后端新字段 + 同文件密集改动 → 不拆 child, 单 task 内按"后端算法/字段 → 前端消费 → statusline 对齐 → check"串行。
