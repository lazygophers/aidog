---
name: crash-safe-db-split
title: 拆库迁移四阶段 Crash-Safe 范式
layer: core
category: db
keywords: [migration,crash-safe,multi-db,state-machine]
created: 1725080438
inclusion: auto
---

多库分离迁移必须走四阶段状态机，确保任一阶段 crash 可恢复：

1. 新库建表 + 读旧库，写旧库
2. 后台增量迁移
3. 读新库，写两库
4. 停写旧库，物理清理

- `crash-safe-db-split-migration` (recall/db) 完整描述
- 多库结构：main (`aidog.db`) / log (`log.db`) / platform (`platform.db`)
