//! 统计查询与聚合结果模型。

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize)]
pub struct StatsQuery {
    pub start: Option<i64>,
    pub end: Option<i64>,
    pub granularity: Option<String>,
    pub group_by: Option<String>,
    pub filter_group: Option<String>,
    pub filter_model: Option<String>,
    pub filter_platform: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatsOverview {
    pub total_requests: i32,
    pub success_rate: f64,
    pub total_input_tokens: i64,
    pub total_output_tokens: i64,
    pub total_cache_tokens: i64,
    pub cache_rate: f64,
    pub avg_duration_ms: f64,
    pub total_cost: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatsBucket {
    pub time_bucket: String,
    pub total_requests: i32,
    pub success_count: i32,
    pub error_count: i32,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub cache_tokens: i64,
    pub avg_duration_ms: f64,
    pub total_cost: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DimensionEntry {
    pub name: String,
    pub total_requests: i32,
    pub success_count: i32,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub cache_tokens: i64,
    pub avg_duration_ms: f64,
    pub total_cost: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatsResult {
    pub overview: StatsOverview,
    pub buckets: Vec<StatsBucket>,
    pub dimension_data: Vec<DimensionEntry>,
    /// 当前筛选范围（日期 + 分组 + 平台，不含 filter_model）内实际有记录的模型名，
    /// 供前端模型筛选下拉使用（避免列出配置过但无请求的模型）。
    pub available_models: Vec<String>,
}
