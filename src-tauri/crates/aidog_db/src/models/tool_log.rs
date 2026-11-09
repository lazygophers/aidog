//! 工具日志模型（tool_log 表行，log.db）。

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// 一次工具执行（网关代执行 / HTTP MCP）。列表接口不带 `result`（正文按需经详情取）。
#[derive(Debug, Clone, Serialize, Deserialize, TS, Default)]
#[ts(export, export_to = "../../../../src/services/api/types/generated/")]
pub struct ToolLog {
    #[ts(type = "number")]
    pub id: i64,
    /// 毫秒 Unix。
    #[ts(type = "number")]
    pub created_at: i64,
    /// web_search | web_fetch | generate_image | generate_video。
    pub tool: String,
    /// `gateway`（网关代执行）| `mcp`（HTTP MCP）。
    pub trigger: String,
    pub group_name: String,
    /// 代执行时被绕开的平台名；MCP 触发为空。
    pub platform: String,
    /// 搜索词 / URL / 提示词。`log_user_request` 关时置空。
    pub query: String,
    #[ts(type = "number")]
    pub result_count: i64,
    /// 各引擎明细 JSON：`[{engine,count,latency_ms,error}]`。
    pub engines: String,
    #[ts(type = "number")]
    pub duration_ms: i64,
    pub success: bool,
    pub error: String,
    /// 结果内容。`log_user_request` 关时置空；列表接口恒为空串。
    pub result: String,
}

/// 列表筛选；`None` / 空串 = 不过滤。
#[derive(Debug, Clone, Default, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../../src/services/api/types/generated/")]
pub struct ToolLogFilter {
    pub tool: Option<String>,
    pub trigger: Option<String>,
    pub group_name: Option<String>,
}

/// 分页结果。
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../../src/services/api/types/generated/")]
pub struct ToolLogPage {
    #[ts(type = "number")]
    pub total: i64,
    pub rows: Vec<ToolLog>,
}
