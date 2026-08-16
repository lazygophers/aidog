pub mod adapter;
pub mod backup;
pub mod billing;
pub mod cli_proxy_parser;
pub mod codex;
pub mod db;
pub mod estimate;
pub mod hooks;
pub mod http_client;
pub mod i18n;
pub mod log_util { pub use aidog_db::log_util::*; }
pub mod logo_sync;
pub mod manual_budget;
pub mod middleware;
pub mod mitm;
pub mod models { pub use aidog_db::models::*; }
pub mod notification;
pub mod peak_hours;
pub(crate) mod presets_cache;
pub(crate) mod presets_const;
pub(crate) mod client_types_const;
pub mod price_sync;
pub mod proxy;
pub mod quota;
pub mod router;
pub mod scheduling;
pub mod scripts;
pub mod skills {
    //! 拆分 shim：实现在独立 crate aidog_skills，此处保持 `gateway::skills::X` 旧路径。
    pub use aidog_skills::*;
}
pub mod time_models;
pub mod usage_color;
pub mod import_export;
pub mod mcp {
    //! 拆分 shim：实现在独立 crate aidog_mcp，此处保持 `gateway::mcp::X` 旧路径。
    pub use aidog_mcp::*;
}
pub mod claude_integration;
