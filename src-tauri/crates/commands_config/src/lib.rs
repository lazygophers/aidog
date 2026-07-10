//! commands-config crate —— settings/defaults 命令（C5 迁入）。
//!
//! 2 域 #[tauri::command] 函数保持 `pub`：root aidog crate startup.rs generate_handler
//! 跨 crate 直引 `commands_config::<domain>::<fn>`（C10 才挪 app crate）。
//!
//! 铁律：禁依赖其他 commands_* crate（跨域边经 aidog_core）。
//! hooks/sync_settings 源文件已 C2 下沉 aidog_core；test_hooks 端到端覆盖随本 crate 走
//! （依赖 aidog_test_util::mock_app_with_db，覆盖 aidog_core::hooks + sync_settings 链路）。

pub mod settings;
pub mod defaults;

// hooks 测试：源文件已下沉 aidog_core，test 依赖 aidog_test_util（mock_app_with_db）
// + tauri MockRuntime，覆盖 aidog_core::hooks + sync_settings 链路。
#[cfg(test)]
#[path = "test_hooks.rs"]
pub(crate) mod test_hooks;
