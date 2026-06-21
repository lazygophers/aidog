//! cc-switch 异源导入子模块。
//!
//! 读取 cc-switch 本地配置（SQLite `cc-switch.db` providers 表 或 旧
//! `config.json` MultiAppConfig），仅筛选 app_type ∈ {claude, codex}，
//! 原样透传成 [`CcProvider`] 中间表示。**不做平台匹配** —— 匹配回退链
//! 在前端纯函数（preset 住前端，记忆 `aidog-add-platform-skill` 反直觉点 1）。
//!
//! 数据结构实证（本地 `~/.cc-switch/cc-switch.db`，2026-06-16）：
//! - claude provider `settings_config` = `{env:{ANTHROPIC_BASE_URL,
//!   ANTHROPIC_AUTH_TOKEN|ANTHROPIC_API_KEY, ANTHROPIC_MODEL,
//!   ANTHROPIC_DEFAULT_*_MODEL, ...}, ...其他 ~/.claude/settings.json 字段}`。
//!   空 provider（如 Claude Official preset 模板）可能为 `{}`。
//! - codex provider `settings_config` = `{auth:{OPENAI_API_KEY},
//!   config:"<config.toml 文本>"}`，config 含 `model_provider` / `model` /
//!   `[model_providers.<id>]` 表的 `base_url` / `wire_api`。
//!
//! 后端只提取 base_url + api_key + (codex 的 config_toml 解析结果)，平台类型
//! 判断全部交给前端 ccswitchMatch.ts。
//!
//! 子模块切分（按领域）：
//! - [`detect`]：cc-switch 数据目录解析 + 探测（SQLite / 旧 JSON）。
//! - [`read`]：按源类型读 provider + 提取便捷字段（base_url / api_key）。
//! - [`codex_config`]：codex provider `config.toml` 轻量解析。
//! - [`import`]：把前端转换好的 platform payload 复用 apply 写入 aidog DB。

use serde::{Deserialize, Serialize};

mod codex_config;
mod detect;
mod import;
mod read;

pub use detect::detect;
pub use import::import;
pub use read::read;

/// 单个 cc-switch provider 的中间表示（原始字段透传 + 提取的便捷字段）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CcProvider {
    pub id: String,
    /// `claude` | `codex`（SQL 已过滤，前端再校验）。
    pub app_type: String,
    pub name: String,
    /// 原始 settings_config JSON。前端按 app_type 自行解析。
    pub settings_config: serde_json::Value,
    pub website_url: Option<String>,
    /// claude: env.ANTHROPIC_BASE_URL；codex: config.toml base_url。
    pub detected_base_url: Option<String>,
    /// claude: env.ANTHROPIC_AUTH_TOKEN / ANTHROPIC_API_KEY；
    /// codex: auth.OPENAI_API_KEY。
    pub detected_api_key: Option<String>,
    /// codex 专用：解析后的 config.toml 键值（顶层 `model` / `model_provider` /
    /// `wire_api` 等 + `[model_providers.<id>]` 的 `base_url` / `name`）。
    /// claude provider 此字段为 None。后端做轻量 TOML 解析避免前端引依赖。
    pub codex_config_parsed: Option<CodexConfigParsed>,
}

/// codex provider config.toml 解析后的结构化字段（前端用）。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexConfigParsed {
    /// 顶层 `model`（主模型 slot）。
    pub model: Option<String>,
    /// 顶层 `model_provider`。
    pub model_provider: Option<String>,
    /// `[model_providers.<id>]` 的 base_url（取 model_provider 对应表）。
    pub base_url: Option<String>,
    /// wire_api：responses / chat。
    pub wire_api: Option<String>,
    /// provider 表里的 name。
    pub provider_name: Option<String>,
}

/// 探测结果。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CcswitchDetection {
    pub found: bool,
    pub path: Option<String>,
    /// `sqlite` | `json` | `none`。
    pub source_type: String,
    /// 若发现 SQLite，预估的 claude+codex provider 数（-1 = 未统计）。
    pub provider_count: i64,
}

/// 读取结果。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CcswitchReadResult {
    pub source_type: String,
    pub path: String,
    pub providers: Vec<CcProvider>,
}
