use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use aidog_db::{self as db, models::SetSettingInput};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Variable {
    pub key: String,
    pub value: String,
    pub claude: bool,
    pub codex: bool,
}

const APPLIED_KEY: &str = "variables_codex_applied";

fn valid_key(key: &str) -> bool {
    let mut chars = key.chars();
    chars
        .next()
        .is_some_and(|c| c == '_' || c.is_ascii_alphabetic())
        && chars.all(|c| c == '_' || c.is_ascii_alphanumeric())
        && key.len() <= 128
}

fn save_codex(
    path: &Path,
    variables: &[Variable],
    previous: &BTreeMap<String, String>,
) -> Result<(BTreeMap<String, String>, Vec<String>), String> {
    let source = match fs::read_to_string(path) {
        Ok(source) => source,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(error) => return Err(error.to_string()),
    };
    let mut parsed: toml::Value = toml::from_str(&source)
        .map_err(|e| format!("invalid Codex config.toml: {e}"))?;
    let set = parsed
        .get("shell_environment_policy")
        .and_then(|v| v.get("set"));
    let mut current = BTreeMap::new();
    if let Some(table) = set.and_then(toml::Value::as_table) {
        for (key, value) in table {
            let value = value
                .as_str()
                .ok_or_else(|| format!("Codex variable {key} is not a string"))?;
            current.insert(key.clone(), value.to_owned());
        }
    }
    let desired: BTreeMap<_, _> = variables
        .iter()
        .filter(|v| v.codex)
        .map(|v| (v.key.clone(), v.value.clone()))
        .collect();
    let mut conflicts = Vec::new();
    let mut next = source.clone();
    let mut owned = BTreeMap::new();

    for (key, value) in &desired {
        if !valid_key(key) || value.len() > 8192 {
            return Err(format!("invalid environment variable: {key}"));
        }
        match (current.get(key), previous.get(key)) {
            (Some(now), Some(written)) if now == written => {
                owned.insert(key.clone(), value.clone());
            }
            (Some(_), _) => conflicts.push(key.clone()),
            (None, _) => {
                owned.insert(key.clone(), value.clone());
            }
        }
    }
    for key in previous.keys().filter(|key| !desired.contains_key(*key)) {
        if current.get(key) == previous.get(key) {
            next = remove_assignment(&next, key);
        }
    }
    for (key, value) in &owned {
        if conflicts.contains(key) {
            continue;
        }
        next = set_assignment(&next, key, value);
    }
    // Reject malformed TOML before any write; parse the edited result as well.
    parsed = toml::from_str(&next)
        .map_err(|e| format!("invalid Codex config.toml after update: {e}"))?;
    let _ = parsed;
    if next != source {
        #[cfg(unix)]
        let mode = {
            use std::os::unix::fs::PermissionsExt;
            fs::metadata(path)
                .ok()
                .map(|metadata| metadata.permissions().mode() & 0o777)
        };
        #[cfg(not(unix))]
        let mode = None;
        super::profile::atomic_write(path, &next, mode)?;
    }
    Ok((owned, conflicts))
}

/// 单行 basic string 转义（toml 1.x 的 `Value` Display 把换行渲染成多行 `"""`，
/// 写出统一走单行 `\n` 转义，保持 config.toml 逐行可 diff）。
fn toml_str(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 2);
    out.push('"');
    for c in value.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c.is_control() => out.push_str(&format!("\\u{:04X}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn set_assignment(source: &str, key: &str, value: &str) -> String {
    let rendered = format!("{key} = {}", toml_str(value));
    let mut lines: Vec<String> = source.lines().map(str::to_owned).collect();
    let start = lines
        .iter()
        .position(|line| line.trim() == "[shell_environment_policy.set]");
    if let Some(start) = start {
        let end = lines
            .iter()
            .enumerate()
            .skip(start + 1)
            .find(|(_, line)| line.trim_start().starts_with('['))
            .map(|(i, _)| i)
            .unwrap_or(lines.len());
        if let Some(index) = (start + 1..end).find(|&i| {
            lines[i]
                .split_once('=')
                .is_some_and(|(k, _)| k.trim() == key)
        }) {
            lines[index] = rendered;
        } else {
            lines.insert(end, rendered);
        }
    } else {
        if !lines.is_empty() && !lines.last().is_some_and(String::is_empty) {
            lines.push(String::new());
        }
        lines.push("[shell_environment_policy.set]".into());
        lines.push(rendered);
    }
    format!("{}\n", lines.join("\n"))
}

fn remove_assignment(source: &str, key: &str) -> String {
    let mut lines: Vec<String> = source.lines().map(str::to_owned).collect();
    let Some(start) = lines
        .iter()
        .position(|line| line.trim() == "[shell_environment_policy.set]")
    else {
        return source.to_owned();
    };
    let end = lines
        .iter()
        .enumerate()
        .skip(start + 1)
        .find(|(_, line)| line.trim_start().starts_with('['))
        .map(|(i, _)| i)
        .unwrap_or(lines.len());
    if let Some(index) = (start + 1..end).find(|&i| {
        lines[i]
            .split_once('=')
            .is_some_and(|(k, _)| k.trim() == key)
    }) {
        lines.remove(index);
    }
    format!("{}\n", lines.join("\n"))
}

crate::tauri_command! {
    /// 将所选变量合并到 Codex config.toml，只撤销 aidog 曾写入且值未被用户改动的键。
    pub async fn variables_codex_save(variables: Vec<Variable>) -> Result<Vec<String>, String> {
        let db = aidog_ctx::db();
        let path = aidog_db::codex_home()?.join("config.toml");
        let previous: BTreeMap<String, String> = db::get_setting(db, "global", APPLIED_KEY).await?
            .and_then(|v| serde_json::from_value(v).ok()).unwrap_or_default();
        let (owned, conflicts) = save_codex(&path, &variables, &previous)?;
        db::set_setting(db, SetSettingInput { scope: "global".into(), key: APPLIED_KEY.into(), value: serde_json::to_value(owned).map_err(|e| e.to_string())? }).await?;
        Ok(conflicts)
    }
}

#[cfg(test)]
#[path = "test_variables.rs"]
mod test_variables;
