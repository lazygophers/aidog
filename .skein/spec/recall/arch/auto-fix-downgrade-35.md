---
title: dedup 禁用设计为空的字段作 key
layer: recall
category: arch
keywords: [dedup,空字段,key,数据丢失,合并]
source: auto-fix-downgrade
authored-by: skein-spec
created: 1784706878
status: active
related: []
updated: 1784706878
---

# dedup 禁用设计为空的字段作 key

## 触发场景
写任何 dedup / 去重 / 合并逻辑(HashSet key / HashMap key / groupBy key)前。

## 陷阱
字段设计为空(待后续回填 / 占位)但被用作 dedup key → N 个对象共享同一空值 → HashSet 全撞 → **静默合并 N-1 个，数据丢失，无报错**。

## 正解
dedup key 选择优先级：
1. **业务唯一键**(user_id / email / name) — 最稳
2. **非空技术键**(url / path / hash) — 次稳
3. **组合键** — 多字段拼，至少一维非空

**禁**用设计为空(待回填 / 占位 / 语义延后)的字段作 dedup key。

## 反例
❌ (provider.source_segment, provider.base_url) 其中 base_url 全空 → 10 个对象只留首个
✅ (provider.source_segment, provider.name) 用非空字段 → 正常 dedup

## 测试
构造 N 个对象(该字段全空但其余不同)，dedup 后必须保留 N 个(非合并为 1)。

## 适用
dedup / 去重 / 合并逻辑、数据导入解析

## 关联
[[shadcn-infra-32]] (数据清理)
