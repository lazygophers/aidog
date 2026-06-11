---
updated: 2026-06-09
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

| Guide | When |
|-------|------|
| [Code Reuse Rules](./code-reuse-rules.md) | 写新函数 / 组件 / utility 前 |
| [Cross-Layer Rules](./cross-layer-rules.md) | 改动跨 Rust↔TypeScript 边界时 |
| [trellisx Conventions](./trellisx-conventions.md) | task 实施 / 检查时 (标准 5 步流程 + check 闭环 + 分工表) |
| [trellisx Worktree](./trellisx-worktree.md) | worktree 隔离 + subtask 异步并行 (单一事实源) |

## Pre-Change Checklist (MUST)

改任何 `src/` 文件前必须执行:

1. `grep -rE '<关键词>' src/` — 查已有实现，命中则复用
2. 确认改动不破坏 Tauri command 契约（字段名 / 类型 / 返回值）
3. 新增 public 函数 / 组件必须有对应类型定义
4. 新增 UI 文案必须走 i18n `t()` 函数，禁硬编码字符串
