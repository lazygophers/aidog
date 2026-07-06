//! Tauri command 实现按领域下沉到子模块（lib.rs 仅做 glue + handler 注册）。
//! 纯结构搬移，零行为变更。

pub mod about;
pub mod app_log;
pub mod auto_update;
pub mod backup;
pub mod coding_tools;
pub mod defaults;
pub mod fs_autocomplete;
pub mod group;
pub mod hooks;
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
pub mod sync_settings;
pub mod tray;
pub mod tray_render;

#[cfg(test)]
#[path = "commands/test_harness.rs"]
pub(crate) mod test_harness;
