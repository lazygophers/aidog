# 每日清理全表软删数据 (deleted_at>0 且 >3d)

## Goal

把现有每日定时清理（`app_setup.rs:183` spawn，目前仅 `purge_old_soft_deleted_platforms` 单表）**泛化为所有含 `deleted_at` 列的表**：新增表驱动统一函数 `purge_all_soft_deleted(db, older_than_secs)`，逐表独立 `DELETE WHERE deleted_at > 0 AND deleted_at < ?`，3 天阈值，返回 per-table 删除计数。

**为什么**：用户要求"每天清理 deleted_at > 0 且三天前的所有数据（所有的表）"。现状仅 platform 表软删 tombstone 被定期清理，其余 8 表（group / group_platform / setting / proxy_log / model_price / middleware_rule / notification / mcp_server）的软删行永久残留，数据库膨胀。

## 现状（已实现）

- `src-tauri/src/app_setup.rs:183-215`：每日 spawn（24h 间隔 + 启动首跑），调 `purge_old_soft_deleted_platforms(platform, 3d)` + `cleanup_notifications` + `cleanup_stats_agg`。
- `src-tauri/src/gateway/db/platform_lifecycle.rs:203`：`purge_old_soft_deleted_platforms`（platform 单表，`DELETE WHERE deleted_at > 0 AND deleted_at < ?`）。
- `src-tauri/src/gateway/db/proxy_log.rs:515`：`purge_deleted_proxy_logs`（无阈值全删软删 tombstone，**语义不同**，保留独立）。
- 9 表有 `deleted_at INTEGER NOT NULL DEFAULT 0`：platform / "group" / group_platform / setting / proxy_log / model_price / middleware_rule / notification / mcp_server。

## Requirements

### R1 新增 `purge_all_soft_deleted`

- R1.1 在 `src-tauri/src/gateway/db/maintenance.rs` 新增 `pub fn purge_all_soft_deleted(db: &Db, older_than_secs: i64) -> impl Future<Output = Result<std::collections::HashMap<String, u64>, String>>`。
- R1.2 表清单 const（含 SQL 标识符引号处理）：
  ```
  const SOFT_DELETE_TABLES: &[&str] = &[
      "platform", "\"group\"", "group_platform", "setting",
      "proxy_log", "model_price", "middleware_rule",
      "notification", "mcp_server",
  ];
  ```
  实施时**逐表核 schema**（`PRAGMA table_info(<table>)` 或 grep schema_early.rs CREATE TABLE）确认每表确有 `deleted_at` 列；缺列的表跳过 + warn（不报错，防 schema 漂移炸全流程）。
- R1.3 每表独立 `DELETE FROM <table> WHERE deleted_at > 0 AND deleted_at < <cutoff>`，cutoff = `now() - older_than_secs`。
- R1.4 逐表独立事务（失败不影响他表）：单表 DELETE 失败 → `tracing::warn!(table, error, ...)` + 该表记 0 + 继续。函数整体仅在所有表都失败时返 Err（罕见，保留 Err 类型满足 Result 语义）；部分成功返 Ok(map)。
- R1.5 返回 `HashMap<String, u64>`：key = 表名（不含引号），value = 删除行数。app_setup 日志记每表删除数。
- R1.6 `deleted_at` 是 Unix 秒（INTEGER），cutoff 用 `chrono::Utc::now().timestamp() - older_than_secs`（复用 `super::now()` 或既有时间 helper，grep 确认 db 模块内 now()）。

### R2 app_setup 接入

- R2.1 `src-tauri/src/app_setup.rs:201` 附近：把 `purge_old_soft_deleted_platforms(&db, older_than_secs)` 调用**替换**为 `purge_all_soft_deleted(&db, older_than_secs)`。
- R2.2 日志：`tracing::info!(purged = ?map, "scheduled: purged old soft-deleted rows across all tables")`（map 含 per-table 计数）；空 map 或全 0 不 log info（debug 即可）。
- R2.3 `cleanup_notifications` + `cleanup_stats_agg` 调用**保留**（它们是 created_at retention 语义，不与 deleted_at purge 冲突；notification 表会被 purge_all_soft_deleted 也清软删行，但 cleanup_notifications 的 retention 仍按 created_at 独立运作——两者互补，不冲突）。
- R2.4 `older_than_secs` 仍为 `3 * 24 * 3600`（3 天，复用现有 const 表达式）。

### R3 保留 / 移除旧函数

- R3.1 `purge_old_soft_deleted_platforms` **保留**（platform 单表快路径，且 `test_platform_lifecycle.rs:457/475` 有专项测试，移除破坏测试）。app_setup 不再调用它，但函数留作可复用 primitive（或被 purge_all_soft_deleted 内部表清单逻辑自然覆盖——若 implement 判断完全冗余则移除 + 删测试，二选一，implement 判）。
- R3.2 `purge_deleted_proxy_logs` 保留（语义不同：无阈值全删，与 3d 阈值 purge 区分）。

### R4 测试

- R4.1 `maintenance.rs` 或 `test_maintenance.rs` 新增 `purge_all_soft_deleted` 测试（沿用 `test_support::test_db` 既有模式）：
  - `purges_old_soft_deleted_across_tables`：插入多表软删行（deleted_at = now - 4d）+ 未软删行（deleted_at=0）+ 近期软删行（deleted_at = now - 1d），跑 purge，断言旧软删行删、未软删保留、近期软删保留。
  - `skips_table_missing_deleted_at_column`：若清单内某表无 deleted_at 列（schema 漂移），跳过不炸（用临时表或 mock 验证容错路径——若难构造可降级为单元逻辑测试）。
  - `returns_per_table_count`：返回 map 含每表删除数。
- R4.2 不破坏现有 `test_platform_lifecycle.rs::purge_old_soft_deleted_*` 测试（若旧函数保留）。

### R5 门禁

- R5.1 `cargo test`（含新测 + 现有 maintenance/platform_lifecycle 测试）全过。
- R5.2 `cargo clippy` 无新 warning。
- R5.3 主仓零改动（改动仅 worktree）。

## Acceptance Criteria

- [ ] `purge_all_soft_deleted` 实现 + 表驱动清单（9 表，逐表核 deleted_at 列存在）
- [ ] 逐表独立 DELETE，失败容错（warn + 继续，不炸全流程）
- [ ] 返回 HashMap<表名, 删除数>
- [ ] app_setup.rs 调用替换为统一函数，日志含 per-table 计数
- [ ] R4 全部测试通过（cargo test maintenance + platform_lifecycle）
- [ ] cargo clippy 无新 warning
- [ ] 主仓零改动

## Definition of Done

- 表驱动统一 purge 函数 + 容错
- app_setup 接入 + 日志
- 单测覆盖跨表 purge + 容错
- journal 记录表清单来源（schema_early grep 证据）+ 保留/移除旧函数决策

## Technical Approach

```
maintenance.rs::purge_all_soft_deleted(db, older_than_secs)
  ├─ cutoff = now() - older_than_secs
  ├─ let mut map = HashMap::new()
  ├─ for table in SOFT_DELETE_TABLES:
  │   ├─ DELETE FROM <table> WHERE deleted_at > 0 AND deleted_at < ?cutoff
  │   ├─ Ok(n) → map.insert(table.trim_quotes(), n)
  │   └─ Err(e) → warn!(table, error) + skip（不插 map 或插 0）
  └─ Ok(map)

app_setup.rs scheduled_cleanup 周期:
  ├─ purge_all_soft_deleted(&db, 3d)   ← 替换原 platform 单表
  ├─ cleanup_notifications(...)        ← 保留 (created_at retention)
  └─ cleanup_stats_agg(...)            ← 保留 (created_at retention)
```

表名引号：SQLite 标识符 `"group"` 是保留字，DELETE FROM "group" 合法；map key 用 `group`（去引号）便于日志可读。

## Decision (ADR-lite)

**Context**：用户要全表 purge；现有仅 platform。
**Decision**：
1. 表驱动统一函数（清单 + 循环），非 per-table fn（避免重复）。
2. 逐表独立事务（容错优先，一表失败不阻塞他表）。
3. 9 表全纳入（用户原话"所有的表"）。
4. 保留旧 `purge_old_soft_deleted_platforms`（测试依赖 + 可作 primitive）。
5. `purge_deleted_proxy_logs` 保留独立（无阈值语义不同）。
**Consequences**：
- setting 表软删配置 3d 后硬删 → 用户误删配置 3d 内可恢复（软删行 setting 查询 WHERE deleted_at=0 已过滤，但行还在 DB），3d 后彻底删。可接受。
- model_price 旧价软删 3d 清理（与 price_sync 下次同步补新价互补）。
- notification 双清理（deleted_at purge + created_at retention）互补不冲突。

## Out of Scope

- per-table 可配阈值（统一 3d）
- 用户可见 UI（保留天数 setting 暂不加，硬编码 3d）
- VACUUM / incremental_vacuum 回收（现有 maintenance.rs 已有独立机制）
- stats_agg_hourly（无 deleted_at 列，走 cleanup_stats_agg retention）

## Technical Notes

- 表清单来源：`grep -n "CREATE TABLE" src-tauri/src/gateway/db/schema_early.rs` + 逐表 `deleted_at` 列存在（line 14/36/52/64/76/128/209/247/272）。
- 软删触发点证据：group_platform.rs:11 / platform_lifecycle.rs:37 / settings.rs:83 等均 `UPDATE ... SET deleted_at = ?`。
- 时间 helper：db 模块 `super::now()`（grep 确认，platform_lifecycle.rs 用 `now()`）。
- 既有 guide：`.trellis/spec/backend/db-conventions.md`（DB 操作规范）+ `.trellis/spec/guides/code-reuse-rules.md`（grep 既有 now() / 时间 helper）。
