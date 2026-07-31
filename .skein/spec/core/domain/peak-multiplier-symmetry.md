---
name: peak-multiplier-symmetry
title: 估算链 Peak 倍率对称应用
description: estimate 链中余额扣减与手动预算必须同步乘 peak 倍率，防止成本统计分裂
layer: core
category: domain
keywords: [peak,multiplier,estimate,budget,symmetry,billing,财务]
created: 1725080438
inclusion: always
---

## 硬约则

estimate 流程中**任意处加 peak 倍率，对边必补同倍率**（既存 bug 根因）：

- 余额扣减（`estimate/db_ops.rs:219`）AND 手动预算（`:237`）MUST 同乘 peak 倍率
- 维持与 `calc_est_cost` 路径的倍率应用对称性

## 禁用

❌ 仅余额扣减乘倍率，手动预算漏乘 → 成本显示 ≠ 实际扣款  
❌ 仅某处乘倍率，其他相关路径不补 → 高峰期估算 ≠ 月结算错账

## 关联

[[rule-66]] [[time-tiers-apply-idiom]]
