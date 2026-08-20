---
title: sqlite-cache-residency-probe-method
category: db
keywords: [sqlite,page-cache,measurement,heap,malloc,probe]
status: active
inclusion: auto
protected: true
---

## SQLite 页缓存常驻量的直接探针方法

## 页缓存常驻量探针

### 方法

用 `heap --addresses 'malloc[5k]'` 的 5KB 块数作为 SQLite page cache 常驻量的直接可靠探针。

**原理**：SQLite page cache 本质是 malloc 分配的 4KB pages，加上 pcache header 开销 ~1KB，合计 ~5KB/page。
每条只读连接的 cache_size 配置决定了该连接可缓存的 page 数量。所有只读连接的 page 块数之和即全局 page cache 驻留量。

### 定量公式

```
实测 5KB 块数 ≈ 只读连接数 × (cache_size_KB ÷ 4096B) × 1.01 (±3.6% 误差)
```

例如：
- 24 条只读连接、cache_size=-64KB → 预期 24 × (64 ÷ 4) ≈ 384 块 phys 驻留
- 实测验证（大库 7GB 场景）：1899~1951 块（误差 3.6% 内）

### 采样方式

```bash
heap --addresses 'malloc[5k]' <pid>
# 输出 Count 列即块数，×5KB 得驻留 MB 数
# 如 1900 块 × 5KB ≈ 9.5MB 常驻
```

这比 vmmap/ps 等全内存采样更快、更聚焦、更准确（不含其他分配尺寸的混杂）。

### 关联

[[sqlite-read-cache-config]] 定值 / [[sqlite-cache-measurement-traps]] 量测陷阱 / [[measure-window-exclusive-env]] 采样环境约束
