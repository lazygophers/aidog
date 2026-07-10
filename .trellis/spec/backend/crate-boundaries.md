---
updated: 2026-07-10
rewrite-version: 2
authored-by: trellisx-spec
mode: sediment
---

# Workspace Crate 边界契约

何时被读: commands_* crate 内改源码 / 迁移 command 文件入 crate / 跨 crate 调用刷新或共用逻辑时（commands-restructure C3-C10 全程 + 后续任何 crate 边界改动）
谁读: trellis-implement sub-agent / main
不遵守的代价: commands crate 间隐性依赖 → 编译期不阻断跨域耦合 → 域边界失守回到「单 crate 任意可达」旧态 / UI 刷新跨 crate 直调 → 迁移即违硬规 build 红 / 重复造 event listener → 同域信号语义割裂

---

## 范式 (MUST，稳态边界规则，与 cargo-workspace.md 重构过程契约互补)

workspace 拓扑（commands-restructure 落地后）：`crates/{aidog_core, commands_*, aidog}`。边界规则 5 条，缺一致命。

### 1. commands_* crate 间禁互依赖 (MUST，编译期阻断)

- commands_* crate 的 `Cargo.toml` `[dependencies]` **禁加任何其他 commands_* crate**
- 跨 crate 边仅 `commands_* → aidog_core`（单向）；aidog_core 禁依赖任何 commands_*
- 违反后果：编译期不阻断 → commands 间隐性耦合 → 域边界失守（回到单 crate 任意可达旧态）
- 验收: `grep -rn 'commands_platform\|commands_proxy\|commands_config\|commands_system\|commands_ai_tools\|commands_tray\|commands_cli_env' crates/commands_*/Cargo.toml` MUST 0 命中（[dependencies] 段）；`grep -rn 'commands_*' crates/commands_<X>/src/` 代码引用 MUST 0（注释解释性提及 OK 但 grep 时记录区分）

### 2. UI 刷新跨 crate 直调禁 → Tauri event emit 解耦 (MUST)

- 跨 crate 边的 UI 刷新触发（如 `refresh_tray_menu`、托盘重绘、platform/proxy 状态变更通知）**禁直调** concrete impl（concrete impl 留域 crate，跨域直调即违规则 1）
- **解耦模式 (MUST)**: emitter 在 commands_* crate 内 `app.emit("<event>", ())`；listener 在 **app crate**（`crates/aidog/`，binary crate 依赖所有 commands_* 合法）注册 `app.listen("<event>", ...)` → 调 concrete impl
- app crate 是唯一可依赖所有 commands_* 的 crate（binary wiring 层），listener 内 `commands_<域>::<Impl>` 引用合法
- 违反后果：commands_X 迁入 crate 后 build 红（unresolved concrete impl）或违规则 1（加 commands dep）
- 验收: 跨 crate 调 concrete impl 的 `refresh_*` / `notify_*` 函数 grep 0；emitter `app.emit` + app crate listener `app.listen` 配对齐全

### 3. 复用现有 event 优先 (MUST，DRY + 同域一致)

- 新增跨 crate 刷新触发时 **MUST 先 grep 现有 event**（如 `tray-refresh`），同语义（同信号：「X 状态变更 → 刷 Y」）则复用，**禁新建重复事件**
- 新建事件仅当：现有 event 语义不重合 + payload 需差异化 + listener 落点不同
- 违反后果：同域信号语义割裂 → N 个 listener 监听 N 个事件表达同一意图 → 维护成本翻倍
- 先例: `tray-refresh` event（`app_setup.rs` listener + `aidog_core::gateway::proxy::log::emit_tray_events` proxy 日志路径 + `commands_proxy::proxy::proxy_start/stop` 全复用，同语义「proxy 状态变更 → 刷托盘」）
- 验收: 新 emitter 前 grep `app.emit\|emit_` 同域现有事件；事件命名对齐既有模式（`aidog-<域>-changed` / `<域>-refresh`）

### 4. 跨 command 域共用业务逻辑下沉 aidog_core (MUST)

- 跨 ≥2 commands_* 域共用的业务逻辑（如 `do_sync_group_settings` 被 platform/proxy/config 共用 / `refresh_tray_menu` fn 被 platform/proxy 共用）**MUST 下沉 aidog_core**（fn 级下沉，非 concrete impl）
- concrete impl（域专属类型如 `TrayMenuBuildImpl`）留域 crate，app crate listener 引用
- commands_* 仅做 `#[tauri::command]` 薄壳调 core fn
- 违反后果：共用逻辑散布多 crate → 修一处漏同步 / commands 间隐性耦合（复制粘贴）
- 先例: `aidog_core::sync_settings::{do_sync_group_settings, try_sync_settings}` 下沉（commands-platform/proxy/config 共用，C2 core-extract）；`aidog_core::tray_render::refresh_tray_menu` fn 下沉（concrete `TrayMenuBuildImpl` 留域 crate）
- 验收: grep 跨 commands_* 共用 fn → MUST 在 aidog_core；commands_* 内仅薄壳调 core

### 5. workspace 结构迁移完整 (MUST)

- 迁 command 文件入 crate 时 MUST 三件齐: ① 源文件 `git mv` 入 `crates/commands_<X>/src/`（含 test_*.rs）② `crates/commands_<X>/{Cargo.toml,src/lib.rs}` 填齐（pub mod + deps 对齐同域先例）③ root `src-tauri/Cargo.toml` `[dependencies]` 加 `commands_<X> = { path = "crates/commands_<X>" }`
- 路径迁移: 源文件内 `crate::commands::<Y>::` → `commands_<X>::`（跨 crate）/ `aidog_core::`（core 下沉的）；`crate::gateway::` → `aidog_core::gateway::`；测试 `use crate::commands::test_harness::*` → `use aidog_test_util::*`
- `src/commands.rs` 删迁出域的 `pub mod <Y>;`；`src/startup.rs` generate_handler 迁出域 `crate::commands::<Y>::` → `commands_<X>::`
- 迁后 root 死代码清理: root `src/commands/test_harness.rs` 内仅被已迁 caller 用的 mock fn 成死代码 → 删（clippy never_used）。**多 crate 共享 mock 需查全 caller**：仅当本 task 迁走**所有** caller 后才删（如 `mock_app_with_db` 被 C6/C7/C8 三批 test 用，C6 迁 5 个后仍有 C7/C8 3 caller → C6 不删，C8 迁最后 caller 后才删）
- 违反后果: build 红（mod 漏 / dep 漏 / 路径漏改）或死代码积累
- 验收: `grep crate::commands::<迁出域>` in `src/` MUST 0 残留；`cargo build/test/clippy --workspace` 全绿（clippy warning 数 = baseline，0 new）

#### 5a. env! / include_bytes! / include_str! 编译期注入独立 build.rs (MUST)

- 含 `env!("VAR")` / `include_bytes!` / 编译期注入的源文件迁 commands_* crate 时，**env! 不跨 crate 传递**（root build.rs 注入的 env 变量仅 root crate 可见）→ 本 crate **MUST 独立 build.rs** 重新注入
- 独立 build.rs 规格：同 root build.rs 注入逻辑（如 `git rev-parse HEAD` + `SystemTime` epoch 秒 + 失败回退 "unknown"）；**lib crate 省略 `tauri_build::build()`**（仅 binary/app crate 需要）；`rerun-if-changed` 指向 root `.git/HEAD` 相对路径（`crates/<X>/` → `../../../.git/HEAD`）
- 违反后果: env! 变量本 crate 读到 "unknown"/空 → about/version 命令返回错误信息（无 `private_interfaces` lint 警告，运行期静默错误）
- 验收: `cargo build` 时 env! 变量实际注入 —— 跑 `cargo test` 专用 test 或运行期验命令返回值非 "unknown"/非空；`grep 'env!("' crates/commands_<X>/src/` 命中 → 该 crate MUST 有 build.rs 注入对应变量
- 先例: task 07-10-cmd-system（`about.rs` 的 `env!("AIDOG_GIT_COMMIT")` / `env!("AIDOG_BUILD_TIME")` → `crates/commands_system/build.rs` 独立注入，rerun-if-changed `../../../.git/HEAD`，省略 tauri_build::build()）

#### 5b. pub 可见性放宽（cross-crate 注册类型 / helper）(MUST)

- `#[tauri::command]` 的**返回类型** MUST ≥ fn 可见度（`private_interfaces` lint 捕获不足，返回 struct 若 `pub(crate)` 而 command fn `pub` → lint 报警）→ 迁 crate 后这些类型 MUST `pub(crate) → pub`
- 被 root/他 crate 直接调用的 helper fn/struct（非 #[tauri::command]，如 `app_setup.rs` startup 期调的迁移/加载函数）MUST `pub(crate) → pub`（cross-crate caller 需公开可见度）
- **私有不必要泄露禁放宽**：域内 helper（仅本 crate 内部 mod 间调用）保持 `pub(crate)`，无 cross-crate caller 禁改 pub
- 违反后果: build 红（`private type in public interface`）或 cross-crate caller `unresolved`；过度放宽 → 域封装泄露
- 验收: `grep 'pub(crate).*fn\|pub(crate).*struct' crates/commands_<X>/src/` 每项核有无 cross-crate caller（root app_setup.rs / startup.rs generate_handler）—— 有则 pub，无则保持 pub(crate)；`cargo build` 零 `private_interfaces` warning
- 先例: task 07-10-cmd-system（`AboutInfo` / `PathEntry` 类型 + `app_log::load_app_log_settings_from_db` / `migrate_log_settings_file_to_db` helper pub 放宽 3 处，均 cross-crate caller 或 lint 必要；域内 `expand_path` 等 helper 保持 pub(crate)）

## C8 复查清单模式 (MUST，迁移期临时合法 → 后续 task 改)

- 迁 command 文件时若发现 **同 crate 内部** 跨域直调（如 `commands_platform::platform` 调 `super::tray::TrayMenuBuildImpl`，tray.rs 尚未迁 commands_tray）→ **临时合法**（同 crate 内 `super::` 解析无跨 crate 边），标 C8 复查清单
- 后续 task（C8 cmd-tray）迁 tray.rs 入 commands_tray 时，原同 crate 内调用即变跨 crate 边 → 此时 MUST 改 event emit 解耦（对齐规则 2）
- 禁在当前 task 提前改（scope 边界）；禁遗漏复查（迁移期临时合法 = 后续 task 必改项）
- 先例: `crates/commands_platform/src/platform.rs:253,276` 直调 `refresh_tray_menu(&app, &super::tray::TrayMenuBuildImpl)` —— C3 cmd-platform 时临时合法（tray.rs 在 commands_platform 内），C8 cmd-tray 迁 tray.rs 时改 emit `tray-refresh`（同 C4 cmd-proxy 模式）

## 验收断言（可复用）

```bash
# 规则 1: commands_* 间零互依赖
grep -rn 'commands_platform\|commands_proxy\|commands_config\|commands_system\|commands_ai_tools\|commands_tray\|commands_cli_env' crates/commands_*/Cargo.toml
# [dependencies] 段 0 命中（dev-dependencies 同禁）

# 规则 1: commands_<X>/src/ 零 commands_* 代码引用
grep -rn 'commands_platform\|commands_proxy\|commands_config\|commands_system\|commands_ai_tools\|commands_tray\|commands_cli_env' crates/commands_<X>/src/
# 0 代码依赖（注释 OK）

# 规则 2: event emit + listener 配对
grep -rn 'app.emit\|emit_' crates/commands_*/src/  # emitter
grep -rn 'app.listen\|listen_' src/app_setup.rs    # listener（app crate）

# 规则 5: 路径迁移完整
grep -rn 'crate::commands::proxy\|crate::commands::config\|...' src/startup.rs  # 迁出域 0 残留
grep -rn 'pub mod proxy\|pub mod config\|...' src/commands.rs                   # 迁出域 mod 删

# workspace 全绿
cargo build --workspace && cargo test --workspace && cargo clippy --workspace --all-targets
# build 0 errors / test 全绿 / clippy warning 数 = baseline（0 new）
```

## 实例

- task 07-10-cmd-proxy（C4 commands-proxy crate 落地）: 5 源文件（proxy/proxy_log/proxy_timeout/middleware/mitm）迁入 + startup.rs 47 处路径改 + Cargo.toml dep 加 + 跨 crate 边发现（proxy_start/stop 调 refresh_tray_menu 的 concrete impl TrayMenuBuildImpl 在 commands_platform）→ 方案 A 解耦（复用现有 `tray-refresh` event + app_setup.rs:391-398 现有 listener，零新代码，同域 precedent aidog_core::gateway::proxy::log::emit_tray_events）
- task 07-10-cmd-platform（C3 commands-platform crate 落地）: 埋点 `platform.rs:253,276` 同 crate 内直调 refresh_tray_menu（super::tray::TrayMenuBuildImpl）临时合法，C8 复查清单
- task 07-10-core-extract（C2 aidog_core 提取）: 规则 4 下沉 `do_sync_group_settings` / `try_sync_settings` / `refresh_tray_menu` fn 到 aidog_core（跨 platform/proxy/config 共用），concrete impl 留域 crate

## Cross-reference

- workspace 重构过程契约（PoC 骨架门禁 + 核心提取下沉防循环范式）: [Cargo Workspace 重构门禁](./cargo-workspace.md)
- 跨 Rust↔TS 公共契约层零改: [Cross-Layer Rules](../guides/cross-layer-rules.md)
- commands-restructure parent prd 决策 3（跨 crate 依赖处理 + 方案 A event 解耦）: `.trellis/tasks/07-10-commands-restructure/prd.md` L46-51
