# 手动预算/配额（无 quota 平台）+ 灵活窗口 + 耗尽阻断

## Goal

为**不支持上游余额自动查询 / coding plan** 的平台，允许用户手动配置一个或多个预算限额，按使用预估扣减；支持总额、滑动 N 小时窗口、固定 N 小时窗口、自然日重置等多种限额并存；任一限额耗尽时**停止对外转发**（返回 402），窗口/次日恢复后自动可用。

## Decisions（ADR-lite，来自 brainstorm）

| 维度 | 决策 |
|------|------|
| 预算模型 | 统一「手动预算」，一平台可**同时启多个限额**；**任一耗尽即阻断** |
| 限额种类 | `total`(总额不重置) / `rolling`(滑动 N 小时，首用起算满额重置) / `fixed`(固定 N 小时，钟点对齐) / `daily`(自然日 00:00 本地重置) — 滑动+固定都支持 |
| 单位 | 每限额可选 `usd`($) 或 `token`；**统一以 $ 展示**（token 单位按价折算显示，尽力） |
| 扣减 | 每请求对平台所有启用限额各扣一次：usd 扣 est_cost、token 扣总 token；先按窗口判定是否重置再扣 |
| 阻断 | 转发**前**判定，任一限额剩余 ≤ 0 → 拒该请求返回 402（体含哪个限额耗尽 + 恢复提示）；**平台保持启用**，窗口/次日恢复后自动放行 |
| 适用范围 | 仅对**无上游 quota 自动支持**（无余额真查 + 非 coding plan）的平台开放手动预算配置 |

## Requirements

### 数据模型（R1）
- R1.1 平台新增字段（JSON，如 `manual_budgets`）：`Vec<ManualBudget>`，每项 `{ id, kind: total|rolling|fixed|daily, unit: usd|token, amount: f64, window_hours?: f64(rolling/fixed), consumed: f64, window_start_at?: i64(ms, rolling/fixed/daily 追踪), enabled: bool }`。全 optional/向后兼容（旧平台无该字段 → 空，无限额、不阻断、行为不变）。
- R1.2 不破坏既有 `est_balance_remaining` / `est_coding_plan` / 真查校准；手动预算是**独立于上游 quota** 的并行机制。

### 扣减 + 窗口（R2，落 estimate.rs 或新模块）
- R2.1 每请求拿到 token 后，对平台每个 enabled 限额：先按 kind 判定窗口是否到期重置（rolling: now-window_start≥window_hours → reset consumed=0,window_start=now；fixed: 对齐钟点边界；daily: 跨本地自然日 → reset），再 `consumed += (unit==usd? est_cost : total_tokens)`。
- R2.2 扣减原子/串行安全（参考现有 `apply_balance_delta` 锁约定，JSON read-modify-write 须同一持锁临界区，禁持锁跨 await）。
- R2.3 est_cost 走 `resolve_price`（与 [[pricing-resolve-single-source]] 一致），含默认价回退。

### 阻断（R3，proxy.rs 转发前）
- R3.1 转发前对平台每个 enabled 限额计算「当前剩余」（含窗口惰性重置判定，不改库或短持锁判定）：任一 `amount - consumed ≤ 0` → **不转发**，返回 402（JSON 体：耗尽的限额 kind + 恢复时间/提示）。
- R3.2 全部限额有余 → 正常转发。无手动预算配置的平台 → 不受影响。

### 前端（R4，Platforms.tsx 编辑表单 + 列表展示）
- R4.1 平台编辑页：仅对**无自动 quota 支持**的平台显示「手动预算」配置区——可增删多条限额，每条选 kind/unit/amount/window_hours/enabled。
- R4.2 列表卡片：手动预算剩余以 $ 展示（复用 BalanceBar/StatChip；token 单位折算 $ 尽力，缺价则显 token + 标注）；多限额显最紧（剩余比例最低）那条或并列。耗尽视觉标记。
- R4.3 文案走 i18n t()，补 7 语言 key。

### 不回退（R5）
- R5.1 有上游 quota 真查 / coding plan 的平台逻辑不变。
- R5.2 旧平台无 manual_budgets → 不阻断、不扣、行为完全不变。

## Acceptance Criteria

- [ ] 无 quota 支持平台编辑页可配多条限额（total/rolling/fixed/daily，usd/token）。
- [ ] 每请求按窗口规则扣减各限额；窗口到期惰性重置满额。
- [ ] 任一限额耗尽 → 该平台请求转发前被拒返回 402（体含原因），平台仍启用，恢复后自动放行。
- [ ] 列表卡片以 $ 展示手动预算剩余 + 耗尽标记。
- [ ] 旧平台/有 quota 平台零回退；扣减锁安全无跨 await 持锁。
- [ ] typecheck 0 / cargo check 通过；新文案 t() 补 7 语言。

## Definition of Done

- 跨层契约一致（字段名/类型 platformApi）；est_cost 走 resolve_price。
- 阻断在转发前、返回明确 402、不串改其他平台/请求。
- 锁安全（JSON read-modify-write 同一临界区，禁持锁跨 await）。
- 向后兼容旧配置。

## Out of Scope

- 不改有上游 quota 真查 / coding plan 平台的既有校准逻辑。
- 不做跨设备同步 / 不做账单导出。
- 不改既有 est_balance/est_coding_plan 字段语义（手动预算独立字段）。

## Technical Notes

- 字段/CRUD：`models.rs` + `db.rs`（platform 表加列或复用 JSON 列）。
- 扣减/窗口：`estimate.rs`（或新 `manual_budget.rs`），复用锁约定。
- 阻断：`proxy.rs` 转发前（route 解析后、send 前）。
- 前端：`Platforms.tsx` 编辑表单 + 列表卡片；`api.ts` 类型；`locales/*`。
- 支持检测：判定平台是否有上游 quota 自动支持（参考 `quota.rs` 支持列表 + `coding_plan` flag）。
- 现有锁约定见 estimate.rs 顶部注释（Db.0 Mutex，禁持锁跨 await，原子自减 / 同临界区 RMW）。
