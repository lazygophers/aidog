//! MCP 数据类型：agent slug / transport / DB 行 / 前端展示 / 扫描导入 / 编辑入参 / 原始配置。

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use super::mask::mask_env;

// ─── agent slug ────────────────────────────────────────────

/// 受管 agent。slug 对齐 skills 模块（claude-code / codex，见 [[npx-skills-cli]]）。
/// 扩展点：后续按需加 Cursor/Windsurf 变体 + 各自 backend 实现。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum McpAgent {
    ClaudeCode,
    Codex,
}

impl McpAgent {
    pub fn slug(&self) -> &'static str {
        match self {
            Self::ClaudeCode => "claude-code",
            Self::Codex => "codex",
        }
    }
    pub fn display(&self) -> &'static str {
        match self {
            Self::ClaudeCode => "Claude Code",
            Self::Codex => "Codex",
        }
    }
    pub fn all() -> &'static [Self] {
        &[Self::ClaudeCode, Self::Codex]
    }
    pub fn from_slug(s: &str) -> Option<Self> {
        Self::all().iter().copied().find(|a| a.slug() == s)
    }
}

// ─── transport ─────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum McpTransport {
    Stdio,
    Http,
    Sse,
}

impl McpTransport {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Stdio => "stdio",
            Self::Http => "http",
            Self::Sse => "sse",
        }
    }
    pub fn parse(s: &str) -> Self {
        match s.trim().to_ascii_lowercase().as_str() {
            "http" => Self::Http,
            "sse" => Self::Sse,
            _ => Self::Stdio,
        }
    }
    /// codex TOML `[mcp_servers.*]` 仅支持 stdio（无 url/headers 字段）。
    pub fn supported_by(self, agent: McpAgent) -> bool {
        match agent {
            McpAgent::ClaudeCode => true,
            McpAgent::Codex => self == Self::Stdio,
        }
    }
}

// ─── DB 行 ─────────────────────────────────────────────────

/// `mcp_server` 表原始行。env_json/headers_json 含原始敏感值。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpServerRow {
    pub id: i64,
    pub name: String,
    pub transport: String,
    pub command: String,
    pub args_json: String,
    pub env_json: String,
    pub url: String,
    pub headers_json: String,
    pub enabled_agents: String,
    pub created_at: i64,
    pub updated_at: i64,
}

impl McpServerRow {
    pub fn transport_enum(&self) -> McpTransport {
        McpTransport::parse(&self.transport)
    }
    pub fn enabled_set(&self) -> Vec<McpAgent> {
        self.enabled_agents
            .split(',')
            .filter(|s| !s.is_empty())
            .filter_map(McpAgent::from_slug)
            .collect()
    }
    pub fn args_vec(&self) -> Vec<String> {
        serde_json::from_str(&self.args_json).unwrap_or_default()
    }
    pub fn env_map(&self) -> BTreeMap<String, String> {
        serde_json::from_str(&self.env_json).unwrap_or_default()
    }
    pub fn headers_map(&self) -> BTreeMap<String, String> {
        serde_json::from_str(&self.headers_json).unwrap_or_default()
    }
    pub fn to_raw_cfg(&self) -> McpConfigRaw {
        McpConfigRaw {
            transport: self.transport_enum(),
            command: self.command.clone(),
            args: self.args_vec(),
            env: self.env_map(),
            url: self.url.clone(),
            headers: self.headers_map(),
        }
    }
}

// ─── 前端展示类型（脱敏） ───────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpServerInfo {
    pub id: i64,
    pub name: String,
    pub transport: String,
    pub command: String,
    pub args: Vec<String>,
    /// 已脱敏（敏感值 → "***"）。
    pub env: BTreeMap<String, String>,
    pub url: String,
    /// 已脱敏。
    pub headers: BTreeMap<String, String>,
    /// enabled agent slug 列表。
    pub enabled_agents: Vec<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

impl From<McpServerRow> for McpServerInfo {
    fn from(r: McpServerRow) -> Self {
        let args = r.args_vec();
        let env = mask_env(r.env_map());
        let headers = mask_env(r.headers_map());
        let enabled_agents = r
            .enabled_set()
            .iter()
            .map(|a| a.slug().to_string())
            .collect();
        Self {
            id: r.id,
            name: r.name,
            transport: r.transport,
            command: r.command,
            args,
            env,
            url: r.url,
            headers,
            enabled_agents,
            created_at: r.created_at,
            updated_at: r.updated_at,
        }
    }
}

// ─── 扫描导入类型 ───────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpScanItem {
    pub name: String,
    pub transport: String,
    pub command: String,
    pub args: Vec<String>,
    /// 已脱敏（仅展示用；导入时从 agent 配置取原值）。
    pub env: BTreeMap<String, String>,
    pub url: String,
    /// 已脱敏。
    pub headers: BTreeMap<String, String>,
    /// 发现该 MCP 的 agent slug 列表。
    pub found_in_agents: Vec<String>,
    /// DB 已有同名 MCP。
    pub already_imported: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpImportPayload {
    pub name: String,
    pub transport: String,
    pub command: String,
    pub args: Vec<String>,
    /// 前端传脱敏值；导入时优先从 agent 配置取原值，取不到才用此值。
    pub env: BTreeMap<String, String>,
    pub url: String,
    pub headers: BTreeMap<String, String>,
    /// 来源 agent slug。
    pub source_agent: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportReport {
    pub imported: Vec<String>,
    pub skipped: Vec<String>,
}

// ─── 编辑 MCP 入参 ─────────────────────────────────────────

/// 编辑 MCP 的入参（camelCase，前端直传）。
/// env/headers 中未改的敏感值由前端以 "***" 占位传回，后端 merge 旧 DB 明文。
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpUpdatePayload {
    pub name: String,
    pub transport: String,
    pub command: String,
    pub args: Vec<String>,
    pub env: BTreeMap<String, String>,
    pub url: String,
    pub headers: BTreeMap<String, String>,
}

// ─── 原始配置（agent 配置文件中的 MCP entry） ───────────────

#[derive(Debug, Clone)]
pub struct McpConfigRaw {
    pub transport: McpTransport,
    pub command: String,
    pub args: Vec<String>,
    pub env: BTreeMap<String, String>,
    pub url: String,
    pub headers: BTreeMap<String, String>,
}

#[cfg(test)]
#[path = "test_types.rs"]
mod test_types;
