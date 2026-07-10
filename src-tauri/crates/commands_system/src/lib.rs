//! commands-system crate —— about/app_log/auto_update/backup/notification/scheduling/fs_autocomplete 命令（C6 迁入）。
//!
//! 7 域 #[tauri::command] 函数保持 `pub`：root aidog crate startup.rs generate_handler
//! 跨 crate 直引 `commands_system::<domain>::<fn>`（C10 才挪 app crate）。
//!
//! 铁律：禁依赖其他 commands_* crate（跨域边经 aidog_core）。
//! 测试依赖 aidog_test_util（mock_app_with_db）+ tauri MockRuntime + tempfile（fs_autocomplete tempdir）。
//! 每个源文件尾部 `#[cfg(test)] #[path="test_<X>.rs"] mod test_<X>;` 同 C5 settings/defaults 模式
//! （test 文件随源 git mv 入本 crate src/，相对路径不变）。

pub mod about;
pub mod app_log;
pub mod auto_update;
pub mod backup;
pub mod fs_autocomplete;
pub mod notification;
pub mod scheduling;
