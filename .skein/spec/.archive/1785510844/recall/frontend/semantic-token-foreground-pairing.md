---
title: semantic-token-foreground-pairing
layer: recall
category: frontend
keywords: [语义色,token,foreground,对比度,contrast,accent,wcag,配对]
status: active
---

## 语义色 token 必须成对达标, --accent 本值禁改

## 判据

任何语义色 `bg-X` token 都必须配达标对比度的 `--X-foreground`。frontend-compositing-purge task 本轮 9 处对比度缺陷里 8 处根因是配对失衡（dark 模式 `--accent-foreground` 浅白配亮金 = 1.8:1；light 模式 `--primary-foreground` 白配金 = 2.62:1，均低于 WCAG AA 4.5:1）。

## 陷阱

本项目 `--accent` 语义**不等于** shadcn 惯例（shadcn 里 accent 通常是低调 hover 背景），本项目里 `--accent` 被当品牌强调金色用，被 `.btn-primary` 渐变 / checkbox `accent-color` / `.badge-accent` 多处依赖其具体色值。改坏 `--accent` 本身会连带破坏这些依赖方。

## 正解

修对比度缺陷时**禁改 `--accent` 等语义色 token 的值本身**，只能改配对的 `-foreground` token 去满足对比度，逐处核对 `bg-X`/`-foreground` 组合。

## 适用

本项目（aidog）任何涉及语义色 token 新增/审计对比度时；同族已有跨项目规则（通用禁写死 #fff/#000 规则），本条补充"本项目 --accent 语义特殊、只能调 foreground 侧"这一项目特定约束。

## 案例

frontend-compositing-purge task 对比度审计：dark `--accent-foreground` 1.8:1、light `--primary-foreground` 2.62:1，均改 foreground 侧修复，未动 accent/primary 本值。
