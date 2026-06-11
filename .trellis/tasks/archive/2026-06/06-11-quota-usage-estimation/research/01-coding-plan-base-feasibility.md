# Research: Coding Plan 配额基数可行性（关键项）

- **Query**: 各 coding plan 平台原始上游 API 响应能否拿到绝对配额基数（总 token / 总请求 / used 绝对量）以支持本地预估增量更新
- **Scope**: internal（quota.rs 解析逻辑反推上游响应结构）
- **Date**: 2026-06-11

## 结论（先说）

| 平台 | coding plan API | 原始响应是否含绝对基数 | 可反推每 token 的 %？ | 推荐基数策略 |
|---|---|---|---|---|
| **Kimi** | `GET api.kimi.com/coding/v1/usages` | **是** — `limit` + `remaining`（5h 桶与周桶都有） | **可**（remaining 是绝对量，直接扣减） | **方案 A（直采绝对量）**，最精确 |
| **GLM / 智谱** | `GET {base}/api/monitor/usage/quota/limit` | **否** — 只有 `percentage`（0-100），无绝对额度 | 否（API 不给基数） | 方案 B（Δutilization 拟合）或方案 C（用户配上限） |
| **MiniMax** | `GET .../coding_plan/remains` | **否** — 只有 `current_interval_remaining_percent` / `current_weekly_remaining_percent` | 否 | 方案 B 或 C |

**核心矛盾**：当前 `QuotaTier` 结构（quota.rs:48-56）只保留 `utilization`(0-100) + `resets_at`，把 Kimi 本来有的 `limit`/`remaining` 绝对量在解析时**丢弃**了。所以「task 已知里说无基数」只是当前 struct 表象 —— **Kimi 上游其实有基数，是被现有解析吃掉了**。

## Findings

### Kimi —— 有绝对基数（被现有解析丢弃）

`query_kimi_coding_plan` quota.rs:245-291：

- 5h 桶 quota.rs:262-275：`limits[].detail.limit`（绝对额度）+ `detail.remaining`（绝对剩余），代码 `used = limit - remaining`，再算 `utilization = used/limit*100`。
- 周桶 quota.rs:277-286：`usage.limit` + `usage.remaining`，同样算法。

```rust
let limit = parse_f64_field(detail, "limit").unwrap_or(1.0);       // 绝对额度
let remaining = parse_f64_field(detail, "remaining").unwrap_or(0.0); // 绝对剩余
let used = (limit - remaining).max(0.0);
let utilization = if limit > 0.0 { (used / limit) * 100.0 } else { 0.0 };
```

→ **Kimi 可直接本地扣减**：拿到 1 次请求的 token 后，`remaining -= used_units`，重算 utilization。但需确认 `limit`/`remaining` 的**单位**（token 数？credit？请求数？）—— 代码未注释。`需要: Kimi limit/remaining 字段单位（token / credit / 请求次数）—— 决定能否用 proxy log 的 token 直接扣减`。

### GLM / 智谱 —— 仅 percentage

`query_zhipu_coding_plan` quota.rs:296-348，解析 `data.limits[]` 中 `type == "TOKENS_LIMIT"` 的条目，只取 `item.percentage`（quota.rs:333）+ `nextResetTime`。响应**无总额度字段**被读取。

```rust
let pct = item.get("percentage").and_then(|v| v.as_f64()).unwrap_or(0.0);  // 只有 %
let reset_ms = item.get("nextResetTime").and_then(|v| v.as_i64());
```

`type == "TOKENS_LIMIT"` 暗示底层是 token 配额，但 API 只暴露百分比。另有 `data.level`（套餐等级，quota.rs:326）可作为方案 C 查表基数的 key。
`需要: GLM /api/monitor/usage/quota/limit 完整响应样例 —— 确认 limits[] 是否含隐藏的绝对额度字段（如 totalTokens/usedTokens）`。

### MiniMax —— 仅 remaining_percent

`query_minimax_coding_plan` quota.rs:353-402，只读 `current_interval_remaining_percent` / `current_weekly_remaining_percent`（quota.rs:385/391），`utilization = 100 - remain_pct`。无绝对额度。按 `model_name == "general"` 过滤（quota.rs:380-382）。
`需要: MiniMax coding_plan/remains 完整响应 —— 确认 model_remains[] 是否含 total/used 绝对量`。

## 推荐策略（供 design）

1. **Kimi（方案 A 直采）**：扩展 `QuotaTier` 增 `limit: f64` + `remaining: f64`（绝对量），首次真实查记录基数，proxy 每请求后 `remaining -= delta_units` 本地扣减。前提是确认单位。
2. **GLM / MiniMax（方案 B 拟合 或 方案 C 配置）**：
   - **方案 B**：存两次真查的 `(utilization, 累计预估 token)`，拟合「每 token → Δ%」斜率，之后用 proxy token 累计 × 斜率 推进 utilization。冷启动（首两次真查之间）无斜率，退化为「不预估只等校准」。窗口重置（resets_at 到点）需清零重新拟合。
   - **方案 C**：用户在平台配置里填 coding plan 的「窗口总 token 上限」（按 level/套餐），proxy token 累计 / 上限 = utilization。最简单稳健但需用户输入。
3. **混合推荐**：Kimi 走 A；GLM/MiniMax 默认走 B（自适应、零配置），B 不可用时（首校准前 / 重置后）UI 标「预估不可用，等待校准」。方案 C 作为可选高级覆盖。

## Caveats / Not Found

- 三个平台的**原始 JSON 响应样例缺失**，结论基于 quota.rs 解析逻辑反推字段存在性，不能 100% 排除响应里有未被读取的绝对量字段（尤其 GLM/MiniMax）。三处 `需要:` 标注需 design/用户提供真实响应或抓包确认。
- `query_quota`（quota.rs:407）按 base_url 字符串匹配分发，未来加平台需同步扩展。
- coding plan 的「units」语义跨平台不一致（Kimi 可能是 token/credit，GLM 是 token%，MiniMax 是 %），预估增量逻辑必须 per-platform 分支，不能统一公式。
