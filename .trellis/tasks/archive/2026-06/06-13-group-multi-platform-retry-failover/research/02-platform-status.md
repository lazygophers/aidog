# Research: 平台状态字段（启用/禁用）现状

- **Query**: Platform 状态字段是 bool 还是 enum? DB schema? 启用/禁用 command + 前端 UI? router 如何过滤禁用平台?
- **Scope**: internal
- **Date**: 2026-06-13

## 结论速览

- **平台状态现在是 `bool`（`enabled`），不是 enum**。仅"启用/禁用"两态。新增第三态 "auto_disabled" 需把 `bool → enum`（或加并存的 `status` 列），是跨 DB/models/router/前端的迁移。
- DB 列：`platform.enabled INTEGER NOT NULL DEFAULT 1`（0/1）。
- 启用/禁用**没有专门 command**，复用 `platform_update`（`UpdatePlatform.enabled: Option<bool>`），前端 toggle 调 `platformApi.update({ id, enabled: !p.enabled })`。
- router 选平台时**只用 `gp.platform.enabled` 这一个 bool 过滤**（failover/load_balance 均是）。

## Findings

### Rust 模型（models.rs）

| File:Line | 说明 |
|---|---|
| `models.rs:340` | `Platform.enabled: bool` — **唯一状态字段** |
| `models.rs:405` | `UpdatePlatform.enabled: Option<bool>` — 更新入参 |
| `models.rs:377-392` | `CreatePlatform` 无 enabled 字段（创建默认启用） |

无 `PlatformStatus` enum、无 `status` 字段。grep `PlatformStatus/status/disabled` 在 models.rs 仅命中 ProxyLog 的 `status_code`、ManualBudget/proxy 的无关字段。

### DB schema + CRUD（db.rs / 001_init.sql）

| File:Line | 说明 |
|---|---|
| `migrations/001_init.sql:16` | `enabled INTEGER NOT NULL DEFAULT 1` |
| `db.rs:78` (`PLATFORM_COLUMNS`) | SELECT 列序含 `enabled`（第 10 列，0-idx 9） |
| `db.rs:152` | 读：`enabled: row.get::<_, i64>(9)? == 1` |
| `db.rs:214` | create 默认 `enabled: true` |
| `db.rs:282` | update：`enabled: input.enabled.unwrap_or(existing.enabled)` |
| `db.rs:299,304-314` | 写：`let enabled = updated.enabled as i64;` → `UPDATE platform SET ... enabled=?...` |
| `db.rs:870` | group_platform JOIN 查询里读平台 enabled（第 11 列） |

新增状态列时需改：001_init.sql（或新 migration ALTER）、`PLATFORM_COLUMNS`、`row_to_platform` 映射、create/update、`db.rs:847` 的 group_platform JOIN SELECT。

### Tauri command（lib.rs）

| File:Line | 说明 |
|---|---|
| `lib.rs:185-187` | `platform_update(input: UpdatePlatform)` → `db::update_platform` — **启用/禁用走这里**，无独立 enable/disable command |

### 前端状态展示 / 切换（Platforms.tsx + api.ts）

| File:Line | 说明 |
|---|---|
| `src/services/api.ts:164` | `Platform.enabled: boolean` |
| `src/services/api.ts:325` | `UpdatePlatform.enabled?: boolean` |
| `Platforms.tsx:1146` | 卡片透明度：`p.enabled ? 1 : 0.5`（禁用变灰） |
| `Platforms.tsx:1283-1287` | toggle 开关：`className={\`toggle ${p.enabled ? "active" : ""}\`}`，点击 `actions.onToggleEnabled(p)`，title `Disable/Enable` |
| `Platforms.tsx:1807` | `onToggleEnabled` 实现：`await platformApi.update({ id: p.id, enabled: !p.enabled })` |
| `Platforms.tsx:2521` | 计数：`${platforms.filter(p => p.enabled).length} / ${platforms.length}` |

UI 是二态 toggle，无第三态视觉表达。新增 "auto_disabled" 需要：新视觉态（如黄/红标记 + 区分"用户禁用 vs 自动禁用"）、可能的"恢复启用"操作。

### router 过滤逻辑

`router.rs:117` `select_failover`: `.find(|gp| gp.platform.enabled)`
`router.rs:124` `select_load_balance`: `.filter(|gp| gp.platform.enabled)`

→ 改 enum 后，这两处过滤条件需改为 "状态 == Enabled"（auto_disabled 与 disabled 都应被排除出候选）。

## Caveats / Not Found

- 无任何"自动禁用 / 自动恢复"机制存在，全是手动 toggle。
- bool→enum 迁移**兼容性关注点**：旧库 `enabled INTEGER 0/1`。若改 enum，需 migration 把 1→"enabled"、0→"disabled"，或保留 `enabled` bool 再加独立 `status` 列（二选一是给 main 的决策点，见 04-fix-points）。
