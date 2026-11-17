//! 扩展 → 插件管理：Codex 插件（Claude 插件捆绑包见 `plugin_bundle.rs`）。
//!
//! Codex：shell out `codex plugin ...`（本机 codex-cli 0.168.0 实测契约，见 test_plugins.rs）：
//!   `plugin list --json` → `{"installed":[{pluginId,name,marketplaceName,version,installed,enabled,...}],"available":[...]}`，
//!   `plugin marketplace list --json` → `{"marketplaces":[{name,root,marketplaceSource:{source,...}}]}`。
//!   stderr 是 WARNING 前缀日志，只解析 stdout；错误透传 stderr；超时 60s。
//!   `CODEX_HOME` 沿 `agent_config::codex_home_dir` 口径传给子进程。

use serde::{Deserialize, Serialize};
use serde_json::Value;

const CODEX_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(60);

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
