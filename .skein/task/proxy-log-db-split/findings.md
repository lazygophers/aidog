# proxy_log 独立库 + 嵌入式库选型落地 — 调研收敛

## 结论: 保留 SQLite 方案 A (独立 proxy_log.db + 独立 Db handle)

### 选型淘汰 (8 维度评估, 详见 research/)

| 候选 | 淘汰理由 |
|---|---|
| **DuckDB** | 单写模型 (read-write 一进程一写) 与高频写冲突; C++ 重依赖加 Tauri 交叉编译成本; OLAP 聚合强项本仓已用 stats_agg 卸载用不上; **致命: duckdb-rs #117 多连接读无法看到已提交数据 (OPEN)**; 无官方异步 API |
| **Turso/Limbo** | 兼容性未 100% (beta/alpha), 兼容盲区可能炸现有 84 写点 + 89 migration, 风险/收益倒挂 |
| **redb / Fjall** | 纯 Rust KV, 无 SQL — 换它们需应用层重建 6+ 二级索引/动态 WHERE/ORDER BY 分页/COUNT/范围 DELETE/stats_agg 全表扫回填, 净增复杂度, 写性能非瓶颈 |
| **RocksDB** | KV 无 SQL + Tauri 编译冲突 (tauri#5961) + librocksdb-sys macOS build 炸 (#471) + 二进制体积大 |
| **LMDB** | KV 无 SQL + 4KB 页限制 + 单写 + Windows 需预分配 map size 全量占盘 + lmdb-rkv 维护放缓 |
| **sled** | 社区共识 abandoned, 未达 1.0 |

### 方案 A 胜出理由 (research/db-selection.md §4)

1. **零迁移成本** — 全部 84 写点 + 89 migration + retention/VACUUM 代码原样复用, 只换文件路径 + 新建一个 `Db` 实例
2. **零新依赖** — 不引入任何 C++/纯 Rust KV 依赖, Tauri 三平台交叉编译零增量风险 (rusqlite bundled 已验证)
3. **真痛点对症** — 用户痛点是「proxy_log 表过大」, 伤的是全库 VACUUM 锁库时长/备份体积/schema migration 扫全库/WAL checkpoint。拆独立 .db 直接隔离。写吞吐非瓶颈 (proxy_log 单写 WAL + 渐进 UPDATE diff 已优化)
4. **WAL 红利独立享** — 每个 .db 各自 WAL, proxy_log.db 密集写不再与主库读竞争同一把写锁
5. **与 stats_agg 解耦清晰** — stats_agg_hourly 写频率与 proxy_log 同级, 同迁 proxy_log.db (回填 `aggregate_proxy_logs` 同库扫, 零跨库成本); 主库留 platform/group/setting/model_price 元数据

### ATTACH 方案淘汰 (research/db-selection.md §5)

ATTACH 单连接: 写锁仍跨整个连接范围 (未隔离, 痛点未解), 跨库事务锁语义更绕。方案 A (独立 .db + 独立 handle) 写隔离完全, 胜出。

### 关键发现 (research/db-selection.md §1)

- **proxy_log 上无任何 JOIN** — 全部 JOIN 已「去 JOIN」重构搬内存
- **proxy_log 上无 SQL GROUP BY** — 重聚合全迁 stats_agg_hourly 预聚合表
- proxy_log 查询 = 高频写 (INSERT/UPDATE) + 简单筛选扫描 (多列 WHERE + ORDER BY created_at + LIMIT/OFFSET) + COUNT + 单 PK 点查 + 时间范围 DELETE
- **耦合点**: stats_agg_hourly 回填依赖全表扫 proxy_log → 同迁 proxy_log.db 解耦

### SPEC

proxy_log 重聚合已全迁 stats_agg_hourly 预聚合表 (SQL 无 JOIN/GROUP BY), 故 proxy_log 库选型以「最小迁移成本 + 写/VACUUM 锁隔离」为标尺。非 SQL 库因重建筛选/分页/聚合的净增复杂度全部淘汰。留 SQLite 分库 (方案 A) 最优。

### 风险 (design 阶段核查)

- 跨库事务: 目前无 proxy_log↔元数据表同事务写 (proxy_log.platform_id 仅存 id 无 FK), 拆库无原子性损失 — **design 需 grep 确认无隐式跨库事务**
- 双 Db handle 生命周期/migration/PRAGMA/compact 调度各自管理 (复用现有 Db 封装)
- 备份/恢复: 「立即压缩」按钮 + auto_update 备份需覆盖两文件 (db_file_size/compact_database 推广多 handle)

### 依据引用

- research/db-selection.md (本地勘察 + 维度评估 + ATTACH 对比)
- research/duckdb-deep-eval.md (DuckDB 8 维度深度评估, 致命 duckdb-rs #117)
- 外部: duckdb.org/connect/concurrency (单写模型), duckdb-rs#117/#378, tauri#5961, librocksdb-sys#471
