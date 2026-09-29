//! MCP 推荐清单：内置兜底数据与 schema（spec mcp-recommend §1）。
//!
//! `defaults/mcp/recommended.json` 编译期 `include_str!` 进二进制（同 registry 模式，
//! 单文件无需 build.rs 目录枚举）；仅当远程拉取从未成功时兜底（拉取/缓存是票 08）。
//! 远程源 = 仓库同路径文件，schema 由本模块类型定义约束。

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// 内置推荐清单原文。改 `defaults/mcp/recommended.json` 后需同步手改 `updated_at` Unix 秒
/// （远程同步按 updated_at 比版本决定覆盖，不盖戳会被视为旧版跳过）。
pub static BUNDLED_MCP_RECOMMENDED: &str = include_str!("../../../defaults/mcp/recommended.json");

/// 清单顶层：schema_version 不识别时整单拒收（票 04）。
/// key 形状 = spec §1.1（snake_case，同 registry 数据文件惯例）；
/// 发前端时由票 08 的命令层自行做 camelCase DTO，不在此混用。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecommendedManifest {
    pub schema_version: u32,
    pub updated_at: i64,
    pub entries: Vec<RecommendedEntry>,
}

/// 推荐条目：McpUpdatePayload 七字段（key 同名直取）+ 展示元数据。
/// env 只存空占位值（键预填、值留空），敏感真值永不进清单。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecommendedEntry {
    pub name: String,
    pub transport: String,
    pub command: String,
    pub args: Vec<String>,
    pub env: BTreeMap<String, String>,
    pub url: String,
    pub headers: BTreeMap<String, String>,
    pub display_name: String,
    /// 8-locale map（zh-Hans 必填；缺语言前端回落 en-US 再回落 zh-Hans）。
    pub description: BTreeMap<String, String>,
    /// 分类 key：browser/docs/code/search/service/tool（显示名走前端 locale）。
    pub category: String,
    /// simpleicons slug；空串走前端 fallback。
    pub icon: String,
    pub docs_url: String,
    pub homepage_url: String,
    /// 必填 env 名单，预填表单提示用（键必须已出现在 env）。
    pub required_env_keys: Vec<String>,
}

pub fn bundled_mcp_recommended() -> &'static str {
    BUNDLED_MCP_RECOMMENDED
}

#[cfg(test)]
#[path = "test_recommended.rs"]
mod test_recommended;
