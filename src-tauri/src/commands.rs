//! Tauri command 实现按领域下沉到子模块（lib.rs 仅做 glue + handler 注册）。
//! 纯结构搬移，零行为变更。

pub mod cli_env;
// hooks / sync_settings / tray_render 下沉 aidog_core（C2 core-extract）；
// startup.rs generate_handler 直接用 `aidog_core::hooks::*` / `aidog_core::sync_settings::*`
// 路径调用，不再走 `crate::commands::*` 别名（C3+ 拆 commands-config crate 时再统一）。
// middleware/mitm/proxy/proxy_log/proxy_timeout 下沉 commands_proxy crate（C4）；
// startup.rs generate_handler 直接用 `commands_proxy::*` 路径调用。
// settings/defaults 下沉 commands_config crate（C5）；
// hooks/sync_settings 源已 C2 下沉 aidog_core，test_hooks 随 commands_config crate 走
// （依赖 aidog_test_util::mock_app_with_db）。
// startup.rs generate_handler 直接用 `commands_config::*` 路径调用。
// about/app_log/auto_update/backup/notification/scheduling/fs_autocomplete 下沉 commands_system crate（C6）；
// startup.rs generate_handler 直接用 `commands_system::*` 路径调用。
// coding_tools/mcp/skills/script_executor/model_test 下沉 commands_ai_tools crate（C7）；
// startup.rs generate_handler 直接用 `commands_ai_tools::*` 路径调用。
// tray/popover 下沉 commands_tray crate（C8）；
// startup.rs generate_handler 直接用 `commands_tray::*` 路径调用。
// test_harness 删除（C8）：mock_app_with_db 已下沉 aidog_test_util，root 测试已迁完。
