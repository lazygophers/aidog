---
task: 06-19-db-vacuum-hard-delete-retention
title: DB 体积治理 — 硬删 retention + auto_vacuum 回收
status: planning
created: 2026-06-19
---

# PRD — DB 体积治理（Tier 1 回收 + Tier 2 硬删）

## 背景

aidog SQLite（`gateway::db`）体积**单调增长**，根因二：

1. **软删不硬删**：`cleanup_proxy_logs`（db.rs:2737）注释写「Delete entire log rows older than overall retention」，**实际 `UPDATE proxy_log SET deleted_at=?` 软删 tombstone**，行永不物理删除。retention_days(90d) 名义过期清除，实际只打标记。注释与实现矛盾。
2. **0 空间回收**：`Db::new`（db.rs:86）pragma 只设 `journal_mode=WAL / foreign_keys=ON / busy_timeout=5000 / synchronous=NORMAL`，**无 `auto_vacuum`、无 `VACUUM`**。软删行 + `*_body` 清空（UPDATE SET ''，db.rs:2756/2773）产生的 free pages **永不回收**，DB 文件只增不减。

目标：DB 文件随 retention 真实收缩，长期稳定在小体积。

## 范围（Tier 1 + Tier 2，**不做** Tier 3 body 压缩 / 外存）

- **Tier 2（硬删）**：retention 流程物理删除旧行。
- **Tier 1（回收）**：auto_vacuum=INCREMENTAL + 一次性 VACUUM 迁移 + retention 后 incremental_vacuum + 手动「压缩 DB」入口。

## 设计决策

### D1 — 硬删策略：直接 DELETE（非软删 grace）
`cleanup_proxy_logs` 改 `DELETE FROM proxy_log WHERE created_at < ?cutoff`。理由：
- retention_days 语义 = 过期清除，软删 tombstone 无消费方（无 un-delete UI，所有查询 `WHERE deleted_at=0`，tombstone 是惰性残留）。
- 同时清理**历史 tombstone**：`DELETE FROM proxy_log WHERE deleted_at != 0`（迁移期一次性清积压）。
- 风险评估：硬删后不可恢复 = 符合 retention 本意；`*_headers`/stats 随行删除，但 retention_days 内的数据/统计完整保留。

### D2 — auto_vacuum 迁移（旧库一次性）
- `auto_vacuum` 只能建库前设；旧库需 `PRAGMA auto_vacuum=INCREMENTAL` + `VACUUM` 重建切换。
- 迁移**幂等 + 持久标记**：settings 存 `db_compact_migrated_v1=true`。启动时（`Db::new` 后）查标记，未迁移 → 后台 spawn 一次 `PRAGMA auto_vacuum=INCREMENTAL; VACUUM;` → 成功后置标记。
- **不阻塞启动**：迁移跑在独立 tokio task（非 setup 关键路径），失败仅 warn + 不置标记（下次重试）。VACUUM 锁库期间代理请求排队（busy_timeout=5000 兜底）。
- 新装用户（空库）：`Db::new` 建表**前**先 `PRAGMA auto_vacuum=INCREMENTAL`，直接生效免迁移。

### D3 — retention 后增量回收
- `cleanup_proxy_logs`（硬删）后立即 `PRAGMA incremental_vacuum(100)`（每次回收至多 100 页，避免长锁）。
- 现有 `proxy_log_settings_set`（lib.rs:1551）调用点不变，db.rs 内部加回收。

### D4 — 手动「压缩 DB」入口
- 新 command `db_compact`：跑全量 `VACUUM`（非 incremental，强收缩到最小）。
- 设置页（ProxyLogSettings 区或 DB 维护区）加按钮「立即压缩数据库」，显示前/后体积对比 + 警示「期间代理请求将短暂排队」。
- 前端 api.ts + i18n（8 locale）。

## 范围边界

- **改**：`src-tauri/src/gateway/db.rs`（`Db::new` pragma + auto_vacuum 探测/迁移 + `cleanup_proxy_logs` 硬删 + incremental_vacuum + 新 `compact_database` fn）、`src-tauri/src/lib.rs`（启动迁移 spawn + `db_compact` command 注册）、`src/services/api.ts`（compact 封装）、设置页组件（按钮）、8 locale。
- **不改**：`*_body` 列存储格式（不压缩）、表结构、retention 设置语义（days 数值含义不变，仅从软删变硬删）、`cleanup_user_request_fields`/`cleanup_upstream_request_fields`（继续 UPDATE SET ''，这些是字段级清理，行未到 retention_days 仍保留 stats）。
- **不涉及**：WAL checkpoint 调参（默认 1000 页够用，非主库体积）、body 压缩（Tier 3 明确否决）。

## 交付矩阵

| ID | 交付 | 验收 |
| --- | --- | --- |
| D1 | cleanup_proxy_logs 硬删 + 清历史 tombstone | R1；cargo test 新增用例 |
| D2 | auto_vacuum=INCREMENTAL 旧库迁移 + 新库直设 | R2；迁移幂等标记 |
| D3 | retention 后 incremental_vacuum | R3 |
| D4 | db_compact command + 设置页按钮 + i18n | R4；yarn build / check:i18n |

## 需求

### R1 — 硬删
- `cleanup_proxy_logs(db, retention_days)`：`DELETE FROM proxy_log WHERE created_at < ?cutoff`（retention_days=0 跳过，保持现行为）。
- 新 fn `purge_deleted_proxy_logs(db)`：`DELETE FROM proxy_log WHERE deleted_at != 0`（清历史 tombstone，迁移期 + 日常可选触发）。
- `proxy_log_settings_set` 调用链在 retention 后调 purge（清本次之前积压的 tombstone）。

### R2 — auto_vacuum 迁移
- `Db::new`：建表前若库为空（`SELECT count(*) FROM sqlite_master`==0）→ `PRAGMA auto_vacuum=INCREMENTAL`。
- 启动 spawn 后台 task：读 setting `db_compact_migrated_v1`；未迁移 → 探测 `PRAGMA auto_vacuum`（0=NONE/1=FULL/2=INCREMENTAL）；若 !=2 → `PRAGMA auto_vacuum=INCREMENTAL; VACUUM;` → 成功置标记。
- 迁移 task 失败 warn 不阻塞，不置标记，下次启动重试。
- VACUUM 不在事务内（rusqlite conn 独立调用）。

### R3 — incremental_vacuum
- `cleanup_proxy_logs` 硬删后 → `PRAGMA incremental_vacuum(100)`。
- `purge_deleted_proxy_logs` 后同样回收。

### R4 — 手动压缩
- command `db_compact(db)` → 全量 `VACUUM`，返回 `{ before_bytes, after_bytes }`（`PRAGMA page_count * page_size` 前后）。
- 设置页按钮：点击 → 确认提示（请求排队）→ 调 command → toast 显「X MB → Y MB（省 Z%）」。
- 8 locale 新键：`settings.dbCompact` / `settings.dbCompactHint` / `settings.dbCompactDone`。

## 验证门禁

```bash
cd src-tauri && cargo build
cd src-tauri && cargo clippy        # 零 warning
cd src-tauri && cargo test          # 新增 db 硬删 / 迁移幂等用例
yarn build
yarn check:i18n
```

手动：旧库（带软删 tombstone + 大量 free pages）启动 → 后台迁移 VACUUM → 文件收缩。改 retention_days 触发 → 行硬删 + incremental_vacuum。手动按钮 → 全量 VACUUM 体积对比。

## 风险

- **VACUUM 锁库**：迁移/手动压缩期间写请求排队。busy_timeout=5000 + 用户手动按钮有警示。后台迁移用独立连接、增量页数限 100。
- **迁移失败重试**：标记不置 → 每次启动重试。幂等安全（auto_vacuum=2 后探测跳过）。
- **硬删不可逆**：符合 retention 语义；retention_days 内数据完整。

## 自检（start 前）

- [ ] D1-D4 全覆盖。
- [ ] 软删 → 硬删：确认无消费方依赖 tombstone（grep `deleted_at != 0` / `deleted_at > 0` 的 SELECT）。
- [ ] VACUUM 不在事务内。
- [ ] 迁移幂等标记 + 后台非阻塞。
- [ ] 新库建表前设 auto_vacuum。
- [ ] 8 locale 键齐全。
