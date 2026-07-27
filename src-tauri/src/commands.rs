//! Tauri command 实现按领域下沉到子模块（lib.rs 仅做 glue + handler 注册）。
//! 纯结构搬移，零行为变更。

// cli_env 下沉 commands_cli_env crate（C9）；
// hooks / sync_settings / tray_render 下沉 aidog_core（C2 core-extract）；
// startup.rs generate_handler 直接用 `aidog_core::hooks::*` / `aidog_core::sync_settings::*`
// 路径调用，不再走 `crate::commands::*` 别名（C3+ 拆 commands-config crate 时再统一）。
// middleware/mitm/proxy/proxy_log/proxy_timeout 下沉 commands_proxy crate（C4）；
// startup.rs generate_handler 直接用 `commands_proxy::*` 路径调用。
// settings/defaults 原下沉 commands_config crate（C5），C3 c3-commands 第 2 批再迁入
// aidog_core，commands_config crate 删除。startup.rs generate_handler 直接用
// `aidog_core::settings::*` / `aidog_core::defaults::*` 路径调用。
// cli_env 原下沉 commands_cli_env crate（C9），同批迁入 aidog_core，crate 删除，
// startup.rs 用 `aidog_core::cli_env::*`。
// cli_proxy provider/platform/import/batch 原下沉 commands_cli_proxy crate，同批迁入
// aidog_core::cli_proxy_cmd，crate 删除，startup.rs 用 `aidog_core::cli_proxy_cmd::*`。
// about/app_log/auto_update/backup/notification/scheduling/fs_autocomplete 原下沉 commands_system
// crate（C6），C3 c3-commands 第 3 批再迁入 aidog_core::system_cmd，crate 删除。startup.rs
// generate_handler 直接用 `aidog_core::system_cmd::*` 路径调用。
// coding_tools/mcp/skills/script_executor/model_test 原下沉 commands_ai_tools crate（C7），
// 同批迁入 aidog_core::ai_tools_cmd，crate 删除。startup.rs generate_handler 直接用
// `aidog_core::ai_tools_cmd::*` 路径调用。
// tray/popover 下沉 commands_tray crate（C8）；C3 c3-commands 第 1 批再迁入 aidog_core，
// commands_tray crate 删除。startup.rs generate_handler 直接用 `aidog_core::popover::*` 路径调用。
// test_harness 删除（C8）：mock_app_with_db 已下沉 aidog_test_util，root 测试已迁完。
