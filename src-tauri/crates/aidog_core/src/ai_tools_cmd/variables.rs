use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use aidog_db::{self as db, models::SetSettingInput};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

/// 独立管理的环境变量。目标不再单选：保存时同名附加并覆盖
/// Claude Code（DB 全局配置 env）与 Codex（config.toml
/// shell_environment_policy.set），两边同时生效。
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Variable {
    pub key: String,
    pub value: String,
}

/// 单变量 own-write 账本：before = aidog 覆盖前该键的值（None = 原先不存在），
/// after = aidog 写入的值。撤销时仅当当前值仍等于 after 才恢复 before；
/// 用户改过的键保留用户值并落账。
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct VarApplied {
    pub before: Option<Value>,
    pub after: Value,
}

pub type AppliedLedger = BTreeMap<String, VarApplied>;

const VARIABLES_KEY: &str = "extension_variables";
const CLAUDE_APPLIED_KEY: &str = "variables_claude_applied";
const CODEX_APPLIED_KEY: &str = "variables_codex_applied";

fn valid_key(key: &str) -> bool {
    let mut chars = key.chars();
    chars
        .next()
        .is_some_and(|c| c == '_' || c.is_ascii_alphabetic())
        && chars.all(|c| c == '_' || c.is_ascii_alphanumeric())
        && key.len() <= 128
}

fn validate(v: &Variable) -> Result<(), String> {
    if !valid_key(&v.key) || v.value.len() > 8192 {
        return Err(format!("invalid environment variable: {}", v.key));
    }
    Ok(())
}

/// 环境变量覆盖应用（Claude env）：同名附加并覆盖，返回（新 env，新账本）。
/// 空值 = 不应用（此前 aidog 写过则撤销）。非字符串旧值原样保留进 before。
pub(crate) fn env_apply(
    mut env: Map<String, Value>,
    ledger: &AppliedLedger,
    variables: &[Variable],
) -> Result<(Map<String, Value>, AppliedLedger), String> {
    let mut next_ledger = AppliedLedger::new();
    for v in variables {
        validate(v)?;
        if v.value.is_empty() {
            continue;
        }
        let value = Value::String(v.value.clone());
        let current = env.get(&v.key).cloned();
        let rec = match ledger.get(&v.key) {
            Some(r) if current.as_ref() == Some(&r.after) => {
                VarApplied { before: r.before.clone(), after: value.clone() }
            }
            _ => VarApplied { before: current, after: value.clone() },
        };
        env.insert(v.key.clone(), value);
        next_ledger.insert(v.key.clone(), rec);
    }
    for (key, r) in ledger {
        if next_ledger.contains_key(key) {
            continue;
        }
        if env.get(key) == Some(&r.after) {
            match &r.before {
                Some(b) => {
                    env.insert(key.clone(), b.clone());
                }
                None => {
                    env.remove(key);
                }
            }
        }
    }
    Ok((env, next_ledger))
}

/// 账本装载：新形状 {k:{before,after}}；旧扁平 {k:"v"} → before=None
/// （旧语义未覆盖用户值，撤销 = 删行）。非法形状一律空账本。
pub(crate) fn parse_ledger(v: Option<Value>) -> AppliedLedger {
    v.as_ref()
        .and_then(Value::as_object)
        .map(|o| {
            o.iter()
                .filter_map(|(k, val)| {
                    let rec = if val.is_object() {
                        serde_json::from_value::<VarApplied>(val.clone()).ok()
                    } else {
                        // 旧扁平形状：值就是 aidog 写入的字符串（旧语义不覆盖用户值，
                        // 撤销 = 删行，无用户原值可恢复）。
                        val.as_str()
                            .map(|s| VarApplied { before: None, after: Value::String(s.to_owned()) })
                    };
                    rec.map(|r| (k.clone(), r))
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Codex config.toml 覆盖应用（语义同 env_apply），返回（新文件内容，新账本）。
pub(crate) fn codex_apply(
    source: &str,
    ledger: &AppliedLedger,
    variables: &[Variable],
) -> Result<(String, AppliedLedger), String> {
    let parsed: toml::Value =
        toml::from_str(source).map_err(|e| format!("invalid Codex config.toml: {e}"))?;
    let mut current = BTreeMap::<String, String>::new();
    if let Some(t) = parsed
        .get("shell_environment_policy")
        .and_then(|v| v.get("set"))
        .and_then(toml::Value::as_table)
    {
        for (k, v) in t {
            let v = v
                .as_str()
                .ok_or_else(|| format!("Codex variable {k} is not a string"))?;
            current.insert(k.clone(), v.to_owned());
        }
    }
    let mut next_ledger = AppliedLedger::new();
    // key → Some(值) = 写入，None = 删行。
    let mut writes = BTreeMap::<String, Option<String>>::new();
    for v in variables {
        validate(v)?;
        if v.value.is_empty() {
            continue;
        }
        let rec = match ledger.get(&v.key) {
            Some(r) if current.get(&v.key).map(String::as_str) == r.after.as_str() => VarApplied {
                before: r.before.clone(),
                after: Value::String(v.value.clone()),
            },
            _ => VarApplied {
                before: current.get(&v.key).map(|s| Value::String(s.clone())),
                after: Value::String(v.value.clone()),
            },
        };
        writes.insert(v.key.clone(), Some(v.value.clone()));
        next_ledger.insert(v.key.clone(), rec);
    }
    for (key, r) in ledger {
        if next_ledger.contains_key(key) {
            continue;
        }
        if current.get(key).map(String::as_str) == r.after.as_str() {
            match &r.before {
                Some(Value::String(b)) => {
                    writes.insert(key.clone(), Some(b.clone()));
                }
                _ => {
                    writes.insert(key.clone(), None);
                }
            }
        }
    }
    let mut next = source.to_string();
    for (k, w) in &writes {
        next = match w {
            Some(v) => set_assignment(&next, k, v),
            None => remove_assignment(&next, k),
        };
    }
    // 编辑后的结果必须仍是合法 TOML 才允许落盘。
    let _: toml::Value = toml::from_str(&next)
        .map_err(|e| format!("invalid Codex config.toml after update: {e}"))?;
    Ok((next, next_ledger))
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

/// 有效全局配置：DB 空时回落内置 oss 预设（同 context_compress::load_claude_cfg）。
async fn claude_cfg(db: &db::Db) -> Value {
    db::get_setting(db, "global", "claude_code")
        .await
        .ok()
        .flatten()
        .filter(|v| v.as_object().is_some_and(|o| !o.is_empty()))
        .unwrap_or_else(crate::sync_settings::bundled_oss_preset)
}

async fn load_ledger(db: &db::Db, key: &str) -> AppliedLedger {
    parse_ledger(
        db::get_setting(db, "global", key)
            .await
            .ok()
            .flatten(),
    )
}

async fn store_ledger(db: &db::Db, key: &str, ledger: &AppliedLedger) -> Result<(), String> {
    db::set_setting(db, SetSettingInput {
        scope: "global".into(),
        key: key.into(),
        value: serde_json::to_value(ledger).map_err(|e| e.to_string())?,
    })
    .await
    .map(|_| ())
}

fn atomic_write_keep_mode(path: &Path, content: &str) -> Result<(), String> {
    #[cfg(unix)]
    let mode = {
        use std::os::unix::fs::PermissionsExt;
        fs::metadata(path)
            .ok()
            .map(|m| m.permissions().mode() & 0o777)
    };
    #[cfg(not(unix))]
    let mode = None;
    super::profile::atomic_write(path, content, mode)
}

crate::tauri_command! {
    /// 变量独立保存：同名附加并覆盖 Claude Code env 与 Codex
    /// shell_environment_policy.set，两边同时生效；撤销仅还原 aidog
    /// 写入且未被用户改动的键（恢复覆盖前原值）。空值 = 撤销该变量。
    pub async fn variables_save(variables: Vec<Variable>) -> Result<(), String> {
        let db = aidog_ctx::db();
        // 先整体校验再动手：任何非法键 / 超长值都不落任何一侧。
        for v in &variables {
            validate(v)?;
        }
        db::set_setting(db, SetSettingInput {
            scope: "global".into(),
            key: VARIABLES_KEY.into(),
            value: serde_json::to_value(&variables).map_err(|e| e.to_string())?,
        })
        .await
        .map(|_| ())?;

        // Claude Code：全局配置 env，写完走与设置页相同的分组同步。
        let cfg = claude_cfg(db).await;
        let ledger = load_ledger(db, CLAUDE_APPLIED_KEY).await;
        let env = cfg.get("env").and_then(Value::as_object).cloned().unwrap_or_default();
        let (env, claude_next) = env_apply(env, &ledger, &variables)?;
        let mut next = cfg;
        if env.is_empty() {
            if let Some(o) = next.as_object_mut() {
                o.remove("env");
            }
        } else {
            next["env"] = Value::Object(env);
        }
        db::set_setting(db, SetSettingInput {
            scope: "global".into(),
            key: "claude_code".into(),
            value: next,
        })
        .await
        .map(|_| ())?;
        crate::sync_settings::try_sync_settings(db).await;
        store_ledger(db, CLAUDE_APPLIED_KEY, &claude_next).await?;

        // Codex：config.toml [shell_environment_policy.set]。
        let path = aidog_db::codex_home()?.join("config.toml");
        let source = match fs::read_to_string(&path) {
            Ok(s) => s,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
            Err(e) => return Err(e.to_string()),
        };
        let codex_ledger = load_ledger(db, CODEX_APPLIED_KEY).await;
        let (next_source, codex_next) = codex_apply(&source, &codex_ledger, &variables)?;
        if next_source != source {
            atomic_write_keep_mode(&path, &next_source)?;
        }
        store_ledger(db, CODEX_APPLIED_KEY, &codex_next).await?;
        Ok(())
    }
}

#[cfg(test)]
#[path = "test_variables.rs"]
mod test_variables;
