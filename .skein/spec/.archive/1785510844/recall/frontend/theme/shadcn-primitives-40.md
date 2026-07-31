---
title: next-themes 与自有主题体系冲突
layer: recall
category: frontend/theme
keywords: [next-themes,theme,conflict,shadcn,sonner]
source: shadcn-primitives
authored-by: skein-spec
created: 1784708034
status: active
related: []
updated: 1784708034
---

# next-themes 与自有主题体系冲突

## 问题
shadcn Sonner 组件导入 next-themes 的 `useTheme`，与本项目自有主题体系（`src/themes/`）冲突。

## 证据
- src/components/ui/sonner.tsx line 3: `import { useTheme } from "next-themes"`
- 本项目有 `src/themes/` 目录，含 index.ts / types.ts / useThemeMode.ts（自有主题管理）

## 待决策
- 留待 pages 层评估：是否切换到 next-themes 统一，或隔离 Sonner 主题逻辑
- 当前：保留冲突 import，暂未迁移

## 适用
shadcn 组件集成 + 主题体系迁移

## 关联
[[modal-state-architecture]] (同 task Modal 保留策略)
