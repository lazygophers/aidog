//! ClaudeCode agent 配置后端：`~/.claude.json` 的 `mcpServers`（JSON object）。

use std::collections::BTreeMap;
use std::path::PathBuf;

use super::types::{McpConfigRaw, McpTransport};
use super::McpAgentBackend;

pub(super) struct ClaudeCodeBackend;

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
/// pub(super)：粘贴导入 (domain::parse_pasted_json) 复用同一 claude 协议解析。
pub(super) fn parse_claude_entry(v: &serde_json::Value) -> Option<McpConfigRaw> {
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

#[cfg(test)]
#[path = "test_backend_claude.rs"]
mod test_backend_claude;
