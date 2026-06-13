# Research: 现有金额颜色逻辑（前端）

- **Query**: src/components/shared 里 colorScale/BalanceBar/StatChip/utilColor 实现 + Platforms.tsx 金额/余额/coding plan tier 当前怎么定色
- **Scope**: internal
- **Date**: 2026-06-13

## Findings

### Files Found

| File Path | Description |
|---|---|
| `src/components/shared/colorScale.ts` | 语义级别 → CSS 变量映射 + 各指标阈值分档函数 |
| `src/components/shared/BalanceBar.tsx` | 余额进度条，内含 `remainingLevel` 按剩余占比分档 |
| `src/components/shared/StatChip.tsx` | 小统计 chip，接受 `level` 或 `color` |
| `src/pages/Platforms.tsx` | 列表页 + 详情下拉的金额/余额/tier 渲染与 `utilColor` |

### 颜色基础设施（已有 var(--color-*) 用法，全量走 CSS 变量）

`colorScale.ts` 是统一色编码层（colorScale.ts:1-3 注释明确"禁直接返回硬编码十六进制主题色；全部走 var(--color-*)"）：

- `ColorLevel = "success" | "warning" | "danger" | "neutral"`（colorScale.ts:6）
- `levelColor(level)` → `var(--color-${level})`（colorScale.ts:9-11）
- `levelBg(level)` → `var(--color-${level}-bg)`（colorScale.ts:14-16）
- `costLevel(cost, warnAt=1, dangerAt=10)`：`<warnAt` success / `<dangerAt` warning / 否则 danger（colorScale.ts:43-48）。**这是"绝对阈值"成本分档，不是按使用速率**。

### 三处当前定色逻辑（file:line + 片段）

**1. 余额条 — `BalanceBar.remainingLevel`（占比阈值，非速率）**
`BalanceBar.tsx:24-28`：
```ts
function remainingLevel(pct: number): ColorLevel {
  if (pct >= 50) return "success";
  if (pct >= 20) return "warning";
  return "danger";
}
```
`pct = remaining/total*100`（BalanceBar.tsx:34），**需要 total 作分母**；无 total → `neutral`（BalanceBar.tsx:35）。列表页调用：`Platforms.tsx:1214 <BalanceBar remaining={quota.balanceRemaining} total={quota.balanceTotal} .../>`。多数预估余额平台 `balanceTotal=null`（computeQuotaDisplay 预估分支 balanceTotal 恒为 null，Platforms.tsx:956），故进度条不渲染、颜色退化 neutral，金额数字本身用 `levelColor(neutral)`。

**2. coding plan tier — `utilColor`（利用率阈值）**
`Platforms.tsx:1002-1006`：
```ts
function utilColor(utilization: number): string {
  if (utilization < 50) return "var(--color-success)";
  if (utilization < 80) return "var(--color-warning)";
  return "var(--color-danger)";
}
```
用于列表页 header tier 徽标（Platforms.tsx:1254-1255 背景 `${utilColor}15` + 前景 `utilColor`）和详情区 StatChip（Platforms.tsx:1356 `color={utilColor(tier.utilization)}`）。**纯利用率分档，不看剩余时间/周期**。

**3. 详情区"已使用"成本 chip — `costLevel`**
`Platforms.tsx:1336`：`<StatChip ... value={$cost} label="cost" level={costLevel(u.total_cost)} />`（绝对成本阈值）。

**4. 手动预算（token 单位）** `Platforms.tsx:1224,1229`：内联三元 `depleted→danger / ratio<0.2→warning / 否则 success/primary`（占比阈值）。

### computeQuotaDisplay 数据装配（Platforms.tsx:938-976）
- `tierRemain(util) = clamp(100 - util)`（Platforms.tsx:939）——tier 展示"剩余%"，颜色仍走 `utilColor(utilization)`。
- 预估分支用 `p.est_coding_plan`（解析为 `EstCodingPlan`，含 `est_utilization`）；真查分支用 `q.coding_plan.tiers`（含 `utilization`/`resets_at`/`limit`/`remaining`）。
- `QuotaDisplay.tiers` 字段：`{name, remainPct, utilization, resetsAt, limit, remaining}`（Platforms.tsx:933）——**resetsAt 仅真查分支有值；预估分支 resetsAt=null**（Platforms.tsx:950）。

### 倒计时已有但未用于颜色
`formatResetCountdown(resetsAt)`（Platforms.tsx:987-999）已能把 `resets_at` ISO/millis 转人类可读剩余时间，但仅用于详情区显示文案（Platforms.tsx:1360），**不参与颜色计算**。

## Caveats / 结论（数据是否已有 / 缺什么 / 改造点）

- **已有**：完整 `var(--color-success/warning/danger)` 语义色基础设施（colorScale.ts），三处定色入口集中（`remainingLevel` / `utilColor` / `costLevel`）。
- **缺**：所有现有定色都是**静态阈值**（占比/利用率/绝对成本），**没有任何"使用速率 / 剩余可用时间"概念**。
- **改造点（前端）**：
  1. 余额色：改 `BalanceBar`（或新增专用函数）按"余额/日用量=剩余天数"分档（<1 红/<3 黄/否则绿），需要外部传入"剩余天数"或"日用量"——前端当前无此数据（见 03 文件）。
  2. coding plan 色：改 `utilColor` → 按"剩余时间% = 剩余时间/周期时长"分档（<40 红/40-60 黄/否则绿），需要"剩余时间"(resetsAt 可推) + "周期时长"(当前无，见 04 文件)。
  3. `resetsAt` 仅真查 coding 分支有；预估分支为 null —— 列表页常态展示预估值，改造需补预估侧的 reset/周期来源。
