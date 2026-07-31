---
name: sqlite-read-cache-config
title: SQLite 只读缓存定值约束
layer: core
category: db
keywords: [sqlite,cache,readonly,memory,hardcoded]
created: 1725080438
inclusion: auto
---

## SQLite 只读缓存定值

通过 `PRAGMA cache_size = -64` 限制每条只读连接的页缓存驻留，实测指标达标。

### 硬约束

- 只读连接必须设 `cache_size = -64` KB —— 由 `READ_CACHE_DEFAULT_KB = 64` 代码常量驱动（`gateway/db/mod.rs:372`）
- 写连接**禁止改动** SQLite 默认 `cache_size = -2000` —— 转发热路径延迟敏感，收益微（仅3×2MB）
- `AIDOG_SQLITE_READ_CACHE_KB` env 保留为 debug 旋钮，用于换库/换硬件时二分新定值（`:374`）

### 背景

- 库体积 ~7GB 时，page cache 必然被填满并长期驻留；27 条连接 × 2MB 默认 cache ≈ 54MB 常驻增量
- 修复后稳态 phys_footprint 回落 ~5MB（1900 5KB 块）；三条查询 p95 相对基线 ≤+6.3%
- 小库 <100MB 时降级 cache_size 对性能无影响（噪声淹没信号）

## 关联

[[sqlite-cache-residency-probe-method]]
