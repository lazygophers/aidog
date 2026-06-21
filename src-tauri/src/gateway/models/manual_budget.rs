//! 手动预算限额模型（窗口单位 + 限额项 + JSON 序列化）。

use super::default_true;
use serde::{Deserialize, Serialize};

/// 窗口时长单位（仅 rolling/fixed 有意义）。
/// serde default = Hour，保证旧 JSON（无 window_unit 字段）解析为「小时」，
/// 即 window_hours 数值原意（向后兼容零回退）。month 固定按 30 天换算。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum WindowUnit {
    Minute,
    #[default]
    Hour,
    Day,
    Week,
    Month,
}

/// 手动预算限额（仅对无上游 quota 自动支持的平台开放）。
/// 一平台可同时启多条；任一耗尽即阻断转发。est_cost/token 由请求驱动累加。
/// 全字段向后兼容：旧平台无 manual_budgets → 空 Vec → 不阻断、不扣、行为不变。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManualBudget {
    /// 限额唯一 id（前端生成，UPDATE 回写按 id 对齐保留 consumed/window_start_at）
    pub id: String,
    /// "total" 不重置 | "rolling" 滑动 N 个 window_unit | "fixed" 固定 N 个 window_unit 钟点对齐 | "daily" 自然日重置
    pub kind: String,
    /// "usd" 扣 est_cost | "token" 扣总 token
    pub unit: String,
    /// 限额额度（usd 为 $ / token 为 token 数）
    pub amount: f64,
    /// 窗口数值（该 window_unit 下的数量），仅 rolling/fixed 有意义。
    /// 历史字段名保留为 window_hours（不改名以最小化迁移）：
    /// 旧数据无 window_unit 时按小时解释（向后兼容）；新数据配合 window_unit 表任意单位。
    #[serde(default)]
    pub window_hours: Option<f64>,
    /// 窗口时长单位（minute/hour/day/week/month），旧数据缺失 → 默认 hour。
    #[serde(default)]
    pub window_unit: WindowUnit,
    /// 当前窗口已消耗（系统维护，请求驱动累加；窗口重置时清零）
    #[serde(default)]
    pub consumed: f64,
    /// 当前窗口起始毫秒戳（系统维护；rolling/fixed/daily 追踪重置基准）
    #[serde(default)]
    pub window_start_at: Option<i64>,
    /// 是否启用此限额
    #[serde(default = "default_true")]
    pub enabled: bool,
}

/// 解析 platform.manual_budgets JSON 列；空/非法 → 空 Vec
pub fn parse_manual_budgets(s: &str) -> Vec<ManualBudget> {
    if s.trim().is_empty() {
        return Vec::new();
    }
    serde_json::from_str(s).unwrap_or_default()
}

/// 序列化 manual_budgets → JSON 字符串（空 Vec → "[]"）
pub fn serialize_manual_budgets(budgets: &[ManualBudget]) -> String {
    serde_json::to_string(budgets).unwrap_or_else(|_| "[]".to_string())
}
