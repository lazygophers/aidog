---
updated: 2026-07-10
rewrite-version: 1
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

## Cross-reference

- parent design：`.trellis/tasks/07-10-commands-restructure/design.md` C1（PoC 门禁 grill G3 风险点）
