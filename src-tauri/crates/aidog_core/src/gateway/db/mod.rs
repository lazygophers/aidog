//! DB 层 shim：全部数据库操作已拆至独立 crate（2026-08-16）——
//! `aidog_db`（连接/schema/models/领域读写/presets）+ `aidog_logs`（proxy_log）+
//! `aidog_stats`（统计）+ `aidog_mcp::store`（mcp_server 表）。本模块仅 re-export 保持
//! `aidog_db::X` 调用路径不变。

pub use aidog_db::*;
pub use aidog_logs::*;
pub use aidog_stats::*;
pub use aidog_stats::DbInitTables;
pub use aidog_mcp::store::{list_mcp_servers, get_mcp_server, upsert_mcp_server, delete_mcp_server, set_mcp_server_enabled_agents, list_mcp_server_names};
pub use aidog_db::test_support;
