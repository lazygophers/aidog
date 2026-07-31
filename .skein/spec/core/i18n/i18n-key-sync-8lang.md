---
name: i18n-key-sync-8lang
title: i18n key 必须同步 8 语言齐全
layer: core
category: i18n
keywords: [i18n,locale,zh-Hans,en-US,ar-SA,fr-FR,de-DE,ru-RU,ja-JP,es-ES]
created: 1725080438
inclusion: auto
---

## 硬约则

`src/locales/` 8 个 locale 文件 MUST 保持 key 集合等值：

- **语言**：zh-Hans / en-US / ar-SA / fr-FR / de-DE / ru-RU / ja-JP / es-ES
- **新增 key** 必须同时补齐 8 份，改完跑 `yarn check:i18n`（退出码非 0 即 fail）
- **CI 不跑此脚本**，仅靠人工/agent 自觉

## 验收

```bash
yarn check:i18n  # 4 类检查 + 清单输出
# 期望 exit 0
```

## 禁用

❌ 漏某语言 → 用户切该语言见裸 key  
❌ 模板变量未展开 → 动态内容显示变量本身

## 关联

[[zh-hans-literal-sync]]
