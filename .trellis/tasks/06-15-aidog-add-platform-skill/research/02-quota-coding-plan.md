# Research: 平台余额 + Coding Plan 配额 + 价格估算接入模式

- **Query**: 摸清「加/改平台」时余额查询、Coding Plan 配额查询、价格估算的可套用子流程模板
- **Scope**: internal
- **Date**: 2026-06-15

涉及文件（全部相对 `/Users/luoxin/persons/lyxamour/aidog`）：

| File | 角色 |
|---|---|
| `src-tauri/src/gateway/quota.rs` | 余额 / Coding Plan 真查实现 + `query_quota` 分派 + New API 两步查询 |
| `src-tauri/src/gateway/estimate.rs` | 请求驱动预估 + 校准 + `EstCodingPlan`/`EstTier` + `calibrate_from_quota` |
| `src-tauri/src/gateway/manual_budget.rs` | 手动预算（无上游 quota 平台的兜底限额） |
| `src-tauri/src/gateway/db.rs` | `resolve_price` 价格回退链 (db.rs:2840) |
| `src-tauri/src/gateway/usage_color.rs` | `cycle_ms_for_tier` tier 周期 + 配色阈值 |
| `src-tauri/src/lib.rs` | `platform_query_quota` / `platform_query_quota_newapi` command + 注册 + 冷启动校准 |
| `src/services/api.ts` | `quotaApi.query` / `queryNewapi` 前端封装 |
| `src/pages/Platforms.tsx` | 唯一前端调用点（Platforms.tsx:1644/1694） |

---

## 1. 余额查询子流程（按量计费平台）

### 通用骨架（照搬 `query_deepseek_balance`，quota.rs:138-156）

所有余额查询函数共享一个结构：用 `quota_get_json`（quota.rs:115-133，统一 GET + 日志 + 错误前缀）发请求，解析字段，返回带 `balance: Some(BalanceInfo{...})` 的 `PlatformQuota`。

签名模式：`async fn query_<plat>_balance(db: Option<&Arc<Db>>, api_key: &str) -> PlatformQuota`
（变体：siliconflow 多个 `is_cn: bool` 控制 cn/com 域名 + 货币，quota.rs:178）

```rust
// 模板：照搬 query_deepseek_balance (quota.rs:138)
async fn query_FOO_balance(db: Option<&Arc<Db>>, api_key: &str) -> PlatformQuota {
    // 1. GET：headers 调用方决定是否加 Bearer 前缀
    let body = match quota_get_json(
        db,
        "https://api.foo.com/user/balance",                    // 固定 URL（多数硬编码全 URL，不用 base_url）
        &[("Authorization", format!("Bearer {api_key}"))],     // 鉴权：多数 Bearer；智谱例外见下
    ).await {
        Ok(v) => v,
        Err(e) => return err_quota_platform("foo", &e),        // 失败兜底：带平台名 (quota.rs:97)
    };
    // 2. 解析（用 parse_f64_field 容错 string/number，quota.rs:87）
    let remaining = parse_f64_field(&body, "balance").unwrap_or(0.0);
    let is_valid = body.get("is_available").and_then(|v| v.as_bool()).unwrap_or(true);
    // 3. 组装 PlatformQuota：余额平台只填 balance，coding_plan: None
    PlatformQuota {
        success: true, error: None, queried_at: now_millis(),
        balance: Some(BalanceInfo {
            remaining,                  // 必填：剩余金额
            total: None, used: None,    // 可选
            currency: "CNY".into(),     // "CNY" | "USD"
            is_valid,
        }),
        coding_plan: None, newapi_user_id: None,
    }
}
```

### 注册分派（quota.rs:373-414 `query_quota`）

`query_quota` 按 `base_url.to_lowercase()` 子串 `if url.contains("...")` 顺序匹配。**Coding Plan 检测在前**（kimi/bigmodel/z.ai），再余额。落空 → `err_quota("Unsupported platform...")`。

新增余额平台：在余额段（quota.rs:392-410）加一行：
```rust
if url.contains("api.foo.com") {
    return query_foo_balance(db, api_key).await;
}
```

### 现有 5 个余额实现的差异点（解析特例参考）

| 函数 | URL | 鉴权 | 解析特点 |
|---|---|---|---|
| `query_deepseek_balance` (quota.rs:138) | `api.deepseek.com/user/balance` | Bearer | 遍历 `balance_infos[]` 累加 `total_balance`；货币 CNY |
| `query_stepfun_balance` (quota.rs:161) | `api.stepfun.com/v1/accounts` | Bearer | 直读 `balance` |
| `query_siliconflow_balance` (quota.rs:178) | `api.siliconflow.{cn,com}/v1/user/info` | Bearer | `is_cn` 切域名 + 货币；读 `data.totalBalance` |
| `query_openrouter_balance` (quota.rs:202) | `openrouter.ai/api/v1/credits` | Bearer | `data.total_credits - data.total_usage` |
| `query_novita_balance` (quota.rs:225) | `api.novita.ai/v3/user/balance` | Bearer | `availableBalance / 10000`（单位 0.0001 USD） |

---

## 2. Coding Plan 配额子流程（订阅制平台）

### 通用骨架（照搬 `query_kimi_coding_plan`，quota.rs:243-282）

签名：`async fn query_<plat>_coding_plan(db: Option<&Arc<Db>>, api_key: &str) -> PlatformQuota`
（智谱多 `base_url: &str` 因要按域名切 endpoint，quota.rs:287）

返回 `coding_plan: Some(CodingPlanInfo{ tiers, level })`，`balance: None`。

```rust
// 模板：照搬 query_kimi_coding_plan (quota.rs:243)
async fn query_FOO_coding_plan(db: Option<&Arc<Db>>, api_key: &str) -> PlatformQuota {
    let body = match quota_get_json(db, "https://api.foo.com/coding/v1/usages",
        &[("Authorization", format!("Bearer {api_key}"))]).await {
        Ok(v) => v,
        Err(e) => return err_quota_platform("foo", &e),
    };
    let mut tiers = Vec::new();
    // 对每个窗口构造一个 QuotaTier
    // name 必须取 cycle_ms_for_tier 已知值之一，否则配色退中性！
    let limit = parse_f64_field(detail, "limit").unwrap_or(1.0);
    let remaining = parse_f64_field(detail, "remaining").unwrap_or(0.0);
    let used = (limit - remaining).max(0.0);
    let utilization = if limit > 0.0 { (used / limit) * 100.0 } else { 0.0 };
    let resets_at = detail.get("resetTime").and_then(|v|
        v.as_str().map(String::from).or_else(|| v.as_i64().and_then(millis_to_iso8601)));
    tiers.push(QuotaTier {
        name: "five_hour".into(),       // ★ 必须 ∈ {"five_hour","weekly_limit","seven_day","mcp_monthly"}
        utilization,                    // 0-100 已用百分比
        resets_at,                      // ISO8601 或由 millis 转；用于推算 window_start
        limit: Some(limit),             // ★ Some → has_base=true → 精确预估；None → 方案 B 拟合
        remaining: Some(remaining),     // 仅暴露绝对量的平台填
    });
    PlatformQuota {
        success: true, error: None, queried_at: now_millis(), balance: None,
        coding_plan: Some(CodingPlanInfo { tiers, level: None }),  // level: 套餐等级名（可选）
        newapi_user_id: None,
    }
}
```

### tier 结构 / name / 倒计时字段

**`QuotaTier`**（真查输出，quota.rs:54-68）：
- `name`：tier 标识，必须命中 `cycle_ms_for_tier`（usage_color.rs:30）已知值：`five_hour`(5h) / `weekly_limit`|`seven_day`(7d) / `mcp_monthly`(30d)。**未知 name → 无周期 → 配色退 Neutral**。
- `utilization`：已用 %（0-100）。
- `resets_at`：ISO8601 重置时刻 → estimate 侧 `derive_window_start` 反推 `window_start = resets_at - cycle`（estimate.rs:236）。
- `limit`/`remaining`：绝对 token 量。`limit.is_some()` 决定 `has_base`（estimate.rs:345）→ Kimi 走精确增量，GLM 等无 limit 走方案 B 拟合。

**`EstTier`**（持久化预估状态，estimate.rs:35-61）多出：`coef_per_token`（方案 B 拟合系数）/ `util_at_last_real` / `tokens_since_real`（拟合基线/分母）/ `has_base` / `limit` / `window_start`（unix ms 周期起点，配色用）。真查→`EstTier` 的转换在 `calibrate_tier`（estimate.rs:188）。

### 注册分派

Coding Plan 在 `query_quota` 段首（quota.rs:380-390）匹配：
```rust
if url.contains("api.foo.com/coding") {
    return query_foo_coding_plan(db, api_key).await;
}
```

### 现有 2 个 coding plan 实现差异

| 函数 | URL | 鉴权 | 特点 |
|---|---|---|---|
| `query_kimi_coding_plan` (quota.rs:243) | `api.kimi.com/coding/v1/usages` | Bearer | `limits[].detail` → five_hour；`usage` → weekly_limit；**带绝对 limit/remaining**（has_base 精确预估） |
| `query_zhipu_coding_plan` (quota.rs:287) | `{open.bigmodel.cn\|api.z.ai}/api/monitor/usage/quota/limit` | **裸 key 无 Bearer** (quota.rs:295) | 按域名切 base；按 `unit` 分类（3→five_hour, 6→weekly）；`TIME_LIMIT` 类型→mcp_monthly；无 limit→方案 B 拟合；带 `level` |

---

## 3. 价格估算接入（按量平台才需要）

### resolve_price 回退链（db.rs:2840-2905，单一事实源，禁绕过）

签名：`resolve_price(db, model_name, platform_type, fallback_input, fallback_output) -> Result<ResolvedPrice>`。回退顺序：
1. `pricing[platform_type]`（平台覆盖价，db.rs:2854）→ source `platform_override`
2. 顶层 `input_cost_per_token`/`output_cost_per_token`（db.rs:2869）→ `top_level`
3. `default_platform` 指向的 `pricing[dp]`（db.rs:2882）→ `default_platform`
4. fallback（`fallback_input/1e6`，db.rs:2898）→ `fallback`

**新平台无需改 resolve_price 代码**——只要 `model_price` 表里该模型的 `price_data.pricing` 含新 `platform_type` 键即可命中第 1 档；否则自动回退顶层/默认价。`platform_type` 即 `Protocol` 枚举字符串。

按量平台扣费链路：proxy 请求后 → `estimate_after_request`（estimate.rs:416）→ `resolve_price`（estimate.rs:434）→ `balance_cost`（estimate.rs:83，in×in_cost + out×out_cost + cache×cache_cost）→ `apply_balance_delta` 原子自减（estimate.rs:276）。

### est_coding_plan 字段格式（持久化于 `platform.est_coding_plan` 列，JSON）

`EstCodingPlan { tiers: Vec<EstTier>, level: Option<String> }`（estimate.rs:28）。
- 不要直写 raw `CodingPlanInfo` JSON 进 `est_coding_plan`！字段名不同（`utilization` ≠ `est_utilization`）会导致 est 显 0。必须经 `build_calibrated_coding_plan`（estimate.rs:336）转换 → `calibrate_from_quota`（estimate.rs:363）统一落库。
- 真查→落库唯一入口：`calibrate_from_quota(db, platform_id, &quota, is_coding_plan)`（estimate.rs:363）。失败保留旧 est（estimate.rs:364 early-return）。

coding plan 平台增量：`apply_coding_plan_delta`（estimate.rs:291，read-modify-write 同一持锁临界区）逐 tier `apply_tier_delta`（estimate.rs:100）。**coding plan 平台不扣金额**（estimate.rs:382 `est_balance=0`）。

---

## 4. 前端暴露（Tauri command + api.ts）

### Command（lib.rs）

| Command | 位置 | 用途 |
|---|---|---|
| `platform_query_quota` | lib.rs:2013 | 通用查询：`query_quota` + 成功则 `persist_quota_to_db` 校准落库 (lib.rs:2018-2021) |
| `platform_query_quota_newapi` | lib.rs:2029 | New API 两步查询（含 extra 配置） |

二者都注册于 `invoke_handler`（lib.rs:3701-3702）。落库走 `persist_quota_to_db`（lib.rs:2048）→ `calibrate_from_quota`（lib.rs:2051）。冷启动批量校准：`cold_start_init_tray_estimates`（lib.rs:2058，每平台 spawn 独立 async，lib.rs:2079-2087）。

### api.ts 封装（api.ts:1192-1197）

```ts
export const quotaApi = {
  query:       (baseUrl, apiKey, platformId?) => invoke<PlatformQuota>("platform_query_quota", {...}),
  queryNewapi: (baseUrl, apiKey, extra, platformId?) => invoke<PlatformQuota>("platform_query_quota_newapi", {...}),
};
```
类型：`PlatformQuota`（api.ts:1182）/ `BalanceInfo` / `CodingPlanInfo`。

唯一前端调用点：`src/pages/Platforms.tsx`（Platforms.tsx:1644/1694，按 New API 与否选 `queryNewapi`/`query`）。**新平台无需改前端**——前端按平台 newapi flag 自动选 command，余额/配额展示由后端 `PlatformQuota` 结构驱动。

---

## 5. 「给新平台加余额/配额查询」完整文件触点清单 + 顺序

**仅当新平台支持上游余额/配额 API 时才需要**（否则用 manual_budget 兜底，见下）。改动几乎全在 `quota.rs` 一个文件：

1. **`quota.rs`**：新增 `query_<plat>_balance` 或 `query_<plat>_coding_plan` 函数（照搬 §1/§2 模板）。
2. **`quota.rs` `query_quota`**（quota.rs:373）：在对应段（coding plan 段优先 quota.rs:380 / 余额段 quota.rs:392）加一行 `if url.contains("...") { return query_<plat>_xxx(...).await; }`。
3. **（仅 coding plan）确认 tier `name`** 命中 `usage_color::cycle_ms_for_tier`（usage_color.rs:30）；若是全新周期语义，需在该 match 加 name→cycle 映射，否则配色退中性。
4. **（仅按量平台）价格数据**：确保 `model_price` 表该模型 `price_data` 能被 `resolve_price`（db.rs:2840）命中（`pricing[<protocol>]` 或顶层/默认价）——通常无需改代码，走价格同步/手填。
5. **无需改动**：lib.rs command（`platform_query_quota` 已泛化按 base_url 分派）、api.ts（已封装）、Platforms.tsx（已调用）、estimate.rs（请求驱动预估对所有平台通用）。

> 即：**典型新平台只改 quota.rs（1 函数 + 1 分派行）**，可选改 usage_color.rs（新 tier 周期）。

### 手动预算兜底（无上游 quota API 的平台，manual_budget.rs）

无上游 quota 的平台用 `manual_budgets`（platform 列，JSON）做本地限额，与请求驱动预估并行：
- 扣减入口 `apply_manual_budgets`（manual_budget.rs:187）在 `estimate_after_request`（estimate.rs:464）无条件调用；无 budget → no-op（manual_budget.rs:202）。
- 4 种窗口语义 `kind`：`total`/`rolling`/`fixed`/`daily`（manual_budget.rs:11-16）；`unit`：`usd`（扣 est_cost）或 `token`（扣总 token，manual_budget.rs:181）。
- 阻断：`evaluate_depletion`（manual_budget.rs:156）proxy 转发前只读判定耗尽 → 402。
- **与 coding plan tier 无直接耦合**：manual_budget 是独立的「本地限额」机制，不读 `est_coding_plan`/tier；二者并行（估 coding plan tier 用 `apply_coding_plan_delta`，估手动预算用 `apply_manual_budgets`，互不依赖）。est_cost 同样走 `resolve_price`（estimate.rs:451）保证默认价回退。

---

## Caveats / Not Found

- coding plan tier `name` 是硬约束：必须 ∈ `cycle_ms_for_tier` 已知集合，否则 estimate 配色（`tier_usage_level`，estimate.rs:170）全退 Neutral，预估增量仍工作但 statusline 不上色。
- 智谱鉴权特例：**不加 Bearer**（quota.rs:295），裸传 key + `Content-Type: application/json`。新平台若鉴权非标准 Bearer，照此调整 headers 数组。
- New API 是特殊两步查询（quota.rs:511），不走 `query_quota` 分派，由独立 command `platform_query_quota_newapi` 驱动，配置在 `platform.extra` JSON 的 `newapi` 节点（`parse_newapi_extra`，quota.rs:437）。新增同类「中转聚合平台」可参考此模式但需新 command。
