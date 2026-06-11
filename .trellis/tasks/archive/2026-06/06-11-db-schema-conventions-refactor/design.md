# Design: DB Schema 规范化

## 命名映射（R2 单数）

| 旧表 | 新表 | 备注 |
| --- | --- | --- |
| platforms | `platform` | |
| groups | `"group"` | SQL 保留字，全程双引号转义 |
| group_platforms | `group_platform` | 加代理主键 |
| model_mappings | （删除 R4） | 迁入 `"group"`.model_mappings JSON |
| proxy_logs | `proxy_log` | 主键 uuid(无连字符) |
| settings | `setting` | 加代理主键 |

## 主键规则（R7 / R8）

| 表 | 主键 | 类型 | Rust | 前端 |
| --- | --- | --- | --- | --- |
| platform / "group" / group_platform / setting | `id INTEGER PRIMARY KEY AUTOINCREMENT` | uint64 自增 | `u64` | `number` |
| proxy_log | `id TEXT`（uuid 去 `-`，hex32 小写） | 字符串 | `String` | `string` |

- proxy_log id 生成：`uuid::Uuid::new_v4().simple().to_string()`（simple = 无连字符 32 hex）

## 时间字段规则（R1 / R9 / R10）

- 每表必含 `created_at` / `updated_at` / `deleted_at`，类型 `INTEGER NOT NULL DEFAULT 0`
- 单位：**毫秒级 Unix 时间戳**（`chrono::Utc::now().timestamp_millis()`，i64）
- `deleted_at = 0` 表示未删除；`> 0` 为软删除时刻
- Rust 模型字段类型 `i64`；前端 `number`
- 所有查询默认追加 `WHERE deleted_at = 0` 过滤软删除行

## 默认值规则（R10 禁 NULL）

- 所有 `TEXT` 列：`NOT NULL DEFAULT ''`
- 所有 `INTEGER` 列：`NOT NULL DEFAULT 0`
- 取消现有 nullable 列：`platform.extra`、`"group".auto_from_platform` → `NOT NULL DEFAULT ''`
- Rust 模型对应 `Option<String>` → `String`（空串代替 None），`Option<u64>` 同理

## protocol → platform_type（R3）

- `platform.protocol` 列 → `platform_type`
- Rust：`Platform.protocol: Protocol` → `platform_type: Protocol`（serde rename 同步）
- 前端：`Platform.protocol` → `platformType`（注意：仅平台主类型列改名；endpoint.protocol / group.source_protocol / proxy_log 协议字段**不改**，它们语义是协议非平台类型）

## model_mappings 内联（R4 + D4）

- `"group"` 表加列 `model_mappings TEXT NOT NULL DEFAULT '[]'`
- 存 JSON 数组：`[{source_model, target_platform_id, target_model, request_timeout_secs, connect_timeout_secs}]`
  - 注意：内联后 `target_platform_id` 为 u64（指向 platform.id）
- 删除：model_mappings 表 / ModelMapping CRUD / mapping_* commands / mappingApi
- 保留：`ModelMapping` struct（去 `id`/`group_id` 字段，作为 JSON 元素类型）
- router.rs：从 `group.model_mappings` 读取，而非 `db::list_model_mappings`
- 前端 Groups.tsx：mapping 编辑改为操作 group.model_mappings 数组（随 group update 保存）

## 新 Schema DDL（migrations/001_init.sql 全量重写）

```sql
PRAGMA journal_mode=WAL;
PRAGMA foreign_keys=ON;

CREATE TABLE IF NOT EXISTS platform (
    id               INTEGER PRIMARY KEY AUTOINCREMENT,
    name             TEXT NOT NULL DEFAULT '',
    platform_type    TEXT NOT NULL DEFAULT '',
    base_url         TEXT NOT NULL DEFAULT '',
    api_key          TEXT NOT NULL DEFAULT '',
    extra            TEXT NOT NULL DEFAULT '',
    models           TEXT NOT NULL DEFAULT '{}',
    available_models TEXT NOT NULL DEFAULT '[]',
    endpoints        TEXT NOT NULL DEFAULT '[]',
    enabled          INTEGER NOT NULL DEFAULT 1,
    created_at       INTEGER NOT NULL DEFAULT 0,
    updated_at       INTEGER NOT NULL DEFAULT 0,
    deleted_at       INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS "group" (
    id                   INTEGER PRIMARY KEY AUTOINCREMENT,
    name                 TEXT NOT NULL DEFAULT '',
    path                 TEXT NOT NULL DEFAULT '',
    routing_mode         TEXT NOT NULL DEFAULT '',
    auto_from_platform   TEXT NOT NULL DEFAULT '',
    source_protocol      TEXT NOT NULL DEFAULT 'anthropic',
    model_mappings       TEXT NOT NULL DEFAULT '[]',
    request_timeout_secs INTEGER NOT NULL DEFAULT 0,
    connect_timeout_secs INTEGER NOT NULL DEFAULT 0,
    created_at           INTEGER NOT NULL DEFAULT 0,
    updated_at           INTEGER NOT NULL DEFAULT 0,
    deleted_at           INTEGER NOT NULL DEFAULT 0,
    UNIQUE(path)
);

CREATE TABLE IF NOT EXISTS group_platform (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    group_id    INTEGER NOT NULL DEFAULT 0,
    platform_id INTEGER NOT NULL DEFAULT 0,
    priority    INTEGER NOT NULL DEFAULT 0,
    weight      INTEGER NOT NULL DEFAULT 1,
    created_at  INTEGER NOT NULL DEFAULT 0,
    updated_at  INTEGER NOT NULL DEFAULT 0,
    deleted_at  INTEGER NOT NULL DEFAULT 0,
    UNIQUE(group_id, platform_id)
);

CREATE TABLE IF NOT EXISTS setting (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    scope      TEXT NOT NULL DEFAULT '',
    key        TEXT NOT NULL DEFAULT '',
    value      TEXT NOT NULL DEFAULT '{}',
    created_at INTEGER NOT NULL DEFAULT 0,
    updated_at INTEGER NOT NULL DEFAULT 0,
    deleted_at INTEGER NOT NULL DEFAULT 0,
    UNIQUE(scope, key)
);

CREATE TABLE IF NOT EXISTS proxy_log (
    id                        TEXT PRIMARY KEY,
    group_name                TEXT NOT NULL DEFAULT '',
    model                     TEXT NOT NULL DEFAULT '',
    actual_model              TEXT NOT NULL DEFAULT '',
    source_protocol           TEXT NOT NULL DEFAULT '',
    target_protocol           TEXT NOT NULL DEFAULT '',
    platform_id               INTEGER NOT NULL DEFAULT 0,
    request_headers           TEXT NOT NULL DEFAULT '{}',
    request_body              TEXT NOT NULL DEFAULT '',
    upstream_request_headers  TEXT NOT NULL DEFAULT '',
    upstream_request_body     TEXT NOT NULL DEFAULT '',
    response_body             TEXT NOT NULL DEFAULT '',
    request_url               TEXT NOT NULL DEFAULT '',
    upstream_request_url      TEXT NOT NULL DEFAULT '',
    upstream_response_headers TEXT NOT NULL DEFAULT '',
    upstream_status_code      INTEGER NOT NULL DEFAULT 0,
    user_response_headers     TEXT NOT NULL DEFAULT '',
    user_response_body        TEXT NOT NULL DEFAULT '',
    status_code               INTEGER NOT NULL DEFAULT 0,
    duration_ms               INTEGER NOT NULL DEFAULT 0,
    input_tokens              INTEGER NOT NULL DEFAULT 0,
    output_tokens             INTEGER NOT NULL DEFAULT 0,
    cache_tokens              INTEGER NOT NULL DEFAULT 0,
    created_at                INTEGER NOT NULL DEFAULT 0,
    updated_at                INTEGER NOT NULL DEFAULT 0,
    deleted_at                INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX IF NOT EXISTS idx_proxy_log_group ON proxy_log(group_name);
CREATE INDEX IF NOT EXISTS idx_proxy_log_created ON proxy_log(created_at);
```

- 注意：因主键/类型彻底变更，废弃旧 001_init.sql 的增量 ALTER 数组（migration 历史压平为单一 init）。新库直接建新 schema；旧库由独立迁移脚本转换。

## 独立迁移脚本（R5 / D2）

- 位置：`scripts/migrate_db_v2.py`（项目根 scripts/，**不进 app 代码**），任务完成后删除
- 输入：`~/.aidog/aidog.db`（先备份 `aidog.db.bak`）
- 步骤：
  1. 建新表（新 schema，临时库或同库新表名）
  2. 旧 uuid → 新自增 id：建 `old_id(TEXT) → new_id(INTEGER)` 映射表（platform/group）
  3. 搬数据：platform（protocol→platform_type, extra NULL→''）/ group（auto_from_platform NULL→'', model_mappings 从旧 model_mappings 表聚合为 JSON, 外键 id 重映射）/ group_platform（id 重映射）/ setting / proxy_log（platform_id uuid→新 int, id uuid 去 `-`）
  4. 时间：`created_at`/`updated_at` 字符串 → `timestamp_millis`（宽松解析 `%Y-%m-%d %H:%M:%S` 与 rfc3339）；`deleted_at`=0
  5. drop 旧表 → rename 新表 / 或 ATTACH 新库替换
  6. 校验行数一致 → 输出报告
- 当前数据量：platform 2 / group 2 / group_platform 2 / model_mapping 0 / proxy_log 9 / setting 3（小，单次事务）

## 验证（R6）

- `cargo build` 退出码 0；`cargo test` 退出码 0；`tsc --noEmit` 退出码 0
- 新 schema 无 NULL 列：`sqlite3 aidog.db "SELECT ... WHERE col IS NULL"` 0 行
- 软删除过滤：list 查询返回不含 deleted_at>0 行
- 迁移后行数与备份一致
