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

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;

use super::db::Db;

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

// ─── 敏感脱敏 ──────────────────────────────────────────────

/// 判定 env/header key 是否敏感（含 token/key/secret/auth/password/pass/credential）。
fn is_sensitive_key(k: &str) -> bool {
    let lk = k.to_ascii_lowercase();
    ["token", "key", "secret", "auth", "password", "pass", "credential"]
        .iter()
        .any(|s| lk.contains(s))
}

/// 脱敏 map：敏感 key 的值替换为 "***"。
pub fn mask_env(map: BTreeMap<String, String>) -> BTreeMap<String, String> {
    map.into_iter()
        .map(|(k, v)| {
            if is_sensitive_key(&k) {
                (k, "***".to_string())
            } else {
                (k, v)
            }
        })
        .collect()
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

// ─── agent 配置后端 trait ───────────────────────────────────

trait McpAgentBackend {
    /// 读该 agent 所有 MCP（name → 配置）。文件不存在或解析失败返回空。
    fn read_all(&self) -> Result<Vec<(String, McpConfigRaw)>, String>;
    /// 写/覆盖某 MCP。返回 Ok 不代表 agent 立即识别（需 agent 重载）。
    fn write(&self, name: &str, cfg: &McpConfigRaw) -> Result<(), String>;
    /// 删某 MCP。不存在视为 Ok。
    fn remove(&self, name: &str) -> Result<(), String>;
}

fn backend_for(agent: McpAgent) -> Box<dyn McpAgentBackend> {
    match agent {
        McpAgent::ClaudeCode => Box::new(ClaudeCodeBackend),
        McpAgent::Codex => Box::new(CodexBackend),
    }
}

// ─── ClaudeCode: ~/.claude.json mcpServers ──────────────────

struct ClaudeCodeBackend;

fn claude_json_path() -> Result<PathBuf, String> {
    let home = dirs::home_dir().ok_or("cannot resolve home directory")?;
    Ok(home.join(".claude.json"))
}

fn read_claude_json(path: &PathBuf) -> Result<serde_json::Value, String> {
    if !path.exists() {
        return Ok(serde_json::json!({}));
    }
    let content = std::fs::read_to_string(path)
        .map_err(|e| format!("read ~/.claude.json: {e}"))?;
    if content.trim().is_empty() {
        return Ok(serde_json::json!({}));
    }
    serde_json::from_str(&content).map_err(|e| format!("parse ~/.claude.json: {e}"))
}

fn write_claude_json(path: &PathBuf, root: &serde_json::Value) -> Result<(), String> {
    let s = serde_json::to_string_pretty(root)
        .map_err(|e| format!("serialize ~/.claude.json: {e}"))?;
    std::fs::write(path, s).map_err(|e| format!("write ~/.claude.json: {e}"))
}

/// 解析 claude.json mcpServers entry → McpConfigRaw。
/// entry 形如 {"type":"stdio","command":..,"args":[..],"env":{..}} 或 http/sse 含 url/headers。
fn parse_claude_entry(v: &serde_json::Value) -> Option<McpConfigRaw> {
    let obj = v.as_object()?;
    let type_str = obj.get("type").and_then(|t| t.as_str()).unwrap_or("stdio");
    let transport = McpTransport::parse(type_str);
    let command = obj
        .get("command")
        .and_then(|c| c.as_str())
        .unwrap_or("")
        .to_string();
    let args = obj
        .get("args")
        .and_then(|a| a.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|x| x.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();
    let env = json_object_to_map(obj.get("env"));
    let url = obj
        .get("url")
        .and_then(|u| u.as_str())
        .unwrap_or("")
        .to_string();
    let headers = json_object_to_map(obj.get("headers"));
    Some(McpConfigRaw {
        transport,
        command,
        args,
        env,
        url,
        headers,
    })
}

fn json_object_to_map(v: Option<&serde_json::Value>) -> BTreeMap<String, String> {
    v.and_then(|e| e.as_object())
        .map(|m| {
            m.iter()
                .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                .collect()
        })
        .unwrap_or_default()
}

fn build_claude_entry(cfg: &McpConfigRaw) -> serde_json::Value {
    let mut obj = serde_json::Map::new();
    obj.insert(
        "type".to_string(),
        serde_json::Value::String(cfg.transport.as_str().to_string()),
    );
    match cfg.transport {
        McpTransport::Stdio => {
            obj.insert(
                "command".to_string(),
                serde_json::Value::String(cfg.command.clone()),
            );
            obj.insert(
                "args".to_string(),
                serde_json::to_value(&cfg.args).unwrap_or(serde_json::json!([])),
            );
            if !cfg.env.is_empty() {
                obj.insert("env".to_string(), serde_json::to_value(&cfg.env).unwrap());
            }
        }
        McpTransport::Http | McpTransport::Sse => {
            obj.insert(
                "url".to_string(),
                serde_json::Value::String(cfg.url.clone()),
            );
            if !cfg.headers.is_empty() {
                obj.insert(
                    "headers".to_string(),
                    serde_json::to_value(&cfg.headers).unwrap(),
                );
            }
        }
    }
    serde_json::Value::Object(obj)
}

impl McpAgentBackend for ClaudeCodeBackend {

    fn read_all(&self) -> Result<Vec<(String, McpConfigRaw)>, String> {
        let path = claude_json_path()?;
        if !path.exists() {
            return Ok(vec![]);
        }
        let root = read_claude_json(&path)?;
        let mut out = vec![];
        if let Some(servers) = root.get("mcpServers").and_then(|v| v.as_object()) {
            for (name, val) in servers {
                if let Some(cfg) = parse_claude_entry(val) {
                    out.push((name.clone(), cfg));
                }
            }
        }
        Ok(out)
    }

    fn write(&self, name: &str, cfg: &McpConfigRaw) -> Result<(), String> {
        let path = claude_json_path()?;
        let mut root = read_claude_json(&path)?;
        let servers = root
            .as_object_mut()
            .ok_or("~/.claude.json root is not an object")?
            .entry("mcpServers".to_string())
            .or_insert_with(|| serde_json::json!({}));
        let servers_obj = servers
            .as_object_mut()
            .ok_or("mcpServers is not an object")?;
        servers_obj.insert(name.to_string(), build_claude_entry(cfg));
        write_claude_json(&path, &root)
    }

    fn remove(&self, name: &str) -> Result<(), String> {
        let path = claude_json_path()?;
        if !path.exists() {
            return Ok(());
        }
        let mut root = read_claude_json(&path)?;
        if let Some(servers) = root
            .get_mut("mcpServers")
            .and_then(|v| v.as_object_mut())
        {
            servers.remove(name);
        }
        write_claude_json(&path, &root)
    }
}

// ─── Codex: ~/.codex/config.toml [mcp_servers.*] ────────────

struct CodexBackend;

fn codex_config_path() -> Result<PathBuf, String> {
    Ok(super::codex::codex_home_public()?.join("config.toml"))
}

fn read_codex_toml(path: &PathBuf) -> Result<toml::Value, String> {
    if !path.exists() {
        return Ok(toml::Value::Table(toml::map::Map::new()));
    }
    let content = std::fs::read_to_string(path)
        .map_err(|e| format!("read config.toml: {e}"))?;
    if content.trim().is_empty() {
        return Ok(toml::Value::Table(toml::map::Map::new()));
    }
    toml::from_str(&content).map_err(|e| format!("parse config.toml: {e}"))
}

fn write_codex_toml(path: &PathBuf, root: &toml::Value) -> Result<(), String> {
    let s = toml::to_string_pretty(root).map_err(|e| format!("serialize config.toml: {e}"))?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("create ~/.codex: {e}"))?;
    }
    std::fs::write(path, s).map_err(|e| format!("write config.toml: {e}"))
}

fn toml_table_to_map(tbl: Option<&toml::map::Map<String, toml::Value>>) -> BTreeMap<String, String> {
    tbl.map(|m| {
        m.iter()
            .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
            .collect()
    })
    .unwrap_or_default()
}

/// 解析 codex `[mcp_servers.<name>]` 段 → McpConfigRaw（codex 仅 stdio）。
fn parse_codex_entry(v: &toml::Value) -> Option<McpConfigRaw> {
    let tbl = v.as_table()?;
    let command = tbl
        .get("command")
        .and_then(|c| c.as_str())
        .unwrap_or("")
        .to_string();
    let args = tbl
        .get("args")
        .and_then(|a| a.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|x| x.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();
    let env = toml_table_to_map(tbl.get("env").and_then(|e| e.as_table()));
    Some(McpConfigRaw {
        transport: McpTransport::Stdio,
        command,
        args,
        env,
        url: String::new(),
        headers: BTreeMap::new(),
    })
}

fn build_codex_entry(cfg: &McpConfigRaw) -> toml::Value {
    let mut tbl = toml::map::Map::new();
    tbl.insert(
        "command".to_string(),
        toml::Value::String(cfg.command.clone()),
    );
    tbl.insert(
        "args".to_string(),
        toml::Value::Array(
            cfg.args
                .iter()
                .map(|s| toml::Value::String(s.clone()))
                .collect(),
        ),
    );
    if !cfg.env.is_empty() {
        let env_tbl: toml::map::Map<String, toml::Value> = cfg
            .env
            .iter()
            .map(|(k, v)| (k.clone(), toml::Value::String(v.clone())))
            .collect();
        tbl.insert("env".to_string(), toml::Value::Table(env_tbl));
    }
    toml::Value::Table(tbl)
}

impl McpAgentBackend for CodexBackend {

    fn read_all(&self) -> Result<Vec<(String, McpConfigRaw)>, String> {
        let path = codex_config_path()?;
        if !path.exists() {
            return Ok(vec![]);
        }
        let root = read_codex_toml(&path)?;
        let mut out = vec![];
        if let Some(servers) = root.get("mcp_servers").and_then(|v| v.as_table()) {
            for (name, val) in servers {
                if let Some(cfg) = parse_codex_entry(val) {
                    out.push((name.clone(), cfg));
                }
            }
        }
        Ok(out)
    }

    fn write(&self, name: &str, cfg: &McpConfigRaw) -> Result<(), String> {
        // codex 仅 stdio；非 stdio 调用方应已过滤，这里再保底跳过。
        if cfg.transport != McpTransport::Stdio {
            return Ok(());
        }
        let path = codex_config_path()?;
        let mut root = read_codex_toml(&path)?;
        let servers = root
            .as_table_mut()
            .ok_or("config.toml root is not a table")?
            .entry("mcp_servers".to_string())
            .or_insert_with(|| toml::Value::Table(toml::map::Map::new()));
        let servers_tbl = servers
            .as_table_mut()
            .ok_or("mcp_servers is not a table")?;
        servers_tbl.insert(name.to_string(), build_codex_entry(cfg));
        write_codex_toml(&path, &root)
    }

    fn remove(&self, name: &str) -> Result<(), String> {
        let path = codex_config_path()?;
        if !path.exists() {
            return Ok(());
        }
        let mut root = read_codex_toml(&path)?;
        if let Some(servers) = root
            .get_mut("mcp_servers")
            .and_then(|v| v.as_table_mut())
        {
            servers.remove(name);
        }
        write_codex_toml(&path, &root)
    }
}

// ─── domain 逻辑（scan / import / set_agent / delete） ──────

/// 扫描所有 agent 配置，去重合并（同名取首次出现的配置）。
pub async fn scan_all(db: &Db) -> Result<Vec<McpScanItem>, String> {
    let existing: std::collections::HashSet<String> =
        super::db::list_mcp_server_names(db)
            .await
            .unwrap_or_default()
            .into_iter()
            .collect();

    // name → (cfg, agents found in)
    let mut merged: BTreeMap<String, (McpConfigRaw, Vec<McpAgent>)> = BTreeMap::new();
    for agent in McpAgent::all() {
        let be = backend_for(*agent);
        let entries = be.read_all().unwrap_or_else(|e| {
            tracing::warn!(agent = agent.slug(), error = %e, "mcp scan: read agent config failed");
            vec![]
        });
        for (name, cfg) in entries {
            merged
                .entry(name)
                .or_insert_with(|| (cfg.clone(), vec![]))
                .1
                .push(*agent);
        }
    }

    let items = merged
        .into_iter()
        .map(|(name, (cfg, agents))| {
            let already = existing.contains(&name);
            McpScanItem {
                transport: cfg.transport.as_str().to_string(),
                found_in_agents: agents.iter().map(|a| a.slug().to_string()).collect(),
                already_imported: already,
                env: mask_env(cfg.env),
                headers: mask_env(cfg.headers),
                args: cfg.args,
                command: cfg.command,
                url: cfg.url,
                name,
            }
        })
        .collect();
    Ok(items)
}

/// 导入：每项从 source agent 配置取原值（优先于前端脱敏值），入 DB，enabled = source agent。
pub async fn import_items(db: &Db, items: Vec<McpImportPayload>) -> Result<ImportReport, String> {
    let mut imported = vec![];
    let mut skipped = vec![];
    for item in items {
        let source_agent = match McpAgent::from_slug(&item.source_agent) {
            Some(a) => a,
            None => {
                tracing::warn!(
                    name = %item.name,
                    source_agent = %item.source_agent,
                    "mcp import: unknown source agent slug"
                );
                skipped.push(item.name.clone());
                continue;
            }
        };
        // 从 source agent 配置取原值（env 含真实密钥，前端只拿到 ***）。
        let raw_cfg = match backend_for(source_agent).read_all() {
            Ok(entries) => entries
                .into_iter()
                .find(|(n, _)| n == &item.name)
                .map(|(_, c)| c),
            Err(e) => {
                tracing::warn!(error = %e, "mcp import: read source agent config failed");
                None
            }
        };
        let cfg = match raw_cfg {
            Some(c) => c,
            None => McpConfigRaw {
                // 取不到原值 → 用前端传值（env 可能是 *** 占位）。
                transport: McpTransport::parse(&item.transport),
                command: item.command,
                args: item.args,
                env: item.env,
                url: item.url,
                headers: item.headers,
            },
        };
        let now = super::db::now();
        let row = McpServerRow {
            id: 0,
            name: item.name.clone(),
            transport: cfg.transport.as_str().to_string(),
            command: cfg.command,
            args_json: serde_json::to_string(&cfg.args).unwrap_or_else(|_| "[]".into()),
            env_json: serde_json::to_string(&cfg.env).unwrap_or_else(|_| "{}".into()),
            url: cfg.url,
            headers_json: serde_json::to_string(&cfg.headers).unwrap_or_else(|_| "{}".into()),
            enabled_agents: source_agent.slug().to_string(),
            created_at: now,
            updated_at: now,
        };
        match super::db::upsert_mcp_server(db, &row).await {
            Ok(_) => imported.push(item.name),
            Err(e) => {
                tracing::warn!(error = %e, "mcp import: upsert failed");
                skipped.push(item.name);
            }
        }
    }
    Ok(ImportReport { imported, skipped })
}

/// per-agent 启用/禁用：改 DB enabled_agents + 同步写/删 agent 配置。
pub async fn set_agent_enabled(
    db: &Db,
    name: &str,
    agent: McpAgent,
    enabled: bool,
) -> Result<(), String> {
    let row = super::db::get_mcp_server(db, name)
        .await?
        .ok_or_else(|| format!("mcp server not found: {name}"))?;

    // transport 兼容检查（codex 仅 stdio）。
    let transport = row.transport_enum();
    if enabled && !transport.supported_by(agent) {
        return Err(format!(
            "transport {} not supported by {}",
            transport.as_str(),
            agent.display()
        ));
    }

    let mut agents = row.enabled_set();
    if enabled {
        if !agents.contains(&agent) {
            agents.push(agent);
        }
    } else {
        agents.retain(|a| *a != agent);
    }
    let csv = agents
        .iter()
        .map(|a| a.slug())
        .collect::<Vec<_>>()
        .join(",");
    super::db::set_mcp_server_enabled_agents(db, name, &csv).await?;

    // 同步 agent 配置文件。
    let be = backend_for(agent);
    if enabled {
        let cfg = row.to_raw_cfg();
        be.write(name, &cfg)?;
    } else {
        be.remove(name)?;
    }
    Ok(())
}

/// 手动添加：校验 name 非空/不重复 → upsert（enabled_agents 空，不写任何 agent 配置）。
/// 用户添加后通过 set_agent_enabled 逐 agent 启用（那时才写配置）。
pub async fn add_server(db: &Db, payload: McpUpdatePayload) -> Result<McpServerInfo, String> {
    let name = payload.name.trim().to_string();
    if name.is_empty() {
        return Err("name is required".into());
    }
    if super::db::get_mcp_server(db, &name).await?.is_some() {
        return Err(format!("mcp server already exists: {name}"));
    }
    let transport = McpTransport::parse(&payload.transport);
    let cfg = McpConfigRaw {
        transport,
        command: payload.command,
        args: payload.args,
        env: payload.env,
        url: payload.url,
        headers: payload.headers,
    };
    let now = super::db::now();
    let row = McpServerRow {
        id: 0,
        name: name.clone(),
        transport: transport.as_str().to_string(),
        command: cfg.command,
        args_json: serde_json::to_string(&cfg.args).unwrap_or_else(|_| "[]".into()),
        env_json: serde_json::to_string(&cfg.env).unwrap_or_else(|_| "{}".into()),
        url: cfg.url,
        headers_json: serde_json::to_string(&cfg.headers).unwrap_or_else(|_| "{}".into()),
        enabled_agents: String::new(),
        created_at: now,
        updated_at: now,
    };
    super::db::upsert_mcp_server(db, &row).await?;
    super::db::get_mcp_server(db, &name)
        .await?
        .map(McpServerInfo::from)
        .ok_or_else(|| format!("mcp server not found after insert: {name}"))
}

/// 删除：从所有 enabled agent 配置移除 + DB 删行。
pub async fn delete_server(db: &Db, name: &str) -> Result<(), String> {
    let row = super::db::get_mcp_server(db, name).await?;
    if let Some(row) = &row {
        for agent in row.enabled_set() {
            let be = backend_for(agent);
            if let Err(e) = be.remove(name) {
                tracing::warn!(
                    agent = agent.slug(),
                    error = %e,
                    "mcp delete: remove from agent config failed"
                );
            }
        }
    }
    super::db::delete_mcp_server(db, name).await?;
    Ok(())
}

/// 重新同步：遍历所有 MCP server，对每个 enabled agent 从 DB 全量重写 agent 配置文件。
/// 修复 agent 配置文件被外部（CLI / app / 手动）污染导致的失效（如 env:null 致 Claude Code 跳过 server）。
/// aidog 的 write 恒为全量 replace（build_claude_entry 重建 entry），所以重写 = 用 DB 干净值覆盖文件。
/// 返回成功重写的 (agent, name) 数量；单条失败记 warn 不中断（best-effort）。
pub async fn resync_all(db: &Db) -> Result<usize, String> {
    let rows = super::db::list_mcp_servers(db).await?;
    let mut count = 0usize;
    for row in rows {
        for agent in row.enabled_set() {
            let be = backend_for(agent);
            let cfg = row.to_raw_cfg();
            match be.write(&row.name, &cfg) {
                Ok(()) => count += 1,
                Err(e) => tracing::warn!(
                    agent = agent.slug(),
                    server = %row.name,
                    error = %e,
                    "mcp resync: write agent config failed"
                ),
            }
        }
    }
    Ok(count)
}

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

/// 脱敏 merge：incoming 中值为 "***" 的 key → 取 old 明文；其余用新值。
/// 前端编辑表单初始用脱敏值，用户未改的字段提交 "***"，此处还原。
fn merge_masked(
    incoming: BTreeMap<String, String>,
    old: &BTreeMap<String, String>,
) -> BTreeMap<String, String> {
    incoming
        .into_iter()
        .map(|(k, v)| {
            if v.as_str() == "***" {
                (k.clone(), old.get(&k).cloned().unwrap_or(v))
            } else {
                (k, v)
            }
        })
        .collect()
}

/// 编辑 MCP：全字段更新（含改名 / transport 切换）+ 同步 enabled agent 配置。
/// - env/headers 脱敏 merge（***→旧 DB 明文）
/// - transport 切换后不支持的 enabled agent 自动移除（agent 配置 remove）
/// - 改名：旧 name agent 配置 remove + DB 旧名删行
/// - upsert 新 row（保留 id/created_at；改名时新行 created_at 沿用旧值）
pub async fn update_server(
    db: &Db,
    old_name: &str,
    payload: McpUpdatePayload,
) -> Result<McpServerInfo, String> {
    let old = super::db::get_mcp_server(db, old_name)
        .await?
        .ok_or_else(|| format!("mcp server not found: {old_name}"))?;

    let new_transport = McpTransport::parse(&payload.transport);

    let env = merge_masked(payload.env, &old.env_map());
    let headers = merge_masked(payload.headers, &old.headers_map());

    let cfg = McpConfigRaw {
        transport: new_transport,
        command: payload.command.clone(),
        args: payload.args.clone(),
        env: env.clone(),
        url: payload.url.clone(),
        headers: headers.clone(),
    };

    // transport 兼容重算：不支持的 enabled agent 移除。
    let enabled = old.enabled_set();
    let kept: Vec<McpAgent> = enabled
        .iter()
        .copied()
        .filter(|a| new_transport.supported_by(*a))
        .collect();
    let dropped: Vec<McpAgent> = enabled
        .into_iter()
        .filter(|a| !new_transport.supported_by(*a))
        .collect();

    // dropped: 旧 name 配置 remove。
    for agent in &dropped {
        if let Err(e) = backend_for(*agent).remove(old_name) {
            tracing::warn!(
                agent = agent.slug(),
                error = %e,
                "mcp update: remove dropped agent config failed"
            );
        }
    }
    // kept: 改名则先 remove 旧 name，再 write 新 name 新 cfg。
    for agent in &kept {
        let be = backend_for(*agent);
        if payload.name != old.name {
            if let Err(e) = be.remove(old_name) {
                tracing::warn!(
                    agent = agent.slug(),
                    error = %e,
                    "mcp update: remove old-name agent config failed"
                );
            }
        }
        be.write(&payload.name, &cfg)
            .map_err(|e| format!("write {} config: {e}", agent.slug()))?;
    }

    // DB：改名删旧 + upsert 新。
    if payload.name != old.name {
        super::db::delete_mcp_server(db, old_name).await?;
    }
    let enabled_csv = kept
        .iter()
        .map(|a| a.slug())
        .collect::<Vec<_>>()
        .join(",");
    let now = super::db::now();
    let row = McpServerRow {
        id: old.id,
        name: payload.name,
        transport: new_transport.as_str().to_string(),
        command: cfg.command,
        args_json: serde_json::to_string(&cfg.args).unwrap_or_else(|_| "[]".into()),
        env_json: serde_json::to_string(&env).unwrap_or_else(|_| "{}".into()),
        url: cfg.url,
        headers_json: serde_json::to_string(&headers).unwrap_or_else(|_| "{}".into()),
        enabled_agents: enabled_csv,
        created_at: old.created_at,
        updated_at: now,
    };
    super::db::upsert_mcp_server(db, &row).await?;

    super::db::get_mcp_server(db, &row.name)
        .await?
        .map(McpServerInfo::from)
        .ok_or_else(|| format!("mcp update: row vanished after upsert: {}", row.name))
}

// ─── Tests ─────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn agent_slug_roundtrip() {
        assert_eq!(McpAgent::ClaudeCode.slug(), "claude-code");
        assert_eq!(McpAgent::Codex.slug(), "codex");
        assert_eq!(
            McpAgent::from_slug("claude-code"),
            Some(McpAgent::ClaudeCode)
        );
        assert_eq!(McpAgent::from_slug("codex"), Some(McpAgent::Codex));
        assert_eq!(McpAgent::from_slug("claude"), None); // 非 "claude"
        assert_eq!(McpAgent::from_slug("unknown"), None);
    }

    #[test]
    fn transport_parse_and_support() {
        assert_eq!(McpTransport::parse("stdio"), McpTransport::Stdio);
        assert_eq!(McpTransport::parse("HTTP"), McpTransport::Http);
        assert_eq!(McpTransport::parse("Sse"), McpTransport::Sse);
        assert_eq!(McpTransport::parse(""), McpTransport::Stdio);
        // codex 仅 stdio
        assert!(McpTransport::Stdio.supported_by(McpAgent::Codex));
        assert!(!McpTransport::Http.supported_by(McpAgent::Codex));
        assert!(!McpTransport::Sse.supported_by(McpAgent::Codex));
        // claude 全支持
        assert!(McpTransport::Http.supported_by(McpAgent::ClaudeCode));
    }

    #[test]
    fn mask_env_sensitive_keys() {
        let mut m = BTreeMap::new();
        m.insert("API_KEY".into(), "sk-secret".into());
        m.insert("AUTH_TOKEN".into(), "tok".into());
        m.insert("PASSWORD".into(), "p".into());
        m.insert("DEBUG".into(), "1".into());
        m.insert("CREDS".into(), "c".into()); // 含 credential? no → 'creds' 不含
        let masked = mask_env(m);
        assert_eq!(masked.get("API_KEY").unwrap(), "***");
        assert_eq!(masked.get("AUTH_TOKEN").unwrap(), "***");
        assert_eq!(masked.get("PASSWORD").unwrap(), "***");
        assert_eq!(masked.get("DEBUG").unwrap(), "1"); // 非敏感保留
        // 'creds' 不含敏感词根 → 保留（credential 子串匹配，creds 无 'credential'）
        assert_eq!(masked.get("CREDS").unwrap(), "c");
    }

    #[test]
    fn parse_claude_entry_stdio() {
        let v = serde_json::json!({
            "type": "stdio",
            "command": "npx",
            "args": ["-y", "foo"],
            "env": {"API_KEY": "x", "DEBUG": "1"}
        });
        let cfg = parse_claude_entry(&v).expect("parse");
        assert_eq!(cfg.transport, McpTransport::Stdio);
        assert_eq!(cfg.command, "npx");
        assert_eq!(cfg.args, vec!["-y".to_string(), "foo".to_string()]);
        assert_eq!(cfg.env.get("API_KEY").unwrap(), "x");
    }

    #[test]
    fn parse_claude_entry_http() {
        let v = serde_json::json!({
            "type": "http",
            "url": "https://mcp.example.com/mcp",
            "headers": {"Authorization": "Bearer x"}
        });
        let cfg = parse_claude_entry(&v).expect("parse");
        assert_eq!(cfg.transport, McpTransport::Http);
        assert_eq!(cfg.url, "https://mcp.example.com/mcp");
        assert_eq!(cfg.headers.get("Authorization").unwrap(), "Bearer x");
        assert!(cfg.command.is_empty());
    }

    #[test]
    fn build_claude_entry_roundtrip() {
        let cfg = McpConfigRaw {
            transport: McpTransport::Stdio,
            command: "uvx".into(),
            args: vec!["duckduckgo-mcp-server".into()],
            env: {
                let mut m = BTreeMap::new();
                m.insert("HTTPS_PROXY".into(), "http://127.0.0.1:7890".into());
                m
            },
            url: String::new(),
            headers: BTreeMap::new(),
        };
        let entry = build_claude_entry(&cfg);
        let back = parse_claude_entry(&entry).expect("roundtrip");
        assert_eq!(back.command, "uvx");
        assert_eq!(back.args, cfg.args);
        assert_eq!(back.env.get("HTTPS_PROXY").unwrap(), "http://127.0.0.1:7890");
    }

    #[test]
    fn parse_codex_entry_stdio() {
        let toml_str = r#"
command = "uvx"
args = ["duckduckgo-mcp-server"]

[env]
ALL_PROXY = "http://127.0.0.1:7890"
"#;
        let v: toml::Value = toml::from_str(toml_str).unwrap();
        let cfg = parse_codex_entry(&v).expect("parse");
        assert_eq!(cfg.transport, McpTransport::Stdio);
        assert_eq!(cfg.command, "uvx");
        assert_eq!(cfg.args, vec!["duckduckgo-mcp-server".to_string()]);
        assert_eq!(cfg.env.get("ALL_PROXY").unwrap(), "http://127.0.0.1:7890");
    }

    #[test]
    fn build_codex_entry_has_env_subtable() {
        let cfg = McpConfigRaw {
            transport: McpTransport::Stdio,
            command: "uvx".into(),
            args: vec!["srv".into()],
            env: {
                let mut m = BTreeMap::new();
                m.insert("KEY".into(), "v".into());
                m
            },
            url: String::new(),
            headers: BTreeMap::new(),
        };
        let entry = build_codex_entry(&cfg);
        let back = parse_codex_entry(&entry).expect("roundtrip");
        assert_eq!(back.command, "uvx");
        assert_eq!(back.env.get("KEY").unwrap(), "v");
    }

    #[test]
    fn row_enabled_set_parse() {
        let row = McpServerRow {
            id: 1,
            name: "foo".into(),
            transport: "stdio".into(),
            command: "".into(),
            args_json: "[]".into(),
            env_json: "{}".into(),
            url: "".into(),
            headers_json: "{}".into(),
            enabled_agents: "claude-code,codex".into(),
            created_at: 0,
            updated_at: 0,
        };
        let set = row.enabled_set();
        assert_eq!(set.len(), 2);
        assert!(set.contains(&McpAgent::ClaudeCode));
        assert!(set.contains(&McpAgent::Codex));
    }

    #[test]
    fn info_masks_sensitive_env() {
        let row = McpServerRow {
            id: 1,
            name: "foo".into(),
            transport: "stdio".into(),
            command: "npx".into(),
            args_json: "[]".into(),
            env_json: r#"{"API_KEY":"secret","DEBUG":"1"}"#.into(),
            url: "".into(),
            headers_json: "{}".into(),
            enabled_agents: "claude-code".into(),
            created_at: 0,
            updated_at: 0,
        };
        let info = McpServerInfo::from(row);
        assert_eq!(info.env.get("API_KEY").unwrap(), "***");
        assert_eq!(info.env.get("DEBUG").unwrap(), "1");
    }

    #[test]
    fn merge_masked_keeps_old_secret_for_placeholder() {
        let mut old = BTreeMap::new();
        old.insert("API_KEY".into(), "sk-real".into());
        old.insert("DEBUG".into(), "0".into());
        // 前端未改 API_KEY（*** 占位），改 DEBUG，加 NEW_VAR，删（不传）无
        let mut incoming = BTreeMap::new();
        incoming.insert("API_KEY".into(), "***".into());
        incoming.insert("DEBUG".into(), "1".into());
        incoming.insert("NEW_VAR".into(), "x".into());
        let merged = merge_masked(incoming, &old);
        assert_eq!(merged.get("API_KEY").unwrap(), "sk-real"); // *** → 旧明文
        assert_eq!(merged.get("DEBUG").unwrap(), "1"); // 新值
        assert_eq!(merged.get("NEW_VAR").unwrap(), "x"); // 新 key
    }

    #[test]
    fn merge_masked_placeholder_without_old_falls_back() {
        // *** 但旧 DB 无该 key → 保留 ***（不应发生，但兜底不 panic）
        let old = BTreeMap::new();
        let mut incoming = BTreeMap::new();
        incoming.insert("X".into(), "***".into());
        let merged = merge_masked(incoming, &old);
        assert_eq!(merged.get("X").unwrap(), "***");
    }
}
