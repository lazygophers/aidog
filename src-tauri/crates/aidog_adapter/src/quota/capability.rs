//! 平台 quota 能力配置（三函数模式之一）：静态声明平台支持哪些查询维度，
//! 前端据此渲染可用的查询入口，不试错。

use serde::{Deserialize, Serialize};

/// 平台 quota 能力静态配置。
///
/// tier_names 取值约定（与 QuotaTier.name 同词汇表）：
/// "five_hour"（5 小时窗口）/ "weekly_limit"（周限制）/ "monthly"（月限制）/
/// "mcp_monthly"（GLM MCP 月用量）。
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct QuotaCapability {
    /// 支持余额查询（BalanceInfo）
    pub supports_balance: bool,
    /// 支持 Coding Plan 配额查询（CodingPlanInfo）
    pub supports_coding_plan: bool,
    /// 支持 MCP 用量查询（如 GLM TIME_LIMIT 月用量）
    pub supports_mcp_query: bool,
    /// 支持的配额层级名
    pub tier_names: Vec<String>,
    /// 支持 JS 脚本自定义查询（通用能力，各平台均可注入 ctx）
    pub custom_query_supported: bool,
}

impl QuotaCapability {
    /// 自定义查询是通用能力，各平台构造 helper 统一补 true。
    pub fn with_custom(mut self) -> Self {
        self.custom_query_supported = true;
        self
    }
}
