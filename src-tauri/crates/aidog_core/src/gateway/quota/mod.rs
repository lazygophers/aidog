//! 平台余额 & Coding Plan 配额查询服务 —— facade。
//! 实现已下沉 aidog_adapter::quota（quota-scripts spec：执行统一走 registry JS 脚本，
//! `run_quota_script` → `script::run_custom_query`）；本模块仅 re-export 保持
//! `gateway::quota::*` 调用路径不变。

pub use aidog_adapter::quota::http::with_cli_proxy_provider_id;
pub use aidog_adapter::quota::{
    query_quota, query_quota_for, quota_config_for, BalanceInfo, CodingPlanInfo, PlatformQuota,
    QuotaCapability, QuotaTier,
};

pub use aidog_adapter::newapi::quota::query_quota_newapi;
pub use aidog_adapter::devin::quota::query_quota_devin;
