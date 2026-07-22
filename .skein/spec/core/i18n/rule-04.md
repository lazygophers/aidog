---
title: 新增 i18n key 必须同步 8 语言
layer: core
category: i18n
keywords: [i18n,locale,翻译,check-i18n,8语言,同步]
source: -
authored-by: skein-spec
created: 1784730604
status: active
related: []
updated: 1784730604
---

## 触发场景
alert() 迁移到 toast() 等新 i18n 机制时，新增翻译 key 必须同步到所有 locale。

## MUST 硬约束
新增 i18n key 必须同时补齐 8 个语言文件（zh-Hans/en-US/ar-SA/fr-FR/de-DE/ru-RU/ja-JP/es-ES）。

## 检查机制
- `check-i18n` 守门：跑 `yarn check-i18n` 检测 key 同步
- 缺失语言会导致对应语言用户看到 key 原文或空白

## 处理流程
```bash
# 新增 key 后检查
yarn check-i18n

# 自动补齐（示例：从 zh-Hans 复制到其他 7 语言）
cp src/locales/zh-Hans.json src/locales/en-US.json
# 手工翻译其他语言...
```

## 适用
- 所有 i18n key 新增/修改
- alert() → toast() 迁移（如 shadcn-pages task）

## 关联
[[i18n-flat-key-convention]]

## 案例
- shadcn-pages m-checkfix：新增 3 key 同步补 8 locale（1db931fe）
