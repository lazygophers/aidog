---
name: aidog-db-migration
description: |
  给 aidog SQLite 数据库加 migration（schema 变更：加表/加列/改约束/拆列/重命名）。固化项目已验证范式：前向幂等、NNN_ 编号、SQLite 无 ALTER 限制的 rebuild-table 范式、**无 down/rollback**（SQLite 前向单线）、加列必带 DEFAULT、UNIQUE 变更走 rebuild 表、列改名禁用 sed 批改 SQL 源列名（历史误伤）。覆盖编号/写 SQL/幂等保护/启动自动执行/测试。触发词：加 migration、数据库迁移、加列、加表、改约束、UNIQUE 冲突、group_key 拆分、schema 变更、migration 011、rebuild table。
when_to_use: 需要改 aidog SQLite schema（加表/加列/改类型/加 UNIQUE 约束/拆列重命名）；现有 migrations/ 最新是 010 要加 011；遇到 UNIQUE 冲突要改约束；列改名/拆分（如 group_key 那类大改）
disable-model-invocation: true
paths:
  - src-tauri/migrations/**
  - src-tauri/src/gateway/db.rs
---

# aidog DB Migration

给 aidog SQLite 加 schema migration。本 skill 给**编号 / 写法 / 幂等 / 执行 / 测试**全流程，并把历史踩坑（sed 误伤列名、UNIQUE 软删冲突、rebuild 表范式）前置。

> 行号漂移，定位以**文件名 / 符号名**为准。

---

## 0. 认知纠偏（动手前必读）

1. **前向单线，无 down/rollback。** migrations/ 现有 001-010 **无任何 down/rollback 标记**。SQLite 在 embedded app 里走启动时自动按序执行未应用的，不支持回滚。新 migration **只写 up，禁写 down**。

2. **SQLite 改不了现存表的列约束 → rebuild 表。** SQLite `ALTER TABLE` 只支持 RENAME TABLE / ADD COLUMN / DROP COLUMN(新版)，**不能给现存表加 UNIQUE / 改列类型 / 删约束**。要加 UNIQUE（如 010 group_key）→ **建 `_new` 表 → 拷数据 → DROP 旧 → RENAME**（范式见 `009_drop_group_path.sql` / `010_group_key.sql`）。

3. **加列必带 `DEFAULT`。** 现有表加 `NOT NULL` 列 → 必须给 DEFAULT，否则老行插不进。所有现存 migration 加列都带 DEFAULT（`''` / `0` / `'[]'`）。

4. **列改名/拆分禁用 `sed` 批改 SQL 源列名。** memory `group-name-group-key-split` 踩坑：用 sed 批改 SQL 误伤源列名（同名字符串在别处）。列改名在 migration SQL 里显式写 rebuild + 列映射，**不动旧 migration 文件**，新列在代码层（db.rs / api.ts / models.rs）同步改名。

5. **UNIQUE 冲突先清软删行。** memory `move_group_platform` 踩坑：给现存表加 UNIQUE 遇历史 `deleted_at IS NOT NULL` 软删行撞约束 → rebuild 表前先在 INSERT SELECT 里过滤或合并软删行，别让历史软删数据撞新约束。

---

## 1. 编号 + 文件

```
src-tauri/migrations/NNN_short_snake_name.sql
```

- NNN = 现有最大 +1（当前 010 → 新建 011）。**三位补零**。
- 文件名 snake_case，语义短（`011_add_platform_manual_budget.sql`）。
- 启动时 `db.rs` 的 migration runner 按文件名排序顺序执行未应用项。**禁跳号、禁改已发布 migration 的文件名**（已发布 = 已在某些用户的库里记录过版本号）。

> ⚠️ 已发布 migration **禁改内容**（用户库已记录该版本号，改了不重跑 → 库与代码不一致）。要修只能加新 migration 补。

---

## 2. 写法范式（按变更类型选）

### A. 简单加列（加 DEFAULT）— 仿 `005_proxy_log_est_cost.sql`

```sql
-- Migration NNN: <一句话说明>
ALTER TABLE platform ADD COLUMN manual_budget_json TEXT NOT NULL DEFAULT '';
```

### B. 加表 — 仿 `001_init.sql` / `003_model_price.sql`

```sql
CREATE TABLE IF NOT EXISTS foo (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    name        TEXT NOT NULL DEFAULT '',
    created_at  INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX IF NOT EXISTS idx_foo_name ON foo(name);
```

全 `IF NOT EXISTS`（幂等）。列全带 DEFAULT。

### C. 加 UNIQUE / 改列类型 / 拆列 — rebuild 表范式（仿 009 / 010）

```sql
-- Migration NNN: <说明>。SQLite 不能给现存表加 UNIQUE，rebuild 表（仿 010_group_key.sql）。
CREATE TABLE IF NOT EXISTS "foo_new" (
    id    INTEGER PRIMARY KEY AUTOINCREMENT,
    name  TEXT NOT NULL DEFAULT '',
    UNIQUE(name)
);

-- 仅当源表存在目标列时取已存值，否则 DEFAULT 兜底（首次迁移 + 兼容已迁移库）。
INSERT INTO "foo_new" (id, name)
SELECT id, COALESCE(name, '') FROM foo
WHERE deleted_at IS NULL OR name IS NOT NULL;   -- ★ 清撞约束的软删/空行

DROP TABLE foo;
ALTER TABLE foo_new RENAME TO foo;
-- 重建索引（DROP TABLE 带走了）
CREATE INDEX IF NOT EXISTS idx_foo_name ON foo(name);
```

🔴 CHECKPOINT：
- INSERT SELECT **过滤或合并**会撞新 UNIQUE 的历史软删行（见 §0-5）。
- rebuild 后**重建所有原索引**（DROP TABLE 删了）。
- `IF NOT EXISTS` 守 `foo_new`（防中途失败重跑撞表）。

### D. 数据回填 migration（不接 schema，纯 UPDATE）

仍用 NNN 编号，SQL 是 `UPDATE ... WHERE ...`。带幂等守（`WHERE NOT EXISTS` / `WHERE col = ''`）。

---

## 3. 执行机制

- migration runner 在 `db.rs`（init/连接时跑）。按文件名顺序执行未应用项，记录到 schema 版本表。
- **dev 验证**：删 dev 库重跑全套（`rm ~/Library/Application\ Support/aidog/aidog.db*` 后 `yarn tauri dev`），确认全套 migration 从空库跑通。
- **存量验证**：保留旧 dev 库直接 `yarn tauri dev`，确认新 migration 在「已有 010 的库」上增量执行成功（这是真实用户路径）。

---

## 4. 代码层同步（schema 变更必伴随）

加列/拆列后，db.rs 的 CRUD（INSERT/UPDATE/SELECT 列清单）+ Rust struct（models.rs）+ TS 类型（api.ts）+ 前端表单同步。新列在 Rust struct 加字段（带 `#[serde(default)]` 兼容旧 JSON）。

> 拆列 / 改名（如 group_key 那种大改）：db.rs 所有 SQL 引用 + models.rs struct + api.ts 类型 + 前端消费**全链同步**，禁只改 SQL 不改代码。memory `group-name-group-key-split`：proxy_log.group_name RENAME→group_key 的 sed 批改误伤源列名是真坑。

---

## 5. 验证门禁

```bash
# 1. Rust 编译（runner 改了 db.rs）
cd src-tauri && cargo build && cargo clippy      # warning 必须清（memory warnings-are-issues）

# 2. 全套 migration 从空库跑通
rm ~/Library/Application\ Support/aidog/aidog.db*    # ⚠️ 仅 dev 库，禁删用户库
yarn tauri dev   # 看启动日志 migration 执行无错

# 3. 存量库增量执行（保留旧 dev 库）
yarn tauri dev   # 确认 011 在已有 010 的库增量成功

# 4. db 相关测试
cd src-tauri && cargo test                         # db.rs 有 #[test]
```

收尾自检：
- [ ] NNN 编号 = 最大+1，三位补零。
- [ ] 前向单线，**无 down**。
- [ ] 加列带 DEFAULT；加 UNIQUE/改约束走 rebuild 表范式。
- [ ] rebuild 表清撞约束的软删行 + 重建索引。
- [ ] 已发布 migration 文件**未改**。
- [ ] 代码层（db.rs SQL + models.rs + api.ts + 前端）全链同步。
- [ ] 空库重跑 + 存量库增量执行双验证通过。

---

## 反例黑名单（不要做）

1. ❌ 写 down/rollback —— 前向单线，无回滚。
2. ❌ 给现存表直接 `ALTER TABLE ADD UNIQUE` —— SQLite 不支持，rebuild 表。
3. ❌ 加 `NOT NULL` 列不带 DEFAULT —— 老行插不进。
4. ❌ 用 `sed` 批改旧 SQL 源列名做改名 —— 误伤同名字符串，rebuild 表显式映射。
5. ❌ 改已发布 migration 文件内容 —— 用户库已记录版本号不重跑。
6. ❌ rebuild 表忘重建索引 —— DROP TABLE 带走索引。
7. ❌ rebuild 表 INSERT SELECT 不过滤软删行 —— 历史 `deleted_at` 行撞新 UNIQUE。
8. ❌ 只改 SQL 不改 db.rs/models.rs/api.ts —— 全链失配运行时炸。

## 相关

- 现有范式：`src-tauri/migrations/009_drop_group_path.sql`、`010_group_key.sql`（rebuild 表 + UNIQUE）
- 加列范式：`005_proxy_log_est_cost.sql`、`008_model_info_columns.sql`
- migration runner：`src-tauri/src/gateway/db.rs`（init 时按序执行）
- memory：`group-name-group-key-split`、`warnings-are-issues`
