---
title: shadcn token 运行时切换
layer: recall
category: frontend
keywords: [shadcn,theme,token,runtime,css,var]
source: shadcn-infra
authored-by: skein-spec
created: 1784706729
status: active
related: []
updated: 1784706729
---

# shadcn token 运行时切换

## 技巧
shadcn 主题 token 在运行时动态切换时，用 `applyTheme` + `setProperty` inline 方式，无需 !important 覆盖。

## 正解
1. applyTheme 函数直接设置 CSS var：
   ```ts
   document.documentElement.style.setProperty('--background', 'new-value')
   ```
2. 或用 @theme inline :root 兜底（避免 !important 级联爆炸）

## 陷阱
- **陷阱**: 用 !important 强制覆盖 → 级联爆炸、难以维护
- **陷阱**: 依赖 @import 静态切换 → 不支持运行时

## 反例
❌ 用 !important 覆盖所有 token → 优先级混乱
❌ 依赖静态 @import → 运行时无法切换

## 案例
- shadcn-infra task: 运行时主题切换用 setProperty inline，避免 !important

## 适用
shadcn 主题运行时切换、动态主题系统、CSS var 运行时更新

## 关联
[[shadcn-infra-30]] (同任务 CSS 技巧)
[[shadcn-infra-28]] (shadcn 依赖)
