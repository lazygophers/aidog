# C1 workspace 骨架 + 空门禁

## Goal
`src-tauri/` 从单 crate 改 cargo workspace：workspace root `Cargo.toml` + 10 空 crate 目录（aidog_core + 7 commands-* + aidog app + aidog_test_util）+ 共享 profile/dependencies。**空骨架门禁先过**（`cargo build/test/clippy --workspace` 全绿 + `yarn tauri dev` 冒烟），才放行 C2 全量迁移。

真值源：parent `07-10-commands-restructure/prd.md` R1 + `design.md` C1。

## Requirements

### R1 workspace root
- R1.1 `src-tauri/Cargo.toml` 改 `[workspace]` root：`members = ["crates/*"]`，`resolver = "2"`，`[workspace.package]`（共享 edition=2024/license/version/authors），`[workspace.dependencies]`（tauri/serde/tokio/rusqlite/axum/reqwest 等共享依赖集中声明版本，子 crate 引 `{ workspace = true }`）。
- R1.2 `[profile.release]` 上提 workspace root（共享）。
- R1.3 现单 crate 内容（src/ 全树）暂**不动**（C2 才移 gateway 入 core）—— 本 child 只建空骨架 + workspace 配置，现 `src-tauri/src/` 仍作为 aidog app crate 的源（或暂留 root 过渡，C10 才搬）。

### R2 10 空 crate 目录
`crates/{aidog_core, commands-platform, commands-proxy, commands-config, commands-system, commands-ai-tools, commands-tray, commands-cli-env, aidog, aidog_test_util}/`，每个含：
- `Cargo.toml`：`name`（commands_* 用下划线）/ `edition = { workspace = true }` / `version = { workspace = true }` / 空 `dependencies`（仅 aidog_core 声明其业务依赖占位，C2 填实）
- `src/lib.rs`（commands_* + aidog_core + aidog_test_util）或 `src/main.rs`（aidog binary）—— 空壳（`//! <域> crate` 注释 + 空 `pub mod` 或空 fn），禁引业务代码

### R3 PoC 门禁（硬门，过才放行 C2）
- R3.1 `cargo build --workspace` 全绿（空 crate 不影响）
- R3.2 `cargo test --workspace` 全绿
- R3.3 `cargo clippy --workspace --all-targets` 无新 warning
- R3.4 **Tauri 冒烟（grill G3 风险点）**：`yarn tauri dev` 能起 —— 验证 build.rs + tauri.conf.json workspace path 解析不炸（binary 探测仍指原 src-tauri/src，未移）
- R3.5 主仓零改动（worktree 内）

## 关键技术约束（design.md）
1. **workspace.dependencies** 共享版本集中声明，子 crate 禁自定版本（冲突）
2. **build.rs + tauri.conf.json**：本 child **不挪** binary crate（C10 才搬 crates/aidog/）—— src-tauri/ 根仍持 build.rs + tauri.conf.json + 现 src/，workspace members 仅加 crates/* 空壳。PoC 门禁验「workspace 声明存在 + 空 crate 编译绿 + Tauri 仍能起」即可，不涉路径迁移
3. **resolver = "2"**（edition 2024 默认，显式声明保险）

## Acceptance
- [ ] workspace root Cargo.toml + [workspace.package]/[workspace.dependencies]/[profile.release] 落地
- [ ] 10 空 crate 目录（Cargo.toml + 空 lib.rs/main.rs）
- [ ] cargo build/test/clippy --workspace 全绿
- [ ] yarn tauri dev 冒烟过（PoC 门禁，grill G3）
- [ ] 主仓零改动

## Dependencies
- 无前置（C1 是 commands-restructure DAG 根）
- 解锁：C2 core-extract（dep C1）

## Out of Scope
- gateway/shared 迁 core（C2）
- commands 源文件迁移（C3-C9）
- app crate wiring + generate_handler 路径改（C10）
- binary crate 挪 crates/aidog/（C10）
