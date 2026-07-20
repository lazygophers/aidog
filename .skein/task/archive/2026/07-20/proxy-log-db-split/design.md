# proxy_log 独立库 + 嵌入式库选型落地 — 详细设计

## 决策 (用户裁)

| 决策点 | 选 | 理由 |
|---|---|---|
| 库选型 | SQLite 方案 A (独立 proxy_log.db + 独立 handle) | 零迁移成本零新依赖, 真痛点对症 (findings) |
| handle 路由 | **B 主 Db 内嵌 proxy_log handle** | Db struct 加 proxy_log_handle 字段 + call_proxy_log_traced; command 层零改 (State<Db> 不变); 代理 ProxyState 改持主 Db Arc 不再独立 open; 锁分离 (proxy_log 独立 Mutex) |
| 跨表读 | **应用层分别查 + 合并** (批量 IN, 非 per-row) | proxy_log 查完按 id set 批量查主库元数据 (group/platform/cli_proxy_provider), Rust 合并; 无 ATTACH 路径 WAL 兼容零风险; 无缓存失效问题 |
| 文件路径 | `~/.aidog/proxy_log.db` | 与主 aidog.db 平级, 备份/迁移路径一致 |

## 架构

```
主 aidog.db (元数据)                 proxy_log.db (日志, 独立文件)
  platform                              proxy_log (32 列 + cli_proxy_provider_id)
  group                                 stats_agg_hourly
  group_platform                        (+ 索引)
  cli_proxy_provider
  setting / hooks / mcp / middleware
  model_price / quota / skill ...

Db struct (单一, 内嵌两 handle):
  Db {
    main_handle:  Arc<Mutex<AsyncConnection>>   // 写元数据
    proxy_log_handle: Arc<Mutex<AsyncConnection>>  // 写 proxy_log/stats_agg
    cache: Arc<DbCache>
    main_read_pool: ReadPoolHandle             // N=8 读元数据
    proxy_log_read_pool: ReadPoolHandle        // N=8 读日志
    reconnect: Arc<ReconnectCtx>               // 各自重连
  }

注入: app.manage(Db) → State<Db> 全仓 151 处不变
代理侧: ProxyState 改持 Arc<Db> (含两 handle), 不再 proxy.rs:32 独立 open
```

## 数据流

### 代理转发写 proxy_log (高频)
1. 请求经 proxy handler → ProxyState.db (现独立 open) **改为** ProxyState 持 Arc<Db>
2. upsert_proxy_log → `db.call_proxy_log_traced(...)` → proxy_log_handle (独立 Mutex, 不争元数据写锁)
3. 落 proxy_log.db (独立 WAL, 不与主库读竞争)

### UI 查 proxy_log (list/count/detail)
1. command State<Db> → db.call_proxy_log_traced (或 call_read_proxy_log_traced)
2. proxy_log_read_pool 读 proxy_log.db
3. 跨表读 (JOIN/展示) → 应用层合并 (见下)

### 跨表读 (应用层分别查 + 合并)
现状跨表读点 (拆库后改应用层):
- `list_request_logs` LEFT JOIN cli_proxy_provider (proxy_log.rs:430) → proxy_log 查完, 按 cli_proxy_provider_id set 批量 `SELECT id,name FROM cli_proxy_provider WHERE id IN (...)` (主 handle), Rust 合并 name
- `today_platform_stats` 读 platform (stats_today.rs:122) → 按 platform_id set 批量查主库
- `query_stats` 读 platform/group (query_stats.rs:308-309) → 应用层合并
- `usage_stats.rs:341` 读 platform/group → 应用层合并
- `aggregate_proxy_logs` 读 group (stats_agg.rs:34, 回填路径) → 应用层合并
- `backfill_stats_agg_if_empty` (schema_late.rs:275) 读 proxy_log + group → 应用层合并

**N+1 防护**: 全部按 id set 批量 IN 查询 (LIMIT 50 → 去重 id ≤50 → 1 次主库 IN 查), 禁 per-row 查。

### stats_agg 回填 (全表扫)
- aggregate_proxy_logs 扫 proxy_log (proxy_log.db 同库) + 读 group (主库, 应用层合并)
- 同库扫零跨库成本, group 部分批量查主库

### retention / VACUUM
- proxy_log retention (cleanup_proxy_logs) + stats_agg cleanup → proxy_log_handle
- compact_database / db_file_size → **双库各跑/求和**
- migrate_auto_vacuum → proxy_log.db 建表前设 INCREMENTAL

## migration 分流

现有 schema_early.rs + schema_late.rs + mod.rs inline 单连接顺序跑。拆库后:

**方案**: migration runner 按 表归属选 handle。proxy_log + stats_agg_hourly 相关 CREATE/ALTER/INDEX 跑 proxy_log_handle, 其余跑 main_handle。

归属清单 (全量 file:line 见 research/design-detail-check.md §2):
- **proxy_log.db**: proxy_log 表 (CREATE schema_early.rs:76 + ALTER 多处) + 索引 + stats_agg_hourly 表 (mod.rs:34 STATS_AGG_HOURLY_SQL) + idx
- **主库**: platform/group/group_platform/cli_proxy_provider/setting/model_price/middleware_rule/notification/mcp_server/skill...

**backfill_stats_agg_if_empty** (schema_late.rs:275): 跨表读 proxy_log + group → 改应用层 (proxy_log 扫 + group 批量查主库)。

**幂等**: 现有 `let _ =` idiom 保留, 各 handle migration 独立幂等。

## 取舍

| 决策 | 选 | 理由 |
|---|---|---|
| handle 组织 | B 内嵌 (非 A 两 State / C ATTACH) | command 层零改, 锁分离, 改造集中 db 层 |
| 跨表读 | 应用层合并 (非 ATTACH / 非内存预取) | 无 ATTACH WAL 风险, 无缓存失效, 简单直接; N+1 靠批量 IN 防 |
| 代理 ProxyState | 持主 Db Arc (非独立 open) | 统一 handle 路由, 代理写走 proxy_log_handle (独立锁) 不争元数据 |
| backup | 不改 (非双库) | collect 数据级不碰 proxy_log, 单 handle 仍 OK |
| compact/file_size | 双库 (非单) | 日志库独立 VACUUM/体积 |

## 关键约束 / 不变量

- **跨库无事务** — proxy_log autocommit, 仅 platform_lifecycle 2 处显式事务纯元数据 (grep 确认, research/design-detail-check.md §1)
- **proxy_log 表无 FK 子句** — platform_id/cli_proxy_provider_id 裸 INTEGER, 拆库无跨库 FK 损失 (schema_early.rs:76-103)
- **command 签名不变** — 方案 B, State<Db> 全仓 151 处零改
- **代理 ProxyState 改持 Arc<Db>** — 不再 proxy.rs:32 独立 open, 统一 handle
- **N+1 防护** — 跨表读全批量 IN, 禁 per-row
- **migration 幂等** — 各 handle 独立 `let _ =` 幂等
- **WAL 独立** — 两 .db 各 -wal/-shm, 各自 busy_timeout/checkpoint; 备份覆盖 6 文件

## 技术选型

- Db struct 加 `proxy_log_handle: Arc<Mutex<AsyncConnection>>` + `proxy_log_read_pool: ReadPoolHandle`
- 新方法 `call_proxy_log_traced` / `call_read_proxy_log_traced` (镜像 call_traced, 走 proxy_log_handle)
- `Db::new` 建 proxy_log.db (PRAGMA WAL/synchronous=NORMAL/busy_timeout/auto_vacuum 全套, 同主库 idiom)
- migration runner 分流: proxy_log/stats_agg 表归 proxy_log_handle, 余归 main_handle
- 代理 ProxyState.db 字段改 `Arc<Db>` (proxy/mod.rs:134), proxy.rs:32 删独立 Db::new
- 跨表读改应用层: 各访问点 (list_request_logs/today_platform_stats/query_stats/usage_stats/aggregate_proxy_logs/backfill) 按 id set 批量查主库 + Rust 合并
- compact_database/db_file_size 双库; migrate_auto_vacuum 覆盖 proxy_log.db

## subtask 拆分 (初拟, 落 task.json)

- s1 Db struct 内嵌 proxy_log handle (Db::new 建 proxy_log.db + WAL/PRAGMA/读池 + call_proxy_log_traced + ReconnectCtx) — 基础设施
- s2 migration 分流 (runner 按表归属选 handle; proxy_log/stats_agg 归 proxy_log; backfill_stats_agg 跨表读改应用层) — deps s1
- s3 db 函数路由 + 代理统一 (~39 站点换 call_proxy_log_traced; ProxyState 改 Arc<Db> 删独立 open) — deps s2
- s4 跨表读应用层合并 (list_request_logs/today_platform_stats/query_stats/usage_stats/aggregate_proxy_logs 按 id set 批量查 + 合并, N+1 防护) — deps s3
- s5 备份/VACUUM 双库 (compact_database/db_file_size/migrate_auto_vacuum/purge_all_soft_deleted/retention 推广两 handle) — deps s3
- s6 启动 + 生命周期 (app_setup 双 handle init/manage + scheduled tasks 双库) — deps s1

依赖 DAG: s1 → s2 → s3 → {s4, s5}; s1 → s6
