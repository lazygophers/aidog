//! MCP (Model Context Protocol) server 集中管理。
//!
//! aidog DB 集中存 MCP server 配置（`mcp_server` 表），per-agent 启用切换。
//! - 启用某 agent = 把 MCP 写入该 agent 配置文件
//! - 禁用某 agent = 从该 agent 配置移除（DB 记录保留，可再启用）
//! - 删除 = DB + 所有 enabled agent 配置全删
//! - 导入 = 扫两 agent 配置去重合并 → 勾选 → 入 DB（enabled = 来源 agent）
//!
//! Agent 配置后端：
//! - ClaudeCode: `~/.claude.json` 的 `mcpServers`（JSON object）
//! - Codex: `~/.codex/config.toml` 的 `[mcp_servers.<name>]`（TOML，仅 stdio）
//!
//! 敏感：env/headers 可能含 token/key/secret，前端展示经 `mask_env` 脱敏（`***`），
//! DB 存原值，写 agent 配置用原值（非脱敏值）。
//!
//! 子模块拆分：
//! - `types`: 数据类型（McpAgent / McpTransport / 各 Row/Info/Payload / McpConfigRaw）
//! - `mask`: 敏感脱敏与脱敏 merge
//! - `backend_claude` / `backend_codex`: 两 agent 配置后端
//! - `domain`: scan / import / set_agent / add / delete / resync / update

mod backend_claude;
mod backend_codex;
mod domain;
mod mask;
mod types;

use backend_claude::ClaudeCodeBackend;
use backend_codex::CodexBackend;

// 对外路径保持 `gateway::mcp::X` 不变。
pub use domain::{
    add_server, delete_server, import_items, import_pasted, resync_all, scan_all,
    set_agent_enabled, share_server, update_server,
};
// `mask_env` / `McpTransport` 为对外公共 API（保留 `gateway::mcp::X` 路径），
// 当前 crate 内无外部引用点，allow 抑制未用 re-export 告警。
#[allow(unused_imports)]
pub use mask::mask_env;
#[allow(unused_imports)]
pub use types::{
    ImportReport, McpAgent, McpConfigRaw, McpImportPayload, McpScanItem, McpServerInfo,
    McpServerRow, McpTransport, McpUpdatePayload,
};

// ─── agent 配置后端 trait ───────────────────────────────────

pub(crate) trait McpAgentBackend {
    /// 读该 agent 所有 MCP（name → 配置）。文件不存在或解析失败返回空。
    fn read_all(&self) -> Result<Vec<(String, McpConfigRaw)>, String>;
    /// 写/覆盖某 MCP。返回 Ok 不代表 agent 立即识别（需 agent 重载）。
    fn write(&self, name: &str, cfg: &McpConfigRaw) -> Result<(), String>;
    /// 删某 MCP。不存在视为 Ok。
    fn remove(&self, name: &str) -> Result<(), String>;
}

pub(crate) fn backend_for(agent: McpAgent) -> Box<dyn McpAgentBackend> {
    match agent {
        McpAgent::ClaudeCode => Box::new(ClaudeCodeBackend),
        McpAgent::Codex => Box::new(CodexBackend),
    }
}
