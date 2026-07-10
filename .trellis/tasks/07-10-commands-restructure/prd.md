# src-tauri commands cargo workspace 多 crate 重构

## Goal

把 `src-tauri/` 从单 crate 重构为 **cargo workspace**（pnpm workspace 同构）：1 个 `aidog_core` lib crate（gateway/shared/models）+ 7 个 `commands-*` 子 crate（按域分包）+ 1 个 `aidog` app crate（binary + Tauri wiring）。**build/run/release 不受影响**（共享 target，单 binary 输出）。

**为什么**：用户要"commands 合理分包"+"拆多个子包，每个维护独立逻辑，减少相互依赖"，并明确要 pnpm workspace 同构的 cargo workspace（"实际 build/run/release 都不会受到影响的那种"）。当前 31 个 command 文件 + gateway/shared/models 全平铺在单 `aidog` crate，跨域依赖隐式（`crate::commands::X` 任意可达），无编译期边界强制。workspace 多 crate 后：每 crate 显式声明依赖 → 非法跨域调用编译期阻断；增量编译并行 per crate；域边界 = crate 边界，所有权清晰。

## 现状（已读）

- `src-tauri/Cargo.toml`：单 crate `aidog`，crate-type `[staticlib, cdylib, rlib]`，lib name `aidog_lib`，edition 2024。非 workspace。
- `src-tauri/src/lib.rs`（22 行）：`mod gateway; mod logging; mod shared; mod commands; mod app_setup; mod startup; mod deep_link;` + `pub use startup::run`。
- `src-tauri/src/commands.rs`（38 行）：31 个 `pub mod X;` 平铺 + `test_harness`。
- `src-tauri/src/commands/*.rs`：54 文件（31 源 + 23 test_*.rs），7311 行。
- `src-tauri/src/gateway/`：db / router / proxy / converter / mitm / models / estimate / quota / price_sync / codex / adapter / http_client / i18n / manual_budget / usage_color / peak_hours + import_export + skills 子树。核心业务逻辑。
- `src-tauri/src/shared/`：跨域 helper / 类型。
- `src-tauri/src/startup.rs`（261 行）：`generate_handler![...]` 注册 186 处 `crate::commands::X::cmd_fn`。
- 跨 command 依赖仅 11 边（已 grep）：
  - `group`/`proxy` → `sync_settings::do_sync_group_settings`
  - `hooks` ↔ `sync_settings`（generate_hook_scripts / enabled_hook_events）
  - `platform`/`proxy` → `tray_render::refresh_tray_menu`
  - `popover` → `tray::tray_layout`
  - `tray_render` → `tray::{TrayColumn, TRAY_FONT_SIZE, build_tray_menu, tray_layout, tray_separator}`（紧耦合）
  - `middleware` → `mitm::ImportDefaultsResult`

## 决策（brainstorm 锁定）

1. **workspace 拓扑**（用户选「core + 7 commands」）：
   ```
   src-tauri/
   ├── Cargo.toml              # [workspace] members + 共享 profile
   ├── tauri.conf.json         # 指向 crates/aidog binary
   ├── build.rs                # workspace root 或 crates/aidog/build.rs
   └── crates/
       ├── aidog_core/         # gateway/ + shared/ + models lib（aidog_core::）
       ├── commands-platform/  # platform/group/model_fetch/stats/popover/price/quota
       ├── commands-proxy/     # proxy/proxy_log/proxy_timeout/middleware/mitm
       ├── commands-config/    # settings/sync_settings/defaults/hooks
       ├── commands-system/    # about/app_log/auto_update/backup/notification/scheduling/fs_autocomplete
       ├── commands-ai-tools/  # coding_tools/mcp/skills/script_executor/model_test
       ├── commands-tray/      # tray/tray_render
       ├── commands-cli-env/   # cli_env（独立复杂，22KB）
       └── aidog/              # binary crate（main.rs + startup.rs + Tauri wiring）
   ```
2. **路径策略**（用户选「嵌套破路径」）：`crate::commands::X::Y` → `commands_X::Y`（跨 crate），`crate::gateway::` → `aidog_core::gateway::`（commands crate 视角）；core 内部保持 `crate::gateway::`。
3. **跨 crate 依赖**（用户要"减依赖"，但选 workspace 边界强制而非 trait 反转）：commands crate 间禁互依赖（编译期阻断），跨 crate 边仅 commands_* → aidog_core（单向）。当前 5 条跨 command 域边按如下处理：
   - `group`/`proxy` → `sync_settings`：commands-platform / commands-proxy 依赖 commands-config？**禁**（commands 间禁互依赖）→ 把 `do_sync_group_settings` / `try_sync_settings` 下沉到 **aidog_core**（它是 platform/proxy/config 共用的业务逻辑，本就属核心）。commands-config 仅做 `#[tauri::command]` 薄壳调 core。
   - `platform`/`proxy` → `tray_render::refresh_tray_menu`：refresh 触发是 UI 状态变更通知 → **下沉到 aidog_core（C2 done）+ Tauri event 解耦 concrete impl 依赖（cmd-proxy 落地，2026-07-10）**。`refresh_tray_menu` fn 已在 aidog_core，但其 concrete impl `TrayMenuBuildImpl` 留 `commands_platform::tray`（C3 临时居所，C8 才迁 commands_tray）。proxy 域迁 commands_proxy 后若直调 `refresh_tray_menu(&app, &commands_platform::tray::TrayMenuBuildImpl)` → commands_proxy→commands_platform 跨边违硬规。**解耦方案 A（复用现有 `tray-refresh` 事件，cmd-proxy exec 实证优于新事件）**：
     - **emitter**: `commands_proxy::proxy::proxy_start` / `proxy_stop` 成功后 `app.emit("tray-refresh", ())`（**复用现有事件**，非新事件 —— 同域 precedent：`aidog_core::gateway::proxy::log::emit_tray_events` proxy 日志路径已 emit `tray-refresh` 刷托盘，同语义「proxy 状态变更 → 刷托盘」）
     - **listener**: **零新代码**，复用 `app_setup.rs:391-398` 现有 `app.listen("tray-refresh", ...)` listener（已调 `refresh_tray_menu(&handle, &commands_platform::tray::TrayMenuBuildImpl)`）—— app crate (crates/aidog) 依赖 commands_platform 合法（binary crate 依赖所有 commands_*）
     - **commands_proxy 零 commands 依赖**（仅 → aidog_core + tauri），违硬规消除
     - **C8 cmd-tray 迁 tray.rs 后**：listener 内 `commands_platform::tray::TrayMenuBuildImpl` 改 `commands_tray::tray::TrayMenuBuildImpl`（app crate dep 改，listener 一行改）
     - **C8 复查清单**：`crates/commands_platform/src/platform.rs:253,276` 仍直调 `refresh_tray_menu(&app, &super::tray::TrayMenuBuildImpl)`（commands_platform 内部 `super::tray::` 解析，无跨 crate 边，C3 临时保留合法）—— C8 cmd-tray 迁 tray.rs 到 commands_tray 时，platform.rs 这两处需同样改 emit `tray-refresh`（同 C4 模式），届时 platform.rs 跨 crate 边才出现
   - `popover` → `tray::tray_layout` + `tray_render` → `tray`：**popover 入 commands-tray crate**（域重划：popover 本就是 tray 派生数据展示），tray_render 留 tray 同包。
   - `hooks` ↔ `sync_settings`：同 commands-config 包内，OK。
   - `middleware` → `mitm::ImportDefaultsResult`：同 commands-proxy 包内，OK。
4. **test 文件**（23 个 test_*.rs）：随源文件入对应 crate 的 `tests/` 或 `src/` 同级 `#[cfg(test)]`。
5. **shared 层**：`shared/` 入 aidog_core。Db / SetSettingInput / RootCa / WhitelistEntry 等公共 struct 入 aidog_core pub API。
6. **build/run/release 不变**：`yarn tauri dev` / `yarn tauri build` / release bundle 走 `crates/aidog` binary crate，workspace 共享 target/ + Cargo.lock。tauri.conf.json 的 `build.beforeBuildCommand` / `build.frontendDist` 不变，仅 binary 路径指 crates/aidog。

## Requirements

### R1 workspace 骨架（C1）

- R1.1 `src-tauri/Cargo.toml` 改为 `[workspace]` root：`members = ["crates/*"]`，`resolver = "2"`，`[workspace.package]`（共享 edition/license/version/authors），`[workspace.dependencies]`（tauri/serde/tokio 等共享依赖集中声明版本，子 crate 引 `{ workspace = true }`）。
- R1.2 `[profile.release]` 上提到 workspace root（共享）。
- R1.3 建 9 crate 目录：`crates/{aidog_core,commands-platform,commands-proxy,commands-config,commands-system,commands-ai-tools,commands-tray,commands-cli-env,aidog}/`，每个含 `Cargo.toml` + `src/lib.rs`（commands_*）或 `src/main.rs`（aidog）/ `src/lib.rs`（aidog_core）。
- R1.4 **空骨架验证门禁**：9 crate 空壳 + workspace root 建完，`cargo build --workspace` + `cargo test --workspace` + `cargo clippy --workspace --all-targets` 全绿（空 crate 不影响）。这条门禁过了才进 R2 迁移。

### R2 aidog_core 提取（C2）

- R2.1 `crates/aidog_core/src/`：移入现有 `src-tauri/src/{gateway,shared,logging,app_setup,deep_link}.rs`（或保留 deep_link/app_setup 在 app crate，按是否含 Tauri 依赖判定）。
- R2.2 aidog_core `Cargo.toml` 声明其依赖（tauri / rusqlite / axum / reqwest / serde 等业务依赖，从原 src-tauri/Cargo.toml 迁）。
- R2.3 aidog_core `lib.rs`：`pub mod gateway; pub mod shared;` + 重导出公共类型（Db / SetSettingInput / RootCa / WhitelistEntry / Protocol 等，grep startup.rs + commands 用到的 `crate::` 类型）。
- R2.4 把"被多 commands 域共用"的业务逻辑下沉到 core（消除 commands 间互依赖）：
  - `sync_settings::{do_sync_group_settings, try_sync_settings}` → core（hooks 的 `generate_hook_scripts` / `enabled_hook_events` 留 commands-config，但若 hooks 也被 core 用则同下沉；grep 确认）。
  - `tray_render::refresh_tray_menu` → core（或经 Tauri event 解耦，二选一，**推荐 core**：最低迁移成本）。
- R2.5 **popover 域重划**：`popover.rs` 入 commands-tray crate（非 commands-platform），因 popover 紧依赖 tray::tray_layout。

### R3 commands 7 crate 迁移（C3-C9，按域）

每 crate：
- R3.X.1 `crates/commands-<domain>/Cargo.toml`：`name = "commands_<domain>"`，`edition = { workspace = true }`，`dependencies = { aidog_core = { path = "../aidog_core" }, tauri = { workspace = true }, ... }`。
- R3.X.2 `crates/commands-<domain>/src/lib.rs`：`pub mod <file>;` 每源文件一个 mod。
- R3.X.3 源文件 `git mv` 从 `src-tauri/src/commands/<file>.rs` → `crates/commands-<domain>/src/<file>.rs`（含对应 test_<file>.rs）。
- R3.X.4 源文件内路径迁移：`crate::commands::X::Y` → `commands_<domain>::Y` 或 `aidog_core::<path>::Y`；`crate::gateway::` → `aidog_core::gateway::`；`crate::shared::` → `aidog_core::shared::`；`crate::Db` → `aidog_core::Db`。
- R3.X.5 `#[tauri::command]` 函数 `pub` 可见性保持（app crate generate_handler 需跨 crate 引）。

7 域文件映射：
- **commands-platform**：platform, group, model_fetch, stats, price, quota（6）+ model_test？（model_test 属 AI 工具，入 ai_tools）
- **commands-proxy**：proxy, proxy_log, proxy_timeout, middleware, mitm（5）
- **commands-config**：settings, sync_settings（薄壳，实调 core）, defaults, hooks（4）
- **commands-system**：about, app_log, auto_update, backup, notification, scheduling, fs_autocomplete（7）
- **commands-ai-tools**：coding_tools, mcp, skills, script_executor, model_test（5）
- **commands-tray**：tray, tray_render, popover（3，popover 域重划）
- **commands-cli-env**：cli_env（1）

### R4 app crate wiring（C10）

- R4.1 `crates/aidog/Cargo.toml`：`name = "aidog"`，binary crate，`dependencies = { commands-platform, commands-proxy, ..., aidog_core, tauri }`。
- R4.2 `crates/aidog/src/main.rs`：调 `aidog::run()` 或直接 `aidog_core::run()`（看 run 函数落点）。
- R4.3 `crates/aidog/src/lib.rs` 或 startup：`mod startup;` + `pub use startup::run`。
- R4.4 `startup.rs` 的 `generate_handler![...]`：186 处 `crate::commands::X::cmd_fn` → `commands_<domain>::cmd_fn`（domain 按文件归属查表）。
- R4.5 `tauri.conf.json`：`build.beforeBuildCommand` 不变；binary 路径若 Tauri 自动探测则无需改，否则显式指 `crates/aidog`。
- R4.6 `build.rs`：`tauri_build::build()`，放 workspace root 或 crates/aidog/（Tauri 2.0 build.rs 属 binary crate）。

### R5 Tauri 集成验证（C10）

- R5.1 `cargo build --workspace` 全绿。
- R5.2 `cargo test --workspace` 全绿（1348 测试基线，参 deps-upgrade 后）。
- R5.3 `cargo clippy --workspace --all-targets` 无新 warning（baseline 124 全为 pre-existing style）。
- R5.4 `yarn tauri dev` 启动冒烟（应用能起、平台列表/代理基本功能）。
- R5.5 `yarn tauri build` 出 bundle（macOS .app/.dmg）成功。
- R5.6 主仓零改动（worktree 内）。

### R6 测试

- R6.1 23 个 test_*.rs 随源入对应 crate，`#[cfg(test)]` 模块路径同步迁移。
- R6.2 test_harness.rs（commands.rs 内引用）按需保留 aidog_core 或 app crate。
- R6.3 跨 crate 测试：commands crate 的测试仅依赖 aidog_core + 自身，禁依赖其他 commands crate。

### R7 记录

- R7.1 design.md 记 workspace 拓扑 + crate 依赖图 + 下沉决策。
- R7.2 journal 记迁移踩坑（路径迁移漏点 / Tauri build.rs 位置 / popover 域重划理由）。

## Acceptance Criteria

- [ ] workspace 9 crate 骨架建完，空骨架门禁（R1.4）绿
- [ ] aidog_core 提取完成，gateway/shared/models + 下沉业务逻辑全入 core
- [ ] 7 commands crate 各含其域文件，源文件路径迁移全完成
- [ ] app crate generate_handler 186 处跨 crate 路径改对
- [ ] commands crate 间零互依赖（Cargo.toml 不声明彼此，仅依赖 aidog_core）
- [ ] cargo build + test + clippy --workspace 全绿，无新 warning
- [ ] yarn tauri dev + yarn tauri build 冒烟过
- [ ] 主仓零改动

## Definition of Done

- workspace 多 crate 落地，单 binary 输出不变
- 域边界 = crate 边界，编译期强制（commands 间禁互依赖）
- build/run/release 不受影响
- design.md + journal 记架构决策 + 踩坑

## Technical Notes

- Tauri 2.0 workspace 支持：官方文档 + 社区案例证实，build.rs 属 binary crate，tauri.conf.json 的 binary 探测走 crate name。
- 路径迁移量：~7000 行 commands + startup.rs 186 处 + gateway 内对 commands 的反向引用（grep 确认是否纯单向；若有 gateway 调 commands 需重构或下沉）。
- **关键风险**：gateway 反向依赖 commands？grep `crate::commands::` in gateway/ — 若有，需下沉或重构（commands 应只被 app crate 注册时引用，gateway 不应调 commands）。
- **popover 域重划**：popover 原属 platform 域（UI 数据），但紧依赖 tray → 入 commands-tray。属 brainstorm 决策，非纯机械迁移。
- 依赖冲突：workspace 共享依赖版本必须一致（rusqlite/tokio/serde），[workspace.dependencies] 集中声明。
- child 拆分：parent + 10 child（C1 骨架 + C2 core + C3-C9 七域 + C10 app+wiring+verify）。child 间依赖：C2 dep C1；C3-C9 dep C2；C10 dep C3-C9。
- 与其他 task 冲突：commands/mitm.rs（mitm-tables-to-setting 改内容）、gateway/models（protocols-rust-enum 改 enum）→ depends_on mitm-tables + protocols epic。
