# proxy_log 独立库 + 嵌入式库选型落地 — PRD (主入口)

## 目标

拆 `proxy_log` (+ `stats_agg_hourly`) 到独立 `proxy_log.db` + 独立 `Db` handle, 隔离写/VACUUM/备份/migration 扫库对主库 (platform/group/setting/model_price) 的锁竞争。**库选型: 保留 SQLite 方案 A** (零迁移成本 + 零新依赖 + 真痛点对症, DuckDB/KV 全淘汰, 详见 findings)。

用户价值: proxy_log 表过大拖慢全库 VACUUM 锁库 / 备份体积 / schema migration 扫全库 / WAL checkpoint。拆独立 .db 直接隔离这些伤害 — proxy_log 的 VACUUM/retention 只锁自己, 不阻塞主库元数据读写; 写吞吐本非瓶颈 (SQLite WAL 单写足够), 痛点是「表过大伤全库」。

成功长什么样:
- 启动建两个 .db (主 aidog.db + proxy_log.db), 各自独立 `Db` handle + WAL + 读池 + PRAGMA
- `proxy_log` + `stats_agg_hourly` 落 proxy_log.db; 其余表 (platform/group/group_platform/cli_proxy_provider/settings/hooks/mcp/middleware/model_price/quota…) 留主库
- 跨库无事务需求 (design 阶段 grep 确认), 拆库无原子性损失
- 备份/恢复/VACUUM/compact/db_file_size 覆盖两 handle
- proxy_log.db 密集写不阻塞主库读写 (WAL 独立)
- cargo test/clippy 过, 现有行为不回归 (UI 列表/筛选/详情/retention/统计/徽章)

- [x] 目标已定

## 边界

**范围内**:
1. 新建 `proxy_log.db` + 独立 `Db` handle (复用现有 `Db` 封装: Arc<Mutex<AsyncConnection>> + ReadPoolHandle + DbCache + ReconnectCtx)
2. `proxy_log` + `stats_agg_hourly` 表迁 proxy_log.db (migration 分流: 这两表 CREATE 归 proxy_log handle, 其余归主 handle)
3. Db 路由: proxy_log 写读走 proxy_log handle, 元数据走主 handle (command 层 / db 层怎么选 handle — design 定)
4. 备份/恢复/VACUUM/compact/db_file_size 推广两 handle (auto_update 备份 + 「立即压缩」按钮)
5. 启动顺序 + 双 handle 生命周期 + ReconnectCtx 各自管理

**范围外 (非目标)**:
- 不换库 (DuckDB 单写 + C++ 编译重 + duckdb-rs #117 多连接读不可见; KV 库无 SQL 需重建筛选/分页/聚合 — 全淘汰, 详见 findings + research/duckdb-deep-eval.md)
- 不改 proxy_log schema (列不变, 只换文件归属; cli_proxy_provider_id 列随表迁)
- 不做跨库事务 (本仓无需求, design grep 确认)
- 不改前端 (Tauri command 签名不变)
- 不改 retention 逻辑 (只换 handle 归属)
- 不改 stats_agg 回填算法 (同库全表扫, 零跨库成本)

**已知约束**:
- Db 结构: `Db(Arc<Mutex<AsyncConnection>>, Arc<DbCache>, ReadPoolHandle, Arc<ReconnectCtx>)` — 1 写 + N=8 读池 (`db/mod.rs:12,193`)
- PRAGMA: WAL + synchronous=NORMAL + busy_timeout=5000 + auto_vacuum=INCREMENTAL (`mod.rs:287-305`)
- migration: schema_early.rs (26) + schema_late.rs (89+ CREATE) + mod.rs inline — 成熟 ALTER 增量体系
- retention + VACUUM 已实现: cleanup_proxy_logs + incremental_vacuum_conn(100) + ANALYZE + compact_database (`proxy_log.rs:517-536`, `maintenance.rs:79-253`)
- proxy_log 无 JOIN/GROUP BY (重聚合已迁 stats_agg_hourly, SQL 复杂度低)
- stats_agg 回填 `aggregate_proxy_logs` 全表扫 proxy_log (同库零跨库成本)
- proxy_log 84 写点 + 89 migration 全部原样复用 (方案 A 零迁移成本核心)

- [x] 边界已定

## 验收标准

- [ ] 启动建 proxy_log.db (独立文件) + 独立 Db handle (各自 WAL/读池/PRAGMA)
- [ ] proxy_log + stats_agg_hourly 落 proxy_log.db (这两表的 CREATE/ALTER 归 proxy_log migration)
- [ ] 主库不含 proxy_log/stats_agg_hourly 表 (迁移期兼容存量数据)
- [ ] 跨库事务确认无 (grep 证据: 无 proxy_log↔元数据表同事务写)
- [ ] 备份/恢复覆盖两 handle (auto_update 备份 + 手动备份)
- [ ] VACUUM/compact/db_file_size 两 handle 各自工作
- [ ] proxy_log.db 密集写不阻塞主库读 (WAL 独立, 各自 busy_timeout)
- [ ] cargo clippy --workspace 无新增; cargo test --workspace 过
- [ ] 现有行为不回归 (UI 列表/筛选/详情/retention/统计/徽章链/请求日志页)

## 索引
- 详细设计: [design.md](design.md)
- 调研收敛: [findings.md](findings.md)
- 调研过程笔记: [research/db-selection.md](research/db-selection.md) + [research/duckdb-deep-eval.md](research/duckdb-deep-eval.md)
- 任务/子任务/调度: task.json (`skein.py subtask list proxy-log-db-split`)
