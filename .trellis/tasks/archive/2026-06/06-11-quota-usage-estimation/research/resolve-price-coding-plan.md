# Research: resolve_price 调用 + coding plan 平台是否有 model price

- **Query**: resolve_price 签名/参数；coding plan 平台是否有 model price（确认余额预估对其无意义）
- **Scope**: internal
- **Date**: 2026-06-11

## resolve_price 签名（db.rs:1074-1139）
```rust
pub fn resolve_price(
    db: &Db,
    model_name: &str,
    platform_type: &str,
    fallback_input: f64,
    fallback_output: f64,
) -> Result<ResolvedPrice, String>
```
- 返回 `ResolvedPrice{input_cost_per_token, output_cost_per_token, cache_read_input_token_cost, source}`（models.rs:762）
- 单位：每 **token** 金额（pricing JSON 直接是 per-token；fallback 分支把 per-1M 除 1e6，db.rs:1134-1135）
- 优先级（db.rs:1073 注释）：`pricing[platform_type]` > top_level > `default_platform` pricing > fallback
- 数据源：`get_model_price(db, model_name)`（db.rs:1081）查 model_price 表（003_model_price.sql），price_data JSON 解析

### 余额预估增量计算
```
delta = input_tokens × input_cost_per_token
      + output_tokens × output_cost_per_token
      + cache_tokens × cache_read_input_token_cost
est_balance_remaining -= delta
```
- `platform_type` 入参：需裸协议名做 pricing JSON key（db.rs:1088 `pd.get("pricing").get(platform_type)`）。proxy 侧 `route.platform.platform_type` 是 `Protocol` enum，需转字符串（确认是否带 serde 引号，见 estimate-trigger 文档 caveat）。
- `model_name` 入参：用 `actual_model`（proxy.rs:323，实际上游 model）而非 requested_model。

## Coding plan 平台是否有 model price？

**结论：coding plan 平台按订阅（窗口配额）计费，非按量，通常无 model price → 余额预估对其无意义，确认成立。**

### 论据
- coding plan 平台（Kimi coding / GLM coding / MiniMax）走 `coding_plan: true` 端点（Platforms.tsx:159/167 配 coding base_url），quota 走 `coding_plan` 分支返回 utilization%（quota.rs query_xxx_coding_plan），**不返回 balance**（PlatformQuota.balance = None，如 quota.rs:288-290）。
- 这些平台订阅制，token 消耗扣的是「窗口配额%」不是「金额余额」→ 即便 model_price 表有该 model 价格，扣金额也无对应"余额"可扣。
- resolve_price 对 coding plan 平台仍可能返回非零价格（若 model_price 表碰巧有该 model），但**不应用于 coding plan 预估**——coding plan 预估走 utilization 路径（见 coding-plan-base-feasibility）。

### 设计含义：预估走双轨
- **按量平台**（DeepSeek/StepFun/SiliconFlow/OpenRouter/Novita 等，quota 返回 balance）→ 用 resolve_price 做金额预估，扣 est_balance_remaining。
- **coding plan 平台**（Kimi/GLM/MiniMax，quota 返回 coding_plan）→ 走 utilization 预估，**不调 resolve_price 扣余额**。
- 判定依据：proxy.rs:326 解出的 `coding_plan` 标记 / 或上次真查 PlatformQuota 是 balance 还是 coding_plan。
- 同一平台理论上可两者皆有（GLM 注释 quota.rs:418 "可能同时返回 coding plan"），设计需处理混合：有 balance 走金额，有 coding_plan 走 utilization。

## Caveats
- 同时具备 balance + coding plan 的平台（GLM）预估需双路并行。
- resolve_price 找不到价格时落 fallback（db.rs:1133，用 settings 的 fallback per-1M），预估精度依赖 model_price 表完整度。
