//! 扩展 → 插件管理（Claude Code 原生插件 + Codex 插件）。
//!
//! - Claude Code：只读 `~/.claude/plugins/installed_plugins.json`（原生插件运行时的安装清单）
//!   列出已装插件；启停写全局配置 `claude_code.enabledPlugins`（`<plugin>@<marketplace>` → bool，
//!   缺键 = 启用，仅显式 false 关闭），经 `sync_settings` 与设置页 PluginsSection 同链落盘。
//!   禁直写 `~/.claude/settings.json`（会被默认分组同步全量覆盖）。
//! - Codex：shell out `codex plugin ...`（本机 codex-cli 0.168.0 实测契约，见 test_plugins.rs）：
//!   `plugin list --json` → `{"installed":[{pluginId,name,marketplaceName,version,installed,enabled,...}],"available":[...]}`，
//!   `plugin marketplace list --json` → `{"marketplaces":[{name,root,marketplaceSource:{source,...}}]}`。
//!   stderr 是 WARNING 前缀日志，只解析 stdout；错误透传 stderr；超时 60s。
//!   `CODEX_HOME` 沿 `agent_config::codex_home_dir` 口径传给子进程。

use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use serde_json::Value;

const CODEX_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(60);

/// Claude Code 已装插件视图（installed_plugins.json 单条 + enabledPlugins 状态）。
#[derive(Serialize, Clone, Debug)]
pub struct ClaudePluginView {
    /// `<plugin>@<marketplace>`
    pub id: String,
    pub version: String,
    pub install_path: String,
    pub last_updated: String,
    /// enabledPlugins 缺键 = 启用（Claude Code 缺省语义）。
    pub enabled: bool,
    /// enabledPlugins 里有无该键。
    pub configured: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct CodexPlugin {
    #[serde(default)]
    #[serde(rename = "pluginId")]
    pub plugin_id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    #[serde(rename = "marketplaceName")]
    pub marketplace_name: String,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub installed: bool,
    #[serde(default)]
    pub enabled: bool,
}

#[derive(Serialize, Clone, Debug, Default)]
pub struct CodexMarketplace {
    pub name: String,
    /// marketplaceSource.source（本地路径 / git 地址等原始来源）。
    pub source: String,
    pub root: String,
}

#[derive(Serialize, Clone, Debug, Default)]
pub struct CodexPluginsView {
    pub installed: Vec<CodexPlugin>,
    pub marketplaces: Vec<CodexMarketplace>,
}

// ─── Claude Code 侧 ─────────────────────────────────────────

fn claude_installed_path() -> Result<PathBuf, String> {
    let root = match std::env::var("CLAUDE_CONFIG_DIR") {
        Ok(v) if !v.trim().is_empty() => PathBuf::from(v),
        _ => dirs::home_dir()
            .ok_or("cannot resolve home directory")?
            .join(".claude"),
    };
    Ok(root.join("plugins").join("installed_plugins.json"))
}

/// 文件缺失（从未装过插件）→ 空对象，不算错。
fn read_installed_json() -> Result<Value, String> {
    let path = claude_installed_path()?;
    if !path.exists() {
        return Ok(Value::Object(Default::default()));
    }
    let text =
        std::fs::read_to_string(&path).map_err(|e| format!("read {}: {e}", path.display()))?;
    serde_json::from_str(&text).map_err(|e| format!("parse {}: {e}", path.display()))
}

/// 有效全局配置：同 `do_sync_group_settings`（`sync_settings.rs:394`），DB 空或缺省时回落
/// 内置 oss 预设。插件开关整份写回 `claude_code`，若空时落空对象，分组同步会采纳该小对象、
/// 整份覆盖 `~/.claude/settings.json`（hooks / env 等预设键全丢）。
async fn load_claude_raw(db: &aidog_db::Db) -> Value {
    aidog_db::get_setting(db, "global", "claude_code")
        .await
        .ok()
        .flatten()
        .filter(|v| v.as_object().is_some_and(|o| !o.is_empty()))
        .unwrap_or_else(crate::sync_settings::bundled_oss_preset)
}

pub(crate) fn claude_views(_installed: &Value, _cfg: &Value) -> Vec<ClaudePluginView> {
    let enabled = _cfg.get("enabledPlugins").and_then(Value::as_object);
    let mut out: Vec<ClaudePluginView> = _installed
        .as_object()
        .map(|o| {
            o.iter()
                .map(|(id, installs)| {
                    let first = installs.get(0);
                    let flag = enabled.and_then(|e| e.get(id)).and_then(Value::as_bool);
                    ClaudePluginView {
                        id: id.clone(),
                        version: first
                            .and_then(|i| i.get("version"))
                            .and_then(Value::as_str)
                            .unwrap_or_default()
                            .to_string(),
                        install_path: first
                            .and_then(|i| i.get("installPath"))
                            .and_then(Value::as_str)
                            .unwrap_or_default()
                            .to_string(),
                        last_updated: first
                            .and_then(|i| i.get("lastUpdated"))
                            .and_then(Value::as_str)
                            .unwrap_or_default()
                            .to_string(),
                        enabled: flag.unwrap_or(true),
                        configured: flag.is_some(),
                    }
                })
                .collect()
        })
        .unwrap_or_default();
    out.sort_by(|a, b| a.id.cmp(&b.id));
    out
}

pub(crate) fn apply_toggle(cfg: &mut Value, plugin_id: &str, enabled: bool) -> Result<(), String> {
    if plugin_id.trim().is_empty() {
        return Err("plugin_id is empty".into());
    }
    if !cfg.is_object() {
        *cfg = Value::Object(Default::default());
    }
    let obj = cfg.as_object_mut().expect("checked object");
    let map = obj
        .entry("enabledPlugins")
        .or_insert_with(|| Value::Object(Default::default()));
    if !map.is_object() {
        *map = Value::Object(Default::default());
    }
    map.as_object_mut()
        .expect("checked object")
        .insert(plugin_id.to_string(), Value::Bool(enabled));
    Ok(())
}

// ─── Codex 侧 ───────────────────────────────────────────────

async fn codex_run(args: &[&str]) -> Result<String, String> {
    let mut c = tokio::process::Command::new("codex");
    c.args(args).stdin(std::process::Stdio::null());
    // GUI 进程 PATH 极简，补登录 shell PATH（同 skills / context_compress 的探测）
    if let Some(p) = aidog_skills::runtime_path() {
        c.env("PATH", p);
    }
    if let Some(h) = super::agent_config::codex_home_dir() {
        c.env("CODEX_HOME", h);
    }
    let out = tokio::time::timeout(CODEX_TIMEOUT, c.output())
        .await
        .map_err(|_| format!("codex {}: timeout after 60s", args.join(" ")))?
        .map_err(|e| format!("codex: {e}"))?;
    if !out.status.success() {
        let text = format!(
            "{}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
        return Err(format!("codex {}: {}", args.join(" "), text.trim()));
    }
    Ok(String::from_utf8_lossy(&out.stdout).to_string())
}

/// stderr 的 WARNING 前缀日志会混进合并输出，从首个 `{` 起解析。
fn parse_json_stdout(stdout: &str) -> Result<Value, String> {
    let start = stdout
        .find('{')
        .ok_or_else(|| "codex: no JSON object in output".to_string())?;
    serde_json::from_str(stdout[start..].trim()).map_err(|e| format!("codex: invalid JSON: {e}"))
}

pub(crate) fn parse_codex_list(stdout: &str) -> Result<Vec<CodexPlugin>, String> {
    let v = parse_json_stdout(stdout)?;
    serde_json::from_value(v.get("installed").cloned().unwrap_or(Value::Null))
        .map_err(|e| format!("codex: installed: {e}"))
}

pub(crate) fn parse_codex_marketplaces(stdout: &str) -> Result<Vec<CodexMarketplace>, String> {
    let v = parse_json_stdout(stdout)?;
    let arr = v
        .get("marketplaces")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    Ok(arr
        .iter()
        .map(|m| CodexMarketplace {
            name: m
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
            source: m
                .get("marketplaceSource")
                .and_then(|s| s.get("source"))
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
            root: m
                .get("root")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
        })
        .collect())
}

async fn codex_view() -> Result<CodexPluginsView, String> {
    let mut installed = parse_codex_list(&codex_run(&["plugin", "list", "--json"]).await?)?;
    installed.sort_by(|a, b| a.plugin_id.cmp(&b.plugin_id));
    let mut marketplaces =
        parse_codex_marketplaces(&codex_run(&["plugin", "marketplace", "list", "--json"]).await?)?;
    marketplaces.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(CodexPluginsView {
        installed,
        marketplaces,
    })
}

// ─── 命令 ───────────────────────────────────────────────────

crate::tauri_command! {
    pub async fn plugins_claude_list() -> Result<Vec<ClaudePluginView>, String> {
        let db = aidog_ctx::db();
        let installed = read_installed_json()?;
        let cfg = load_claude_raw(db).await;
        Ok(claude_views(&installed, &cfg))
    }
}

crate::tauri_command! {
    /// 启停一个 Claude Code 插件：写 `claude_code.enabledPlugins` 并触发分组同步（与设置页同链）。
    pub async fn plugins_claude_set(plugin_id: String, enabled: bool) -> Result<Vec<ClaudePluginView>, String> {
        let db = aidog_ctx::db();
        let mut cfg = load_claude_raw(db).await;
        apply_toggle(&mut cfg, &plugin_id, enabled)?;
        aidog_db::set_setting(
            db,
            aidog_db::models::SetSettingInput {
                scope: "global".to_string(),
                key: "claude_code".to_string(),
                value: cfg.clone(),
            },
        )
        .await
        .map_err(|e| e.to_string())?;
        crate::sync_settings::try_sync_settings(db).await;
        let installed = read_installed_json()?;
        Ok(claude_views(&installed, &cfg))
    }
}

crate::tauri_command! {
    pub async fn plugins_codex_list() -> Result<CodexPluginsView, String> {
        codex_view().await
    }
}

crate::tauri_command! {
    /// `codex plugin add <plugin_id>`（`name@marketplace`），返回变更后的列表。
    pub async fn plugins_codex_add(plugin_id: String) -> Result<CodexPluginsView, String> {
        codex_run(&["plugin", "add", &plugin_id]).await?;
        codex_view().await
    }
}

crate::tauri_command! {
    /// `codex plugin remove <plugin_id>`，返回变更后的列表。
    pub async fn plugins_codex_remove(plugin_id: String) -> Result<CodexPluginsView, String> {
        codex_run(&["plugin", "remove", &plugin_id]).await?;
        codex_view().await
    }
}

crate::tauri_command! {
    /// `codex plugin marketplace add <source>`（本地路径 / git 地址等），返回变更后的列表。
    pub async fn plugins_codex_marketplace_add(source: String) -> Result<CodexPluginsView, String> {
        codex_run(&["plugin", "marketplace", "add", &source]).await?;
        codex_view().await
    }
}

crate::tauri_command! {
    /// `codex plugin marketplace remove <name>`，返回变更后的列表。
    pub async fn plugins_codex_marketplace_remove(name: String) -> Result<CodexPluginsView, String> {
        codex_run(&["plugin", "marketplace", "remove", &name]).await?;
        codex_view().await
    }
}

#[cfg(test)]
#[path = "test_plugins.rs"]
mod test_plugins;
