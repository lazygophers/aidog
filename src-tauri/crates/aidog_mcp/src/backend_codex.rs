//! Codex agent 配置后端：`~/.codex/config.toml` 的 `[mcp_servers.<name>]`（TOML，stdio + streamable http）。

use std::collections::BTreeMap;
use std::path::PathBuf;

use super::McpAgentBackend;
use super::types::{McpConfigRaw, McpTransport};

pub(super) struct CodexBackend;

fn codex_config_path() -> Result<PathBuf, String> {
    Ok(aidog_db::codex_home()?.join("config.toml"))
}

fn read_codex_toml(path: &PathBuf) -> Result<toml::Value, String> {
    if !path.exists() {
        return Ok(toml::Value::Table(toml::map::Map::new()));
    }
    let content = std::fs::read_to_string(path).map_err(|e| format!("read config.toml: {e}"))?;
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

fn toml_table_to_map(
    tbl: Option<&toml::map::Map<String, toml::Value>>,
) -> BTreeMap<String, String> {
    tbl.map(|m| {
        m.iter()
            .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
            .collect()
    })
    .unwrap_or_default()
}

/// 解析 codex `[mcp_servers.<name>]` 段 → McpConfigRaw（带 `url` = http，否则 stdio）。
fn parse_codex_entry(v: &toml::Value) -> Option<McpConfigRaw> {
    let tbl = v.as_table()?;
    if let Some(url) = tbl.get("url").and_then(|u| u.as_str()) {
        return Some(McpConfigRaw {
            transport: McpTransport::Http,
            command: String::new(),
            args: vec![],
            env: BTreeMap::new(),
            url: url.to_string(),
            headers: toml_table_to_map(tbl.get("http_headers").and_then(|e| e.as_table())),
        });
    }
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
    if cfg.transport == McpTransport::Http {
        tbl.insert("url".to_string(), toml::Value::String(cfg.url.clone()));
        if !cfg.headers.is_empty() {
            let h: toml::map::Map<String, toml::Value> = cfg
                .headers
                .iter()
                .map(|(k, v)| (k.clone(), toml::Value::String(v.clone())))
                .collect();
            tbl.insert("http_headers".to_string(), toml::Value::Table(h));
        }
        return toml::Value::Table(tbl);
    }
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
        // codex 不支持 sse；调用方应已过滤，这里再保底跳过。
        if cfg.transport == McpTransport::Sse {
            return Ok(());
        }
        let path = codex_config_path()?;
        let mut root = read_codex_toml(&path)?;
        let servers = root
            .as_table_mut()
            .ok_or("config.toml root is not a table")?
            .entry("mcp_servers".to_string())
            .or_insert_with(|| toml::Value::Table(toml::map::Map::new()));
        let servers_tbl = servers.as_table_mut().ok_or("mcp_servers is not a table")?;
        servers_tbl.insert(name.to_string(), build_codex_entry(cfg));
        write_codex_toml(&path, &root)
    }

    fn remove(&self, name: &str) -> Result<(), String> {
        let path = codex_config_path()?;
        if !path.exists() {
            return Ok(());
        }
        let mut root = read_codex_toml(&path)?;
        if let Some(servers) = root.get_mut("mcp_servers").and_then(|v| v.as_table_mut()) {
            servers.remove(name);
        }
        write_codex_toml(&path, &root)
    }
}

#[cfg(test)]
#[path = "test_backend_codex.rs"]
mod test_backend_codex;
