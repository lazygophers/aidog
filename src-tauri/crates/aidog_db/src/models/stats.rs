/// 单个时段窗口。serde 字段名直接对齐 JSON / TS `PeakWindow`。
///
/// 时段基准：`timezone`（IANA 名，如 `Asia/Shanghai`）；缺省 / 非法 = UTC（向后兼容）。
/// `start_hour` / `end_hour` / `days_of_week` / `days_of_month` 均按该时区的**本地时刻**解释。
///
/// 向后兼容：旧数据无 `start_minute` / `end_minute` / `days_of_month` / `timezone` → None
/// （`start_minute`/`end_minute` None=0，`days_of_month` None=不过滤，`timezone` None=UTC）。
/// `Serialize` 供 `PlatformExtra`（gateway/models/platform.rs）整体往返测试用。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PeakWindow {
    pub start_hour: i32,
    pub end_hour: i32,
    pub multiplier: f64,
    /// 窗口时区（IANA 名，如 `Asia/Shanghai`）；缺省 = UTC（向后兼容）。
    /// 时段按该时区本地 xx:xx–xx:xx 解释；与 TS `PeakWindow.timezone?: string` 对称。
    #[serde(default)]
    pub timezone: Option<String>,
    #[serde(default)]
    pub days_of_week: Option<Vec<i32>>,
    /// 分钟精度起点（0-59）；缺省 = 0（仅 hour 精度，向后兼容）。
    #[serde(default)]
    pub start_minute: Option<i32>,
    /// 分钟精度终点（0-59）；缺省 = 0（仅 hour 精度，向后兼容）。
    #[serde(default)]
    pub end_minute: Option<i32>,
    /// 月内日过滤（1-31）；缺省 = 不过滤；与 `days_of_week` 在 UI 层互斥（hit 层同时 Some 时取 AND 兜底）。
    #[serde(default)]
    pub days_of_month: Option<Vec<i32>>,
    /// model scope（model 维度过滤，PRD 07-09 D2）；缺省 / None = 全平台模型生效（向后兼容）。
    /// 元素支持 `"glm-5.2*"` 后缀通配（覆盖 `glm-5.2` / `glm-5.2-turbo`），exact-first。
    /// 与 TS `PeakWindow.models?: string[]` 对称（跨层一致，见 cross-layer-rules.md）。
    #[serde(default)]
    pub models: Option<Vec<String>>,
    /// 生效期起点（Unix 秒，PRD 07-09 D2 福利期自动切换）；缺省 / None = 立即可用。
    /// `epoch_sec < start_at` → 窗口尚未启用，跳过（first-match 继续后续窗口）。
    /// 与 TS `PeakWindow.start_at?: number` 对称。
    #[serde(default)]
    pub start_at: Option<i64>,
    /// 生效期终点（Unix 秒，PRD 07-09 D2）；缺省 / None = 永久。
    /// `epoch_sec >= end_at` → 窗口已失效，跳过。
    /// 与 TS `PeakWindow.end_at?: number` 对称。
    #[serde(default)]
    pub end_at: Option<i64>,
}


// 统计查询与聚合结果模型。

use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, Deserialize, TS)]
#[ts(export, export_to = "../../../../src/services/api/types/generated/")]
pub struct StatsQuery {
    #[ts(optional, type = "number | null")]
    pub start: Option<i64>,
    #[ts(optional, type = "number | null")]
    pub end: Option<i64>,
    #[ts(optional)]
    pub granularity: Option<String>,
    #[ts(optional)]
    pub group_by: Option<String>,
    #[ts(optional)]
    pub filter_group: Option<String>,
    #[ts(optional)]
    pub filter_model: Option<String>,
    #[ts(optional)]
    pub filter_platform: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../../src/services/api/types/generated/")]
pub struct StatsOverview {
    pub total_requests: i32,
    pub success_rate: f64,
    #[ts(type = "number")]
    pub total_input_tokens: i64,
    #[ts(type = "number")]
    pub total_output_tokens: i64,
    #[ts(type = "number")]
    pub total_cache_tokens: i64,
    pub cache_rate: f64,
    pub avg_duration_ms: f64,
    pub total_cost: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../../src/services/api/types/generated/")]
pub struct StatsBucket {
    pub time_bucket: String,
    pub total_requests: i32,
    pub success_count: i32,
    pub error_count: i32,
    #[ts(type = "number")]
    pub input_tokens: i64,
    #[ts(type = "number")]
    pub output_tokens: i64,
    #[ts(type = "number")]
    pub cache_tokens: i64,
    pub avg_duration_ms: f64,
    pub total_cost: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../../src/services/api/types/generated/")]
pub struct DimensionEntry {
    pub name: String,
    pub total_requests: i32,
    pub success_count: i32,
    #[ts(type = "number")]
    pub input_tokens: i64,
    #[ts(type = "number")]
    pub output_tokens: i64,
    #[ts(type = "number")]
    pub cache_tokens: i64,
    pub avg_duration_ms: f64,
    pub total_cost: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../../src/services/api/types/generated/")]
pub struct StatsResult {
    pub overview: StatsOverview,
    pub buckets: Vec<StatsBucket>,
    pub dimension_data: Vec<DimensionEntry>,
    /// 当前筛选范围（日期 + 分组 + 平台，不含 filter_model）内实际有记录的模型名，
    /// 供前端模型筛选下拉使用（避免列出配置过但无请求的模型）。
    pub available_models: Vec<String>,
}
