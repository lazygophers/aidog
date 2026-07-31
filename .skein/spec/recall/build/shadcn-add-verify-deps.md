---
name: shadcn-add-verify-deps
title: shadcn-add-verify-deps
layer: recall
category: build
keywords: [build,rule,spec]
created: 1725080438
inclusion: auto
---


# shadcn add 漏装 cva 依赖

## 触发场景
运行 `npx shadcn add` 批量添加组件后，依赖树中仅含 `@radix-ui/react-slot` 等 UI 组件依赖，**缺少 `class-variance-authority` (cva)** 导致运行时 "Cannot find package 'cva'" 错误。

## 陷阱-正解
- **陷阱**: shadcn CLI 在 yarn 4+ / pnp 环境下可能未正确解析 cva 传递依赖，只装直接依赖
- **正解**: 批量 add 后必须 grep 验证 cva 在依赖树中：
  ```bash
  yarn why class-variance-authority  # 确认在依赖树
  yarn list --pattern class-variance-authority  # 备选命令
  ```
- 缺失时手动装：`yarn add class-variance-authority`

## 反例
❌ 只加 UI 组件不验证 cva → 运行时崩
❌ 改 package.json 后不 yarn install → 锁文件未更新

## 案例
- shadcn-infra task: 首次 `shadcn add` 后运行时崩，发现 cva 缺失
- 根因: yarn 4+pnp 机制下，shadcn 未正确标记 cva 为直接依赖

## 适用
yarn 4+ / pnp 环境，shadcn 批量 add 场景

## 关联
[[shadcn-infra-31]] (同任务产出的前端规则)

---

