# 现状锚点

## 后端 backup.rs（src-tauri/src/gateway/backup.rs）

| 锚点 | 行 | 说明 |
| --- | --- | --- |
| `BackupSettings` struct | 47-64 | 5 字段：`enabled`(serde default→false) / `interval_hours`(=24) / `retention_days`(=7) / `last_backup_at`(=0) / `last_backup_error`(="") |
| `Default::default()` | 73-83 | `enabled: false` ← **本次要改为 true** |
| `load` | 87-92 | db 有记录 → serde 反序列化（缺字段走 serde default）；无 → `Self::default()`。**迁移要插这里** |
| `save` | 95-106 | 全字段 upsert（`db::set_setting`），不动 |
| `sanitized` | 109-117 | 只钳 interval/retention，不管 enabled，不动 |
| `maybe_backup` | 153-163 | `enabled && (last_backup_at==0 || 超间隔)` → 跑备份。首启立即备份逻辑已存在，不动 |
| `spawn_scheduler` | 264-287 | setup 启动首检（`maybe_backup`）+ 常驻 loop。不动 |
| 现有单测 | 305-435 | 5 个 case：roundtrip / default_when_missing / sanitized / cleanup / maybe_backup。**需补迁移 case**，且 `settings_default_when_missing` 因 Default 翻 true 需同步改断言 |

## 后端命令（src-tauri/src/lib.rs）

| 锚点 | 行 | 说明 |
| --- | --- | --- |
| `backup_settings_get` | 1918-1922 | `load().await.sanitized()`。**迁移在 load 内完成，get 自动受益** |
| `backup_settings_set` | 1925-1934 | 收 `settings: BackupSettings` → `sanitized()` → `save()`。**需在此强制 `defaults_version = CURRENT` 后再 save**（标记「用户手动确认」） |
| `backup_run_now` | 1937-1955 | 不动（run_backup 内部 load/save 只更新 last_backup_at/error，不动 enabled/version） |
| handler 注册 | 3744-3746 | 不动 |
| setup spawn_scheduler | 3489 | 不动 |

## 前端

| 锚点 | 行 | 说明 |
| --- | --- | --- |
| `BackupSettings` TS interface | `src/services/api.ts:1625-1635` | 5 字段，与后端对齐。**设计决策见下** |
| UI section | `src/components/settings/ImportExport.tsx:723-` `ScheduledBackupSection` | `backupApi.get()` → `setSettings`；`patch()` spread merge → `backupApi.set(merged)`（行 747） |
| i18n | `src/locales/*.json` `settings.backup.*` | 文案不改 |

# 设计决策

## Q1 迁移方案：defaults_version 版本门控

**选型**：加 `defaults_version: u32` 字段做版本门控（brainstorm 推荐方案，直接采纳）。

- 新常量 `const CURRENT_DEFAULTS_VERSION: u32 = 1;`
- `BackupSettings` 加字段 `#[serde(default)] pub defaults_version: u32`（老 JSON 无此字段 → 反序列化为 0，标记「从未手动确认」）。
- 新 `Default`：`enabled: true, defaults_version: CURRENT_DEFAULTS_VERSION, ...`（全新用户走 Default，直接拿到 version=1 的开态）。
- `load` 加一次性迁移：
  ```
  let mut s = <serde 或 default>;
  if s.defaults_version < CURRENT_DEFAULTS_VERSION {
      // 老数据/旧默认：若 enabled 仍是 false（旧默认值），视为「从未手动改」→ 翻 true。
      // 若老数据恰好 enabled=true，无需翻，但仍要升级 version。
      if !s.enabled {
          s.enabled = true;
      }
      s.defaults_version = CURRENT_DEFAULTS_VERSION;
      let _ = s.save(db).await;  // 持久化迁移结果，幂等
  }
  s
  ```
- **为什么不区分 enabled 的「翻」与「不翻」**：老数据 `enabled: false` 有两种来源——(a) 旧默认从未碰过 (b) 用户手动关。两者在 db 中**完全同形**（无 version 字段区分），无法分离。门控策略选择「version=0 即视为未手动确认，统一翻」，代价是「在本次升级前恰好手动关过 backup 的老用户会被重新打开」——这是可接受的一次性回归（用户会在下次打开 app 时看到备份开始，若不想要可在 UI 关，关的瞬间 version 置 1 永久尊重）。权衡理由：覆盖「从未碰过的沉默大多数」价值 >> 极少数「手动关且升级后不再关」用户的打扰成本；且首备落盘 `~/.aidog/backups/` 是加文件非破坏性，可删可关。

## Q2 defaults_version 是否暴露给前端

**选型：不暴露给前端 interface，后端在 `backup_settings_set` 强制覆写**。

理由：
- 前端 `BackupSettings` interface（`api.ts:1625`）若加 `defaults_version`，UI `patch()`（`ImportExport.tsx:743-751`）做 `{...settings, ...next}` spread 时会原样回传当前值（=CURRENT），语义上等于「每次 UI 保存都标记手动确认」——正是我们想要的。但让前端背负 version 语义增加耦合，且 `settings` state 来自 `backupApi.get()`，get 返回带 version 的完整 struct，前端只透传不解释。
- 更简洁：**前端 interface 不加 `defaults_version`**（保持 5 字段），后端 `backup_settings_set` 收到的 `settings: BackupSettings` 反序列化时 `#[serde(default)]` → version=0（因前端没传），命令体内 `settings.defaults_version = CURRENT_DEFAULTS_VERSION;` 强制置位再 `sanitized().save()`。这样：
  - 前端零改动（`api.ts` / `ImportExport.tsx` 都不动）。
  - 语义清晰：只要走过 `backup_settings_set`（UI 保存入口），一律标记「已手动确认」。
  - 唯一注意点：`backup_settings_set` 的 `settings` 参数从 serde 反序列化前端 JSON，前端没传 `defaults_version` → serde default 填 0 → 命令内覆写为 CURRENT。**不要**依赖前端传值。
- 副作用：`run_backup` 内部的 `BackupSettings::load(...).save(...)`（`backup.rs:175-188`）会保存 load 返回的 struct（已含迁移后的 version=1），不触发误降级。`backup_settings_set` 是唯一从「外部输入」构造 settings 的路径，只在它这里强制 version=CURRENT 即可。

## Q3 迁移落库时机

在 `load` 内 `save`。代价：每次 app 启动首次 load 会多一次 db write（仅当 version<1 时触发，迁移后 version=1 不再触发）。可接受。

备选：在 `spawn_scheduler` 启动块显式调一次迁移命令。否决：`load` 是唯一权威读入口（get/run_backup/scheduler 都走它），迁移放 load 内可保证所有调用方都拿到迁移后状态，无需每个 caller 各自记得调迁移。

# 改动清单

## 1. backup.rs

### 1.1 加常量 + 字段
- 文件顶（`SETTING_KEY` 附近，行 24 左右）加 `const CURRENT_DEFAULTS_VERSION: u32 = 1;`
- `BackupSettings` struct（行 47-64）末尾加：
  ```rust
  /// 默认值版本号：0 = 老数据/从未手动确认；<CURRENT = 待迁移。
  /// 用户经 UI 保存一次后写为 CURRENT（标记「已手动确认」，此后尊重存储值）。
  #[serde(default)]
  pub defaults_version: u32,
  ```

### 1.2 改 Default（行 73-83）
```rust
impl Default for BackupSettings {
    fn default() -> Self {
        Self {
            enabled: true,                          // ← false → true
            interval_hours: default_interval_hours(),
            retention_days: default_retention_days(),
            last_backup_at: 0,
            last_backup_error: String::new(),
            defaults_version: CURRENT_DEFAULTS_VERSION,
        }
    }
}
```

### 1.3 load 加迁移（行 87-92）
```rust
pub async fn load(db: &Db) -> Self {
    let mut s = match db::get_setting(db, SETTING_SCOPE, SETTING_KEY).await {
        Ok(Some(v)) => serde_json::from_value(v).unwrap_or_default(),
        _ => Self::default(),
    };
    if s.defaults_version < CURRENT_DEFAULTS_VERSION {
        if !s.enabled {
            s.enabled = true;
        }
        s.defaults_version = CURRENT_DEFAULTS_VERSION;
        let _ = s.save(db).await;  // 持久化，幂等：下次 load version=1 跳过
    }
    s
}
```

### 1.4 sanitized 不动（行 109-117）

### 1.5 补单测（行 305-435 mod tests）
新增 case：
- `migration_flips_enabled_for_legacy_default_false`：手写老 JSON `{"enabled":false,"interval_hours":24,"retention_days":7}`（无 version）→ save → load → assert `enabled==true && defaults_version==CURRENT`。
- `migration_respects_user_disabled_after_confirm`：save `{"enabled":false,"defaults_version":CURRENT}` → load → assert `enabled==false`（不翻）。
- `migration_idempotent`：连续 load 两次，第二次不重复触发 save（可用 `get_setting` 计数或观察字段稳定）。
- `default_is_enabled_true`：`BackupSettings::default().enabled == true && defaults_version == CURRENT`。
- 修正既有 `settings_default_when_missing_fields`（行 326-333）：该 case 构造的 `{"enabled":true}` 缺 version → serde default=0，断言 `enabled==true` 仍成立（恰好 true），但需补 `defaults_version==0` 断言以反映 serde 行为（此 case 不经 load，不走迁移，version 保持 0）。确认此 case 语义不被破坏。
- `settings_roundtrip`（行 309-323）struct 字面量构造缺 `defaults_version` → 编译失败，需补 `defaults_version: CURRENT_DEFAULTS_VERSION`（或 0）字段。
- 同理 `sanitized_clamps_invalid_values`、`maybe_backup_throttles_within_interval`、`backup_settings_load_save_roundtrip` 等所有 struct 字面量构造处补字段。

## 2. lib.rs backup_settings_set（行 1925-1934）

```rust
async fn backup_settings_set(
    db: State<'_, Db>,
    mut settings: gateway::backup::BackupSettings,   // ← 加 mut
) -> Result<gateway::backup::BackupSettings, String> {
    tracing::debug!(command = "backup_settings_set", "command invoked");
    settings.defaults_version = gateway::backup::CURRENT_DEFAULTS_VERSION;  // 标记手动确认
    let sanitized = settings.sanitized();
    sanitized.save(&db).await?;
    Ok(sanitized)
}
```

`CURRENT_DEFAULTS_VERSION` 需在 backup.rs 设为 `pub const`。

## 3. 前端

**不改**。`api.ts` `BackupSettings` interface 保持 5 字段（不加 `defaults_version`）；`ImportExport.tsx` 不改。

## 4. i18n / DB

无改动。

# subtask 拆分

## ST1 backup.rs 加字段 + Default + load 迁移 + 单测

- **目标**：`BackupSettings` 加 `defaults_version`，Default 翻 enabled=true，load 做版本门控迁移并补全单测。
- **产出**：`src-tauri/src/gateway/backup.rs` 改动 + `cargo test` 全绿。
- **验证**：`cd src-tauri && cargo test backup` 通过含 4 新 case；`cargo clippy` 无 warning。
- **资源**：backup.rs（本文件锚点表）。
- **依赖**：无（后续 ST2 依赖本 ST 的 `CURRENT_DEFAULTS_VERSION` pub 暴露）。

## ST2 lib.rs backup_settings_set 写 version

- **目标**：`backup_settings_set` 命令体在 save 前强制 `defaults_version = CURRENT_DEFAULTS_VERSION`。
- **产出**：`src-tauri/src/lib.rs:1925-1934` 改动。
- **验证**：`cargo test`；手动或单测验证 UI 保存后 db 中 version=CURRENT（即便 enabled=false）。
- **资源**：lib.rs 锚点；ST1 暴露的 pub const。
- **依赖**：ST1 完成（字段与常量存在）。

## ST3 手动验证（可选，dev 环境跑）

- **目标**：端到端验 AC1-AC9。
- **产出**：dev 跑 `yarn tauri dev`，清 db / 造老数据 / 手动关三种场景验证。
- **验证**：对照 prd.md AC 逐项勾。
- **资源**：本任务 dev 环境。
- **依赖**：ST1+ST2 完成。

**并行性**：ST1 → ST2 严格串行（共享 backup.rs 符号）。ST3 在 ST2 后。

# 迁移幂等性测试要点

1. **首次 load 触发 save**：version=0 → load 内 save 写 version=1。
2. **二次 load 不触发 save**：version=1 → 跳过迁移块，无 save 调用。（单测可用 `save` 返回值或 db 计数验证；或观察 `defaults_version` 字段稳定即可证明逻辑幂等。）
3. **并发安全**：`load` 可能被 get / scheduler / run_backup 并发调用。迁移块内 `save` 是全字段 upsert（`db::set_setting` → INSERT OR REPLACE），最后写者胜。因迁移结果确定性（version 升 1 + enabled 可能翻 true），多写者写同值，幂等无冲突。**无需额外锁**。
4. **迁移失败不阻断**：`let _ = s.save(db).await;` 吞错——即使 save 失败，本次 load 仍返回迁移后的内存态（enabled=true），下次启动会再试迁移（因 db 未更新，version 仍 0）。可接受：最坏情况是每次启动多一次失败 save 尝试，不影响功能。
