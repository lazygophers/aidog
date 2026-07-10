//! commands-ai-tools crate —— coding_tools/mcp/skills/script_executor/model_test 命令（C7 迁入）。
//!
//! 5 域 #[tauri::command] 函数保持 `pub`：root aidog crate startup.rs generate_handler
//! 跨 crate 直引 `commands_ai_tools::<domain>::<fn>`（C10 才挪 app crate）。
//!
//! 铁律：禁依赖其他 commands_* crate（跨域边经 aidog_core）。
//! 测试依赖 aidog_test_util（mock_app_with_db）+ tauri MockRuntime + tokio。
//! 每个源文件尾部 `#[cfg(test)] #[path="test_<X>.rs"] mod test_<X>;` 同 C5/C6 模式
//! （test 文件随源 git mv 入本 crate src/，相对路径不变）。

pub mod coding_tools;
pub mod mcp;
pub mod skills;
pub mod script_executor;
pub mod model_test;
