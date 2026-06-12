# 修复 est_cost 无模型价时未回退默认价格（返回 0）

## Goal

修复 bug：模型未匹配到价格时，预估成本 est_cost 应按默认价格（PriceSyncSettings.fallback_input/output_price）计算，但当前成功请求 est_cost=0，严重偏离预期。

## 根因（已定位）

- 实时记日志路径 `upsert_log`(proxy.rs:77) → `calc_est_cost`(db.rs:468, proxy.rs:96 调用) **自己查 `model_price` 表**：
  - 无匹配行 → `price=None` → **直接返回 0.0**（db.rs:501），不回退默认价。
  - 即便有行但 input=output=0，仅尝试该行内 default_platform，仍可能 0。
  - 不接收 platform_type，无法用 `pricing[platform_type]` override。
- 而正确的回退解析器 `resolve_price`(db.rs:1387) 链：`pricing[platform_type] > top_level > default_platform > fallback(默认价)`，step4 用 `fallback_input/1e6`。**但 calc_est_cost 没用它**——只有 preview 命令 `model_price_resolve`(lib.rs:1308) 用了。
- 默认价来源：`PriceSyncSettings.fallback_input_price/fallback_output_price`（models.rs:898-915，默认 `default_fallback_price()=3.0` $/M），经 `gateway::price_sync::get_sync_settings(&db)` 加载。

## Requirements

- R1 `calc_est_cost` 改为走 `resolve_price` 的回退语义：无模型价/价为 0 时回退到 `PriceSyncSettings` 的 fallback 默认价（与 preview 命令 `model_price_resolve` 行为一致）。
- R2 尽量传入 platform_type 以启用 `pricing[platform_type]` override；若调用点（proxy.rs:96）拿 platform_type 不便，**至少保证 fallback（step4）生效**（platform_type 传 "" 仍能回退默认价）。
- R3 **锁安全**：`resolve_price`/`get_model_price`/`get_sync_settings` 各自获取 db 锁；`calc_est_cost` 不得在持有 `db.0.lock()` 时调用它们（避免重入死锁）。重写 calc_est_cost 不再直接长持锁。
- R4 行为对齐：实时 est_cost 与 PricingTab preview 对同一 (model, platform_type) 应得到一致单价。

## Acceptance Criteria

- [ ] 无 model_price 匹配的成功请求（有 token）→ est_cost = tokens × 默认 fallback 单价（3.0/M 默认），非 0。
- [ ] 有 model_price 且价格 >0 → 用真实价（行为不回退）。
- [ ] platform override（pricing[platform_type]）若可传 platform_type 则生效。
- [ ] 无死锁、无 panic；cargo check/build 通过。
- [ ] 实时 est_cost 与 model_price_resolve 命令单价一致。

## Definition of Done

- `cargo check`（或项目 build）通过，无 warning 回归。
- 不破坏 calc_est_cost 其他调用点（grep 确认所有 caller 适配新签名）。
- 锁安全，无重入。

## Out of Scope

- 不改前端 PricingTab / est_cost 持久化与聚合（SUM(est_cost) 逻辑不变）。
- 不改 resolve_price 既有链逻辑（复用）。
- 不改默认 fallback 价数值（3.0）。

## Technical Notes

- 关键文件：`src-tauri/src/gateway/db.rs`（calc_est_cost:468 / resolve_price:1387）、`src-tauri/src/gateway/proxy.rs`（调用点:96）、`models.rs`（PriceSyncSettings）、`price_sync.rs`（get_sync_settings）。
- proxy.rs:96 调用点的 `log` 有 platform_id / model / actual_model / target_protocol；platform_type 的 serde 裸名见 spawn_estimate(proxy.rs:121) 的取法（`serde_json::to_string(protocol).trim_matches('"')`）。
- 选项：① calc_est_cost 加 platform_type 参数 + 内部调 resolve_price + get_sync_settings；② 或保持签名、内部 best-effort 取 platform_type。优先 ① 更清晰。
