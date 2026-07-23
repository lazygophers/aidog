---
title: 删 enum 变体前先 migration DB
layer: recall
category: arch
keywords: [enum,serde,db,migration,rust,panic]
source: auto-fix-downgrade
authored-by: skein-spec
created: 1784706967
status: active
related: []
updated: 1784706967
---

# 删 enum 变体前先 migration DB

## 触发场景
删 serde 落库的 enum 变体时。

## 硬约束
**删 serde 落库的 enum 变体前必须先 migration DELETE DB 旧值**，否则代码中 `from_str` / `unwrap` 读到旧值会 panic。

## MUST 流程
1. 写 migration: DELETE FROM table WHERE enum_column = 'deleted_variant'
2. migration 上线: 清空所有旧值
3. 删 enum 变体: 改代码删变体定义 + TS union 同步
4. 验证: 单测 + DB 状态确认无残留

## 反例
❌ 先删代码再 migration → migration 期间所有访问 panic
❌ 只改 TS 未改 Rust enum → 前端过、后端崩
❌ 不 migration 直接删 → 生产读旧数据 panic

## 适用
serde enum 变体删除、DB schema enum 迁移、前后端 enum 同步

## 关联
[[shadcn-infra-32]] (locale 清理)
[[trellis-04]] (TS ↔ Rust enum 同步)
