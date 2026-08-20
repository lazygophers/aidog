---
title: platform-creation-entry-consolidation
name: platform-creation-entry-consolidation
description: cli-proxy 平台创建入口唯一性
layer: recall
keywords: [cli-proxy,平台创建,入口,CliProxy]
created: 1785516136
inclusion: auto
---

## platform-creation-entry-consolidation

## cli-proxy 平台创建入口唯一性

cli-proxy 平台的创建路径需要统一化，唯一入口是 CliProxy 页的「建平台行」按钮。

## 约束

cli-proxy 平台的唯一创建入口是 **CliProxy 页 src/pages/CliProxy/index.tsx 的「建平台行」按钮**。PlatformEditForm 新建态禁带「从 cli-proxy 添加」旁路入口。

## 正解

- 添加平台表单（PlatformEditForm）只用于编辑现有平台
- 创建新 cli-proxy 平台必须走 CliProxy 页的按钮
- 该页按钮负责维护平台创建的入口单一性

## 反例

❌ 在 PlatformEditForm 新建态混入「从 cli-proxy 导入」选项 → 创建路径分裂
❌ 允许多个地方可以触发 cli-proxy 平台创建 → 维护成本增加

## 适用

- CLI Proxy 平台管理流程设计
- 添加平台表单重构

## 关联

[[i18n-key-deletion-safety]]、[[modal-state-architecture]]
