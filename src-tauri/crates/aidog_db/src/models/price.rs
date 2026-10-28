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
    /// 缓存写入单价（registry `price.cache_write`，Anthropic 1.25×/2× input 价）。
    /// 2026-09-28 起消费；缺省 0 = 条目未标（计价层按 0 加项，不回退 input 价）。
    #[serde(default)]
    pub cache_write_input_token_cost: f64,
    /// 缓存写入 1h TTL 单价（registry `price.cache_write_1h`，Anthropic 2× input 价；
    /// spec B3）。缺省回落 `cache_write` 值——无该字段的平台 5m/1h 同价，零回归。
    #[serde(default)]
    pub cache_write_1h_input_token_cost: f64,
    pub source: String, // "platform_override" | "default_platform" | "top_level" | "fallback"
}

/// 价格设置（远程同步已删 2026-10-21：registry 只读编译期内置，启动灌种进镜像表；
/// setting key 仍叫 `price_sync`，历史持久化行里已删键的 JSON 会被 serde 忽略）。
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../../src/services/api/types/generated/")]
pub struct PriceSyncSettings {
    /// 上次灌种的 registry index `last_updated`（Unix 秒），诊断用
    #[serde(default)]
    #[ts(type = "number")]
    pub registry_last_updated: i64,
    /// 兜底默认价格 $/M tokens
    #[serde(default = "default_fallback_price")]
    pub fallback_input_price: f64,
    #[serde(default = "default_fallback_price")]
    pub fallback_output_price: f64,
}

fn default_fallback_price() -> f64 {
    3.0
}

impl Default for PriceSyncSettings {
    fn default() -> Self {
        Self {
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
