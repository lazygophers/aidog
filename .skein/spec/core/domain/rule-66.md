---
name: resolve-price-now-ms-convention
description: resolve_price 末位参数 now_ms 传值约定，违反导致定价口径分裂
type: core
category: domain
---

## 硬约束

`resolve_price` 新增末位参数 `now_ms: i64`，调用点按用途选传值：

| 调用点 | 传值 | 理由 |
|---|---|---|
| `billing.rs` 计费 | `created_at_ms` | 审计重放按日志自身时刻定价，跨日窗口定价准确 |
| `estimate/db_ops.rs` 余额扣减 + 手动预算 | `gateway::db::now()` | 无 created_at 可取，实时操作用当前时刻 |
| `platform_cmd/price.rs` 前端预览 | `gateway::db::now()` | 前端请求当前价格，实时最新 |
| 测试 | `0` | `now_ms <= 0` 语义 = 跳过 time_tiers，既有测试逐字不变 |

## 禁用

❌ 所有调用点统一传 0（会导致时段定价形同虚设）
❌ 测试传 `now()`（会让既有基准价断言失败）
❌ 签名改动后漏掉任一调用点（9 处全须补）

## 关联

[[time-tiers-apply-idiom]] [[bundled-models-fallback]]

## 案例

原错 (billing.rs 未传参) → 日志字段时刻定价与当前时刻定价混杂 → 审计重放价格错
修后 → created_at_ms 驱动选档 → 日志口径一致
