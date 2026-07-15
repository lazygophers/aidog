# DuckDB 深入专项评估 (vs SQLite WAL) — proxy-log-db-split

task: `proxy-log-db-split` (planning, 无 worktree, 主仓只读 + 外部检索)
日期: 2026-07-15
数据源: 本地代码勘察 (Read/Grep/Bash) + 外部检索 **经 agent-reach 的 gh CLI (dev 平台)** + **WebSearch (agent-reach search 后端本轮无输出, 降级 WebSearch/Z.ai web_search_prime)**
覆盖缺口声明: agent-reach 的 search/social/video 后端本轮未产出有效结果, 中文社区 (小红书/微博) + 视频 (B站/YouTube) + Reddit (Chrome 扩展未装) 未覆盖; 结论主要依赖官方文档 + GitHub issue + 英文基准博客, 覆盖足以终判。

## 0. 评估对象与场景回顾

- **aidog proxy_log 负载本质**: OLTP-ish — 高频小事务写 (每代理请求 1×INSERT 建行 + N×UPDATE 渐进 diff, `proxy_log.rs:242-298`) + 简单多列 WHERE 筛选列表 + ORDER BY created_at DESC + LIMIT/OFFSET 分页 + COUNT + PK 点查 + 时间范围 DELETE (retention) + 偶发全表扫回填 stats_agg_hourly。**无 JOIN/GROUP BY/窗口函数** (重聚合已迁预聚合表, 见首轮 db-selection.md)。
- **SQLite 现状基线** (本地勘察): rusqlite 0.32 + tokio-rusqlite 0.6 (async bundled); Db = 1 写槽 `Arc<Mutex<AsyncConnection>>` + 8 读池 `READ_ONLY` (`mod.rs:12,193,337-355`); WAL + synchronous=NORMAL + busy_timeout=5000 + auto_vacuum=INCREMENTAL; rusqlite API 面: **153 .execute / 118 params! / 87 query_row / 65 prepare / 61 query_map / 29 ToSql 动态 / 15 .optional() / 14 prepare_cached**; 152 call_traced + 55 call_read_traced = 207 DB 访问点; proxy_log 写调用点 24 处 (非 test)。
- **用户已定**: stats_agg_hourly 随 proxy_log 同迁独立库 (无论选 DuckDB 或 SQLite)。

---

## 1. 八维度逐条结论 (每条带引用)

### 维度 1: 写并发模型 — DuckDB 单写者, 与 SQLite WAL 等价但读池失效更严重

**结论**: DuckDB 是**单进程单写者**模型 — 「read-write 模式下, 一个进程可读写; 多写线程仅在单写进程内通过 MVCC + 乐观并发控制, appends 永不冲突, 但同行的 update/delete 并发会冲突报错」。多进程写仅靠 Quack (v1.5.2 beta, v2.0 fall 2026 才成熟) 或 DuckLake+PostgreSQL。
- 来源: DuckDB 官方 concurrency 文档 (https://duckdb.org/docs/current/connect/concurrency.html)
- 来源: duckdb-rs issue #508 「Concurrency within a single process, Why is my test blocked and only one thread can write?」(closed) 确认单写瓶颈
- 来源: getorchestra.io 指南「single-writer, multiple-reader ideal for embedded analytics, bottleneck for high-concurrency」

**对比 SQLite WAL**: SQLite 也是 1 写者, 但**读不阻塞写、写不阻塞读** (WAL 核心红利)。DuckDB 单写者 + 多读快照在「语义」上与 SQLite WAL 相似 (都是 1写+N读)。**但关键差异在维度 2 (duckdb-rs 读池)**: DuckDB 多读要求所有连接共享同一 `duckdb_database` 实例 (官方 try_clone 模式), 而 SQLite 每条连接独立 open 即共享同一 WAL 文件。

**对 aidog 影响**: aidog 的 upsert 是**同行渐进 UPDATE** (`update_proxy_log_columns`, `proxy_log.rs:268-298`, WHERE id=? UPDATE 同一行多列)。不同请求 = 不同行, appends 不冲突 — 这点 DuckDB 能撑。但「单写者串行」与 SQLite 单写槽 `Arc<Mutex>` **等价**, DuckDB 在写并发上**无优势**, 反而丢掉 SQLite WAL「读不阻塞写」的成熟隔离。

### 维度 2: 读并发模型 — **致命**: duckdb-rs `Connection::open` 建独立 DB 实例, 读池看不到已提交数据

**结论 (硬证据, 终判关键)**: duckdb-rs 的 `Connection::open()` **每次调用都创建独立的 `duckdb_database` 实例** (经 `duckdb_open_ext()`), 即使指向同一文件路径, 这些连接**不共享事务状态** — 一个连接 COMMIT 后, 另一连接**看不到已提交数据**。必须用 `con1.try_clone()` 派生共享同一 database handle 的连接才能跨连接可见。
- 来源: duckdb-rs issue #711「Committed Data Not Visible to Other Open Connections」(closed, 2026-03) — 维护者明确「each `Connection::open()` call creates a separate `duckdb_database` instance ... They are independent in-process databases that don't share transaction state. Workaround: `try_clone()`」
- 来源: duckdb-rs issue #117「Interleaved connections results in table does not exist error」(**OPEN since 2023-02**, 未修) — 独立 open 的连接连 DDL 都互相不可见

**对 aidog 影响 (致命)**: aidog 读池 `build_read_pool` (`mod.rs:340-355`) 开 `READ_POOL_SIZE=8` 条**独立 `Connection::open` + READ_ONLY** 连接给 UI 热读 (列表/筛选/统计)。这套模型直接搬 duckdb-rs → **8 个读连接看不到写连接 (upsert) 提交的新日志** → UI 列表永远显示旧数据或空。必须**重写读池为 try_clone 派生模型** (全部连接共享同一 database handle), 且 #117 仍 open 说明多连接可靠性未稳定。SQLite WAL 无此问题 (每条连接独立 open 即共享同一 WAL 文件, 天然读已提交)。

### 维度 3: duckdb-rs 成熟度与 async 支持 — SYNC-only, 无 tokio 等价物

**结论**:
- **版本活跃度**: duckdb-rs 活跃, 最新 v1.10504.0 (2026-06-17), v1.4.5 LTS (2026-06-17)。维护节奏紧跟 DuckDB 主版本。来源: `gh release list --repo duckdb/duckdb-rs`
- **async/await**: duckdb-rs **官方只有同步 API** (类 rusqlite), **无 tokio-rusqlite 等价物**。需 async 有两条路: ① 第三方 `async-duckdb` crate (crates.io, 非官方, 测 tokio + async_std); ② 自包 `tokio::task::spawn_blocking`。来源: crates.io/crates/async-duckdb; duckdb.org/docs/current/clients/rust
- **API 稳定性**: 仿 rusqlite (params!/prepare/query_row/query_map), 表面相似, 但: ① 无 `prepare_cached` 等价 (14 处用) — DuckDB 自带 plan cache 但 API 不同; ② `.optional()` (15 处) 需查 duckdb-rs 是否提供 OptionalExtension; ③ 动态 `Box<dyn ToSql>` (29 处) 在 duckdb-rs 的 ToSql trait 下需逐一验证。

**对 aidog 影响**: aidog 全栈基于 tokio-rusqlite 的 `AsyncConnection::call` + `call_traced`/`call_read_traced` (207 访问点)。换 duckdb-rs = 要么引非官方 async-duckdb (额外依赖风险) 要么 207 处全改 spawn_blocking (线程模型重写)。async-duckdb 非官方且功能滞后官方 crate, 与「官方 tokio-rusqlite 已验证」不对称。

### 维度 4: 写吞吐基准 — DuckDB 单行 INSERT 对 OLTP 是弱项 (比 SQLite 慢)

**结论 (多源一致)**: DuckDB 列存 + 向量化执行 (row group 122,880 行) **为批量 APPEND 优化, 单行/逐行 INSERT 有高 per-statement 开销**。SQLite 为 OLTP 点查/小事务而生。
- 来源: medium「7 DuckDB vs SQLite Benchmarks」—「Single-Row Insert Speed: SQLite is designed for OLTP; DuckDB is slower for small, transactional inserts, shines with batch loads」
- 来源: lukas-barth.net/blog/sqlite-duckdb-benchmark —「point/simple queries, SQLite outperforms DuckDB by one or two orders of magnitude」
- 来源: duckdblab.org —「complements: SQLite for writes, DuckDB for analytics; DuckDB 80-200x faster on analytical」
- 来源: reddit r/dataengineering —「SQLite only ~2x as fast as duckdb for transactional workloads」(SQLite 在 OLTP 仍胜, 虽幅度小于 OLAP 场景的反差)
- 来源: duckdb discussion #13371 — 用户报告逐行 insert 慢 (row group 122,880 行的批导向设计)

**对 aidog 影响 (负面)**: aidog proxy_log 写 = **高频逐行 INSERT + 同行渐进 UPDATE** (每请求多轮 upsert), 正是 DuckDB 最弱 workload。SQLite WAL 单写 + prepare_cached + 渐进 diff UPDATE 已是优化的 OLTP 路径。换 DuckDB 写吞吐**预期持平或劣化**, 无收益。

### 维度 5: UI 热读性能 — DuckDB 列存对「多列筛选 + 分页」无优势 (聚合已卸载)

**结论**: DuckDB 列存加速的是**全表扫 + 聚合 (SUM/GROUP BY/JOIN)**, 对「多列 WHERE 等值/范围 + ORDER BY + LIMIT 分页 + COUNT + PK 点查」这类**点查询/小范围扫描**, SQLite 行存 + B-tree 索引持平甚至快 1-2 个数量级。
- 来源: lukas-barth.net —「point queries SQLite 1-2 orders of magnitude faster」
- 来源: motherduck.com/learn/duckdb-vs-sqlite —「SQLite's advantage lies in simple/point queries and transactional workloads」

**对 aidog 影响 (无优势 + 负面)**: aidog proxy_log 的 UI 热读全是点查/小范围 (筛选列表 + 分页 + COUNT + 详情点查)。DuckDB 强项 (聚合扫描) 已被 stats_agg_hourly 预聚合卸载 (首轮 db-selection.md 已确认 proxy_log 上无 GROUP BY/JOIN)。唯一可能沾边 DuckDB 优势的是 `aggregate_proxy_logs` 偶发全表扫回填 (`stats_agg.rs:34-60`), 但那是**低频重建/回填路径**, 不是热读, 用 SQLite 扫也足够。**为低频回填换库 = 杀鸡用牛刀**。

### 维度 6: 迁移成本量化 — 极高, API 差异点 + SQL 方言 + 读池重写

**API 差异点 (本地勘察, 量化)**:
| rusqlite API | 使用次数 | duckdb-rs 状态 | 迁移动作 |
|---|---|---|---|
| `.execute(` | 153 | 有 (同名) | 类型绑定验证 |
| `params![...]` | 118 | duckdb-rs 有 params! 宏 (仿 rusqlite) | 逐一验证类型映射 |
| `query_row` | 87 | 有 | 验证 |
| `prepare(` | 65 | 有 | 验证 |
| `query_map` | 61 | 有 | 验证 |
| `Box<dyn ToSql + Send>` 动态绑定 | 29 | duckdb-rs ToSql trait 不同, 需改 | **29 处重写** (filter where 动态参数, `proxy_log.rs:174-222,390-449`) |
| `.optional()` | 15 | 需查 duckdb-rs OptionalExtension | 验证/改 |
| `prepare_cached` | 14 | **无直接等价** (DuckDB 内部 plan cache, API 不同) | 14 处改写 |
| `call_traced`/`call_read_traced` (tokio-rusqlite) | 207 | **无 async 等价** | 207 处改 async 模型 (spawn_blocking 或 async-duckdb) |

**SQL 方言差异 (本地勘察)**:
- `AUTOINCREMENT` keyword: **DuckDB 不支持** (issue #15436), 需 `CREATE SEQUENCE + DEFAULT nextval()`。本地 schema_early 8 处 + schema_late 14 处 = **22 处**。proxy_log.id 本身是 TEXT 主键 (无 AUTOINCREMENT, OK), 但 stats_agg_hourly.id (`mod.rs:35`) 用了 — 用户已定 stats_agg 同迁, **必须改写**。
- `INSERT OR REPLACE` (6 处) / `INSERT OR IGNORE` (6 处) / `ON CONFLICT` (7 处): DuckDB 支持 `INSERT ... ON CONFLICT` (兼容) 和 `INSERT OR REPLACE` (部分), 需逐一验证; DuckDB 无 `INSERT OR IGNORE` (改 `ON CONFLICT DO NOTHING`)。
- 时间函数: 无 `strftime`/`julianday`/`localtime` 依赖 (proxy_log 用 Unix 秒整数列, 应用层格式化) — 这块**兼容**。

**读池重写 (致命, 维度 2)**: `build_read_pool` (`mod.rs:340-355`) 从 8×独立 open+READ_ONLY 改为 try_clone 派生共享 database handle; ReconnectCtx 重连逻辑 (`mod.rs:193` `Arc<ReconnectCtx>`) 需适配 duckdb 重连语义。

**retention/压缩**: DuckDB 用 `CHECKPOINT`/`FORCE CHECKPOINT`/`VACUUM` (DuckDB 有 VACUUM 语句), 但无 `incremental_vacuum` 等价 (`proxy_log.rs:527,549` cleanup_proxy_logs/purge_deleted 调用); auto_vacuum=INCREMENTAL 模型 (`mod.rs:296-305`, `maintenance.rs:79-149 migrate_auto_vacuum`) 在 DuckDB 无对应。retention DELETE 仍可用, 但 free page 回收机制需重设计。

**工时粗估 (按 .subtask)**:
- API 批量改写 (params/execute/query_row 类型验证 + 14 prepare_cached + 15 optional): ~3-4 subtask
- 29 处动态 ToSql 重写 + filter_where 适配: ~1-2 subtask
- 207 处 async 模型迁移 (spawn_blocking 包装或 async-duckdb 引入 + call_traced 重实现): ~3-4 subtask
- SQL 方言 (22 AUTOINCREMENT + 6 OR IGNORE + 验证 OR REPLACE/ON CONFLICT): ~2 subtask
- 读池 try_clone 重写 + ReconnectCtx: ~2 subtask
- retention/VACUUM/CHECKPOINT 重设计 + 测试: ~2 subtask
- schema migration 体系重建 (schema_early 26 + schema_late 132 migration 标记, DuckDB 无 sqlite_master/user_version, 需自建 migration 跟踪表): ~2-3 subtask
- 全量测试改写 (proxy_log.rs test 620-686 + test_proxy_log.rs 21K + stats_agg test 等): ~2-3 subtask
- **合计 ~17-22 subtask**。vs SQLite 方案 A (零 API 改 + 仅换文件路径 + 新建 Db 实例) = **0-1 subtask**。

### 维度 7: 场景错配分析 — proxy_log OLTP-ish vs DuckDB OLAP 定位, 根本错配

**结论**: DuckDB 官方自我定位为**分析型 (OLAP)** 数据库, 与 SQLite 的 OLTP 定位互补而非竞争。多源共识: 「SQLite for writes/OLTP, DuckDB for analytics/OLAP」。
- 来源: motherduck.com/learn/duckdb-vs-sqlite —「SQLite optimized for transactional (OLTP) ... DuckDB designed for analytical (OLAP)」
- 来源: duckdblab.org —「not competitors but complements」
- 来源: datacamp.com —「SQLite OLTP excels at small inserts/updates/single-row; DuckDB OLAP」

**对 aidog 影响 (根本错配)**: proxy_log = OLTP-ish (写密集 + 点查/范围筛选), **正是 SQLite 的主场、DuckDB 的弱场**。DuckDB 强项 (OLAP 聚合扫描) 已被 stats_agg_hourly 预聚合卸载, proxy_log 上无 GROUP BY/JOIN。**用 DuckDB 存 proxy_log = 用分析库干事务库的活, 能力倒挂**。

### 维度 8: 混合方案 (主库 SQLite + 日志库 DuckDB 异构) — 不值得

**结论**: 混合方案引入: ① 两套 DB 依赖 (rusqlite + duckdb-rs C++), Tauri 交叉编译增量风险翻倍; ② 两套 async 模型 (tokio-rusqlite + spawn_blocking/async-duckdb), 认知与维护负担; ③ 异构库间无跨库事务 (本就无此需求, 但同栈 SQLite 分库也无此需求且更简单); ④ DuckDB 在 proxy_log 场景无任何局部优势能抵消上述成本 (写慢、点读无优势、聚合已卸载)。

唯一**理论**沾边 DuckDB 的场景 = `aggregate_proxy_logs` 全表扫回填 stats_agg, 但该路径**低频** (仅重建/空表回填), 用 SQLite 扫同样足够, 不构成换库理由。同栈 SQLite 分库 (方案 A) 完全覆盖, 且零新依赖。

---

## 2. DuckDB vs SQLite 对照表

| 维度 | DuckDB | SQLite WAL (现状/方案 A) | 胜方 |
|---|---|---|---|
| 写并发模型 | 单进程单写者 (多写线程仅 append 不冲突, 同行 update 冲突) | 单写槽 `Arc<Mutex>` + WAL 读不阻塞写 | **SQLite** (读不阻塞写, 隔离成熟) |
| 读并发模型 | 多读需共享 database handle (try_clone); duckdb-rs 独立 open **看不到已提交数据** (#711) | 每条连接独立 open 即共享 WAL, 天然读已提交 | **SQLite** (致命差异) |
| 写吞吐 (单行 INSERT/OLTP) | 列存向量化, 逐行 INSERT 弱 (为 batch 优化) | OLTP 主场, 点查/小事务快 1-2 数量级 | **SQLite** |
| 筛选读 (多列 WHERE+ORDER+LIMIT) | 列存对点查/小范围无优势 | 行存+B-tree, 点查快 | **SQLite** (持平或胜) |
| 聚合扫描 | 强 (但已卸载到 stats_agg) | 弱 (但 proxy_log 无聚合) | 平 (用不上) |
| 迁移成本 | ~17-22 subtask (API+async+方言+读池+migration+test) | 0-1 subtask (换文件路径+新建 Db) | **SQLite** |
| Tauri 桌面集成 | C++ 重依赖, Windows link.exe 失败需 build.rs hack (#544), bundled 体积大 | C bundled, 已验证跨三平台 | **SQLite** |
| 场景匹配 | OLAP (proxy_log 是 OLTP-ish, 错配) | OLTP (proxy_log 主场) | **SQLite** |
| 依赖增量 | duckdb-rs (C++) 或 + async-duckdb | 零新依赖 | **SQLite** |

**SQLite 8:0 DuckDB, 聚合维度平局用不上。**

---

## 3. 迁移成本量化 (汇总, 详见维度 6)

- **API 改写点**: 207 async 访问点 + 29 动态 ToSql + 14 prepare_cached + 15 optional 验证 = **~265 触点**
- **SQL 方言**: 22 AUTOINCREMENT + 6 OR IGNORE + 读池 try_clone 重写 + retention/VACUUM 重设计
- **migration 体系重建**: schema_early 26 + schema_late 132 标记, DuckDB 无 user_version 需自建跟踪表
- **工时档**: ~17-22 subtask (vs SQLite 方案 A 的 0-1)
- **风险点**: ① duckdb-rs 读池跨连接不可见 (#711, 致命) ② #117 仍 open (多连接 DDL 不可见) ③ async 无官方方案 ④ Windows 交叉编译 (#544) ⑤ 非官方 async-duckdb 滞后

---

## 4. 终选推荐

### 推荐: **留 SQLite, 方案 A (独立 proxy_log.db + 独立 Db handle, 双库双连接)。DuckDB 淘汰。**

**硬理由 (数据支撑, 非推测)**:
1. **场景根本错配** (维度 7): proxy_log OLTP-ish vs DuckDB OLAP, 多源共识「SQLite for writes, DuckDB for analytics」。proxy_log 无聚合 (已卸载 stats_agg), DuckDB 强项用不上。
2. **读池致命缺陷** (维度 2, 终判关键): duckdb-rs `Connection::open` 建独立 DB 实例, 8 读连接看不到写连接提交的日志 (#711, closed 2026-03); #117 (2023 至今 OPEN) 多连接 DDL 不可见。aidog UI 热读模型直接失效。SQLite WAL 每连接独立 open 天然读已提交, 无此问题。
3. **写吞吐倒挂** (维度 4): proxy_log 高频逐行 INSERT + 同行渐进 UPDATE = DuckDB 最弱 workload (列存为 batch 优化), SQLite OLTP 主场快 1-2 数量级。
4. **迁移成本悬殊** (维度 6): DuckDB ~17-22 subtask + ~265 API 触点 + 读池/async/migration 体系全重写; SQLite 方案 A = 0-1 subtask (换文件路径 + 新建 Db 实例, 84 写点 + 89 migration 原样复用)。
5. **写并发无优势** (维度 1): DuckDB 单写者与 SQLite 单写槽等价, 但 SQLite WAL「读不阻塞写」更成熟。
6. **Tauri 集成风险** (维度 6/8): DuckDB C++ 重依赖, Windows link.exe 失败需 build.rs hack (#544); SQLite C bundled 已验证跨三平台。混合方案 (维度 8) 翻倍依赖成本, 无局部优势抵消。

### DuckDB 局部优势说明 (诚实评估)
- DuckDB 唯一理论沾边场景 = `aggregate_proxy_logs` 全表扫回填 stats_agg_hourly (`stats_agg.rs:34-60`), 列存扫表快。但该路径**低频** (仅重建/空表回填, 非热读), 用 SQLite 扫同样足够, **不构成换库理由**。
- 若未来 aidog 出现「在原始 proxy_log 上做重度即席 OLAP 分析 (跨月 GROUP BY 多维聚合) 且预聚合表不够用」的场景, 可再评估 DuckDB 作只读分析副库 (ATTACH 或独立导出 .parquet)。当前无此需求, YAGNI。

### 方案 A 确认 (无变化, 与首轮 db-selection.md 一致)
- 独立 proxy_log.db + 独立 Db handle, stats_agg_hourly 同迁 (用户已定)。
- 零 API 改、零新依赖、写/VACUUM 锁隔离 (proxy_log 的 retention/VACUUM 只锁自己, 不阻塞主库元数据读写)、各库独立 WAL。
- 待 design 核查: 确认无「proxy_log 与元数据表同事务写」的隐式假设 (本轮只读未穷尽, 留 design 阶段 grep)。

---

## 5. 证据来源索引

**官方文档**:
- DuckDB Concurrency: https://duckdb.org/docs/current/connect/concurrency.html (单写者 + 多读快照 + MVCC + 乐观并发; 多进程写靠 Quack beta / DuckLake)
- DuckDB Rust client: https://duckdb.org/docs/current/clients/rust (官方 crate 同步 API)
- DuckDB CREATE SEQUENCE: https://duckdb.org/docs/lts/sql/statements/create_sequence (AUTOINCREMENT 不支持, 用 SEQUENCE+nextval)

**GitHub issue (duckdb-rs)**:
- #711 Committed Data Not Visible to Other Open Connections (closed 2026-03) — Connection::open 建独立 duckdb_database 实例, 必须 try_clone
- #117 Interleaved connections results in table does not exist error (OPEN since 2023-02) — 多连接 DDL 不可见未修
- #508 Concurrency within a single process blocked (closed) — 单写瓶颈
- #544 failed to link with link.exe on Windows (closed) — 交叉编译需 build.rs hack
- releases: v1.10504.0 (2026-06-17), v1.4.5 LTS

**基准/对比 (英文)**:
- medium「7 DuckDB vs SQLite Benchmarks」(单行 INSERT SQLite 胜)
- lukas-barth.net/blog/sqlite-duckdb-benchmark (点查 SQLite 快 1-2 数量级)
- duckdblab.org (complements: SQLite writes, DuckDB analytics; DuckDB 80-200x on analytical)
- motherduck.com/learn/duckdb-vs-sqlite-databases (SQLite point/transactional advantage)
- datacamp.com/blog/duckdb-vs-sqlite-complete-database-comparison (OLTP vs OLAP 定位)
- reddit r/dataengineering「Why We Moved SQLite to DuckDB」(SQLite OLTP 仍 ~2x 快)
- duckdb discussion #13371 (逐行 insert 慢, row group 122,880 批导向)

**本地代码 (file:line)**:
- `gateway/db/proxy_log.rs:242-298` (INSERT 建行 + UPDATE 渐进 diff, OLTP 写)
- `gateway/db/proxy_log.rs:301-449` (列表/筛选/COUNT/filter_where, 点查+小范围读)
- `gateway/db/mod.rs:12,193,337-355` (READ_POOL_SIZE=8 读池独立 open READ_ONLY)
- `gateway/db/mod.rs:287-319` (WAL + synchronous=NORMAL + auto_vacuum=INCREMENTAL PRAGMA)
- `gateway/db/schema_early.rs:76` + `schema_early.rs:8` + `schema_late.rs:14` (proxy_log TEXT 主键 OK; 22 处 AUTOINCREMENT 需改写)
- rusqlite API 面: 153 execute / 118 params! / 87 query_row / 65 prepare / 61 query_map / 29 ToSql / 15 optional / 14 prepare_cached / 207 call_traced+read_traced

**覆盖缺口**: agent-reach search/social/video 后端本轮无有效输出; 中文社区 (小红书/微博) + 视频 + Reddit 未覆盖。但官方文档 + GitHub issue + 英文基准已覆盖全部 8 维度且结论一致 (SQLite 胜), 缺口不影响终判。
