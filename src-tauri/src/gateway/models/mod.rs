//! gateway 数据模型层。
//!
//! 原单文件 `models.rs` 按领域拆分到本目录子模块。所有对外项经下方 `pub use`
//! 重导出，保持 `gateway::models::X` 路径不变（大量调用方依赖该路径）。

// 共享 serde default 辅助：被多个领域结构体引用（serde `default = "default_true"`
// 字符串路径依赖名称在模块作用域可见；各子模块 `use super::default_true;` 引入）。
pub(crate) fn default_true() -> bool {
    true
}

mod group;
mod manual_budget;
mod middleware;
mod model_test;
mod notification;
mod platform;
mod price;
mod protocol;
mod proxy_log;
mod settings;
mod stats;
mod tray;

pub use group::*;
pub use manual_budget::*;
pub use middleware::*;
pub use model_test::*;
pub use notification::*;
pub use platform::*;
pub use price::*;
pub use protocol::*;
pub use proxy_log::*;
pub use settings::*;
pub use stats::*;
pub use tray::*;
