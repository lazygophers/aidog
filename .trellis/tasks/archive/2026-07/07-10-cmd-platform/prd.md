# C3 commands-platform crate 迁移

## Goal

把 platform 域 6 commands 文件（platform/group/model_fetch/stats/price/quota）从 root `aidog` crate 搬到 `crates/commands_platform/` 独立 crate，源文件路径迁移 + root commands.rs/startup.rs/Cargo.toml 同步清理。**纯结构搬移，零行为变更**。

## Parent

- 真值源：`.trellis/tasks/07-10-commands-restructure/prd.md` R3 + design.md C3 行
- parent 已锁 scope + 路径策略 + 下沉决策，本 child 不重开 brainstorm，纯机械执行

## Scope（6 源文件 + 对应 test_）

| 文件 | 行 | test_*.rs |
|---|---|---|
| platform.rs | 298 | test_platform.rs |
| group.rs | 193 | test_group.rs |
| model_fetch.rs | 216 | test_model_fetch.rs（若存）|
| stats.rs | 80 | — |
| price.rs | 100 | — |
| quota.rs | 99 | — |

> grep `src/commands/test_*.rs` 确认实际 test 文件清单。

## Requirements

### R1 crate Cargo.toml 填充
- `crates/commands_platform/Cargo.toml`：`[dependencies]` 加 `aidog_core = { path = "../aidog_core" }` + `tauri = { workspace = true }` + 业务依赖（serde / serde_json / rusqlite / reqwest / tokio / chrono 等，按源文件 `use` 扫）
- `[dev-dependencies]`：`tauri = { workspace = true, features = ["test"] }` + `aidog_test_util = { path = "../aidog_test_util" }`（test_group 用 MockRuntime + mock_app_with_db）

### R2 crate lib.rs 填充
- `crates/commands_platform/src/lib.rs`：`pub mod platform; pub mod group; pub mod model_fetch; pub mod stats; pub mod price; pub mod quota;`
- `#[tauri::command]` 函数保持 `pub`（app crate generate_handler 跨 crate 引）

### R3 源文件迁移
- `git mv src/commands/<file>.rs crates/commands_platform/src/<file>.rs`（6 源 + 对应 test_*.rs）
- test 文件随源入 crate `src/` 同级

### R4 源文件内路径迁移
- `crate::commands::X::Y` → `commands_platform::Y` 或 `aidog_core::<path>::Y`
- `crate::gateway::` → `aidog_core::gateway::`
- `crate::shared::` → `aidog_core::shared::`
- `crate::Db` / `crate::SetSettingInput` → `aidog_core::Db` / `aidog_core::SetSettingInput`
- `aidog_core::sync_settings::*` / `aidog_core::tray_render::*`（下沉函数直调 core，不引其他 commands crate）
- **禁** 依赖其他 commands_* crate（编译期阻断，铁律）

### R5 root 清理
- `src/commands.rs`：删 `pub mod platform/group/model_fetch/stats/price/quota;`（6 行）+ 对应 test mod 声明
- `src/lib.rs` 或 root Cargo.toml：root `aidog` crate 加 `commands_platform = { path = "crates/commands_platform" }` dependency（过渡期 root 仍引，C10 才挪 app crate）
- `startup.rs` `generate_handler![...]`：`crate::commands::platform::X` → `commands_platform::X`（6 域所有 command fn 路径改）

### R6 验证
- `cargo build --workspace` 全绿
- `cargo test --workspace` baseline 不回归（1382 passed，参 config-externalization 后）
- `cargo clippy --workspace --all-targets` 无新 warning
- `commands_platform` crate 独立 `cargo build -p commands_platform` 绿
- crate 间零互依赖（`commands_platform/Cargo.toml` 不声明其他 commands_*）

## Acceptance

- [ ] 6 源文件 + test 入 `crates/commands_platform/src/`，root `src/commands/` 对应文件删
- [ ] 源文件路径迁移全完成（grep `crate::commands::` 在 commands_platform/src/ = 0）
- [ ] root commands.rs 删 6 mod + startup.rs generate_handler 路径改对
- [ ] commands_platform 独立编译绿
- [ ] cargo build/test/clippy --workspace 全绿，无新 warning
- [ ] crate 零互依赖（仅 aidog_core + tauri + 业务）
- [ ] 主仓零改动（worktree 内）

## Out of Scope

- 其他域 commands（C4-C9 各自 task）
- app crate wiring + generate_handler 全量路径（C10）
- 行为变更 / 重构（纯搬移）

## Technical Notes

- 跨 command 域边引用：platform/group 调 `tray_render::refresh_tray_menu` / `sync_settings::do_sync_group_settings` → 已下沉 aidog_core（C2 done），路径改 `aidog_core::tray_render::*` / `aidog_core::sync_settings::*`
- test_harness（mock_app_with_db）：test_platform/test_group 引 → 用 `aidog_test_util` crate（dev-deps）
- 路径迁移正则参 parent design.md §6
