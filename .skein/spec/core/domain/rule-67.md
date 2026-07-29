---
name: peak-multiplier-symmetry
description: estimate 链中余额扣减+手动预算必须同步乘 peak 倍率，防止前后端不一致
type: core
category: domain
---

## 硬约束

estimate 流程中**任一分支加 peak 倍率，对边必补**（既存 bug 根因）：

- `estimate/db_ops.rs:214` 余额扣减 AND `estimate/db_ops.rs:233` 手动预算
  必须同时乘 peak 倍率（与 `calc_est_cost` 的倍率应用对称）
- 防护：review 同侧分支若新增倍率乘算，peer 检查对边是否也补

## 禁用

❌ 仅余额扣减乘倍率，手动预算不乘（口径分裂：扣数 ≠ 前端显示）
❌ 仅某一段乘倍率，其他相关路径不补（隐性 bug，后续请求命中倍率期间崩坏）

## 关联

[[rule-66]] [[time-tiers-apply-idiom]]

## 案例

原错 → estimate 的两处取价未乘 peak_hours·multiplier，而 calc_est_cost 有乘
  → 用户点「预算」看到扣 100，实际按月结算被扣 300（高峰 ×3）
修后 → `maybe_peak_multiplier` 同时护住两处扣价 → 前后端一致
