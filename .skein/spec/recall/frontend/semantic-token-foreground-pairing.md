---
title: semantic-token-foreground-pairing
name: semantic-token-foreground-pairing
description: 语义色 token 必须成对达标对比度
layer: recall
keywords: [语义色,token,foreground,对比度]
created: 1785516136
inclusion: auto
---

## semantic-token-foreground-pairing

## 语义色 token 必须成对达标对比度

任何语义色 `bg-X` token 都必须配达标对比度的 `-foreground` token。本项目 `--accent` 被当品牌强调金色用，改坏本值会连带破坏多处依赖。

## MUST 约束

修对比度缺陷时**禁改 `--accent` 等语义色 token 的值本身**，只能改配对的 `-foreground` token。

## 陷阱

补 preflight 缺失的 UA reset 时若改了语义色 token 本值（如 `--accent` 色），会连带破坏 `.btn-primary` 渐变 / checkbox `accent-color` / `.badge-accent` 等多处依赖。

## 正解

逐处核对 `bg-X`/`-foreground` 组合对比度，修改 foreground 侧色值不修改 accent/primary 本值。

## 案例

frontend-compositing-purge task 对比度审计：dark `--accent-foreground` 1.8:1、light `--primary-foreground` 2.62:1，均改 foreground 侧修复。

## 关联

[[tailwind-cascade-layer-base]]
