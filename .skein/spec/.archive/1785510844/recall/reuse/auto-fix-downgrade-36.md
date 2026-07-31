---
title: 写代码前查复用 (grep 已有实现)
layer: recall
category: reuse
keywords: [grep,reuse,复用,组件,utility,抽象,dry]
source: auto-fix-downgrade
authored-by: skein-spec
created: 1784706897
status: active
related: []
updated: 1784706897
---

# 写代码前查复用 (grep 已有实现)

## 触发场景
写新函数 / 新组件 / 新 utility 前。

## MUST
- 写新函数前必须 `grep -rE '<关键词>' src/` 查已有实现；命中则复用，禁重写
- 新增平台协议必须扩展 `Protocol` union type + `PROTOCOLS` 数组
- 新增主题必须遵循 `ThemeDefinition` 接口并在 `themeMap` 注册
- 新增 locale 必须加入 `ALL_LOCALES` 数组 + `resources` 对象 + `RTL_LOCALES`
- 同一逻辑 ≥ 2 调用点必须提取到共享函数
- 提取共享函数必须放在语义正确的目录(UI→components/ 数据→services/ 主题→themes/ i18n→locales/)

## MUST NOT
- 禁止为新页面复制已有页面的 CRUD 模板代码而不提取公共组件
- 禁止定义与 `api.ts` 中已有 namespace 功能重叠的新 API 函数
- 禁止在 >1 个文件中硬编码相同的字符串常量
- 禁止绕过已有 utility 函数直接实现相同逻辑

## Abstract Threshold
- ≥ 3 处相同逻辑 → 必须 abstract
- 2 处相同逻辑 → 必须 grep 确认，commit message 说明是否 abstract
- 1 处 → ≥20 行 MUST 抽象，<20 行允许 inline

## 适用
写新代码前查复用、防止重复实现

## 关联
[[shadcn-infra-28]] (shadcn 依赖复用)
