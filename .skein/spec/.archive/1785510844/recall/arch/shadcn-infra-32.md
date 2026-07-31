---
title: locale 死键清理归属
layer: recall
category: arch
keywords: [locale,dead-key,cleanup,responsibility,theme]
source: shadcn-infra
authored-by: skein-spec
created: 1784706760
status: active
related: []
updated: 1784706760
---

# locale 死键清理归属

## 流程约定
**删除主题/功能导致的 locale 死键，由删该主题/功能的 task 同源清理**，不甩给下游消费 task。

## 正解
1. **删 palette 主题**: 清理所有 `theme.color.{palette}` 相关 locale 键
2. **删 enum 变体**: 清理所有该变体相关的 UI 文案键
3. **删功能模块**: 清理该模块所有 locale 键

## 陷阱
- **陷阱**: 删代码只删 TS 类型，locale 死键留给后续清理 → 下次改 locale 人困惑
- **陷阱**: 多个消费 task 各自清理 → 重复工作或遗漏

## 反例
❌ 删 palette 只改代码不清理 locale → 死键残留
❌ 甩给「下次整理 locale 时」→ 永远不清理
❌ 下游 task 清理上游残留 → 责任不清

## 案例
- shadcn-infra task: 删 palette 时应同步清理 theme.color.* locale 键

## 适用
locale 清理、主题删除、功能下架、enum 变体删除

## 关联
[[auto-fix-downgrade-38]] (同任务 enum 删约定)
