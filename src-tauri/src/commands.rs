//! Tauri command 实现按领域下沉到子模块（lib.rs 仅做 glue + handler 注册）。
//! 纯结构搬移，零行为变更。

pub mod about;
pub mod app_log;
pub mod auto_update;
pub mod backup;
pub mod cli_env;
pub mod coding_tools;
pub mod defaults;
pub mod fs_autocomplete;
pub mod group;
// hooks / sync_settings / tray_render 下沉 aidog_core（C2 core-extract）；
// startup.rs generate_handler 直接用 `aidog_core::hooks::*` / `aidog_core::sync_settings::*`
// 路径调用，不再走 `crate::commands::*` 别名（C3+ 拆 commands-config crate 时再统一）。
pub mod mcp;
pub mod middleware;
pub mod mitm;
pub mod model_fetch;
pub mod model_test;
pub mod notification;
pub mod platform;
pub mod popover;
pub mod price;
pub mod proxy;
pub mod proxy_log;
pub mod proxy_timeout;
pub mod quota;
pub mod scheduling;
pub mod script_executor;
pub mod settings;
pub mod skills;
pub mod stats;
pub mod tray;

#[cfg(test)]
#[path = "commands/test_harness.rs"]
pub(crate) mod test_harness;

// hooks 测试：源文件已下沉 aidog_core，但 test 依赖 root 的 test_harness
// （mock_app_with_db）+ tauri MockRuntime，留 root 测试（C3+ 拆 commands-config 时随 crate 走）。
#[cfg(test)]
#[path = "commands/test_hooks.rs"]
pub(crate) mod test_hooks;
