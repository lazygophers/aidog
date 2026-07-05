# 应用日志启用时强制输出到文件 (无视 debug 模式)

## Goal

`AppLogSettings.file_enabled=true` 时，无论 dev (debug build / `make run`) 还是 release，都写文件日志。当前 `init_logging` dev 分支强制 console-only，跳过文件层 —— 用户 debug 模式跑时拿不到日志文件供诊断。

## Decision (ADR-lite)

- **仅加 dev file 层**（用户裁定）：dev 分支按 `file_enabled` 决定是否挂 file 层，与 release 共用 file 层构造逻辑
- console dev 仍强制 debug 级（不变）
- file 层 level / rotation / retention / 路径与 release 一致

## What I already know

- `src-tauri/src/logging.rs::init_logging` 三分支：
  - `cfg!(debug_assertions)` (dev) → console only（强制 debug 级），**完全跳过 file 层**
  - release + `file_enabled=true` → console + RollingFileAppender (hourly, `data_dir/logs/aidog-*.log`)
  - release + `file_enabled=false` → console only
- `AppLogSettings` (DB settings `app.logging`)：`file_enabled` / `level` / `retention_hours`，default `true/info/3`
- `make run` = `yarn tauri dev` = debug build = `cfg!(debug_assertions)=true`
- 文件 appender: `RollingFileAppender` (HOURLY rotation, prefix `aidog`, suffix `log`, `max_log_files` by retention)

## Requirements

- `file_enabled=true` 时，**dev 与 release 都启用 file 层**
- dev 模式 console 仍强制 debug 级（不变）
- file 层 level：dev 与 release 一致遵循 `settings.level`（用户配置；RUST_LOG 覆盖优先）
- `file_enabled=false` 时仍不写文件（用户显式关）
- file 路径 / rotation / retention 与 release 现状一致

## Acceptance Criteria

- [ ] `make run` 启动后，`<data_dir>/logs/aidog-*.log` 存在并被写入
- [ ] 关 `file_enabled` 后无文件输出
- [ ] release 行为不回归
- [ ] 单测：dev 分支 + file_enabled=true 时构造 file 层（如可隔离验证；否则手动 dev 实测）

## Out of Scope

- 调整 console 层行为（dev 仍 console-only 强制 debug 不变）
- 调整 retention / rotation 策略
- 跨平台日志路径变化

## Technical Notes

- 唯一改动点：`src-tauri/src/logging.rs::init_logging` 的 `cfg!(debug_assertions)` 分支
- 改法：dev 分支不再 early return，按 `settings.file_enabled` 决定是否加 file 层（与 release 共用 file 层构造逻辑）
- ponytail：抽出 `build_file_layer(data_dir, settings)` 复用，dev/release 两分支都按需挂载
