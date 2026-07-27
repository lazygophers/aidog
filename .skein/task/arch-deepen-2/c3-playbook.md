# C3 commands_* → aidog_core 迁移 playbook（试点批 c3-commands，样本 commands_tray）

## 结论

commands_tray（4 cmd：popover_data / popover_config_get / popover_config_set / popover_platform_today）已全部迁入 `aidog_core`，crate 已删。可作为剩余 5 批（commands_cli_env 5 / commands_config 13 / commands_system 31 / commands_ai_tools 32 / commands_platform 48 / commands_proxy 47）的机械复制模板。

## 落地位置选择规则

- 每个 `#[tauri::command]` 按其调用的领域逻辑落到 `aidog_core` 对应模块（本例：tray 相关落 `tray_render.rs`；无归属模块的独立命令族新建同名 `.rs`，本例新建 `popover.rs`）。
- 命令函数体如果不是薄转发（直接调 db/service 后返回），照抄整段逻辑一起搬，不要只搬签名。本批未遇到非薄转发案例。
- **非 `#[tauri::command]` 但跨 crate 被引用的普通函数必须一并搬**：本批 `commands_tray/src/tray.rs` 里 `TrayMenuBuildImpl`（struct + trait impl）和 `build_tray_menu`（pub fn）不是 tauri command，但被 `src-tauri/src/app_setup.rs` 直接 `use commands_tray::tray::{...}` 引用。删 crate 前必须先搬完这类函数，否则根包编译直接失败。**后续批次务必在删 crate 前 grep 该 crate 除 `#[tauri::command]` 外的所有 `pub` 项，确认无遗漏引用点。**

## tauri_command! 宏（tracing 消boilerplate，本批新增）

位置：`crates/aidog_core/src/command_macro.rs`，`#[macro_export]`，`aidog_core::command_macro::tauri_command!`（crate 内用 `crate::tauri_command!`）。

用法：
```rust
crate::tauri_command! {
    pub async fn cmd_name(args...) -> Result<T, String> {
        // 函数体，内部 ? 正常工作
    }
}
```
展开后自动加 `#[tauri::command]` + `#[tracing::instrument(skip_all, fields(trace_id = %crate::logging::new_trace_id()))]`，body 包一层 `(async move { ... }).await`，`Err` 分支自动 `tracing::error!`。原 194 个命令仅 49 个有错误日志的问题，迁移时顺手用此宏即可全覆盖，不必逐个手写 `.map_err(|e| { tracing::error!(...); e })`。

后续批次直接照抄该宏调用形式即可，宏本身不用改。

## Cargo.toml / 注册表编辑步骤

1. 删除 `src-tauri/Cargo.toml` 根 `[dependencies]` 里对应 `commands_xxx = { path = "crates/commands_xxx" }` 行（含注释）。
2. `src-tauri/src/startup.rs` 的 `invoke_handler(tauri::generate_handler![...])` 里，把 `commands_xxx::mod::cmd_name` 替换为 `aidog_core::new_mod::cmd_name`（函数名不变，只换路径 —— 前端 invoke 名不受影响，因为 invoke 名取自 `#[tauri::command]` 函数名而非模块路径）。
3. 若有其它文件（如本例 `app_setup.rs`）直接 `use commands_xxx::...`，同步改成 `use aidog_core::...`。
4. `rm -rf src-tauri/crates/commands_xxx`。
5. 更新 `src-tauri/src/commands.rs` 顶部注释表（记录哪批下沉到哪里），保持文档同步。

## 已知债务（本批不处理，留给后续专项）

`aidog_core/Cargo.toml` 的 `tauri = { workspace = true }` 是**非 optional、非 feature-gated 的硬依赖**，`tauri::` 已在 11 个文件内使用（约 41 处）：
`hooks.rs`, `sync_settings.rs`, `tray_render.rs`, `shared.rs`, `gateway/proxy/log.rs`, `gateway/proxy/test_connect.rs`, `gateway/proxy/mod.rs`, `gateway/notification/tts.rs`, `gateway/notification/dispatch.rs`, `gateway/codex.rs`, `gateway/backup/scheduler.rs`。

原计划要求「tauri 做成 optional + feature 门禁 + `cargo test -p aidog_core --no-default-features` 通过」，经与 team-lead/用户确认，本批（及全部后续 6 批）**明确排除**该项 —— 这是先于本批就存在的基线状态，非本批引入，改造范围过大需独立立项。后续批次搬命令时**不要**顺手碰 `Cargo.toml` 的 optional/feature 部分。

## 循环测试依赖坑

`aidog_test_util` 依赖 `aidog_core`（path dep），所以 `aidog_core` 不能反向 dev-dep `aidog_test_util`。原命令若有基于 `aidog_test_util::mock_app_with_db`（构造 `tauri::State`/`AppHandle`）的测试，搬进 `aidog_core` 后这类测试**编译不过**（循环依赖）。

workaround（本批 `popover.rs::test_popover` 已验证）：改测底层 `db::` 函数而非 `#[tauri::command]` 包装层，用 `aidog_core::gateway::db::test_support::test_db()` 构造裸内存 `Db`（`Db::new(":memory:")` + `init_tables()`），绕开 tauri State 机制直接测业务逻辑。后续批次遇到同类测试，照此模式改写。

## 验收结果（本批实测）

- `cargo check --workspace --all-targets`：0 error（23 warning，均为迁移前既存的 ts-rs serde 解析警告 + `block` crate future-incompat，与本批无关）
- `cargo clippy --workspace --all-targets`：0 warning（同上，仅既存 ts-rs/block 提示，无 clippy lint）
- `cargo test -p aidog_core`：1518 passed / 1 failed / 4 ignored（基线 1517 passed/1 failed；唯一失败 `gateway::quota::http::test_http::quota_get_json_network_error` 是环境依赖的已知基线失败，非本批引入，未劣化，反而净增 1 pass 即 `popover::test_popover::config_roundtrip_and_today`）
- 4 个 invoke 名逐字未变：`src/services/api/tray.ts` 的 `popover_config_get`/`popover_config_set`/`popover_platform_today`，`src/popover.tsx:105` 的 `popover_data`（直调非走 api 封装层）均确认字符串未改。
