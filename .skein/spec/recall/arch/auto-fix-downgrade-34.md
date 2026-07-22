---
title: DB 拆库访问点归属审计三形式
layer: recall
category: arch
keywords: [db,sqlite,拆库,handle,审计,call_traced,write_conn,read_conn]
source: auto-fix-downgrade
authored-by: skein-spec
created: 1784706845
status: active
related: []
updated: 1784706845
---

# DB 拆库访问点归属审计三形式

## 触发场景
表从一个 SQLite 库拆到另一个库（主库→log.db / platform.db），需把该表所有访问点切到新 handle 时。

## 陷阱-正解
- **陷阱**: 只查 `call_*_traced` chokepoint → 漏掉 `.write_conn()` / `.read_conn()` 直访形式 + `conn` 直接持有形式 → 运行时 `no such table` 崩
- **正解**: 必须同时查三种形式（缺一不可）：
  1. wrapper 形式：`call_platform_traced` / `call_group_traced`
  2. 直访形式：`\.\write_conn\|\.read_conn`
  3. 裸 SQL：`FROM "group"\|FROM platform`

## 验收命令
```bash
# 1. wrapper 形式
grep -rn "call_platform_traced\|call_group_traced" src-tauri

# 2. 直访形式（必须！）
grep -rn "\.write_conn\|\.read_conn" src-tauri/crates/aidog_core/src/gateway

# 3. 裸 SQL（按被拆表名 grep）
grep -rn 'FROM "group"\|FROM platform\|FROM group_platform\|FROM cli_proxy_provider' src-tauri/crates/aidog_core/src
```

## 反例
❌ 只 grep `call_traced` → 6 处 `write_conn` 漏网（s3 错误模式）
❌ 只 grep helper 函数名 → 裸 SQL 漏查
❌ 人工 audit → 人肉易疏漏

## 适用
DB 拆库迁移、表访问点归属审计

## 关联
[[cross-db-read-two-phase]] (跨库读两阶段)
[[dedup-empty-field-key]] (空字段作 key)
