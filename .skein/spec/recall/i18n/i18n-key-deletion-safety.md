---
title: i18n-key-deletion-safety
name: i18n-key-deletion-safety
description: i18n key 删除的安全规矩
layer: recall
keywords: [i18n,key删除,grep,引用清零]
created: 1785516136
inclusion: auto
---

## i18n-key-deletion-safety

## i18n key 删除的安全规矩

删除项目中的 i18n key 时，需要确保引用点已清零，避免遗留的 key 引用导致运行时错误。

## 约束

删 i18n key 时必须**逐 key grep 确认引用点完全归零**。直接删文件内容是常见陷阱。

## 正解

1. 确认该 key 的所有调用点
   ```bash
   grep -r "platform.cliProxy.inheritedEndpoint" src/ --include="*.ts" --include="*.tsx"
   ```
2. 对每个调用点逐个审查和更新，确认没有漏网
3. 最后才能从 i18n 文件删除该 key
4. 用 `scripts/check-i18n.mjs` 做风险检测器（verify 引用点归零）

## 反例

❌ 在 i18n JSON 直接删键，不检查代码里还有没有调用 → 运行时缺键报错
❌ 只 grep 常见调用模式（如直接 `t('key')`），遗漏深层嵌套或动态拼接的引用

## 分类注意

关键词 `platform.cliProxy.inherited*` 系列（如 `inheritedEndpoint`, `inheritedModels`）涉及编辑态显示，删除时需格外谨慎，应保留编辑态仍在用的部分。

## 适用

- i18n 文件清理
- 界面流程重构后的 key 梳理
- 删除冗余翻译项

## 关联

[[platform-creation-entry-consolidation]]（同批 task remove-cliproxy-add-entry 的成果）
