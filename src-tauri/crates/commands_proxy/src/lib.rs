//! commands-proxy crate —— proxy/proxy_log/proxy_timeout/middleware/mitm 命令（C4 迁入）。
//!
//! 5 域 #[tauri::command] 函数保持 `pub`：root aidog crate startup.rs generate_handler
//! 跨 crate 直引 `commands_proxy::<domain>::<fn>`（C10 才挪 app crate）。
//!
//! 铁律：禁依赖其他 commands_* crate（跨域边经 aidog_core）。

pub mod proxy;
pub mod proxy_log;
pub mod proxy_timeout;
pub mod middleware;
pub mod mitm;
