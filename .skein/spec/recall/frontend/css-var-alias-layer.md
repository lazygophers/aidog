---
title: css-var-alias-layer
name: css-var-alias-layer
description: Tailwind cascade layer: 裸写规则反压 layer 内 utility
layer: recall
keywords: [CSS,变量,Tailwind,layer,cascade]
created: 1785516136
inclusion: auto
---

## css-var-alias-layer

## Tailwind cascade layer: 裸写规则反压 layer 内 utility

Tailwind v4 项目里若分层导入 CSS，任何裸写（不在 `@layer` 块内）的规则优先级都高于 layered utility（CSS cascade layer 规范）。

## 陷阱

补 preflight 缺失的 UA reset（如 button/input/select 色继承）时若裸写在 globals.css 顶层，会反压 utilities 层 —— 所有 `text-*-foreground` utility class 失效。

## 正解

补 UA reset 规则必须包进 `@layer base {}` 块，与 globals.css 顶部声明的 layer 顺序对齐，禁裸写元素选择器规则。

## 检查

globals.css 顶部若见 `@layer <names>;` 声明 + `@import ... layer(...)`，改动前先确认新增规则是否包在对应 `@layer` 块内。

## 案例

frontend-compositing-purge task：commit c3f9515e 裸写 UA reset 引入 button 文字色失效 → ce3d5dd5 改为 `@layer base {}` 包裹修复。

## 适用

Tailwind v4 + cascade layer 项目，补 preflight/UA reset 规则时。

## 关联

[[semantic-token-foreground-pairing]]

## CSS var live resolution 别名层

CSS 变量改名迁移时，用 :root 别名层实现 live resolution，替代批量 sed 替换。

## 正解

1. 在 :root 定义别名：`--legacy: var(--shadcn);`
2. 所有引用用旧名 `--legacy`，实际指向新名 `--shadcn`
3. 迁移完成后删别名行（自动失效）

## 对比

| 方式 | 改动量 | 误伤风险 | 回滚 |
|------|--------|---------|------|
| sed 批量替换 | 700+ 行 | 高（误伤类似变量名） | 难 |
| 别名层 | 10 行 | 无（CSS 引用透明） | 易（删别名） |

## 案例

shadcn-infra task: 主题变量改名用别名层，globals.css 加 10 行 vs sed 700+ 行

## 适用

CSS 变量迁移、主题重构、大型 CSS 重构中间状态

## 关联

[[theme-token-runtime-switch]]
