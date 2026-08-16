//! 平台余额 & Coding Plan 配额查询服务 —— facade。
//! 实现已下沉 aidog_adapter::quota（按平台目录组织，三函数模式：
//! quota_config 能力配置 / query_quota 完整结果 / run_custom_query JS 脚本自定义查询），
//! 本模块仅 re-export 保持 `gateway::quota::*` 调用路径不变。

pub use aidog_adapter::quota::http::with_cli_proxy_provider_id;
pub use aidog_adapter::quota::{
    query_quota, query_quota_for, quota_config_for, BalanceInfo, CodingPlanInfo, PlatformQuota,
    QuotaCapability, QuotaTier,
};

pub use aidog_adapter::newapi::quota::{parse_newapi_extra, query_quota_newapi};
pub use aidog_adapter::devin::quota::{parse_devin_extra, query_quota_devin};
