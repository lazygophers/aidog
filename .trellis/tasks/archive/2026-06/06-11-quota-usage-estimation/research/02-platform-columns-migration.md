# Research: platform 加列方案 + migration

- **Query**: platform 表现有列 + 需加哪些列 + migration 编号 + 同步改动点
- **Scope**: internal
- **Date**: 2026-06-11

## 现状

### platform 表当前列（migrations/001_init.sql:6-21）
```
id, name, platform_type, base_url, api_key, extra,
models, available_models, endpoints, enabled,
created_at, updated_at, deleted_at
```

### 代码侧列序常量（db.rs:74-79）
- `PLATFORM_COLUMNS`（db.rs:74）：`id, name, platform_type, base_url, api_key, extra, models, available_models, endpoints, enabled, created_at, updated_at`（**不含 deleted_at**，row_to_platform 硬编码 `deleted_at: 0`，见 db.rs:100）
- `PLATFORM_COLUMNS_PREFIXED`（db.rs:78）：同上加 `p.` 前缀（JOIN 用，db.rs:403 group 查询）

### Rust Platform struct（models.rs:261-281）
13 字段，与列一一对应（deleted_at `#[serde(default)]`）。

## 建议新增列（遵 db-conventions：NOT NULL DEFAULT，ms 时间）

| 列名 | 类型 | DEFAULT | 用途 |
|---|---|---|---|
| `est_balance_remaining` | REAL | `0` | 预估余额（按量平台） |
| `est_coding_plan` | TEXT | `'{}'` | 预估 coding plan JSON（tiers + 基数/拟合系数） |
| `last_real_query_at` | INTEGER | `0` | 上次真实 quotaApi.query 毫秒戳（校准判定） |
| `estimate_count` | INTEGER | `0` | 自上次真查累计预估次数（校准判定 >100） |

> 注：db-conventions「No NULL」要求 REAL/INTEGER 也 NOT NULL DEFAULT 0。`est_coding_plan` 用 TEXT JSON DEFAULT '{}'（与 models/endpoints 同模式）。
> 可选：`est_balance_valid INTEGER DEFAULT 0` 标记预估值是否有效（首次真查前无基数）。设计可决定是否需要。

## Migration 编号

**下一个可用编号 = 004**（现有 001_init / 002_log_filter_indexes / 003_model_price，见 `migrations/` 目录）。
- 新建 `migrations/004_platform_quota_estimate.sql`，内容用 `ALTER TABLE platform ADD COLUMN ...`（SQLite ADD COLUMN 幂等性差，需注意重跑——见下方冲突点）。
- **注册点**：db.rs:46-55 `init_tables()` 当前 `execute_batch` 顺序加载 001/002/003（include_str!）。需追加 `include_str!("../../migrations/004_...")`（db.rs:53 后）。

### 规范冲突（设计必须裁决）
- **db-conventions.md:61-65「Migration」明确写**：`init_tables()` 仅执行 001_init.sql，schema 破坏式变更走独立一次性 `scripts/` 脚本（带备份 + 行数校验 + 幂等），迁移后删除。
- **但实际代码**（db.rs:48-53）已堆叠 002/003 到 `init_tables()` 经 `CREATE ... IF NOT EXISTS` 实现幂等。002/003 是别窗口已 commit 的既成事实，与 spec 冲突。
- **`ALTER TABLE ADD COLUMN` 无 `IF NOT EXISTS`**（旧 SQLite 不支持），重跑 init_tables 会报 "duplicate column"。两条路：
  - (a) 遵现有 002/003 模式塞进 init_tables，但 ALTER 需先 `PRAGMA table_info` 检测列存在再 ALTER（或忽略 duplicate column 错误）；
  - (b) 遵 spec 走 scripts/ 一次性迁移。
- `需要: design 决定走 init_tables include_str 模式（与既有 002/003 一致）还是 scripts/ 一次性迁移（与 spec 一致）`。

## 同步改动点清单（file:line）

1. **migrations/004_*.sql** — 新建（ALTER ADD COLUMN ×4）
2. **db.rs:46-55** `init_tables()` — 追加 004 include_str!（若走 init 模式）
3. **db.rs:74** `PLATFORM_COLUMNS` — 追加新列名
4. **db.rs:78** `PLATFORM_COLUMNS_PREFIXED` — 追加 `p.` 前缀新列
5. **db.rs:82-102** `row_to_platform` — 新增 `row.get(N)` 读取（注意索引：现有列到 11=updated_at，新列从 12 起；deleted_at 当前未在 SELECT 列里）
6. **db.rs:104-143** `create_platform` — INSERT 默认值（新平台 est_* 初始 0/'{}'，可不在 INSERT 显式写，靠 DEFAULT）
7. **db.rs:169-210** `update_platform` — 是否纳入 UPDATE？**注意**：现有 update_platform 全量覆盖式 UPDATE（db.rs:191-206），若不加 est_* 列则用户编辑平台**不会清空**预估值（因为没 SET）；但若把 est_* 也放进全量 UPDATE 会被 UpdatePlatform 默认值覆盖 → **预估写入应走独立 UPDATE 语句**（如 `update_platform_estimate`），不混入通用 update_platform。
8. **models.rs:261-281** `Platform` struct — 新增字段（前端要读则需 serde）
9. **db.rs:403** group JOIN 查询用 PLATFORM_COLUMNS_PREFIXED，row_to_platform 复用——改 row_to_platform 索引时此处自动生效，但需确认 group 查询的列偏移（PLATFORM_COLUMNS_PREFIXED 前还有 `gp.priority, gp.weight` 两列，row_to_platform 是独立调用还是同一 row？需读 db.rs:403 附近确认偏移）。

### 关键架构建议
- 预估写入用**独立窄 UPDATE**（只 SET est_* + last_real_query_at + estimate_count），避免与全量 update_platform 互相覆盖、避免高频预估触发全字段写。

## Caveats
- `row_to_platform` 当前不读 deleted_at（硬编码 0）。新列索引从 12 开始（updated_at=11）。
- group JOIN 查询（db.rs:403）列偏移需设计时复核。
