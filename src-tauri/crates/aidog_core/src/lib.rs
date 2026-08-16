//! aidog core crate —— gateway + shared + models + sync + hooks + tray_refresh + logging
//! （C2 core-extract 提取）。
//!
//! 业务下沉此 crate；commands_* crate（C3+）+ root aidog package 过渡期均依赖此 crate。
//! core 内部 `crate::gateway::` / `crate::shared::` / `crate::logging::` 路径不变；
//! 外部 crate 用 `aidog_core::gateway::...` 或顶层 re-export。
//!
//! 铁律：core 不依赖任何 commands_* crate（禁循环）。

pub mod gateway;
pub mod shared;
pub mod logging { pub use aidog_db::logging::*; }
pub mod sync_settings;
pub mod hooks;
pub mod tray_render;
mod command_macro;
// C3 c3-commands 第 1 批：commands_tray 4 个 popover command 下沉（薄转发，纯搬运）。
pub mod popover;
// C3 c3-commands 第 2 批：commands_cli_env / commands_config / commands_cli_proxy 下沉。
pub mod cli_env;
pub mod settings;
pub mod defaults;
// C3 c3-commands 第 3 批：commands_system / commands_ai_tools 下沉。
pub mod system_cmd;
pub mod ai_tools_cmd;
pub mod platform_cmd;
pub mod proxy_cmd;

// 顶层 re-export：commands 域 / root package / 测试常用类型直引 `aidog_core::<X>`。
pub use gateway::models::*;
pub use aidog_db::Db;
pub use gateway::models::SetSettingInput;
pub use tray_render::{refresh_tray_menu, TrayMenuBuild, TrayLayout, TrayColumn};
