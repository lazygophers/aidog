---
updated: 2026-07-10
rewrite-version: 2
authored-by: trellisx-spec
mode: sediment
---

# Cargo Workspace 重构门禁

何时被读: 单 crate → cargo workspace 多 crate 重构时（如 commands 按域分包、core 提取）
谁读: trellis-implement sub-agent / main
不遵守的代价: Tauri build.rs/tauri.conf.json workspace path 解析炸 → 全量迁移完才发现，回滚成本高；workspace 依赖版本冲突；binary 同名冲突

---

## PoC 空骨架门禁 (MUST)

单 crate → workspace 多 crate 重构 **MUST 先建空骨架 PoC 门禁**，过才放行全量迁移（业务代码移入子 crate）。禁跳过 PoC 直接全量迁移。

- **空骨架内容**：`[workspace] members=["crates/*"] resolver="2"` + `[workspace.package]`（共享 edition/version/authors/license）+ `[workspace.dependencies]`（共享依赖版本集中声明）+ `[profile.release]` 上提 workspace root；N 空 crate 目录（`Cargo.toml` + 空 `src/lib.rs`，业务代码零引入）
- **现 root package 过渡保留**：原单 crate 的 `[package]`/`[lib]`/`[dependencies]` 全树原样保留（业务代码 C2+ 才移），workspace members 仅加 `crates/*` 空壳
- **binary crate 同名延后**：若目标 binary crate name 与现 root package 同名（如 root `aidog` + 拟建 `crates/aidog`），binary crate **MUST 延后**到 app-wiring 阶段建（root package 迁出后），避免 workspace 内同名冲突

## workspace.dependencies 版本对齐 (MUST)

- `[workspace.dependencies]` 版本号 + features **MUST 逐项照抄**现 root `[dependencies]`，禁版本漂移
- 子 crate 引用 `{ dependency = { workspace = true } }`，禁子 crate 自定版本（冲突）
- 平台特异依赖（如 macOS-only objc2-app-kit）若仅 root 用，可不进 `[workspace.dependencies]`（子 crate 未涉及）

## PoC 门禁验收 (MUST，全量迁移前必过)

1. `cargo build --workspace`：0 errors（含现 root crate + N 空壳 + 依赖全绿）
2. `cargo test --workspace`：baseline 不回归（如 1348 passed）
3. `cargo clippy --workspace --all-targets`：无新 warning（baseline 持平）
4. **Tauri / build.rs path 解析（核心风险点）**：build.rs 在 workspace 声明下成功执行（cdylib/staticlib/rlib 产物落在 `target/debug/`）= workspace path 解析不炸。证据链：build.rs + tauri.conf.json `git diff=0` + cdylib 产物存在 + `cargo build --workspace` 触发 build.rs 成功执行

## GUI 冒烟降级（worktree 无 display 时）

worktree 无 `node_modules` / 无 display 无法跑 `yarn tauri dev` 全链路时，**降级证据链充分可放行**（G3 核心「workspace path 解析」已被 build.rs 执行覆盖）：
- build.rs + tauri.conf.json 文件零改动
- `cargo build --workspace` 触发 build.rs 成功执行（否则 cdylib 不产出）
- cdylib/staticlib/rlib/.rmeta 产物落在 `target/debug/`

**主仓 post-merge 验**：merge 后在主仓（有 node_modules + display）跑一次 `yarn tauri dev` 确认窗口能起，作最终闭环。若炸开补 task。

## 子 crate 规范 (MUST)

- `name` 用下划线（`commands_platform` 等，非 hyphen；目录名连字符是 Cargo 惯例，`name=` 字段下划线）
- `edition = { workspace = true }` / `version = { workspace = true }`（非自定）
- 空 `[dependencies]`（业务依赖 C2+ 按需填实）
- `src/lib.rs` 仅 `//! <域> crate` 文档注释 + 空体，禁引业务代码

## 验收断言（可复用）

```bash
# baseline 不回归
cargo test --workspace --lib | grep -E 'passed|failed'  # passed >= baseline

# workspace.dependencies 版本无漂移（与原 root [dependencies] 逐项比对）
grep -A100 '\[workspace.dependencies\]' src-tauri/Cargo.toml  # 版本逐项核对

# N 空 crate 编译独立绿
cargo build -p <crate>  # 各 crate 0 errors

# build.rs / tauri.conf.json 零改动（PoC 阶段）
git diff src-tauri/build.rs src-tauri/tauri.conf.json | wc -l  # 0 或仅空行

# cdylib 产物在（Tauri path 解析通过证据）
ls src-tauri/target/debug/libaidog_lib.{dylib,a,rlib}  # 存在
```

## 实例

task 07-10-ws-skeleton（commands-restructure C1）：src-tauri 单 crate → workspace，9 空 crate（aidog_core + 7 commands_* + aidog_test_util），crates/aidog binary 延后 C10（避免与 root aidog 同名）。PoC 门禁全绿，1348 baseline 不回归，Tauri 冒烟降级证据链充分，主仓 post-merge 验。

task 07-10-core-extract（commands-restructure C2）：gateway/shared/logging/sync/hooks/tray_render 整 `git mv` 入 aidog_core，root 加 `aidog_core = { path = "crates/aidog_core" }` 过渡依赖 + 37 文件 `crate::gateway::` → `aidog_core::gateway::` 路径迁移（`crate::commands::` 不动，C3+ 才移）。下沉决策（互耦 sync↔hooks 同移 / refresh_tray_menu trait 桥接 / logging 随依赖链下沉）。1353 passed baseline 不回归，124 clippy warnings 持平，Tauri 冒烟降级证据链充分，主仓 post-merge 验。

## 核心提取下沉防循环范式 (MUST)

PoC 空骨架过门后，业务代码入 `aidog_core` 时**MUST** 据依赖关系分类下沉，防 core→commands / core→root 反向依赖循环。禁盲目整目录平移。

- **互耦模块同移 (MUST)**：互相调用的模块（如 `sync_settings` ↔ `hooks`：sync 用 hooks::generate_hook_scripts，hooks 用 sync::do_sync_group_settings）**MUST 同移 core**。只移其一 → 另一留在 commands crate 反向调 core = 循环依赖（design grill G1 捕获）。grep 双向调用链确认互耦 → 同移。
- **trait 桥接防反向依赖 (MUST)**：函数 A（core 域）调函数 B（UI/commands 域），A 移 core 时 B 不能留在 root 反向调（循环）。解：B 的数据类型 + trait 下沉 core，A 调 trait method；B 的实现留 commands crate impl trait。实例：`refresh_tray_menu` 移 core，调 `build_tray_menu`（UI 构造）；`TrayMenuBuild` trait + `TrayLayout/TrayColumn` 数据类型下沉 core，`refresh_tray_menu(&app, &impl TrayMenuBuild)`；`TrayMenuBuildImpl` 实现 + `build_tray_menu/tray_layout` UI 函数留 root（C8 才迁 commands-tray）。
- **共享基建随依赖链下沉 (MUST)**：gateway 等核心域深依赖的共享基建（logging `new_trace_id`/`spawn_traced`/`current_trace_id` 注 `#[tracing::instrument]`）**MUST 随核心域同移 core**。不下沉 → core 引 `crate::logging` 命中 root = 反向依赖循环。grep 确认依赖链：核心域 `crate::<基建>::` 命中 ≥3 处 → 随核心域下沉。
- **过渡期 test_support 去 `#[cfg(test)]` gate (MUST)**：root 测试跨 crate 引 `aidog_core::gateway::db::test_support::*`，但 `#[cfg(test)]` **仅对当前 crate 生效不跨 crate** → core 的 test_support 模块**MUST 去 cfg gate** 始终 `pub`，否则 root 测试编译失败。
  - **代价**：test_support 编入 release binary（dead code elimination 处理，几 KB 膨胀可忽略，无运行时副作用——ENV_LOCK/HomeGuard 仅显式调触变）
  - **回归路径 (MUST)**：C3+ 抽 `aidog_test_util` crate 后，test_support 迁入 + 回归 `#[cfg(test)]` / feature gate + 关联 dep（tempfile 等）回 dev-deps
  - **关联 dep**：test_support 字段类型（如 `HomeGuard { dir: tempfile::TempDir }`）非 cfg(test) gated → tempfile 需升 core 主依赖（原 dev-deps）；回归 feature gate 时 tempfile 同回 dev-deps

## root 过渡路径迁移 (MUST)

core 提取后 root package **过渡保留**（binary crate C10 才建），加 `aidog_core = { path = "crates/aidog_core" }` 依赖 + 路径引用迁移。禁一次性删 root package。

- **迁移正则**：
  - `crate::gateway::<...>` → `aidog_core::gateway::<...>`（root 视角）
  - `crate::shared::<...>` → `aidog_core::shared::<...>`
  - `crate::logging::<...>` → `aidog_core::logging::<...>`（已下沉时）
  - `crate::commands::<file>::<fn>` → **不动**（commands C3+ 才移各 crate，C2 阶段仍在 root）
  - core 内部 `crate::gateway::` / `crate::shared::` → **不动**（core 内部路径）
- **root `[package]`/`[lib]`/commands 业务代码不动**（仅路径引用改 + 加 aidog_core dep）
- **generate_handler 路径**：root `startup.rs` 内 `aidog_core::hooks::*` / `aidog_core::sync_settings::*` 全路径（禁别名 re-export `pub use aidog_core::{hooks}`——双路径并存徒增 grep 噪声，C3+ 拆 crate 再多一份迁移）

## 验收断言（核心提取，可复用）

```bash
# 路径迁移彻底（root 残留核心域路径 = 漏改）
grep -rn 'crate::gateway::\|crate::shared::\|crate::logging::' src-tauri/src/  # 0
grep -rln 'aidog_core::' src-tauri/src/  # root 引用文件清单
ls src-tauri/src/gateway src-tauri/src/shared.rs  # No such file（已移 core）

# core crate 填实
grep 'pub mod' crates/aidog_core/src/lib.rs  # gateway/shared/logging/sync/hooks/tray_render + re-export

# commands_* 保持空壳（C3+ 才填）
wc -l crates/commands_*/src/lib.rs  # 各 3 行（文档注释 + 空体）

# workspace.dependencies 无漂移（仅平台特异 / 新增 dep，无既有版本改）
git diff master -- src-tauri/Cargo.toml | grep -E '^\+.*workspace = true'  # 新增项

# baseline 不回归
cargo test --workspace --lib | grep -E 'passed|failed'  # >= baseline
cargo clippy --workspace --all-targets 2>&1 | grep warning | wc -l  # <= baseline

# GUI 冒烟降级证据链
git diff <base> -- src-tauri/build.rs src-tauri/tauri.conf.json | wc -l  # 0
ls target/debug/libaidog_lib.dylib  # 存在（build.rs workspace path 解析通过）
```

## Cross-reference

- parent design：`.trellis/tasks/07-10-commands-restructure/design.md` C1（PoC 门禁 grill G3）+ C2（下沉决策表 grill G1/G6）
