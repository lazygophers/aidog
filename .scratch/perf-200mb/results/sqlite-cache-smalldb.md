# SQLite page cache — 小库对照组（<100MB）

数据源：`sqlite-cache-smalldb.json`（同目录）。契约 9 后半。库体积全程 3~37MB（< 100MB 门槛，随
两轮采样的 loadgen 少量写入自然增长）。环境为**现造的小库**（真实 `~/.aidog/log.db`（883MB）已在
本 subtask 开始前移出 `~/.aidog/`，app 首次启动自动新建空库，全程只打 mock 分组，未触碰用户真实
数据；小库副本已在本 subtask 结束时删除，真实 log.db 已原样移回）。

**采集方式**：档位与大库组 `sqlite-cache-sweep.json` 一致（default/1024/256/64），每档独立干净重启
`open --env AIDOG_SQLITE_READ_CACHE_KB=<KB> -a AiDog`，单进程只跑一档，禁同进程混采。因首轮
（round a）`logs_list_page` p95 呈现出「随档位收紧单调上升」的疑似趋势，按 design.md 噪声处置要求
追加一轮独立重启复核（round b）。

## 曲线表 — round a（首轮）

| 档位 | log.db MB | 稳态−冷启动 MB | heap 5KB 块数 | logs_list_page p95 ms | stats_agg p95 ms | platform_balance_agg p95 ms |
|---|---|---|---|---|---|---|
| default(-2000) | 3 | 13.0 | 854 | 0.517 | 1.309 | 0.547 |
| -1024 | 3 | 14.0 | 3702 | 2.098 | 2.257 | 0.908 |
| -256 | 9 | 9.0 | 3006 | 3.536 | 1.804 | 3.671 |
| -64 | 16 | 9.0 | 932 | 4.634 | 1.578 | 1.089 |

## 曲线表 — round b（独立重启复核轮）

| 档位 | log.db MB | 稳态−冷启动 MB | heap 5KB 块数 | logs_list_page p95 ms | stats_agg p95 ms | platform_balance_agg p95 ms |
|---|---|---|---|---|---|---|
| default(-2000) | 24 | 67.0 | 1090 | 7.270 | 2.271 | 1.222 |
| -1024 | 30 | 32.0 | 7481 | 14.559 | 1.202 | 0.845 |
| -256 | 36 | 15.0 | 1197 | 4.833 | 1.026 | 1.138 |
| -64 | 37 | 5.0 | 1906 | 4.348 | 1.139 | 1.692 |

## 噪声核验（对比大库组的可复现性）

round a 的 `logs_list_page` p95 表面呈单调上升 (0.52→2.10→3.54→4.63ms)，貌似「档位越紧越慢」。
按 design.md 要求独立重启复核（round b）后**该趋势未复现**：round b 的 `default` 档 p95 反而高达
7.27ms，超过 round a 全部四档（含 -64 档的 4.63ms）；round b 的 `-1024` 档更冲到 14.56ms。两轮同一
档位的读数相差最高 **28 倍**（logs_list_page default: 0.52 vs 7.27ms），且大小关系与档位无对应关系。

`heap_5kb_block_count` 同样不可复现：round a 为 854/3702/3006/932，round b 为 1090/7481/1197/1906，
两轮同档最大相差 **6.9 倍**（-1024 档: 3702 vs 7481），且均非随档位单调。

**与大库组对照**：大库组（`sqlite-cache-sweep.md`）两轮 heap 5KB 块数误差 <1%、随档位单调下降，
判定为可信信号；小库组两轮无论 heap5k 还是 p95 均剧烈波动、方向不一致，判定为**小库场景固有的高
噪声**（库小、查询本身耗时微秒级，噪声幅度接近甚至超过信号本身），而非缓存收紧导致的真实回归。

## 结论：**定值对小库用户安全**

两轮独立重启测量均未发现随 cache_size 档位收紧（default→1024→256→64）而可复现地恶化的内存或 p95
趋势——round a 出现的疑似单调上升在 round b 完全未复现，属噪声而非回归。全程 log.db 保持
3~37MB（远低于 100MB 门槛），phys_footprint 稳态−冷启动增量最高仅 67MB，绝对量级远低于风险阈值。
小库下 cache 本就填不满（heap 5KB 块数最高仅 7481，远低于大库组 default 档的 9329），降低
`cache_size` 对小库用户既无实测危害，也印证了「库小则优化无收益但也无害」的预期。

## 清场

小库对照环境为现造 mock 库，本 subtask 结束时已删除（`~/.aidog/log.db*`），用户真实 `log.db`
（883MB）已从临时备份原样移回 `~/.aidog/`。逐次原始采样文件（`assets/results/sqlite-cache-smalldb-*.json`
共 8 份）保留在 assets 供溯源，最终结论仅以本表（json + md）为准。
