# proxy-log-db-split 库选型调研

task: `proxy-log-db-split` (planning, 无 worktree, 主仓只读 + 外部检索)
日期: 2026-07-15
数据源: 本地代码勘察 (Read/Grep) + WebSearch (agent-reach 搜索后端 Exa/mcporter 不可用, 已降级 WebSearch; gh CLI 可用但本轮未用; Twitter/Reddit 中文社区未覆盖 — 覆盖缺口)

## 0. 现状基线 (本地勘察确认)

- 依赖: `rusqlite 0.32` (bundled+trace) + `tokio-rusqlite 0.6` (bundled) — workspace 级, aidog_core + commands_platform 共用 (`src-tauri/Cargo.toml:45-46,174-175`, `crates/aidog_core/Cargo.toml:20-21`)
- 连接模型: `Db(Arc<Mutex<AsyncConnection>>, Arc<DbCache>, ReadPoolHandle, Arc<ReconnectCtx>)` — 1 写 + N=8 读池 (`db/mod.rs:12 READ_POOL_SIZE=8`, `mod.rs:193`)
- PRAGMA: WAL + synchronous=NORMAL + busy_timeout=5000 + auto_vacuum=INCREMENTAL (`mod.rs:287-305`, `maintenance.rs:79-149 migrate_auto_vacuum`)
- 自建池, 无 r2d2/deadpool/bb8
- migration 体系: schema_early.rs (26 migration 提及) + schema_late.rs (89 ALTER/CREATE, 96 migration 提及) + mod.rs inline — 成熟 ALTER 增量体系, 深度大
- retention + 压缩已实现: `cleanup_proxy_logs` 硬删 + `incremental_vacuum_conn(100)` + ANALYZE (`proxy_log.rs:517-536`); 侧级 retention 清 body (`maintenance.rs:210-253`); 全量 VACUUM (`compact_database` `maintenance.rs:157`)

## 1. proxy_log SQL 查询模式 (决定能否换 KV) — 关键

直接 grep `FROM proxy_log` (非 test/schema) 的全部读写点:

| 查询类型 | SQL 形态 | 位置 | 频率 |
|---|---|---|---|
| 渐进写 INSERT | `INSERT INTO proxy_log (32列)` | `proxy_log.rs:242-262` | 每请求 1 次 (高频) |
| 渐进写 UPDATE (diff 列) | `UPDATE proxy_log SET {changed} WHERE id=?` | `proxy_log.rs:268-298` | 每请求 N 次 (高频) |
| 全量 upsert | `INSERT OR REPLACE` | `proxy_log.rs:50-69` | 兼容路径 |
| 列表 (UI) | `SELECT 15列 FROM proxy_log WHERE deleted_at=0 ORDER BY created_at DESC LIMIT ? OFFSET ?` | `proxy_log.rs:301-316` | UI 热读 |
| 筛选列表 | 动态 WHERE (platform_id/group_key/status/time/model/path LIKE) + ORDER + LIMIT | `proxy_log.rs:340-366` (`build_filter_where:390-449`) | UI 热读 |
| COUNT | `SELECT COUNT(*) FROM proxy_log WHERE ...` | `proxy_log.rs:369-386, maintenance.rs:256-266` | UI |
| 单行取 (点查) | `SELECT 32列 WHERE id=?` | `proxy_log.rs:452-466` | 详情页 |
| 硬删 retention | `DELETE WHERE created_at < ? AND deleted_at=0` | `proxy_log.rs:517-536` | 定时 |
| 软删清 tombstone | `DELETE WHERE deleted_at != 0` | `proxy_log.rs:543-555` | 迁移期 |
| 补写终态 | `UPDATE SET status_code,duration WHERE id=? AND status_code=0` | `proxy_log.rs:488-508` | Drop guard |
| recent health | `SELECT COUNT/SUM(CASE) FROM (SELECT status_code ... LIMIT 5)` | `usage_stats.rs:56-62` | 平台卡 |
| **回填聚合 (全表扫)** | `SELECT created_at,model,group_key,platform_id,status_code,tokens... FROM proxy_log WHERE deleted_at=0 AND request_url NOT LIKE '%count_tokens%'` → 内存 GROUP BY | `stats_agg.rs:34-60` | 偶发 (重建/回填) |
| 读 eff_pid 列 | `SELECT platform_id,group_key,actual_model,model FROM proxy_log WHERE ...` | `query_stats.rs:524` | 统计 |

**关键发现**:
1. **proxy_log 上无任何 JOIN** — 全部 JOIN 已被「去 JOIN」重构搬到内存 (grep 注释遍布 group_platform.rs/platform.rs/settings.rs/stats_today.rs/usage_stats.rs/query_stats.rs)。SQL 复杂度低。
2. **proxy_log 上无 SQL GROUP BY** — 重聚合 (SUM/GROUP BY) 全部已迁到预聚合表 `stats_agg_hourly` (`query_stats.rs:122-229`, `usage_stats.rs:8-11`, `stats_today.rs:42-111`)。proxy_log 的唯一「聚合」是 `aggregate_proxy_logs` 全表扫 + Rust HashMap 内存 GROUP BY (`stats_agg.rs:34`)。
3. proxy_log 本身的查询 = 高频写 (INSERT/UPDATE) + 简单筛选扫描 (多列 WHERE + ORDER BY created_at + LIMIT/OFFSET 分页) + COUNT + 单 PK 点查 + 时间范围 DELETE。
4. **耦合点**: `stats_agg_hourly` 回填依赖扫全 proxy_log (`stats_agg.rs:39-46`)。若 proxy_log 拆独立库, 回填路径需跨库读 (ATTACH 或 co-locate 或 stats_agg 同迁)。

**结论**: SQL 复杂度低, 理论上 KV 库可行; 但 (a) 多列动态筛选 + 分页 + 时间范围删除 需要 ≥6 个二级索引/范围扫描能力, (b) 与 stats_agg 的回填耦合, (c) 现有 84 处写点 + 全套 rusqlite 代码迁移成本极大。换 KV = 在应用层重写筛选/分页/聚合/retention, 换不来写性能的足够提升 (SQLite WAL 单写已足够, 见下)。

## 2. 候选评估维度表

维度: 写性能 / 关系+SQL 聚合 / Rust 生态成熟度 / Tauri bundled 交叉编译 / retention+压缩 / migration+schema / 迁移成本 / 内存占用 / 活跃维护 2025-26

| 候选 | 写性能 | SQL/聚合 | Rust 生态 | Tauri 交叉编译 | retention/压缩 | migration | 迁移成本 | 活跃维护 | 保留? |
|---|---|---|---|---|---|---|---|---|---|
| **SQLite (WAL, 现状)** | 高 (单写串行, WAL 读不阻塞写, 实测足够) | 完整 SQL, 已用 | rusqlite/tokio-rusqlite 成熟 | bundled C, 已验证跨三平台 | DELETE+INCREMENTAL VACUUM 已实现 | ALTER 增量体系成熟 (89+ CREATE) | 0 (已是基线) | SQLite 常青 | **保留** |
| **Turso/Limbo** | 接近 SQLite, 部分场景更快, tail latency 改善 | SQLite 兼容 (未 100%) | Rust 原生, async | 纯 Rust, 易 | 兼容 SQLite 语义 | 兼容 | 中 (API 类似但不 drop-in, 兼容性盲区) | 活跃但仍 beta, 兼容性 gap | 淘汰 |
| **DuckDB** | **单写** (one writer per process), 非 OLTP | 极强 OLAP 聚合 | duckdb-rs (仿 rusqlite) | C++ 重依赖, 交叉编译重 | 无原生 retention/VACUUM 等价 | 弱 (schema evolve 工具少) | 高 (SQL 方言差异, 单写瓶颈) | 活跃 | 淘汰 |
| **redb** | 高 (B-tree, 纯 Rust) | **无 SQL**, KV only | 纯 Rust, 1.0+ stable 2.x | 纯 Rust, 零 C 依赖, 最佳 | 需自建 (范围删 + compaction) | 无 (KV 无 schema) | 极高 (重写全部筛选/分页/聚合) | 活跃稳定 | 淘汰 |
| **Fjall** | 高 (LSM, 写强) | **无 SQL**, KV only | 纯 Rust forbid(unsafe), 3.0 | 纯 Rust, 零 C 依赖 | LSM compaction 自带, blob GC | 无 | 极高 (同 redb) | 活跃 (3.0 2026-01) | 淘汰 |
| **sled** | 中 | 无 SQL, KV | — | 纯 Rust | — | — | — | **停滞/废弃** (未达 1.0, 社区共识 abandoned) | 淘汰 |
| **RocksDB** | 极高 (LSM, 写极强) | **无 SQL**, KV | rust-rocksdb (C++ binding) | **重**: Tauri 已知编译冲突 (tauri#5961), librocksdb-sys macOS 历史炸 (#471), 二进制体积大 | compaction 自带 | 无 | 极高 + 交叉编译风险 | 活跃 (Facebook) | 淘汰 |
| **LMDB (lmdb-rkv)** | 读极快 (mmap), 写中等 | 无 SQL, KV | lmdb-rkv Mozilla 维护放缓 (RUSTSEC-2022-0001 原 lmdb 废弃) | C, 轻量 | 需自建 | 无 | 极高 + 单写 + 4KB 页限制 + 需预分配 map size (Windows 全量分配) | 半停滞 | 淘汰 |

## 3. 淘汰理由 (逐个)

- **Turso/Limbo**: 兼容性未 100% (官方 "not at 100% yet"), 仍 beta/alpha 混称; 性能部分场景不如 SQLite (维护者 penberg 承认 "still has catching up to do"); 迁移收益 = 仅换 Rust 原生 + async, 但风险 = 兼容盲区可能炸现有 84 写点 + 89 migration。**风险/收益倒挂**。([github.com/tursodatabase/turso](https://github.com/tursodatabase/turso), [turso.tech/blog/introducing-limbo](https://turso.tech/blog/introducing-limbo-a-complete-rewrite-of-sqlite-in-rust))
- **DuckDB**: **单写模型** (read-write 模式仅一进程一写, [duckdb.org/connect/concurrency](https://duckdb.org/docs/current/connect/concurrency)), 与 proxy_log 高频写 + UI 并发读冲突 (WAL 的「读不阻塞写」红利丢失); OLAP 强项 (聚合) 在本仓已用 stats_agg 预聚合卸载, 用不上; C++ 依赖加重 Tauri 交叉编译。([duckdb-rs#378](https://github.com/duckdb/duckdb-rs/issues/378), getorchestra.io 指南明确 "not designed for parallel writes")
- **redb / Fjall / LMDB / RocksDB**: 全是 KV, **无 SQL**。换它们 = 应用层重建: 6+ 二级索引 (platform_id/group_key/status/time/model/path)、动态组合 WHERE、ORDER BY created_at DESC 分页、COUNT、时间范围批量 DELETE、以及 stats_agg 回填的全表扫。这是把 SQL 引擎的工作搬进应用层, 净增复杂度。写性能提升对本仓非瓶颈 (proxy_log 单写 WAL + 渐进 UPDATE diff 已优化, 真实痛点是「表过大拖慢全库 VACUUM/备份」而非写吞吐)。
  - sled: 额外废弃 (社区共识 abandoned, 未达 1.0)。
  - RocksDB: 额外 Tauri 编译冲突 (tauri#5961)、librocksdb-sys macOS 历史 build 炸 (#471)、二进制体积。
  - LMDB: 额外 4KB 页限制、单写、Windows 需预分配 map size 全量占盘、lmdb-rkv 维护放缓。

## 4. 推荐选型

**留 SQLite, 方案 A: 独立 proxy_log.db + 独立 Db handle (双库双连接)**。

理由:
1. **零迁移成本** — 全部 84 写点 + 89 migration + retention/VACUUM 代码原样复用, 只换文件路径 + 新建一个 `Db` 实例。
2. **零新依赖** — 不引入任何 C++/纯 Rust KV 依赖, Tauri 三平台交叉编译零增量风险 (rusqlite bundled 已验证)。
3. **真痛点对症** — 用户痛点是「proxy_log 表过大」。表过大伤的是: 全库 VACUUM 锁库时长、备份体积、schema migration 扫全库、WAL checkpoint。**拆独立 .db 文件直接隔离这些伤害** (proxy_log 的 VACUUM/retention 只锁自己, 不阻塞主库 platform/group/setting 的读写), 写吞吐本就不是瓶颈。
4. **WAL 红利独立享** — 每个 .db 各自 WAL, proxy_log.db 的密集写不再与主库读竞争同一把写锁。
5. **与 stats_agg 解耦决策清晰** — stats_agg_hourly 是「每请求增量写 + 偶发全表扫回填」, 写频率与 proxy_log 同级。建议 stats_agg 随 proxy_log 同迁 proxy_log.db (回填 `aggregate_proxy_logs` 仍是同库扫, 零跨库成本); 主库留 platform/group/setting/model_price 等元数据。

**风险**:
- 跨库事务: 目前无跨 proxy_log↔元数据表的事务 (proxy_log.platform_id 仅存 id, 无 FK 强制), 拆库无原子性损失。需 design 阶段确认无将来需要跨库事务的场景。
- 两条 Db handle 的生命周期/迁移/PRAGMA/compact 调度需各自管理 (复用现有 Db 封装即可)。
- 备份/恢复: 用户「立即压缩数据库」按钮、auto_update 备份需覆盖两个文件 (现存 `db_file_size`/`compact_database` 需推广到多 handle)。

## 5. ATTACH 方案对比 (为何不 ATTACH)

| 方案 | 写隔离 | 复杂度 | 风险 |
|---|---|---|---|
| **A: 独立 .db + 独立 handle (推荐)** | proxy_log 写锁完全独立于主库 | 中 (两 Db 实例, 各自迁移/调度) | 跨库无事务 (本仓无此需求, OK) |
| B: ATTACH 单连接 | **无隔离** — ATTACH 后跨库写仍受单连接写锁串行, 且跨库事务更复杂 | 高 (ATTACH DATABASE, schema 跨库) | 写负载未隔离, 痛点未解; ATTACH 锁语义更绕 |
| C: 不拆, 优化单库 | — | 低 | 表过大痛点持续恶化 |

ATTACH 不解决核心痛点 (写锁/VACUUM 锁仍跨整个连接范围), 方案 A 胜出。

## 6. 数据不足 / 需用户裁

- **数据不足**: 各 KV 库在本仓具体数据规模下的写性能基准未实测 (外部基准为通用 workload, 非 proxy_log 32 列含 8 个大 body 字段的写模式)。
- **需用户裁**: 是否接受 stats_agg_hourly 随 proxy_log 同迁 proxy_log.db (推荐), 还是 stats_agg 留主库 (回填需跨库读, 复杂度上升)。这是唯一需要用户拍板的设计分歧, 交 main 转达。
- **需 design 核查**: 确认无任何「proxy_log 与元数据表在同一事务内写」的隐式假设 (grep 建议在 design 阶段做, 本轮只读未穷尽)。

SPEC: proxy_log 重聚合已全迁 stats_agg_hourly 预聚合表 (SQL 无 JOIN/GROUP BY), 故 proxy_log 库选型应以「最小迁移成本 + 写/VACUUM 锁隔离」为标尺, 非 SQL 库因重建筛选/分页/聚合的净增复杂度全部淘汰, 留 SQLite 分库 (方案 A) 最优。
