---
title: platform-creation-entry-consolidation
category: frontend
keywords: [cli-proxy,平台创建,表单设计,入口收敛]
status: active
inclusion: auto
---

## cli-proxy 平台创建入口唯一性

## 触发场景

cli-proxy 平台的创建路径需要统一化，避免表单旁路导致的创建入口分裂。

## 约束

cli-proxy 平台的唯一创建入口是 **CliProxy 页 src/pages/CliProxy/index.tsx 的「建平台行」按钮**。PlatformEditForm 新建态禁带「从 cli-proxy 添加」旁路入口。

## 正解

- 添加平台表单（PlatformEditForm）只用于编辑现有平台
- 创建新 cli-proxy 平台必须走 CliProxy 页的按钮，该按钮触发表单新建态
- 该页按钮负责维护平台创建的入口单一性

## 反例

❌ 在 PlatformEditForm 新建态混入「从 cli-proxy 导入」选项 → 导致创建路径分裂，后续改表单结构时易遗漏
❌ 允许多个地方可以触发 cli-proxy 平台创建 → 维护成本增加，流程不清晰

## 适用

- CLI Proxy 平台管理流程设计
- 添加平台表单重构

## 关联

[[i18n-key-deletion-safety]]
