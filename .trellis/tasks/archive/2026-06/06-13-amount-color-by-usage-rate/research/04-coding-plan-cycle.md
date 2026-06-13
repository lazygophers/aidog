# Research: coding plan 周期时长来源

- **Query**: QuotaTier 的 resets_at + utilization；"预估周期时长"(five_hour=5h, weekly=7d) 从哪来——硬编码还是 API；怎么算"剩余时间/周期时长"得剩余时间%
- **Scope**: internal
- **Date**: 2026-06-13

## Findings

### QuotaTier 结构（`src-tauri/src/gateway/quota.rs:55-68`）
```rust
pub struct QuotaTier {
    pub name: String,            // "five_hour" | "weekly_limit"（部分平台另有 mcp_monthly）
    pub utilization: f64,        // 0-100
    pub resets_at: Option<String>, // ISO8601 或 null
    pub limit: Option<f64>,
    pub remaining: Option<f64>,
}
```
TS 对应 `QuotaTier`（`src/services/api.ts:750-754`）。

### "周期时长"当前**没有显式字段** —— 仅靠 tier name 隐含约定

- tier name 是规范化档名：`five_hour` / `weekly_limit`（注释 quota.rs:48,56；estimate.rs EstTier 注释 estimate.rs:37 "five_hour | weekly_limit"）。
- 各平台解析器把上游不同窗口映射到这两个名（如 GLM `unit=3→5h, unit=6→weekly`，quota.rs:312-328；前端 tierLabel `five_hour→"5h"`、`weekly_limit/seven_day→"7d"`，Platforms.tsx:980-981、editors.tsx:1784）。
- **没有任何地方持久化"周期秒数 / 窗口起止时间戳"**。estimate.rs `EstTier`（estimate.rs:35-56）字段为 `est_utilization/coef_per_token/util_at_last_real/tokens_since_real/has_base/limit`——**无 window_start / period_duration**。
- estimate.rs `tier_pace` 注释明说"数据不足（**无窗口起止时间戳持久化**）时退化为按当前 est_utilization 阈值估算"（estimate.rs:125-126）——即**项目从未持久化周期时长，pace 全靠利用率阈值**。

### `resets_at` / `reset_at` 来源
- **仅真查（quota.rs）从上游 API 取**：上游 JSON 的 `resetTime` 字段（Kimi: quota.rs:258-260,272-274；GLM: quota.rs:340 `reset_iso`），可能是 ISO 字符串或毫秒（`millis_to_iso8601` 兜底）。
- **预估侧（estimate.rs EstTier）无 reset_at**；group-info 把预估 tier 的 `reset_at` 写死 `None`（proxy.rs:219）。manual budget 压成的 tier 也 `reset_at: None`（proxy.rs:252）。
- 故 statusline coding 段的 `reset_at`（editors.tsx:1789 取 `.coding_plan[].reset_at`）在纯预估场景拿不到值——红色时的"(reset Xh Ym)"只在真查 + 上游给 resetTime 时才出现。

### 当前 pace（颜色依据）算法——**不是按剩余时间速率**
`tier_pace`（estimate.rs:147-159）：
```rust
let util = tier.est_utilization;
if util >= 80.0 { Fast } else if util >= 40.0 { Normal } else { Busy }
```
纯利用率阈值。语义注释（estimate.rs:118-126）声称"以窗口内利用率随时间的预期推进判定"，但实现退化为静态利用率分档（因无窗口时间线数据）。

### 要算"剩余时间% = 剩余时间 / 周期时长"需要的两个量
1. **剩余时间**：`resets_at - now`（ISO/millis 可推；前端 `formatResetCountdown` 已做差值，Platforms.tsx:987-999）。**但仅真查 + 上游给 resetTime 才有；预估常态缺失。**
2. **周期时长**：**当前无任何来源**。可选实现路径：
   - (a) 按 tier name 硬编码：`five_hour=5h=18000s`、`weekly_limit=7d=604800s`、`mcp_monthly≈30d`（与前端 tierLabel/editors.tsx 既有映射一致，最小改动）。
   - (b) 从两次 resets_at 间隔或上游窗口字段推断（数据不全，不可靠）。
   → 任务文案"预估周期时长(如 five_hour=5h, weekly=7d)"指向 **(a) 按 tier name 硬编码**。

剩余时间% = `(resets_at - now) / period_duration(name)`，再按 `<40% 红 / 40-60% 黄 / 否则绿` 上色。

## Caveats / 结论

- **"预估周期时长"怎么得：当前项目无该数据，需新增——最务实方案是按 tier name 硬编码周期常量（five_hour→5h、weekly_limit→7d、mcp_monthly→30d），与既有 name 规范化约定一致。**
- **"剩余时间"依赖 `resets_at`，当前仅真查路径 + 上游返回 resetTime 时才有**；纯预估（est_coding_plan）路径 `reset_at=None`（proxy.rs:219）。改造需决定：
  - 预估侧也补 reset_at（需持久化窗口起点 + 周期常量推算 reset = window_start + period），否则预估场景算不出"剩余时间%"，只能退回利用率阈值。
- **最大不确定点**：
  1. **周期时长来源**——确认走"按 name 硬编码常量"(本研究判断的唯一可行路径)，并定义 mcp_monthly 等非标准档的周期。
  2. 预估常态下 `resets_at` 缺失——是否需要在 estimate.rs EstTier 新增窗口起点持久化以支撑预估侧"剩余时间%"，否则该新颜色规则在最常见的预估展示路径上无法生效，会静默退回旧利用率阈值。
