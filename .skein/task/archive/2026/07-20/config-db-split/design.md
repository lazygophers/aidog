# platform 表拆独立平台库 — 详细设计

## 1. 架构：Db 三库三写槽三读池

现状（2 库）：
```
Db(
  Arc<Mutex<AsyncConnection>>,  // 0 主库写槽 (aidog.db)
  Arc<DbCache>,                 // 1
  ReadPoolHandle,               // 2 主库读池
  Arc<ReconnectCtx>,            // 3
  Arc<Mutex<AsyncConnection>>,  // 4 log.db 写槽
  ReadPoolHandle,               // 5 log.db 读池
)
```

目标（+2 元，3 库）：
```
Db(
  ...0..5 同上,
  Arc<Mutex<AsyncConnection>>,  // 6 platform.db 写槽
  ReadPoolHandle,               // 7 platform.db 读池
)
```

`ReconnectCtx` 加 `platform_path: Option<String>`（同 `proxy_log_path` idiom）。

## 2. 新增方法（mirror proxy_log 对）

| 现有 (log.db) | 新增 (platform.db) |
|---|---|
| `open_proxy_log_conn(path)` | `open_platform_conn(path)` |
| `call_proxy_log_traced` | `call_platform_traced` |
| `call_read_proxy_log_traced` | `call_read_platform_traced` |
| `reconnect_proxy_log`（ConnectionClosed 兜底） | `reconnect_platform` |

闭包体 pragma / profile / auto_vacuum / busy_timeout 全套与 `open_proxy_log_conn` 逐字一致（独立 SQLite 文件库无差别）。

## 3. Migration 分流

### 3.1 DDL 搬迁
4 表 CREATE 从 `run_migrations_early` / `run_migrations_late` 抽出，入新函数：
- `run_migrations_platform_early(conn)` ← platform / group / group_platform / setting 外的 early 语句
- `run_migrations_platform_late(conn)` ← cli_proxy_provider (045/048) + platform 后期 ALTER

**注意**：`middleware_rule`(013/015 seed) / `mcp_server`(020) / `model_price`(003) 留主库 `run_migrations_early`/`late` 不动。

### 3.2 数据迁移（read → INSERT OR IGNORE → DROP，crash-safe 改造）
**notification 049 的 read+DROP(Phase 1) → INSERT(Phase 2) 模式对 transient 通知可接受，但 platform/group 是用户路由配置，crash 在 DROP 后 INSERT 前会丢数据 → 改 crash-safe 四阶段**：

`init_tables` 改四阶段：
- **Phase 1（主库 `call_traced`）**：`run_migrations_early` + `run_migrations_late` + **只读** 4 表全部行入 4 Vec（**不 DROP**）。
- **Phase 2（log.db，不变）**：proxy_log migration。
- **Phase 3（新，`call_platform_traced`）**：`run_migrations_platform_early` + `run_migrations_platform_late` + `INSERT OR IGNORE` 4 Vec 回 platform.db（保 id，id PK 冲突跳过）。
- **Phase 4（主库 `call_traced`）**：`DROP TABLE IF EXISTS` × 4（仅 Phase 3 成功后才达此）。

**crash 恢复矩阵**：
| crash 点 | 重启行为 |
|---|---|
| Phase 1 前/中 | main 表在，重读，正常走 |
| Phase 3 前 | main 表在，重读，INSERT OR IGNORE，DROP |
| Phase 3 INSERT 中（部分） | main 表在，重读，OR IGNORE 跳已存补缺，DROP |
| Phase 4 前 | platform 已全量，main 表仍在，重读 → OR IGNORE 全跳 → DROP |
| Phase 4 后 | main 表已无，重读 Vec 空 → 全 no-op |

**幂等关键**：`INSERT OR IGNORE`（保原 id）让 Phase 3 可任意重放无重复；DROP 延后到 Phase 4 保证数据落地 platform.db 后才清源。

### 3.3 migration 046 CPA 跨库修正
现状：`fetch_cpa_platform_ids` 在主库闭包预查 → 传 `run_migrations_proxy_log_late` 清 log.db CPA proxy_log。
拆库后 `platform` 表在 platform.db → 预查改 Phase 3 闭包（platform handle），或 Phase 1 主库已无 platform 表 → 必须挪到 platform.db 闭包内做。
`group_platform` DELETE 同理迁 platform.db。

## 4. 访问点换 handle（58 点）

逐文件改 `call_traced` → `call_platform_traced`，`call_read_traced` → `call_read_platform_traced`：
- `db/platform.rs` / `db/platform_lifecycle.rs` — platform CRUD/状态
- `db/group.rs` / `db/group_platform.rs` — 分组/关联
- `db/cli_proxy.rs` — cli_proxy_provider
- `db/settings.rs:140` — group/platform id→name 映射部分
- `db/query_stats.rs` / `stats_today.rs` — 已是 Rust 内存合并，仅 platform map 读取源换 handle
- `import_export/apply/db_rows.rs` — 导入导出 4 表
- `db/mod.rs::load_auto_from_map` — group 表查询换 handle（调用方 init_tables Phase 改）

**log.db 侧不动**：`proxy_log.rs` / `stats_agg` / `query_stats_inner` 留 `call_proxy_log_traced`。

## 5. 取舍

### 5.1 保 id vs 自增 id（数据迁移）
- **保 id（推荐）**：`INSERT INTO platform SELECT *` 保原 id，log.db.proxy_log.platform_id 引用不失效。
- 自增 id：简单但 platform_id 全错位 → 历史日志平台名解析崩。
- **决定保 id**。

### 5.2 maintenance 按表归属分流
memory `migration-maintenance-by-table-owner`：retention/file_size/vacuum 聚合按表所在 handle 分流。4 表迁 platform.db 后，maintenance 循环按 handle 分组：主库 set/middleware/model_price/mcp + platform.db 4 表 + log.db proxy_log/stats_agg/notification。

### 5.3 backup（已天然覆盖，无需改动）
**s4 勘察修正原错误假设**：backup 非「物理 .db 文件级备份」，而是 scope-driven `.aidogx` payload（`scheduler.rs::run_backup_inner` → `import_export::collect::collect(db, &ALL_SCOPES)` → encrypt → 落 `backups/*.aidogx`）。
- `ALL_SCOPES` 含 `SCOPE_PLATFORM`/`SCOPE_GROUP`/`SCOPE_GROUP_PLATFORM`，其 collect 调 `list_platforms`/`list_groups`/`list_all_group_platform_pairs` —— s3 已将这些查询换 `call_*_platform_traced` → **platform.db 数据已通过 scope 查询纳入备份 payload**。
- 全仓无 `fs::copy` / `DB_FILES` / raw `.db` 物理备份机制（仅 `enforce_db_file_permissions` chmod 0600，非备份）。
- **结论**：契约 8「platform 数据可备份/恢复」意图已满足，无需额外 backup 改动。

## 6. 风险

| 风险 | 缓解 |
|---|---|
| 58 访问点遗漏漏改 → 运行时查主库无表崩 | grep `call_traced` + 4 表名交叉核；cargo test db:: 全量 |
| 数据迁移中途崩溃 → 数据半迁 | 单事务包裹每表 INSERT；Phase 1 DROP 在 read 后，最坏丢已读 Vec（下次启动重试，主库表还在） |
| 保 id 与 platform.db 自增冲突 | INSERT 显式列含 id；AUTOINCREMENT 不复用已存 id，无冲突 |
| 内存库 fallback 三库同物理库 | `is_memory` 短路：platform handle = 主内存 conn clone（同 proxy_log idiom） |
| maintenance 聚合跨库 is_memory 短路漏 | memory `dual-db-aggregate-is-memory-shortcut`：file_size/vacuum 必 is_memory 短路 platform 分支 |
