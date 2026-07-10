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
pub mod logging;
pub mod sync_settings;
pub mod hooks;
pub mod tray_render;

// 顶层 re-export：commands 域 / root package / 测试常用类型直引 `aidog_core::<X>`。
pub use gateway::models::*;
pub use gateway::db::Db;
pub use gateway::models::SetSettingInput;
pub use tray_render::{refresh_tray_menu, TrayMenuBuild, TrayLayout, TrayColumn};
