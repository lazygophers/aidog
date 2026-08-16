pub mod billing;
pub mod codex;
pub mod estimate;
pub mod http_client;
pub mod i18n;
pub mod log_util { pub use aidog_db::log_util::*; }
pub mod logo_sync;
pub mod manual_budget;
pub mod models { pub use aidog_db::models::*; }
pub mod peak_hours;
pub(crate) mod presets_cache {
    //! 拆分 shim：实现在 aidog_db::presets_cache，此处保持旧路径。
    pub use aidog_db::presets_cache::*;
}
pub(crate) mod presets_const {
    //! 拆分 shim：实现在 aidog_db::presets_const，此处保持旧路径。
    
}
pub(crate) mod client_types_const {
    //! 拆分 shim：实现在 aidog_db::client_types_const，此处保持旧路径。
    pub use aidog_db::client_types_const::*;
}
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
