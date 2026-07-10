//! commands-platform crate —— platform/group/model_fetch/stats/price/quota 命令（C3 迁入）。
//!
//! 6 域 #[tauri::command] 函数保持 `pub`：root aidog crate startup.rs generate_handler
//! 跨 crate 直引 `commands_platform::<domain>::<fn>`（C10 才挪 app crate）。
//!
//! 铁律：禁依赖其他 commands_* crate（跨域边经 aidog_core）。

pub mod platform;
pub mod group;
pub mod model_fetch;
pub mod stats;
pub mod price;
pub mod quota;
// tray.rs 暂居此 crate（C3 阶段）：platform 域调用 refresh_tray_menu 需注入 TrayMenuBuildImpl
// 实现；tray.rs 与 platform 在 C3 同 crate 内可直调，C8 cmd-tray 阶段再迁 commands-tray。
// root aidog crate（popover/app_setup/proxy）经 commands_platform::tray::* 路径引。
pub mod tray;
