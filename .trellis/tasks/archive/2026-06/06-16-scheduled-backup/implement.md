# Implement — 定时备份 (scheduled-backup)

> Task: `06-16-scheduled-backup` | 依赖 prd.md / design.md
> 单一交付, 单 worktree, 不拆 parent/child。顺序敏感 (后端 → 前端 → i18n → check)。

## subtask 序列

### S1 后端 backup 模块
- **目标**: 新建 `src-tauri/src/gateway/backup.rs`, 实现 BackupSettings / load+save / maybe_backup / run_backup / cleanup_expired / spawn_scheduler。
- **产出**: backup.rs 完整, 含 `#[cfg(test)]` 3 测试 (cleanup / settings round-trip / throttle)。
- **验证**: `cargo test backup` 通过; `cargo clippy` 零 warning。
- **依赖**: 复用 `import_export::collect::collect` + `import_export::encrypt` + `aidog_data_dir` (现有)。
- **关键**: 重入 `AtomicBool`; UTC 文件名 `aidog-backup-YYYYMMDD-HHMMSS.aidogx`; ALL_SCOPES 常量复用 import_export 的 SCOPE_*。
- **设置 key**: `backup.enabled` / `backup.interval_hours` / `backup.retention_days` / `backup.last_backup_at` / `backup.last_backup_error` (走 db settings KV)。

### S2 lib.rs 接线
- **目标**: `mod backup;` + setup spawn scheduler + 3 command 注册。
- **产出**: lib.rs diff: mod 声明、setup 内 `backup::spawn_scheduler(handle)`、`backup_settings_get/set/run_now` 三个 `#[tauri::command]`、invoke_handler 追加。
- **验证**: `cargo build` + `cargo clippy` 通过。
- **依赖**: S1。

### S3 前端 api + UI
- **目标**: api.ts 加类型 + invoke 封装; ImportExport.tsx 加 ScheduledBackup section。
- **产出**: api.ts `BackupSettings`/`BackupResult` + `backupSettingsApi`; ImportExport.tsx 新增 section (开关 / 间隔下拉 / 保留天数 / 状态展示 / 立即备份按钮 / 目录打开)。
- **验证**: `yarn build` (tsc + vite) 通过; UI 手测开关 + 立即备份。
- **依赖**: S2 (command 存在)。
- **关键**: 时间展示走 `utils/formatters.ts`; debounce set; toast 结果。

### S4 i18n 7 语言
- **目标**: 7 语言文件补 `settings.backup.*` key (~20 个)。
- **产出**: `src/locales/*/` 7 文件新 key。
- **验证**: `node scripts/check-i18n.mjs` (或现有检查脚本) 通过, 无裸 key。
- **依赖**: S3 (key 列表定型)。

### S5 check
- **目标**: 全量质量门。
- **命令**: `cd src-tauri && cargo clippy && cargo test`; `yarn build`; i18n 检查; grep 确认 backup.rs 复用 collect/encrypt (无独立导出实现)。
- **验证**: 全绿零 warning。
- **依赖**: S1–S4。

## 文件清单 (预估改动)

| 文件 | 动作 |
| --- | --- |
| `src-tauri/src/gateway/backup.rs` | 新建 |
| `src-tauri/src/gateway/mod.rs` | +`pub mod backup;` |
| `src-tauri/src/lib.rs` | +3 command + setup spawn |
| `src/services/api.ts` | +类型 + invoke 封装 |
| `src/components/settings/ImportExport.tsx` | +section |
| `src/locales/{7}/*.json` | +key |

## 并行性

S1→S2 串行 (lib 依赖 backup 模块); S3 依赖 S2 (command 存在); S4 依赖 S3 (key 定型)。关键路径 = S1→S2→S3→S4→S5, 全串行。无并行组 (单一交付)。

## 风险节点

- backup.rs 测试需临时 dir (用 `tempfile` crate 或 std tempdir); 检查 Cargo.toml 是否已有 tempfile (db.rs 测试应有)。
- spawn_scheduler 取 db: 从 app state 取 `Db` (handle.state::<Db>())。
