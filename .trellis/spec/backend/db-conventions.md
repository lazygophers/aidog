---
updated: 2026-06-11
rewrite-version: 1
authored-by: trellisx-spec
mode: optimize
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
