# aidog_core 的 tauri 依赖收敛为 optional — PRD (主入口)

## 来源

`arch-deepen-2` 的 c3-commands（删 6 个 commands_* crate、`#[tauri::command]` 上移 aidog_core）原计划配套「`feature = "tauri"` gate 保 core 可独立测」。c3-pilot 勘察发现**该前提在迁移前就已不成立**：`aidog_core` 早已硬依赖 tauri，`cargo test -p aidog_core --no-default-features` 在 c3 动手之前就跑不了。

用户 2026-07-27 拍板：**债务单独立项，arch-deepen-2 本轮只迁 command，不碰 Cargo.toml**。

## 目标

`cargo test -p aidog_core --no-default-features` 可跑 —— 纯逻辑层（router / converter / db / estimate / quota）脱离 tauri runtime 独立测试，缩短测试反馈环，同时把 tauri 耦合收敛成显式 seam。

## 边界

**范围内**：`aidog_core` 的 `Cargo.toml` tauri / tauri-plugin-notification 改 optional + feature gate；11 个文件的 `tauri::` 用法抽 trait / 回调 seam。

**范围外**：commands_* crate 的迁移（归 arch-deepen-2 的 c3）；root crate 的接线只做被动适配。

## 现状（亲验，2026-07-27）

```
src-tauri/crates/aidog_core/Cargo.toml:11   tauri = { workspace = true }              ← 非 optional
src-tauri/crates/aidog_core/Cargo.toml:12   tauri-plugin-notification = { workspace = true }
```

`tauri::` 引用分布**已随 arch-deepen-2 的 c3-commands 膨胀：11 文件 → 60 文件**（2026-07-28 复核，`rg -l 'tauri::' src-tauri/crates/aidog_core/src/ | wc -l`）。

**原 11 个非 command 文件**（真正的难点，需抽 trait / 回调 seam 才能 gate）：

- `src/hooks.rs`
- `src/sync_settings.rs`
- `src/tray_render.rs`
- `src/shared.rs`
- `src/gateway/proxy/log.rs`
- `src/gateway/proxy/test_connect.rs`
- `src/gateway/proxy/mod.rs`
- `src/gateway/notification/tts.rs`
- `src/gateway/notification/dispatch.rs`
- `src/gateway/codex.rs`
- `src/gateway/backup/scheduler.rs`

**c3 新增 49 个 command 文件**（天然属 tauri feature，整体 `#[cfg(feature = "tauri")]` gate 掉即可，非难点）：
`system_cmd/*`(7) / `platform_cmd/*`(10) / `proxy_cmd/*`(7) / `ai_tools_cmd/*`(6) / `cli_proxy_cmd/*`(5) + 独立 `cli_env.rs` / `settings.rs` / `defaults.rs` / `popover.rs` 等。

工作量评估随之上调：gate 面从 11 文件扩到 60，但增量的 49 个是机械 cfg 标注（可用 `command_macro.rs` 的 `tauri_command!` 宏统一挂 cfg，单点改）。

## 已知约束

- `#[tauri::command]` 本体（c3 迁入的）天然属于 tauri feature，gate 掉即可，不是难点
- 真正的难点是上面 11 个文件里的**非 command** 用法：AppHandle 取用、事件 emit、通知 plugin —— 需抽 trait / 回调 seam 才能 gate
- 开工前必须重跑 `rg -l 'tauri::' src-tauri/crates/aidog_core/src/` 刷新清单（c3 会改变分布）

## 验收标准

- [ ] `cargo test -p aidog_core --no-default-features` 通过
- [ ] `cargo check --workspace --all-targets` 0 error
- [ ] `cargo clippy --workspace --all-targets` 0 warning
- [ ] `cargo test -p aidog_core`（默认 feature）不劣于开工时基线
- [ ] `aidog_core/Cargo.toml` 中 tauri / tauri-plugin-notification 为 `optional = true`

## 索引
- 详细设计: [design.md](design.md)
- 调研收敛: [findings.md](findings.md) (仅真调研时生)
- 任务/子任务/调度: task.json (脚本真值, `skein.py subtask list core-tauri-optional`)
