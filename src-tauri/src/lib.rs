//! Crate 根：仅模块声明 + 启动入口转发。
//!
//! Tauri command 实现按领域下沉到 `commands/` 子模块；共享 helper / 类型在 `shared`；
//! 启动 Builder + handler 注册在 `startup`，setup 初始化逻辑在 `app_setup`。
//! 纯结构搬移，零行为变更。

mod gateway;
mod logging;
mod shared;
mod commands;
mod app_setup;
mod startup;

// 单元测试（test_coding_tools / commands 内部测试）历史用 `crate::Db` / `crate::SetSettingInput`
// 直引；保留 crate-root 再导出维持路径不变。
#[cfg(test)]
pub(crate) use gateway::db::Db;
#[cfg(test)]
pub(crate) use gateway::models::SetSettingInput;

pub use startup::run;
