# Research: Coding Plan 配额基数可行性（关键项）

- **Query**: 各 coding plan 上游 API 是否含绝对配额（总 token / used 绝对量）能反推基数？
- **Scope**: internal（从 quota.rs 解析逻辑反推上游响应结构）
- **Date**: 2026-06-11

## 核心结论：分平台，不统一

| 平台 | 上游 API | 上游原始响应含绝对量？ | 当前 quota.rs 是否保留 | 基数策略 |
|---|---|---|---|---|
| **Kimi** | `GET api.kimi.com/coding/v1/usages` | **是**：`limit`+`remaining`（5h `limits[].detail`；周 `usage`） | **否，已丢弃**（只算 utilization%） | **方案 A 直接可行** |
| **GLM (智谱)** | `GET {base}/api/monitor/usage/quota/limit` | **否**：仅 `percentage`+`nextResetTime` | 仅 utilization% | 方案 B / C |
| **MiniMax** | `GET .../coding_plan/remains` | **否**：仅 `*_remaining_percent` | 仅 utilization% | 方案 B / C |

### 证据 (file:line)
- Kimi 5h：`quota.rs:265-268` 取 `detail.limit` / `detail.remaining` 算出 utilization 后**丢弃绝对值**（只 push tier，:272）
- Kimi 周：`quota.rs:278-285` 同样有 `usage.limit`/`usage.remaining`，同样丢弃
- GLM：`quota.rs:333` 上游字段名即 `percentage`，无绝对 token 数（:333-336 只取 percentage + nextResetTime）
- MiniMax：`quota.rs:385/391` `current_interval_remaining_percent` / `current_weekly_remaining_percent`，仅百分比

## 推荐策略

### Kimi — 方案 A（推荐，零猜测，精度同余额）
上游 `limit`（绝对 token 配额）/`remaining` 已精确。改造：`query_kimi_coding_plan` 返回时附带绝对 `limit`/`remaining`（需扩展 `QuotaTier` 或 `CodingPlanInfo` 字段）。增量：同窗口 `remaining -= input+output+cache`，`utilization=(limit-remaining)/limit*100`。校准时上游覆盖。

### GLM / MiniMax — 上游无基数，三选一
- **方案 B（utilization 拟合，推荐默认）**：两次真查间记 `Δutilization` 与期间 `Σtoken`，算 `pct_per_token = Δutilization/Σtoken`；后续每请求 `utilization += token×pct_per_token`。
  - 冷启动：首校准周期无系数 → 该周期不预估 coding plan，显示上次真值。
  - 窗口 reset（5h/weekly）时 utilization 跳变会污染拟合 → 检测 `resets_at` 跨越或 utilization 下降丢弃样本。
- **方案 C（用户配置每窗口 token 上限）**：需用户输入，多数不知道，不推荐为默认。

### 统一设计建议
- enum 区分预估能力：`Precise`(Kimi 有绝对基数) / `Fitted`(GLM/MiniMax 拟合) / `ConfigRequired`(用户填)。
- 拟合方案需 platform 新列存 `pct_per_token` 系数 + 上次真查 utilization 基线。
- coding plan 是「按窗口配额」非「累计余额」，预估须感知 `resets_at` 重置，重置后归零重累计。

## Caveats / Not Found
- `需要: 抓取 GLM/MiniMax coding plan API 完整响应样例` — quota.rs 只解析部分字段，无法 100% 排除隐藏绝对量字段。推测: 订阅制按 % 暴露，绝对基数大概率不可得。
