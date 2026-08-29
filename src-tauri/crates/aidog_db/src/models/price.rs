//! 模型价格模型：解析结果 / 同步设置与结果。
//!
//! 旧 `ModelPrice` / `ModelPriceSummary`（`model_price` 表的行与列表摘要）随票 T6
//! 与该表一并删除：模型条目的真值源已是 `model_entry`（见 `models/model_entry.rs`）。

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// 价格解析结果
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../../src/services/api/types/generated/")]
pub struct ResolvedPrice {
    pub input_cost_per_token: f64,
    pub output_cost_per_token: f64,
    pub cache_read_input_token_cost: f64,
    pub source: String,  // "platform_override" | "default_platform" | "top_level" | "fallback"
}

/// 模型价格同步设置
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../../src/services/api/types/generated/")]
pub struct PriceSyncSettings {
    #[serde(default)]
    pub auto_sync_enabled: bool,
    /// 同步间隔（秒），默认 86400 = 24h
    #[serde(default = "default_sync_interval")]
    #[ts(type = "number")]
    pub sync_interval_secs: u64,
    /// 上次同步时间（ms timestamp）
    #[serde(default)]
    #[ts(type = "number")]
    pub last_sync_at: i64,
    /// 上次入库的 registry index `last_updated`（Unix 秒）；远程不比它新则整轮跳过
    #[serde(default)]
    #[ts(type = "number")]
    pub registry_last_updated: i64,
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
            registry_last_updated: 0,
            fallback_input_price: default_fallback_price(),
            fallback_output_price: default_fallback_price(),
        }
    }
}

/// registry 同步中单个文件的失败记录（best-effort：该文件的 DB 旧数据原样保留）。
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../../src/services/api/types/generated/")]
pub struct SyncFailure {
    /// registry 内的相对路径，如 `platforms/glm/models/glm-4.6.json`。
    pub file: String,
    pub error: String,
}

/// 同步结果。`total` = 本轮尝试拉取的文件数（platform.json + 模型条目），
/// `failures` 非空即 partial：成功的文件已入库，失败的保留 DB 旧值。
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../../src/services/api/types/generated/")]
pub struct PriceSyncResult {
    pub added: u32,
    pub updated: u32,
    pub unchanged: u32,
    pub failed: u32,
    pub total: u32,
    #[serde(default)]
    pub failures: Vec<SyncFailure>,
}
