# 背景

定时备份子系统 (`src-tauri/src/gateway/backup.rs`) 提供按用户设定间隔把全量数据加密导出为 `.aidogx` 落盘 `~/.aidog/backups/`、超期自动清理。当前默认总开关 `enabled = false`（`backup.rs:76` `Default::default()`），新用户安装后不会自动开始备份，需手动到「设置 → 导入导出 → 定时备份」勾选启用，存在「忘记开 → 丢数据」风险。

`interval_hours` 默认 24h、`retention_days` 默认 7d（`backup.rs:67/70`），与用户诉求「每天一份留 7 天」一致，不动。

核心难点：现存老用户 db 中存的是旧默认 `enabled: false`，serde 反序列化（`backup.rs:88-89`）原样读出，无法区分「用户手动关闭」与「旧默认 false 从未碰过」。直接把 `Default` 翻为 `true` 只能惠及全新用户（db 无记录走 `Self::default()`），对老用户无效；若强写迁移把所有 `enabled == false` 翻成 `true`，会覆盖用户显式关闭的意图。

# 目标

1. **新用户**（db 无 backup 设置）首启即得 `enabled = true`、`interval_hours = 24`、`retention_days = 7`。
2. **老用户未手动改过 backup 开关**：一次性迁移把 `enabled` 翻为 `true`，等价新用户体验。
3. **老用户已手动确认过 backup 设置**（无论开/关）：尊重存储值，迁移不翻。
4. 首启立即备份逻辑（`backup.rs:153-163` `maybe_backup`，`last_backup_at == 0` 即跑）现状已满足，不改。
5. 迁移幂等：重复 `load` 不反复翻 `enabled`。
6. interval/retention 默认值与钳制区间不动。

# 非目标

- 不改备份加密容器 / 文件格式 / 目录布局。
- 不改调度 loop / throttle 逻辑。
- 不改 UI 文案（开关标签仍是「启用定时备份」）。
- 不做 per-scope 备份粒度（仍全量）。
- 不引入云备份 / 远程同步。

# 验收标准

- [ ] **AC1 新用户默认开**：全新 db（`setting` 表无 scope=`backup` key=`settings`）首启后 `backup_settings_get` 返回 `enabled = true`、`interval_hours = 24`、`retention_days = 7`，且 db 已写入 `defaults_version = <CURRENT>`、`enabled = true`。
- [ ] **AC2 老用户迁移开**：db 存在旧记录 `{"enabled":false,"interval_hours":24,"retention_days":7}`（无 `defaults_version` 字段）→ 下一次 `load` 返回 `enabled = true` 且 `defaults_version = <CURRENT>`，db 持久化。
- [ ] **AC3 手动关不翻**：db 存在 `{"enabled":false,...,"defaults_version":<CURRENT>}` → `load` 返回 `enabled = false`（尊重用户）。
- [ ] **AC4 手动开后不翻**：db 存在 `{"enabled":true,...,"defaults_version":<CURRENT>}` → `load` 返回 `enabled = true`（无副作用）。
- [ ] **AC5 首启立即备份**：AC1/AC2 触发后，因 `last_backup_at == 0`，`spawn_scheduler` 启动检查（`backup.rs:269` `maybe_backup`）立即执行一次备份，无需等 interval。
- [ ] **AC6 幂等**：AC2 迁移后再次 `load`，`enabled` 保持 `true`，不因重复 `load` 反复触发 save。
- [ ] **AC7 默认值不动**：`interval_hours` 默认 24、`retention_days` 默认 7、`sanitized` 钳制区间（interval ≥1、retention 1..=90）不变。
- [ ] **AC8 UI 显式保存标记手动确认**：用户在「设置 → 定时备份」点开关或改间隔/保留天数并保存后，db 中 `defaults_version` 写为 `<CURRENT>`（即使本次保存 `enabled = false`，也算「已手动确认」，后续迁移不再翻）。
- [ ] **AC9 cargo test 全绿**：新增迁移幂等性 / 老用户翻 / 手动关不翻 / 新用户默认开等 case 通过，既有 case 不回归。

# 影响范围

| 层 | 文件 | 改动类型 |
| --- | --- | --- |
| Rust 后端 | `src-tauri/src/gateway/backup.rs` | 加 `defaults_version` 字段 + 改 `Default` + `load` 加迁移逻辑 + 补单测 |
| Rust 命令 | `src-tauri/src/lib.rs:1926-1934` `backup_settings_set` | 保存路径写 `defaults_version = CURRENT`（标记手动确认） |
| 前端类型 | `src/services/api.ts:1625-1635` `BackupSettings` | 见 implement.md 设计决策：`defaults_version` 是否暴露给前端（推荐**不暴露**，后端在 set 时强制覆写为 CURRENT，前端 merged spread 自然丢弃） |
| i18n | 无 | UI 文案不改 |
| DB | 无 schema 变更 | `setting.value` JSON 多一字段，serde 容错 |
