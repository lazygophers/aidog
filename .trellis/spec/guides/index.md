---
updated: 2026-06-12
rewrite-version: 1
supersedes:
  - guides/index.md (v0 descriptive)
authored-by: trellisx-spec
mode: optimize
---

# Guides

何时被读: 任何实现任务开始前
谁读: main / sub-agent
不遵守的代价: 跳过检查清单 → 重复犯错 / 跨层 bug

---

## Available Guides

| Guide | When | 跳过代价 |
|-------|------|---------|
| [Code Reuse Rules](./code-reuse-rules.md) | 写新函数 / 组件 / utility 前 | 重复逻辑散布，bug fix 不传播 |
| [Cross-Layer Rules](./cross-layer-rules.md) | 改动跨 Rust↔TypeScript 边界时 | 字段名/类型错位，运行时静默失败 |
| [trellisx Conventions](./trellisx-conventions.md) | task 实施 / 检查时 (标准 5 步流程 + check 闭环 + 分工表) | 跳步漏 check，劣质改动直接落盘 |
| [trellisx Worktree](./trellisx-worktree.md) | worktree 隔离 + subtask 异步并行 (单一事实源) | 并行改冲突，commit 丢失/覆盖 |

## Pre-Change Checklist (MUST)

❌ 禁不读对应 guide 直接动手改 `src/`（跳过 → 重复犯错 / 跨层 bug，代价见上表）。

改任何 `src/` 文件前必须先读对应 guide 再动手:

- 写新函数 / 组件前 → [Code Reuse Rules](./code-reuse-rules.md) (grep 查已有实现)
- 跨 Rust↔TS 边界 → [Cross-Layer Rules](./cross-layer-rules.md) (契约 / 字段名 / 类型)
- 新增 UI / 改前端模式 → [Frontend Conventions](../frontend/conventions.md) (组件 / 状态 / API / 类型 / i18n)
