# C2 aidog_core 提取 + 业务下沉

## Goal

C1 ws-skeleton 已建 workspace 骨架 + 10 空 crate（含 aidog_core 空壳）。C2 填实 `crates/aidog_core`：gateway / shared / models 整目录移入 + 下沉 sync/hooks/refresh_tray_menu（消除 commands 间互依赖）+ lib.rs pub re-export。root `aidog` package 过渡保留（C10 才移），改依赖 `aidog_core` 路径引用。commands_* crate C3+ 才填，C2 阶段保持空壳。

## Context

- C1 ws-skeleton 已 done（commit 279d017f/ed870849/0beb1427）：workspace root Cargo.toml + [workspace.dependencies] + 9 空 crate + PoC 门禁全绿
- C1 遗留：主仓 `yarn tauri dev` GUI 冒烟 post-merge 验（worktree 无 display 降级证据链放行）
- protocols-rust-enum 已 done：Protocol enum 含 3 cp 变体（C2 提取 gateway 时已含）
- spec：`.trellis/spec/backend/cargo-workspace.md`（PoC 门禁 + workspace.dependencies 版本对齐 + 子 crate 规范）

## Requirements

### R1 gateway/shared/models 整目录移入 aidog_core

- `src-tauri/src/gateway/` → `crates/aidog_core/src/gateway/`（全部子模块：adapter/codex/db/estimate/http_client/i18n/manual_budget/models/peak_hours/proxy/quota/router/usage_color/mitm/commands 等）
- `src-tauri/src/shared/` → `crates/aidog_core/src/shared/`
- models 已在 gateway/models/，无需独立移
- core `lib.rs`：`pub mod gateway; pub mod shared;` + 顶层 `pub use gateway::models::*;` 等关键 re-export（供 commands crate 与 root package 用）

### R2 下沉 sync + hooks（互耦同移，防 core→commands 循环）

- `src-tauri/src/sync_settings.rs`（或 sync/）→ `crates/aidog_core/src/sync.rs`
- `src-tauri/src/hooks.rs`（或 hooks/）→ `crates/aidog_core/src/hooks.rs`
- **两者同移**（互耦：sync 用 hooks::generate_hook_scripts/enabled_hook_events；hooks 用 sync::do_sync_group_settings；design v1 漏 hooks → core 反向依赖 commands-config = 循环，grill G1 捕获）
- core lib.rs `pub use sync::*; pub use hooks::*;`（或 pub mod）

### R3 refresh_tray_menu 下沉 core（函数移，commands-tray 留 UI 薄壳）

- `tray_render::refresh_tray_menu` 函数 → `crates/aidog_core/src/tray_refresh.rs`
- core lib.rs `pub use tray_refresh::refresh_tray_menu;`
- commands-tray crate（C8）留 UI 渲染薄壳调 core（C2 不动 commands-tray，仅下沉函数定义到 core）

### R4 root aidog package 过渡依赖 aidog_core

- root `src-tauri/Cargo.toml` `[dependencies]` 加 `aidog_core = { path = "crates/aidog_core" }`
- root `lib.rs` / `startup.rs` / `commands/*.rs` 路径迁移：
  - `crate::gateway::<...>` → `aidog_core::gateway::<...>`
  - `crate::shared::<...>` → `aidog_core::shared::<...>`
  - `crate::commands::<file>::<fn>` 不动（commands C3+ 才移，C2 阶段仍在 root）
  - sync/hooks 调用 → `aidog_core::sync::*` / `aidog_core::hooks::*`
  - refresh_tray_menu 调用 → `aidog_core::refresh_tray_menu`
- **root package `[package]`/`[lib]`/业务代码不动**（仅路径引用改 + 加 dep），commands 仍在 root（C3+ 移）

### R5 aidog_core crate 填实

- `crates/aidog_core/Cargo.toml`：`[dependencies]` 填实（tauri/serde/tokio/rusqlite/axum 等从 `[workspace.dependencies]` 引 `{ workspace = true }`，禁自定版本）+ 平台特异依赖（macOS objc2-app-kit 等若 core 用）
- `crates/aidog_core/src/lib.rs`：删空壳注释，填 `pub mod gateway; pub mod shared; pub mod sync; pub mod hooks; pub mod tray_refresh;` + 关键 re-export
- core 内部 `crate::gateway::` 不动（core 内部路径）

### R6 test 路径迁移

- gateway/shared/sync/hooks 的 test 随源文件入 core（cargo test 自动跟随）
- root package 残留 test（commands 域）路径改 `aidog_core::gateway::db::Db` 或 core 顶层 re-export
- `crate::Db` / `crate::SetSettingInput`（test-only re-export, root lib.rs:18,20）→ core 加 `pub use gateway::db::{Db, SetSettingInput};` 顶层再导出，root test 改 `aidog_core::Db`

### R7 workspace.dependencies 已 C1 建（禁漂移）

- C1 已建 [workspace.dependencies]，C2 禁改版本（design 约束 1）
- aidog_core 引用 `{ workspace = true }`，新增 core 独有依赖须先加 [workspace.dependencies] 再引

## Acceptance Criteria

- [ ] `cargo build --workspace` 0 errors（含 root aidog + aidog_core + 8 空 commands crate + aidog_test_util）
- [ ] `cargo test --workspace` baseline 不回归（≥ C1 baseline 1348 passed + protocols-rust-enum 增量）
- [ ] `cargo clippy --workspace --all-targets` 无新 warning（baseline 持平）
- [ ] grep 确认：`src-tauri/src/gateway/` 目录已移除（移到 crates/aidog_core/src/gateway/）
- [ ] grep 确认：root `lib.rs`/`startup.rs`/`commands/*.rs` 的 `crate::gateway::` / `crate::shared::` 已改 `aidog_core::`（commands 域 `crate::commands::` 不动）
- [ ] `crates/aidog_core/src/lib.rs` 含 `pub mod gateway/shared/sync/hooks/tray_refresh` + re-export
- [ ] commands_* crate 保持空壳（C3+ 填，C2 不动）
- [ ] root package 仍可编译（过渡保留，业务代码不动，仅路径引用改）
- [ ] **yarn tauri dev GUI 冒烟**（worktree 无 display 降级：build.rs + tauri.conf.json git diff=0 + cdylib 产物在 + cargo build --workspace 触发 build.rs 成功；主仓 post-merge 验）

## Out of Scope

- C3-C9 commands crate 填实（各自 child task）
- C10 app binary crate wiring（root package 移出 + crates/aidog binary 建）
- generate_handler 186 处跨 crate 路径（C10）
- mitm/domain 重划（C4/C8）

## Technical Notes

- spec：`.trellis/spec/backend/cargo-workspace.md`（PoC 门禁 / workspace.dependencies 版本对齐 / 子 crate 规范 / GUI 冒烟降级证据链）
- parent design：`.trellis/tasks/07-10-commands-restructure/design.md`（C2 条目 + 下沉决策表 + 技术约束 1-7 + Grill trace）
- 路径迁移正则（design 约束 6）：
  - `crate::commands::<file>::<fn>` → 不动（C3+）
  - `crate::gateway::<...>` → `aidog_core::gateway::<...>`（root/commands 视角）
  - `crate::shared::<...>` → `aidog_core::shared::<...>`
  - core 内部 `crate::gateway::` 不动
- 下沉决策表（design line 40-49）：sync/hooks 同移 / refresh_tray_menu 函数移 core / test_harness 独立 aidog_test_util（C1 已建空壳，C2 不动）

## Definition of Done

- cargo build/test/clippy --workspace 全绿
- grep 验证路径迁移完成（gateway/shared 已移 core，root 引用改 aidog_core::）
- worktree 内 commit；主仓 post-merge yarn tauri dev 验
