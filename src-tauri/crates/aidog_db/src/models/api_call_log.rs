//! 管理接口调用记录模型（api_call_log 表行，log.db）。

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// 管理接口调用记录。只存元数据，不存请求与响应正文。
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../../src/services/api/types/generated/")]
pub struct ApiCallLog {
    #[ts(type = "number")]
    pub id: i64,
    /// 毫秒 Unix。
    #[ts(type = "number")]
    pub created_at: i64,
    /// 命令名（`/api/rpc/<命令>` 末段）；`events` / `ui` 为非命令路径。
    pub command: String,
    pub status_code: i32,
    /// status < 400。
    pub success: bool,
    #[ts(type = "number")]
    pub duration_ms: i64,
    pub user_agent: String,
    /// `third_party`（第三方）| `settings_test`（设置页测试面板）。
    pub source: String,
}
