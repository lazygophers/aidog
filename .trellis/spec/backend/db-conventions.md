---
updated: 2026-07-10
rewrite-version: 2
supersedes:
  - db-conventions.md (v1, 无「专属表 → setting 迁移模式」段)
authored-by: trellisx-spec
mode: sediment
---

# DB Conventions

何时被读: 新增 / 修改任何数据库表、字段、模型、CRUD、迁移时
谁读: trellis-implement sub-agent / main
不遵守的代价: schema 漂移 / 前后端契约断裂 / 数据迁移失败 / NULL 引发运行时 panic

---

## Table Naming (MUST)

- 表名必须**单数**，禁复数：`platform` / `group` / `group_platform` / `setting` / `proxy_log`
- SQL 保留字表名（如 `group`）必须全程双引号转义 `"group"`
- 验证: `sqlite3 ~/.aidog/aidog.db ".tables"` 不得出现复数表名

## Primary Key (MUST)

- 业务表主键必须 `id INTEGER PRIMARY KEY AUTOINCREMENT`，Rust 映射 `u64`，前端 `number`
- 例外：`proxy_log` 主键为 `TEXT`，值为 **无连字符** uuid（`uuid::Uuid::new_v4().simple()`，32 hex）
- uuid 一律禁 `-` 连接符
- INSERT 业务表禁写 id 列；插入后用 `conn.last_insert_rowid()` 取回
- 验证: `grep -rn "Uuid::new_v4().to_string()" src-tauri/src` 必须 0 行（须用 `.simple()`）

## Time Fields (MUST)

- 每个表必须含 `created_at` / `updated_at` / `deleted_at`，类型 `INTEGER NOT NULL DEFAULT 0`
- 时间值必须为**毫秒级 Unix 时间戳**（`chrono::Utc::now().timestamp_millis()`，Rust `i64`，前端 `number`）
- 禁用 ISO 字符串 / rfc3339 存时间列
- 时间比较 / retention cutoff 必须整数比较；strftime 聚合必须 `strftime(fmt, created_at/1000, 'unixepoch')`
- 验证: `grep -rn "to_rfc3339\|%Y-%m-%dT" src-tauri/src/gateway/db.rs` 必须 0 行

## Soft Delete (MUST)

- 删除必须逻辑删：`UPDATE <table> SET deleted_at = <now_ms> WHERE id = ?`，禁物理 `DELETE`
- 所有 list / get 查询必须追加 `WHERE deleted_at = 0`（与其他条件 AND）
- `deleted_at = 0` 表示未删除，`> 0` 为删除时刻毫秒戳

## No NULL (MUST)

- 所有 `TEXT` 列 `NOT NULL DEFAULT ''`；所有 `INTEGER` 列 `NOT NULL DEFAULT 0`
- 禁可空列；Rust 模型禁 `Option<String>` 映射 DB 列（用空串代替 None）
- 验证: 新建表 DDL `grep -c "DEFAULT" ` 覆盖每个非主键列；`SELECT * FROM <t> WHERE <col> IS NULL` 必须 0 行

## Column Naming (MUST)

- 平台主类型列名为 `platform_type`（禁 `protocol`）；其值用 `serde_json::to_string` 序列化（带引号 JSON 字面量），读用 `serde_json::from_str`
- 协议语义字段（endpoint.protocol / group.source_protocol / proxy_log 的 source/target_protocol）保留 `protocol` 命名，不与平台类型混用

## Relations & Mappings (MUST)

- 关联表（如 `group_platform`）加代理 `id` 自增主键 + 保留业务复合 `UNIQUE(group_id, platform_id)`
- KV 表（`setting`）加代理 `id` + `UNIQUE(scope, key)`
- 模型映射禁独立表，必须内联为 `"group".model_mappings TEXT NOT NULL DEFAULT '[]'`（JSON 数组：`{source_model, target_platform_id, target_model, request_timeout_secs, connect_timeout_secs}`）

## Migration (MUST)

- schema 破坏式变更必须提供独立一次性迁移脚本（`scripts/`，非 app 运行时代码），迁移完成后删除
- 迁移脚本必须先备份 `aidog.db.bak` + 迁移后行数校验 + 幂等（可重跑）
- `init_tables()` 仅执行 `migrations/001_init.sql`，禁堆叠历史 ALTER / 数据修复（这些归迁移脚本）

## 专属表 → setting 迁移模式 (MUST)

域数据从专属表迁通用 `setting` 表时（`scope=<域>, key=<实体>` JSON），走 app 内置编号 migration 序列（`schema_early.rs` / `schema_late.rs`），**单 migration 入口**完成三步，禁拆多 migration：

- **单 migration 原子**：① 旧表数据 JSON 化写 `setting`（`INSERT OR IGNORE` 幂等）② `DROP TABLE IF EXISTS <旧表>` ③ 新库 seed 守卫（`need_seed = NOT EXISTS(setting WHERE scope=<域> AND key=<实体>)`，仅新库或未 seed 时执行）
- **禁** CREATE 在 migration N / seed 在 N+1 / DROP 在 N+2 散布多 migration —— 中间态库（N 应用但 N+2 未跑）查无表，迁移逻辑断裂
- **禁** 留 `CREATE TABLE <旧表>` 旧 schema 残留：迁移并入后 `grep -rn "CREATE TABLE.*<旧表名>" src-tauri/src/gateway/db` 必须 0（含 `schema_early.rs` / `schema_late.rs` / 历史 migration）
- **禁** 留旧 `seed_<entity>_if_empty` 运行时函数 —— seed 并入 migration 单源（运行时函数与 migration 双源会漂移）
- **幂等**：重跑 migration 零副作用（`INSERT OR IGNORE` + `DROP IF EXISTS` + `need_seed` 守卫三重保障）

**验收断言（双路径 + 幂等，可复用为测试）**：
1. 旧库 fixture（含旧表行）经 migration → `setting` JSON 含迁移数据 + 旧表 `table_exists=0`
2. 新库（无旧表）经 migration → `setting` 含 seed 默认值
3. 重跑 migration → 行数 / JSON 零变化（幂等）

实例：task 07-09-mitm-tables-to-setting migration 043 `migrate_mitm_legacy_tables_to_setting`（删旧 040/041/042 + `seed_default_whitelist_if_empty`，并入单入口；测试 `migrations_late_043_legacy_tables_to_setting` + `migrations_late_mitm_seed_to_setting_043`）

## Verification

```bash
# 复数表名残留
sqlite3 ~/.aidog/aidog.db ".tables" | grep -E '\b(platforms|groups|settings|proxy_logs|model_mappings)\b'  # 必须 0

# 时间列整数
sqlite3 ~/.aidog/aidog.db "SELECT typeof(created_at) FROM platform LIMIT 1"  # 必须 integer

# 无 NULL（逐表逐列）
sqlite3 ~/.aidog/aidog.db "SELECT COUNT(*) FROM platform WHERE extra IS NULL"  # 必须 0

# model_mappings 表已删
sqlite3 ~/.aidog/aidog.db "SELECT name FROM sqlite_master WHERE name='model_mappings'"  # 必须空
```
