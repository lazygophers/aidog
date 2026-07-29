---
title: tailwind v4 禁 preflight 方式
layer: recall
category: build
keywords: [tailwind,v4,preflight,migration,css]
source: shadcn-infra
authored-by: skein-spec
created: 1784706713
status: active
related: []
updated: 1784706713
---

# tailwind v4 禁 preflight 方式

## 硬约束
Tailwind v4 迁移过程中**禁使用旧 v3 的三行导入方式**，必须用 v4 的 @import 方式。

## MUST 迁移方式
1. 仅 import theme/utilities（跳过 preflight/base）
2. 或单行总导入：@import "tailwindcss";

## 禁用的旧方式
❌ @tailwind base;  /* v3 方式，v4 崩盘 */
❌ @tailwind components;
❌ @tailwind utilities;

## 适用
Tailwind v3 → v4 迁移、新项目用 v4

## 关联
[[shadcn-infra-30]] [[shadcn-infra-28]]
