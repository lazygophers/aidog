//! 模型信息中枢数据模型：`model_entry`（平台视角模型条目）与 `platform_preset`（协议预设整份 JSON）。
//!
//! registry(`defaults/registry/`) 是真值源，本组结构是它落库后的行形状。
//! 与旧 `ModelPrice` 的区别：主键从 `model_name` 变成 `(platform_code, model_id)`——
//! 同一模型在不同平台是**各自独立的条目**，跨平台聚合靠 `canonical_model`。

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// `model_entry` 行：某平台视角下的一条模型条目。
///
/// `price_data` 保留整份 registry 模型 JSON（同旧 `ModelPrice.price_data` idiom），
/// 计费需要的 `default_price` / `peak` / `context_tiers` / `time_tiers` 等结构化子树都在里面，
/// 不为每个价格维度开列。被查询/排序/搜索用到的字段才提列。
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../../src/services/api/types/generated/")]
pub struct ModelEntry {
    /// registry 平台目录名（= `index.json` 的 `code`，也是 protocol 裸名）。
    pub platform_code: String,
    /// 该平台上的真实请求名。
    pub model_id: String,
    /// 内部统一 id，跨平台聚合键。缺省回落 `model_id`（写入层保证非空）。
    pub canonical_model: String,
    pub family: String,
    pub version: String,
    pub predecessor: String,
    /// text / vision / image_gen / tool_use / reasoning / audio / video / embedding
    pub capabilities: Vec<String>,
    /// Claude Code 内置工具**黑名单**，空 = 全支持。
    pub builtin_tools_excluded: Vec<String>,
    #[ts(type = "number | null")]
    pub max_input_tokens: Option<i64>,
    #[ts(type = "number | null")]
    pub max_output_tokens: Option<i64>,
    #[ts(type = "number | null")]
    pub context_window: Option<i64>,
    /// 本平台这条价格是否厂商自营。聚合行的代表条目取 `official = true` 那条。
    pub official: bool,
    /// 整份 registry 模型 JSON 文本。
    pub price_data: String,
    #[ts(type = "number")]
    pub updated_at: i64,
}

/// `platform_preset` 行：一份 `platform.json` 的整体快照。
///
/// `preset_data` 是整份 JSON（endpoints / models / model_list / peak_hours + 品牌字段
/// name / logo_url / color / homepage / keywords / source_urls）。
/// 品牌字段不拆列——前端拿到整份即可渲染，同步也能整体覆盖（票 12）。
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../../src/services/api/types/generated/")]
pub struct PlatformPreset {
    pub code: String,
    pub preset_data: String,
    #[ts(type = "number")]
    pub updated_at: i64,
}

/// 模型维度聚合行：一个 `canonical_model` 下的全部平台条目。
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../../src/services/api/types/generated/")]
pub struct ModelEntryGroup {
    pub canonical_model: String,
    /// 代表条目所在平台：优先 `official = true`，否则按 `platform_code` 字典序第一条。
    pub primary_platform: String,
    /// 该 canonical 下全部平台条目，`platform_code` 升序。
    pub entries: Vec<ModelEntry>,
}

/// 模型信息页一次性数据源：聚合行 + 全部平台预设（含品牌字段），前端不做二次 RPC 拼装。
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../../src/services/api/types/generated/")]
pub struct ModelInfoSnapshot {
    pub groups: Vec<ModelEntryGroup>,
    pub platforms: Vec<PlatformPreset>,
    /// true = DB 尚无同步数据，本次返回的是编译期内置 registry 兜底。
    pub bundled: bool,
}
