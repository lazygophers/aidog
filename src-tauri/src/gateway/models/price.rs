//! 模型价格模型：价格记录 / 摘要 / 解析结果 / 同步设置与结果。

use serde::{Deserialize, Serialize};

/// 模型价格记录
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelPrice {
    pub id: u64,
    pub model_name: String,
    /// "github" | "manual"
    pub source: String,
    /// JSON: {input_cost_per_token, output_cost_per_token, cache_read_input_token_cost, pricing: {platform_type: {...}}, default_platform, ...}
    pub price_data: String,
    /// 最大输入 token（模型固有，平台无关）。NULL = 未知。
    #[serde(default)]
    pub max_input_tokens: Option<i64>,
    /// 最大输出 token（出站裁剪用）。NULL = 未知/无限制。
    #[serde(default)]
    pub max_output_tokens: Option<i64>,
    /// 上下文窗口。NULL = 未知。
    #[serde(default)]
    pub context_window: Option<i64>,
    pub created_at: i64,
    pub updated_at: i64,
    #[serde(default)]
    pub deleted_at: i64,
}

/// 模型价格摘要（列表展示用，解析了关键字段）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelPriceSummary {
    pub id: u64,
    pub model_name: String,
    pub source: String,
    pub default_platform: Option<String>,
    /// $/M input tokens
    pub input_price: Option<f64>,
    /// $/M output tokens
    pub output_price: Option<f64>,
    /// $/M cache read tokens
    pub cache_read_price: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_input_tokens: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_output_tokens: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context_window: Option<i64>,
    pub updated_at: i64,
}

/// 价格解析结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolvedPrice {
    pub input_cost_per_token: f64,
    pub output_cost_per_token: f64,
    pub cache_read_input_token_cost: f64,
    pub source: String,  // "platform_override" | "default_platform" | "top_level" | "fallback"
}

/// 模型价格同步设置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriceSyncSettings {
    #[serde(default)]
    pub auto_sync_enabled: bool,
    /// 同步间隔（秒），默认 86400 = 24h
    #[serde(default = "default_sync_interval")]
    pub sync_interval_secs: u64,
    /// 上次同步时间（ms timestamp）
    #[serde(default)]
    pub last_sync_at: i64,
    /// 兜底默认价格 $/M tokens
    #[serde(default = "default_fallback_price")]
    pub fallback_input_price: f64,
    #[serde(default = "default_fallback_price")]
    pub fallback_output_price: f64,
}

fn default_sync_interval() -> u64 { 86400 }
fn default_fallback_price() -> f64 { 3.0 }

impl Default for PriceSyncSettings {
    fn default() -> Self {
        Self {
            auto_sync_enabled: false,
            sync_interval_secs: default_sync_interval(),
            last_sync_at: 0,
            fallback_input_price: default_fallback_price(),
            fallback_output_price: default_fallback_price(),
        }
    }
}

/// 同步结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriceSyncResult {
    pub added: u32,
    pub updated: u32,
    pub unchanged: u32,
    pub failed: u32,
    pub total: u32,
}
