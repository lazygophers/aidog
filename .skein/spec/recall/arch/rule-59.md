---
title: 抽组件必须 grep 确认所有调用点已切换
layer: recall
category: arch
keywords: [refactor,component,extraction,grep,dead-code]
source: -
authored-by: skein-spec
created: 1785226190
status: active
related: []
updated: 1785226190
---

## 触发场景
从大文件抽出独立组件或把函数迁移到新位置时。

## 陷阱
只 import 不渲染 = 死代码副本。原文件可能仍有内联副本，抽取后遗漏切换会导致两份代码。

## 正解
1. grep 搜索原位置组件名，确认所有调用点
2. 逐个改为新 import 路径
3. 最后删旧副本前再 grep 一次验证

## 检查清单
```bash
# 抽前 & 抽后各一次
grep -r "ProviderRow" --include="*.tsx" src/
grep -r "ProviderFormPanel" --include="*.tsx" src/
# 都应只含新 import + 调用点，无内联副本
```

## 案例
- arch-deepen-2 commit `1eee3975`：删 ImportDialog 内联 91 行副本前先 grep 确认所有调用点
- ProviderRow/ProviderFormPanel 迁移也走同样模式

## 适用
- UI 组件抽取重构
- 函数迁 crate 时
- 任何多处定义的重复

## 关联
[[写代码前查复用_grep_已有实现]]
