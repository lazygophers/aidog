# 余额 / coding plan 配额查询子流程模板

照着改即可。全部改动几乎集中在 `src-tauri/src/gateway/quota.rs` 一个文件（1 函数 + 1 分派行）。file:line 校对于 2026-06-15。

---

## 0. 分派机制（先懂这条）

`query_quota`（`quota.rs:373`）按 `base_url.to_lowercase()` 子串顺序匹配，**与 Protocol 枚举无关**：

```rust
// quota.rs:373 骨架
pub async fn query_quota(db: Option<&Arc<Db>>, base_url: &str, api_key: &str) -> PlatformQuota {
    let url = base_url.to_lowercase();
    // ── coding plan 段（优先，~quota.rs:380）──
    if url.contains("api.kimi.com/coding") { return query_kimi_coding_plan(db, api_key).await; }
    if url.contains("bigmodel.cn") || url.contains("z.ai") { return query_zhipu_coding_plan(db, base_url, api_key).await; }
    // ── 余额段（~quota.rs:392）──
    if url.contains("api.deepseek.com") { return query_deepseek_balance(db, api_key).await; }
    // ... 其余余额平台 ...
    err_quota("Unsupported platform ...")   // 落空兜底
}
```

新增平台：在对应段加**一行** `if url.contains("...") { return query_foo_xxx(...).await; }`。coding plan 检测放 coding 段（优先于余额段）。

New API 是例外：两步查询，独立 command `platform_query_quota_newapi`（`quota.rs:511`），不走此分派。

---

## 1. 余额查询（按量计费平台）

照搬 `query_deepseek_balance`（`quota.rs:138`）。签名：
`async fn query_<plat>_balance(db: Option<&Arc<Db>>, api_key: &str) -> PlatformQuota`

```rust
async fn query_foo_balance(db: Option<&Arc<Db>>, api_key: &str) -> PlatformQuota {
    // 1. GET：quota_get_json（quota.rs:115，统一 GET + 日志 + 错误前缀）
    let body = match quota_get_json(
        db,
        "https://api.foo.com/user/balance",                 // 多数硬编码全 URL，不用 base_url
        &[("Authorization", format!("Bearer {api_key}"))],  // 鉴权：多数 Bearer
    ).await {
        Ok(v) => v,
        Err(e) => return err_quota_platform("foo", &e),     // 失败兜底带平台名（quota.rs:97）
    };
    // 2. 解析：parse_f64_field 容错 string/number（quota.rs:87）
    let remaining = parse_f64_field(&body, "balance").unwrap_or(0.0);
    let is_valid  = body.get("is_available").and_then(|v| v.as_bool()).unwrap_or(true);
    // 3. 组装：余额平台只填 balance，coding_plan: None
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

分派（余额段 ~`quota.rs:392`）：`if url.contains("api.foo.com") { return query_foo_balance(db, api_key).await; }`

### 现有 5 个余额实现（解析特例参考）

| 函数 | URL | 鉴权 | 解析特点 |
|---|---|---|---|
| `query_deepseek_balance`(`:138`) | `api.deepseek.com/user/balance` | Bearer | 遍历 `balance_infos[]` 累加 `total_balance`；CNY |
| `query_stepfun_balance`(`:161`) | `api.stepfun.com/v1/accounts` | Bearer | 直读 `balance` |
| `query_siliconflow_balance`(`:178`) | `api.siliconflow.{cn,com}/v1/user/info` | Bearer | `is_cn` 切域名+货币；读 `data.totalBalance` |
| `query_openrouter_balance`(`:202`) | `openrouter.ai/api/v1/credits` | Bearer | `data.total_credits - data.total_usage` |
| `query_novita_balance`(`:225`) | `api.novita.ai/v3/user/balance` | Bearer | `availableBalance / 10000`（单位 0.0001 USD） |

---

## 2. coding plan 配额（订阅制平台）

照搬 `query_kimi_coding_plan`（`quota.rs:243`）。签名：
`async fn query_<plat>_coding_plan(db: Option<&Arc<Db>>, api_key: &str) -> PlatformQuota`

```rust
async fn query_foo_coding_plan(db: Option<&Arc<Db>>, api_key: &str) -> PlatformQuota {
    let body = match quota_get_json(db, "https://api.foo.com/coding/v1/usages",
        &[("Authorization", format!("Bearer {api_key}"))]).await {
        Ok(v) => v,
        Err(e) => return err_quota_platform("foo", &e),
    };
    let mut tiers = Vec::new();
    // 对每个窗口构造一个 QuotaTier
    let limit     = parse_f64_field(detail, "limit").unwrap_or(1.0);
    let remaining = parse_f64_field(detail, "remaining").unwrap_or(0.0);
    let used      = (limit - remaining).max(0.0);
    let utilization = if limit > 0.0 { (used / limit) * 100.0 } else { 0.0 };
    let resets_at = detail.get("resetTime").and_then(|v|
        v.as_str().map(String::from).or_else(|| v.as_i64().and_then(millis_to_iso8601)));
    tiers.push(QuotaTier {
        name: "five_hour".into(),    // ★ 必须 ∈ {"five_hour","weekly_limit","seven_day","mcp_monthly"}
        utilization,                 // 0-100 已用百分比
        resets_at,                   // ISO8601 或由 millis 转；推算 window_start 用
        limit: Some(limit),          // ★ Some → has_base=true → 精确预估；None → 方案 B 拟合
        remaining: Some(remaining),  // 仅暴露绝对量的平台填
    });
    PlatformQuota {
        success: true, error: None, queried_at: now_millis(), balance: None,
        coding_plan: Some(CodingPlanInfo { tiers, level: None }),  // level: 套餐等级名（可选）
        newapi_user_id: None,
    }
}
```

分派（coding 段 ~`quota.rs:380`，**优先于余额段**）：
`if url.contains("api.foo.com/coding") { return query_foo_coding_plan(db, api_key).await; }`

### 🔴 tier `name` 硬约束

`QuotaTier.name` 必须命中 `cycle_ms_for_tier`（`usage_color.rs:30`）已知集合：

| name | 周期 | usage_color.rs |
|---|---|---|
| `five_hour` | 5h | `:32` |
| `weekly_limit` / `seven_day` | 7d | `:33` |
| `mcp_monthly` | 30d | `:34` |

**未知 name → 无周期 → estimate 配色（`tier_usage_level`）全退 Neutral**，预估增量仍工作但 statusline 不上色。若是全新周期语义，需在 `cycle_ms_for_tier` 的 match 加 name→cycle 映射。

### 现有 2 个 coding plan 实现差异

| 函数 | URL | 鉴权 | 特点 |
|---|---|---|---|
| `query_kimi_coding_plan`(`:243`) | `api.kimi.com/coding/v1/usages` | Bearer | `limits[].detail`→five_hour；`usage`→weekly_limit；**带绝对 limit/remaining**（has_base 精确预估） |
| `query_zhipu_coding_plan`(`:287`) | `{open.bigmodel.cn\|api.z.ai}/api/monitor/usage/quota/limit` | 🔴 **裸 key 无 Bearer**（`:295`） | 按域名切 base；按 `unit` 分类（3→five_hour,6→weekly）；`TIME_LIMIT`→mcp_monthly；无 limit→方案 B 拟合；带 `level` |

> 鉴权特例：智谱不加 Bearer，裸传 key + `Content-Type: application/json`。新平台鉴权非标准 Bearer 时照此调整 headers 数组。

---

## 3. tier 结构 / 持久化字段（背景，通常不用手碰）

- **`QuotaTier`**（真查输出，`quota.rs:54-68`）：`name` / `utilization` / `resets_at` / `limit` / `remaining`。`limit.is_some()` 决定 `has_base`（精确增量 vs 方案 B 拟合）。
- **`EstTier`**（持久化预估态，`estimate.rs:35-61`）：多出 `coef_per_token`（拟合系数）/ `util_at_last_real` / `tokens_since_real` / `has_base` / `limit` / `window_start`。
- 真查 → 落库**唯一入口**：`calibrate_from_quota(db, platform_id, &quota, is_coding_plan)`（`estimate.rs:363`）。失败保留旧 est。
- 🔴 **不要直写 raw `CodingPlanInfo` JSON 进 `platform.est_coding_plan` 列**！字段名不同（`utilization` ≠ `est_utilization`）会致 est 显 0。必须经 `build_calibrated_coding_plan`（`estimate.rs:336`）转换。command 层 `persist_quota_to_db`（`lib.rs`）已自动走这条，手写函数只要返回正确 `PlatformQuota` 即可，落库链路无需碰。

---

## 4. 价格估算（按量平台，0 代码改动）

`resolve_price`（`db.rs:2840`，单一事实源，禁绕过）回退链：
1. `pricing[platform_type]`（平台覆盖价）→ source `platform_override`
2. 顶层 `input_cost_per_token`/`output_cost_per_token` → `top_level`
3. `default_platform` 指向的 `pricing[dp]` → `default_platform`
4. fallback → `fallback`

`platform_type` = `Protocol` 枚举 rename 字符串。新平台**无需改 resolve_price 代码**——只要 `model_price` 表该模型 `price_data.pricing` 含新 `platform_type` 键即命中第 1 档，否则自动回退顶层/默认价。靠价格同步/手填，不动代码。

扣费链路（背景）：proxy 请求后 → `estimate_after_request`（`estimate.rs:416`）→ `resolve_price`（`:434`）→ `balance_cost`（`:83`）→ `apply_balance_delta` 原子自减（`:276`）。coding plan 平台**不扣金额**（`est_balance=0`），走 `apply_coding_plan_delta`（`:291`）逐 tier 增量。

---

## 5. 无上游 quota API 的平台 — manual_budget 兜底

无上游余额/配额接口的平台用 `manual_budgets`（platform 列，JSON）做本地限额，与请求驱动预估并行（`manual_budget.rs`）：
- 扣减入口 `apply_manual_budgets`（`manual_budget.rs:187`）在 `estimate_after_request`（`estimate.rs:464`）无条件调用，无 budget → no-op。
- 4 种窗口 `kind`：`total`/`rolling`/`fixed`/`daily`；`unit`：`usd`（扣 est_cost）或 `token`。
- 阻断：`evaluate_depletion`（`manual_budget.rs:156`）转发前判耗尽 → 402。
- 与 coding plan tier 无耦合，独立机制。**无需改 quota.rs**。

---

## 6. 完整顺序（仅当平台支持上游查询）

1. `quota.rs` 新增 `query_foo_balance` 或 `query_foo_coding_plan`（照 §1/§2 模板）。
2. `quota.rs` `query_quota`（`:373`）对应段加一行分派。
3. （仅 coding plan）确认 tier `name` ∈ `cycle_ms_for_tier`（`usage_color.rs:30`）；全新周期才加映射。
4. （仅按量）确保 `model_price` 该模型 `price_data` 能被 `resolve_price` 命中（通常走价格同步/手填，不改码）。
5. **无需改**：`lib.rs` command（已按 base_url 泛化）、`api.ts`（已封装）、`Platforms.tsx`（已调用）、`estimate.rs`（请求驱动预估通用）。

验证：`cd src-tauri && cargo build && cargo clippy && cargo test`。
