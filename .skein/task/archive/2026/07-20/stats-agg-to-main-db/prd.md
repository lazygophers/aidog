# stats_agg_hourly 表迁回主库 — PRD (主入口)

## 目标
把 `stats_agg_hourly`（小时级聚合统计表）从 `log.db` 迁回**主库**，`proxy_log` 留 log.db 不动。反转 proxy-log-db-split s3 中该表的归属决策。

动机（用户确认）:
- [ ] retention/VACUUM 误伤：log.db 走 proxy_log retention + VACUUM，连累聚合数据被锁库/误清。移主库后不受 log retention 影响。
- [ ] 语义归属：stats_agg 是聚合派生数据（非原始日志），应与 platform/group 同库便备份/迁移。
- [ ] backup 归属：随主库备份。

**非动机**：跨库 JOIN 痛点 → 本次**不动层合并代码**，仅换 handle。JOIN 仍禁跨库（proxy_log 留 log.db）。

## 边界
范围内:
- [ ] DDL 迁移：`STATS_AGG_HOURLY_SQL` 从 `run_migrations_proxy_log_late`（log.db）→ 主库 `run_migrations_late`
- [ ] Migration 050 修正：仅 DROP 主库 `proxy_log` 孤儿，不再 DROP stats_agg_hourly
- [ ] 新增数据搬迁 migration：ATTACH log.db → INSERT INTO main.stats_agg_hourly SELECT FROM log.db.stats_agg_hourly（幂等：目标表非空跳过）
- [ ] 写入 handle：`stats_agg.rs` UPSERT/rebuild/backfill `call_proxy_log_traced` → `call_traced`
- [ ] 读取 handle：`usage_stats.rs` + `stats_today.rs` 多处 `call_read_proxy_log_traced` → `call_read_traced`
- [ ] retention/backup/VACUUM/maintenance 适配（stats_agg 归主库）
- [ ] 文档注释更新 + 测试 handle 适配

范围外（非目标）:
- [ ] 重启用 SQL JOIN 简化跨表读（层合并代码保留不动）
- [ ] proxy_log 表迁移（留 log.db）
- [ ] 改 stats_agg schema（列/索引不变）
- [ ] 改聚合逻辑（aggregate_proxy_logs 内存聚合不变，仅落库 handle 换）

已知约束:
- [ ] 拆库架构（config-db-split + proxy-log-db-split）已上线，本 task 反转其中一表的归属
- [ ] 内存库 fallback：三 handle 共享主内存连接，stats_agg 走主库无重复 VACUUM
- [ ] ATTACH 跨 db 文件拷贝：主连接 ATTACH log.db 路径需从 `Db.proxy_log_path` 取

## 验收标准
- [ ] 主库 `run_migrations_late` 建 stats_agg_hourly；`run_migrations_proxy_log_late` 不再建
- [ ] Migration 050 仅 DROP proxy_log；stats_agg_hourly 主库保留
- [ ] 新 migration 幂等拷贝 log.db → main 现有数据（目标非空跳过；log.db 无表跳过）
- [ ] stats_agg.rs 全写入走 `call_traced`（主库写槽）
- [ ] usage_stats.rs + stats_today.rs 全读取走 `call_read_traced`（主库读池）
- [ ] maintenance VACUUM/backup 三库覆盖逻辑无回归
- [ ] cargo clippy 零新增 + cargo test 全过 + yarn build
- [ ] 内存库 fallback 路径正确

## 索引
- [ ] 详细设计: [design.md](design.md)
- [ ] 调研收敛: [findings.md](findings.md)
- [ ] 任务/子任务/调度: task.json (`skein subtask list stats-agg-to-main-db`)
