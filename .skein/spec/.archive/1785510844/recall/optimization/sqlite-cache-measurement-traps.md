---
title: sqlite-cache-measurement-traps
category: optimization
keywords: [sqlite,measurement,profiling,memory,phys_footprint,noise]
status: active
inclusion: auto
protected: true
---

## SQLite 页缓存量测陷阱

## SQLite 页缓存量测三大陷阱

实测 SQLite 默认 cache_size 与各档位定值方案时踩过的坑。

### 陷阱一：内存计量工具选错

**禁用 `ps rss` / `vmmap`**。二者对 page cache 计量不全（遗漏 graphics footprint 等），见数值表面小但实际大。

**正确做法**：
- 用 `ps -p <pid> -o phys_footprint` 或 Instruments.app「Memory」→「Resident Memory」取 `phys_footprint`
- 配合 `heap --addresses 'malloc[5k]'` 数块数（page cache 驻留量的直接探针）

### 陷阱二：同进程改档混采无法对比

每档必须**独立重启进程**才能采集稳态。同进程改动 PRAGMA 后采样得到的是两个稳态的插值 —— 旧档 cache 不会瞬间清空，新档 cache 也需时间填满，中间若干采样点无效。

**正确做法**：
1. 测试档 1（如 cache_size=-2000）：重启进程 → 稳定后采样 5 分钟 → 记录数值
2. 完全杀进程、清临时文件
3. 测试档 2：全新重启 → 重复采样
4. 各档**独立重启** —— 禁同一进程内 PRAGMA 改档再采样

### 陷阱三：微秒级噪声与尖峰复核

小库（<100MB）下查询 p95 数值本身微秒级，噪声极大。同档两轮采样的 p95 差异最高达 28 倍。单次检出的尖峰（如 8-9ms）需**独立重启后复核确认为真回归**，否则视作噪声（>90% 情况下复核后回落）。

**正确做法**：
1. 小库场景仅验证**定性**（该档有无显著下降），不依赖定量 p95 数值
2. 尖峰出现 → 标记 ⚠️、独立重启一轮复核
3. 若复核后尖峰消失 → 存档为噪声注记
4. 若复核后仍现 → 才当真回归，需追踪根因

### 小库安全性保证

库 <100MB 时，cache 本就填不满。降 cache_size 对性能无害（信号被噪声淹没）。故可安心降级小库的 cache 配置，无需定制档位。
