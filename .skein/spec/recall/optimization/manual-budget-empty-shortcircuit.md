---
title: manual-budget-empty-shortcircuit
layer: core
category: optimization
keywords: [manual-budget,optimization,db-write,shortcircuit,loadgen]
status: active
---

## manual_budget 零配额短路：进写连接前预检

## 问题

`apply_manual_budgets`（`manual_budget.rs:211-246`）处理用户手动配额时，原有逻辑无条件进**单线程写连接**进行配额查询与扣减。

50 路并发 mock 压测时，这是唯一真正排队的 DB 写路径（tokio-rusqlite 串行执行）。虽然压测用的是无配额 mock 平台，但仍触发无条件 SELECT + 缓存失效，污染了内存/CPU 数字。

## 方案

**分两阶段：**

1. **只读池预检**（`has_any_budget`，line:189-203）：用只读池（允许 8 条并发）判 `platform` 是否有 `manual_budgets` 配置。
2. **短路返回**（line:218-220）：无配额则直接 `return Ok(())`，不进写连接、不失效缓存。

有配额时仍走完整写连接临界区（line:221-242），扣减逻辑与失效时机**逐字不变**（红线 2：计费路径）。

## 关键点

- **硬约束**：配额存在时行为不变，短路仅对「零配额」分支生效
- **非 mock 专属**：真实转发路径共用同一 `apply_manual_budgets`，这是真实热路径优化
- **门禁**：修改时必须同时验证 ① 零配额请求不触及平台写连接（trace 或计数器）② 配额扣减结果与改动前逐条一致

## 用途

高频转发路径的每请求冷路径优化，减少单线程 DB 写锁争。适用于：
- mock/真实平台混用的压测
- 用户未配额时的常态路径（绝大多数请求）
- 真实计费路径的性能基线改进
