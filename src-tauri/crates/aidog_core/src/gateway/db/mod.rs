//! 协议转换/领域 DB 层：Db 与连接设施已拆至 `aidog_db` crate（2026-08-16 logs-stats-crates），
//! 本模块保留领域读写（platform/group/schema/maintenance 等）并 re-export 保持
//! `crate::gateway::db::X` 调用路径不变。日志读写在 `aidog_logs`，统计在 `aidog_stats`。

use aidog_db::models::*;
use serde::Serialize;

pub use aidog_db::{Db, DbCache, now, settings::*};
pub use aidog_db::schema::*;
pub(crate) use aidog_db::{retention_cutoff, retention_cutoff_secs};
pub use aidog_logs::*;
pub use aidog_stats::*;
pub use aidog_stats::DbInitTables;
pub use aidog_db::{CURRENT_DB_CTX, DbCallCtx, fmt_caller, READ_POOL_SIZE};
pub use aidog_db::test_support;

// ─── 领域子模块（按 concern 拆分，纯结构搬移，行为零变更）───
mod platform;
mod platform_lifecycle;
mod group;
mod group_platform;
mod middleware;
mod maintenance;
mod model_price;
mod mcp;
mod cli_proxy;
mod ui_extra;
#[cfg(test)]
mod test_ui_extra;

// 对外 re-export：保持 `gateway::db::X` 调用路径不变（外部代码无需改）。
// pub use 按各项自身可见性导出（pub → pub，pub(crate) → pub(crate)），
// 故跨子模块 `use super::*` 也能拿到 pub(crate) 共享 helper。
pub use platform::*;
pub use platform_lifecycle::*;
pub use group::*;
pub use group_platform::*;
pub use middleware::*;
pub use maintenance::*;
pub use model_price::*;
pub use mcp::*;
pub use cli_proxy::*;
pub use ui_extra::*;

// 测试模块：test_<源文件名> 1:1 命名，每个源文件 X.rs 的测试只在 test_X.rs（同目录）。
// 因 db/ 为扁平目录，所有子模块声明须由父模块 mod.rs 持有（test_X.rs 是 db 的兄弟文件，
// 非 X 的子目录文件，无法挂在 X.rs 名下）。test_support 持共享夹具（test_db / sample_* 等）。
//
// C2 core-extract：test_support 由 root package 测试跨 crate 引（`aidog_core::gateway::db::
// test_support::*`），`#[cfg(test)]` 仅对当前 crate 生效不跨 crate，故去 cfg gate 始终 pub。
// 编译进 release 的代价 = 几个 helper fn（test_db / sample_*），无运行时副作用，可接受。
// C3+ aidog_test_util crate 抽出后由 dev-deps / feature gate 控制可见性。
#[cfg(test)]
mod test_mod;
#[cfg(test)]
mod test_trace;
#[cfg(test)]
mod test_model_price;
#[cfg(test)]
mod test_group;
#[cfg(test)]
mod test_group_platform;
#[cfg(test)]
mod test_platform;
#[cfg(test)]
mod test_platform_lifecycle;
#[cfg(test)]
mod test_middleware;
#[cfg(test)]
mod test_maintenance;
#[cfg(test)]
#[cfg(test)]
mod test_mcp;
#[cfg(test)]
mod test_cli_proxy;
#[cfg(test)]
mod test_rw_pool;

