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
pub mod logging {
    pub use aidog_db::logging::*;
}
mod command_macro;
// 票 07：`tauri_command!` 的 HTTP 形态支撑（参数取值 / 返回值归一 / 错误响应）。
#[cfg(feature = "http")]
pub mod http_command;
// 票 07：宏展开里用的第三方路径经 `$crate::` 转发，这样 aidog_backup / aidog_cli_proxy
// 这类只借宏定义命令的 crate 不必自己再引 axum。
#[cfg(feature = "http")]
#[doc(hidden)]
pub use axum as __axum;
#[cfg(feature = "http")]
#[doc(hidden)]
pub use serde_json as __serde_json;
pub mod hooks;
pub mod sync_settings;
// 票 06：AppCtx 的桌面壳实现（唯一把 AppHandle 接进命令层的地方）。
// 票 08：随 `desktop` feature 一起开关 —— 无界面内核不链 tauri，自带 `HeadlessCtx`。
#[cfg(feature = "desktop")]
pub mod tauri_ctx;
pub mod tray_render;
// 票 08：内核管理面设置（绑定开关 + Bearer 凭据）。与代理的 `bind_lan` 相互独立。
pub mod kernel_settings;
// C3 c3-commands 第 1 批：commands_tray 4 个 popover command 下沉（薄转发，纯搬运）。
pub mod popover;
// C3 c3-commands 第 2 批：commands_cli_env / commands_config / commands_cli_proxy 下沉。
pub mod cli_env;
pub mod defaults;
pub mod settings;
// C3 c3-commands 第 3 批：commands_system / commands_ai_tools 下沉。
pub mod ai_tools_cmd;
pub mod platform_cmd;
pub mod proxy_cmd;
pub mod system_cmd;

// 顶层 re-export：commands 域 / root package / 测试常用类型直引 `aidog_core::<X>`。
pub use aidog_db::Db;
pub use gateway::models::SetSettingInput;
pub use gateway::models::*;
pub use tray_render::{TrayColumn, TrayLayout};
#[cfg(feature = "desktop")]
pub use tray_render::{TrayMenuBuild, refresh_tray_menu};
