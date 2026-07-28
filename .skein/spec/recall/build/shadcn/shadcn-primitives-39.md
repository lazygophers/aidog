---
title: shadcn add 依赖验证需补装检查
layer: recall
category: build/shadcn
keywords: [shadcn,add,dependencies,yarn,tailwind,verification]
source: shadcn-primitives
authored-by: skein-spec
created: 1784708026
status: active
related: []
updated: 1784708026
---

# shadcn add 依赖验证需补装检查

## 问题
shadcn add 在 yarn4+tailwind4 下 "Installing dependencies" 阶段不可靠：
- p1/p2 阶段漏装 cva/lucide-react/@radix-ui/*/vaul/sonner/cmdk
- p4 菜单组件正常

## 正解
add 后必 grep package.json 验证依赖在，缺则 `yarn add <pkg>` 补。

## 规则
不预设必漏也不预设必装，每次 add 后验证。

## 证据
commit 2b79767a "补 class-variance-authority 依赖 (shadcn add 漏装)"

## 适用
yarn 4+ + tailwind 4 + shadcn add 操作

## 关联
[[shadcn-infra-02]]
