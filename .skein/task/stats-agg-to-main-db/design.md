# stats_agg_hourly 表迁回主库 — 详细设计

## 现状（proxy-log-db-split s3 决策）

- `stats_agg_hourly` DDL 跑在 `run_migrations_proxy_log_late`（log.db 写连接）
- 写入：`stats_agg.rs` UPSERT 走 `call_proxy_log_traced`（log.db 写槽 self.4）
- 读取：`usage_stats.rs` + `stats_today.rs` 多处走 `call_read_proxy_log_traced`（log.db 读池 self.5）
- Migration 050（schema_late.rs:42-48）：DROP 主库 `proxy_log` + `stats_agg_hourly` 孤儿表（b2ef9811 搬 log.db 时清主库遗留）
- 跨库 JOIN 禁 → stats_today/usage_stats 用 Rust 层合并（main 查 platform/group + log.db 查 stats_agg）

## 目标架构

`stats_agg_hourly` 回到**主库**，proxy_log 留 log.db：
- DDL 跑主库 `run_migrations_late`
- 写入走 `call_traced`（主库写槽 self.1）
- 读取走 `call_read_traced`（主库读池 self.3）
- 层合并代码**保留不动**（非动机，最小 diff）

## 改动点

### 1. DDL 迁移（schema_late.rs + mod.rs）
- 把 `STATS_AGG_HOURLY_SQL` 常量保留（或移位），CREATE 调用从 `run_migrations_proxy_log_late` 移到主库 `run_migrations_late`（紧随 Migration 050 之后，编号 051）
- `run_migrations_proxy_log_late` 内 Migration 032 stats_agg_hourly 建表段删除（回填段也搬主库，因依赖主库 group 表的 auto_map，本来就在主库预查）
- 回填 `backfill_stats_agg` 内部 `SELECT 1 FROM stats_agg_hourly LIMIT 1` 空表守卫：改走主库连接

### 2. Migration 050 修正（schema_late.rs:48）
- 删 `DROP TABLE IF EXISTS stats_agg_hourly`（主库要保留）
- 保留 `DROP TABLE IF EXISTS proxy_log`（proxy_log 仍归 log.db）
- 注释更新：说明 stats_agg_hourly 已迁回主库（Migration 051）

### 3. 新增 Migration 051：数据搬迁 log.db → main
幂等三守卫：
1. 主库 stats_agg_hourly 已有数据 → 跳过（`SELECT COUNT(*) FROM stats_agg_hourly` > 0）
2. log.db 无 stats_agg_hourly 表 → 跳过（新装或已迁）
3. 内存库（is_memory）→ 跳过（三 handle 共享主内存，无独立 log.db 文件可 ATTACH）

搬迁用 ATTACH（单 SQL，原子）：
```sql
ATTACH '{log_db_path}' AS src_log;
INSERT INTO stats_agg_hourly SELECT * FROM src_log.stats_agg_hourly;
DETACH DATABASE 'src_log';
```
- `{log_db_path}` 从 `Db.proxy_log_path` 取（Option<String>，内存库为 None）
- 失败不阻断启动：log.warning + 继续（数据可在后续 rebuild）

搬迁后**不删 log.db 旧表**（YAGNI，下次 log.db VACUUM 自然回收；避免 migration 内跨库 DROP 复杂度）。

### 4. 写入 handle 换（stats_agg.rs）
- `upsert_stats_agg`（INSERT INTO stats_agg_hourly ... ON CONFLICT）：`call_proxy_log_traced` → `call_traced`
- `rebuild_stats_agg` / `backfill_stats_agg` 调度层：同换
- 检查所有 stats_agg.rs 内 `call_proxy_log_traced` 调用点

### 5. 读取 handle 换（usage_stats.rs + stats_today.rs）
grep `call_read_proxy_log_traced` 全调用点（usage_stats 多处 line 175/230/462/483 等 + stats_today line 39/101 等）→ `call_read_traced`。
注释「stats_agg_hourly 在 log.db（proxy-log-db-split s3），走专用读池」全删/改。

### 6. retention / maintenance / backup 适配
- **maintenance.rs VACUUM 三库覆盖**（line 163-242）：主库 VACUUM 已覆盖 stats_agg（同库），无需额外；platform.db + log.db VACUUM 不变。**stats_agg 不再随 log.db VACUUM**（正合用户动机 1）。
- **retention**：检查是否有 stats_agg retention 逻辑（grep `DELETE FROM stats_agg`），若有改走主库 handle。当前调研：retention 主要针对 proxy_log，stats_agg 无独立 retention（永久保留）→ 无改动。
- **backup**（backup.rs + gateway/backup/）：检查 backup 是否枚举 log.db 表。stats_agg 随主库 backup 自动覆盖。
- **is_memory 短路**（[[dual-db-aggregate-is-memory-shortcut]] 内存记忆）：file_size/compact 聚合函数 is_memory() 短路 proxy_log 分支。stats_agg 现归主库，主库 is_memory 短路已覆盖，无需改。

### 7. 文档注释更新
- mod.rs:202「proxy_log / stats_agg_hourly 写走此槽」→ 仅 proxy_log
- mod.rs:359「log.db 承载 proxy_log / stats_agg_hourly 写」→ 仅 proxy_log
- mod.rs:726/809 call_proxy_log_traced doc
- schema_late.rs:52/93、schema_early.rs:101 注释
- usage_stats.rs / stats_today.rs 行内注释

## 关键取舍

| 取舍 | 选 | 理由 |
|---|---|---|
| 数据搬迁方式 | ATTACH + INSERT SELECT | 单 SQL 原子；vs 读出再批量插（多轮 IPC 慢） |
| 搬迁后删 log.db 旧表 | 不删 | YAGNI；log.db VACUUM 自然回收；跨库 DROP 增复杂度 |
| 重启用 JOIN 简化层合并 | 不动 | 用户明确「禁跨库 JOIN」；层合并代码已工作；最小 diff |
| Migration 编号 | 051（紧随 050） | 050 是 DROP 孤儿，051 是 CREATE + 搬迁回主库 |
| 内存库搬迁 | 跳过 | 无独立 log.db 文件；三 handle 共享主内存连接 |

## 风险

1. **ATTACH 路径权限**：log.db 路径 `~/.aidog/log.db`，主连接 ATTACH 需读权限（同用户进程，无问题）
2. **搬迁中途崩溃**：INSERT 非事务包裹 → 部分插入。幂等守卫 1（COUNT > 0 跳过）会误判已迁完。缓解：INSERT 包 BEGIN/COMMIT 事务；或用 `INSERT OR IGNORE`（UNIQUE 约束 time_hour+model+group_key+platform_id 兜底重复）
3. **schema 不一致**：log.db stats_agg 历史 schema 可能列少（早期 migration）。缓解：`INSERT INTO main(col1,col2,...) SELECT col1,col2,... FROM src` 显式列名，缺列报错时 log.warning 跳过该批
4. **测试 fixture**：test_stats_agg/test_stats_today/test_usage_stats 若显式建 stats_agg 在 proxy_log handle，需改主库 handle
