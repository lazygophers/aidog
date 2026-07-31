# SQLite page cache — 基线指标集（SQLite 默认档）

数据源：`sqlite-cache-baseline.json`（同目录），由 `sqlite-cache-harness.sh default baseline 100` 产出，
独立重启单跑（`open -a AiDog`，不设 `AIDOG_SQLITE_READ_CACHE_KB`，回落 SQLite 默认 `cache_size=-2000`）。

## 库体积（采样时）

- `log.db` = 868MB（> 500MB 门槛）
- `log.db-wal` = 8MB

## 内存

- 冷启动 `phys_footprint` = 43.0MB（t=25s）
- 稳态 `phys_footprint` = 86.0MB（驱动完三条查询后）
- 增量 = 43.0MB
- heap 5KB 块数 = 9329（与 `probe` 独立采样的 9006/9077 同量级，量级对得上「default 档约 9000」）

## 三条查询 p95（分列，n=100）

| 查询 | p95_ms | max_ms |
|---|---|---|
| logs_list_page | 0.935 | 7.30 |
| stats_agg | 0.754 | 1.17 |
| platform_balance_agg | 3.106 | 94.27 |

**基线最慢 SQL（重点跟踪项）：`platform_balance_agg`**（平台余额聚合查询），p95=3.106ms，且 max 有 ~90ms 级冷路径尖峰（大概率是首次查询计划编译，非稳态代表值，p95 已过滤掉这类离群点）。

## 样本量与噪声处置

按 design.md 噪声处置段，先用 n=30 跑一轮做基线（steady-cold=41.0MB, heap5k=9077），再用 n=100 独立重启复跑一轮
（steady-cold=43.0MB, heap5k=9329）核对稳定性。两轮各项指标同量级、无趋势性漂移（内存增量差 2MB、5KB 块数差 2.8%），
判定 n=100 在本库体积（868MB，非小库）下噪声已可控，**采用 n=100 一轮的数据作为最终基线**（本文件 + json 均取该轮）。
`platform_balance_agg` 的 max 尖峰（90-130ms 级）在两轮均出现，判定为查询首跑冷路径特征而非随机噪声，不影响 p95 读数。

## 清场

原始逐次采样（n=30 试跑的中间 json）已被 n=100 复跑覆盖（harness 按 label 落同一文件名），无残留临时文件。
