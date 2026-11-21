//! 换客户端继续（扩展-会话页 票 10，spec D5）。
//!
//! - Claude Code ⇄ Codex：转写。用户点选时才读源会话原文，按目标格式在目标客户端自己的存储写一个新会话，
//!   再用目标的继续命令在原工作目录打开。规则对齐 Magpie（yetone/magpie:internal/sessions/carry_convert.go）：
//!   Claude 源取最后一条主线行沿 `parentUuid` 回溯（compact 边界无 parent，回溯自然停在那里），丢弃
//!   sidechain / `isMeta` / 斜杠命令回显 / `<system-reminder>`；Codex 源取 `response_item`，遇 `compacted`
//!   用其 `replacement_history`，丢弃 `<environment_context>` / `<user_instructions>` / AGENTS.md / developer 消息。
//!   只搬用户与助手的文字；原文不进 AiDog 任何数据库表与日志。
//! - 其余组合：交接。不读原文，在目标客户端以原工作目录启动新会话，首条提示指向原会话文件路径。
//!
//! 闸门：转写要求「允许读取对话原文」（`transcript_consent`）开且源客户端读取开关开（`consent_allows`），
//! 两者在读任何文件之前判定。

use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use crate::proxy_cmd::session_cmd::{SessionResumeResult, build_session_resume_line, shell_squote};

/// 客户端码 → PATH 上的二进制名。
const CLIENT_BINS: &[(&str, &str)] = &[
    ("claude_code", "claude"),
    ("codex", "codex"),
    ("gemini_cli", "gemini"),
    ("opencode", "opencode"),
];

/// 转写对（其余组合走交接）。
pub(crate) fn is_transcribe_pair(from: &str, to: &str) -> bool {
    matches!((from, to), ("claude_code", "codex") | ("codex", "claude_code"))
}

/// 一条要搬过去的话。
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Item {
    User(String),
    Assistant(String),
}

fn str_of<'a>(v: &'a Value, k: &str) -> &'a str {
    v.get(k).and_then(Value::as_str).unwrap_or("")
}

/// Claude Code 自己写进用户行的内容（斜杠命令回显、提醒、命令前的提示语）。
fn claude_own_words(text: &str) -> bool {
    let t = text.trim();
    t.is_empty()
        || ["<command-name>", "<command-message>", "<command-args>", "<local-command-", "<system-reminder>", "Caveat: The messages below were generated"]
            .iter()
            .any(|p| t.starts_with(p))
}

/// 读 Claude Code 会话原文 → 主线文字。
pub(crate) fn claude_items(text: &str) -> Vec<Item> {
    let mut parent: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    let mut said: std::collections::HashMap<String, Value> = std::collections::HashMap::new();
    let mut leaf = String::new();
    for l in text.lines() {
        let Ok(v) = serde_json::from_str::<Value>(l) else { continue };
        let uuid = str_of(&v, "uuid").to_string();
        if uuid.is_empty() {
            continue;
        }
        parent.insert(uuid.clone(), str_of(&v, "parentUuid").to_string());
        let ty = str_of(&v, "type");
        if (ty == "user" || ty == "assistant") && v.get("isSidechain").and_then(Value::as_bool) != Some(true) {
            leaf = uuid.clone();
            said.insert(uuid, v);
        }
    }
    let mut chain = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let mut id = leaf;
    while !id.is_empty() && seen.insert(id.clone()) {
        if let Some(v) = said.remove(&id) {
            chain.push(v);
        }
        id = parent.get(&id).cloned().unwrap_or_default();
    }
    let mut items = Vec::new();
    for v in chain.into_iter().rev() {
        if v.get("isMeta").and_then(Value::as_bool) == Some(true) {
            continue;
        }
        let user = str_of(&v, "type") == "user";
        let content = &v["message"]["content"];
        if let Some(s) = content.as_str() {
            if user && claude_own_words(s) {
                continue;
            }
            items.push(if user { Item::User(s.to_string()) } else { Item::Assistant(s.to_string()) });
            continue;
        }
        let texts = content.as_array().into_iter().flatten().filter(|b| str_of(b, "type") == "text").map(|b| str_of(b, "text"));
        if user {
            let words: Vec<&str> = texts.filter(|t| !claude_own_words(t)).collect();
            if !words.is_empty() {
                items.push(Item::User(words.join("\n\n")));
            }
        } else {
            items.extend(texts.filter(|t| !t.trim().is_empty()).map(|t| Item::Assistant(t.to_string())));
        }
    }
    items
}

/// 一条 Codex response item（message）→ 话；developer / Codex 自己写的上下文行丢弃。
fn codex_item(p: &Value) -> Option<Item> {
    if str_of(p, "type") != "message" {
        return None;
    }
    let words: Vec<&str> = p["content"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|c| matches!(str_of(c, "type"), "input_text" | "output_text" | "text"))
        .map(|c| str_of(c, "text"))
        .collect();
    let text = words.join("\n");
    let t = text.trim();
    match str_of(p, "role") {
        // `<environment_context>` / `<user_instructions>` 等以 `<` 开头；AGENTS.md 注入以标题开头。
        "user" if !(t.is_empty() || t.starts_with('<') || t.starts_with("# AGENTS.md")) => Some(Item::User(text)),
        "assistant" if !t.is_empty() => Some(Item::Assistant(text)),
        _ => None,
    }
}

/// 读 Codex rollout 原文 → 文字。
pub(crate) fn codex_items(text: &str) -> Vec<Item> {
    let mut items = Vec::new();
    for l in text.lines() {
        let Ok(v) = serde_json::from_str::<Value>(l) else { continue };
        let p = &v["payload"];
        match str_of(&v, "type") {
            "response_item" => items.extend(codex_item(p)),
            "compacted" => {
                if let Some(h) = p.get("replacement_history").and_then(Value::as_array) {
                    items = h.iter().filter_map(codex_item).collect();
                } else if !str_of(p, "message").is_empty() {
                    items.retain(|i| matches!(i, Item::User(_)));
                    items.push(Item::User(str_of(p, "message").to_string()));
                }
            }
            _ => {}
        }
    }
    items
}

/// Claude Code 项目目录名：每个非字母数字的 UTF-16 单元换成 `-`。
pub(crate) fn claude_slug(p: &str) -> String {
    p.encode_utf16()
        .map(|c| match char::from_u32(c as u32) {
            Some(ch) if ch.is_ascii_alphanumeric() => ch,
            _ => '-',
        })
        .collect()
}

fn stamp(now: chrono::DateTime<chrono::Utc>) -> String {
    now.format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string()
}

/// 新文件落盘：先写同目录临时文件再改名；目标已存在不覆盖。
fn write_new(path: &Path, lines: &[Value]) -> Result<(), String> {
    let dir = path.parent().ok_or("bad target path")?;
    std::fs::create_dir_all(dir).map_err(|e| format!("create {}: {e}", dir.display()))?;
    if path.exists() {
        return Err(format!("{} already exists", path.display()));
    }
    let mut body = String::new();
    for l in lines {
        body.push_str(&l.to_string());
        body.push('\n');
    }
    let tmp = dir.join(format!(".{}.tmp", path.file_name().and_then(|n| n.to_str()).unwrap_or("carry")));
    std::fs::write(&tmp, body).map_err(|e| format!("write {}: {e}", tmp.display()))?;
    std::fs::rename(&tmp, path).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        format!("rename {}: {e}", path.display())
    })
}

/// 写一个新的 Claude Code 会话：`<projects_root>/<slug>/<uuid>.jsonl`。返回 (会话 id, 文件路径)。
pub(crate) fn write_claude(
    projects_root: &Path,
    cwd: &str,
    items: &[Item],
    now: chrono::DateTime<chrono::Utc>,
) -> Result<(String, PathBuf), String> {
    let slug = claude_slug(cwd);
    // shortcut: Claude Code 对超 200 字符的目录名另加哈希，未实现；遇到再补。
    if slug.len() > 200 {
        return Err("workdir path too long for a Claude Code project".into());
    }
    let id = uuid::Uuid::new_v4().to_string();
    let path = projects_root.join(slug).join(format!("{id}.jsonl"));
    let ts = stamp(now);
    let mut parent = Value::Null;
    let mut msg_id = String::new();
    let mut lines = Vec::new();
    for it in items {
        let (ty, message) = match it {
            Item::User(t) => {
                msg_id.clear();
                ("user", json!({"role": "user", "content": t}))
            }
            Item::Assistant(t) => {
                if msg_id.is_empty() {
                    msg_id = format!("msg_{}", uuid::Uuid::new_v4().simple());
                }
                ("assistant", json!({
                    "id": msg_id, "type": "message", "role": "assistant", "model": "<synthetic>",
                    "content": [{"type": "text", "text": t}], "stop_reason": "end_turn", "stop_sequence": null,
                    "usage": {"input_tokens": 0, "cache_creation_input_tokens": 0, "cache_read_input_tokens": 0, "output_tokens": 0},
                }))
            }
        };
        let u = uuid::Uuid::new_v4().to_string();
        lines.push(json!({
            "parentUuid": parent, "isSidechain": false, "type": ty, "message": message,
            "uuid": u, "timestamp": ts, "userType": "external", "entrypoint": "cli",
            "cwd": cwd, "sessionId": id, "version": "2.0.0",
        }));
        parent = Value::String(u);
    }
    write_new(&path, &lines)?;
    Ok((id, path))
}

/// Codex 当前使用的 provider（config.toml 顶层 `model_provider`，缺省 openai），新会话照填才会出现在其历史里。
fn codex_provider(codex_home: &Path) -> String {
    std::fs::read_to_string(codex_home.join("config.toml"))
        .ok()
        .and_then(|s| toml::from_str::<toml::Value>(&s).ok())
        .and_then(|v| v.get("model_provider").and_then(|p| p.as_str()).map(str::to_string))
        .unwrap_or_else(|| "openai".to_string())
}

/// 写一个新的 Codex 会话：`<codex_home>/sessions/YYYY/MM/DD/rollout-<本地时间>-<uuid>.jsonl`。
pub(crate) fn write_codex(
    codex_home: &Path,
    cwd: &str,
    items: &[Item],
    now: chrono::DateTime<chrono::Utc>,
) -> Result<(String, PathBuf), String> {
    let id = uuid::Uuid::new_v4().to_string();
    let local = now.with_timezone(&chrono::Local);
    let path = codex_home
        .join("sessions")
        .join(local.format("%Y/%m/%d").to_string())
        .join(format!("rollout-{}-{id}.jsonl", local.format("%Y-%m-%dT%H-%M-%S")));
    let ts = stamp(now);
    let mut lines = vec![json!({"timestamp": ts, "type": "session_meta", "payload": {
        "session_id": id, "id": id, "timestamp": ts, "cwd": cwd, "originator": "codex_cli_rs",
        "cli_version": "0.0.0", "source": "cli", "model_provider": codex_provider(codex_home),
    }})];
    // 每句话同时写成 Codex 自己的 event_msg：其 TUI 续会话时据此显示历史。
    for it in items {
        let (item, event) = match it {
            Item::User(t) => (
                json!({"type": "message", "role": "user", "content": [{"type": "input_text", "text": t}]}),
                json!({"type": "user_message", "message": t, "images": [], "local_images": [], "text_elements": []}),
            ),
            Item::Assistant(t) => (
                json!({"type": "message", "role": "assistant", "content": [{"type": "output_text", "text": t}]}),
                json!({"type": "agent_message", "message": t}),
            ),
        };
        lines.push(json!({"timestamp": ts, "type": "response_item", "payload": item}));
        lines.push(json!({"timestamp": ts, "type": "event_msg", "payload": event}));
    }
    write_new(&path, &lines)?;
    Ok((id, path))
}

/// 拒绝会被当选项或含控制字符的参数（单引号挡不住这两类）。
fn check_arg(name: &str, v: &str) -> Result<(), String> {
    if v.starts_with('-') || v.chars().any(char::is_control) {
        return Err(format!("invalid {name}"));
    }
    Ok(())
}

/// 交接命令：`cd -- <workdir> && <目标启动命令> '<请阅读 原文件 …>'`。
pub(crate) fn build_handoff_line(
    target: &str,
    group_key: &str,
    workdir: &str,
    source_file: &str,
    aidog_dir: &Path,
) -> Result<String, String> {
    if workdir.is_empty() {
        return Err("session has no workdir".into());
    }
    if source_file.is_empty() {
        return Err("source file not found".into());
    }
    for (n, v) in [("group_key", group_key), ("workdir", workdir), ("source_file", source_file)] {
        check_arg(n, v)?;
    }
    let prompt = shell_squote(&format!("请阅读 {source_file} 了解上一会话的上下文"));
    let g = shell_squote(group_key);
    let cmd = match target {
        "claude_code" if group_key.is_empty() => format!("claude {prompt}"),
        "claude_code" => format!(
            "claude --settings {} {prompt}",
            shell_squote(&aidog_dir.join(format!("settings.{group_key}.json")).to_string_lossy())
        ),
        "codex" if group_key.is_empty() => format!("codex {prompt}"),
        "codex" => format!("AIDOG_KEY={g} codex -p {g} {prompt}"),
        "gemini_cli" => format!("gemini -i {prompt}"),
        "opencode" => format!("opencode --prompt {prompt}"),
        other => return Err(format!("unsupported client: {other}")),
    };
    Ok(format!("cd -- {} && {cmd}", shell_squote(workdir)))
}

/// 目标客户端的存储根（生产 = 真实 HOME 下；测试 = tempdir）。
pub(crate) struct CarryHomes {
    /// `~/.claude/projects`
    pub claude_projects: PathBuf,
    /// `$CODEX_HOME` 或 `~/.codex`
    pub codex_home: PathBuf,
}

/// 换客户端继续的可测面：`run` 拿到整行命令执行（生产 = osascript 开终端）。
#[allow(clippy::too_many_arguments)]
pub(crate) async fn session_carry_with(
    db: &aidog_db::Db,
    from: &str,
    session_id: &str,
    workdir: &str,
    to: &str,
    homes: &CarryHomes,
    aidog_dir: &Path,
    run: &(dyn Fn(&str) -> Result<bool, String> + Send + Sync),
) -> Result<SessionResumeResult, String> {
    if from == to {
        return Err("target client is the source client".into());
    }
    if workdir.is_empty() {
        return Err("session has no workdir".into());
    }
    check_arg("workdir", workdir)?;
    let transcribe = is_transcribe_pair(from, to);
    // 闸门先于一切文件读取。
    if transcribe
        && !(super::load_transcript_consent(db).await && super::consent_allows(&super::load_consent(db).await, from))
    {
        return Err("transcript consent is off".into());
    }
    let detail = aidog_db::client_usage_session_detail(db, from, session_id, Some("agents")).await?;
    let source = detail
        .files
        .iter()
        .find(|f| !f.is_sidechain)
        .map(|f| f.source_file_id.clone())
        .ok_or("source file not found")?;
    let group = if matches!(from, "claude_code" | "codex") {
        aidog_logs::session_group_key(db, session_id).await?.unwrap_or_default()
    } else {
        String::new()
    };
    let command = if transcribe {
        let (src_client, dst_client, wd) = (from.to_string(), to.to_string(), workdir.to_string());
        let (claude_root, codex_home) = (homes.claude_projects.clone(), homes.codex_home.clone());
        let new_id = tokio::task::spawn_blocking(move || -> Result<String, String> {
            let text = std::fs::read_to_string(&source).map_err(|e| format!("read source session: {e}"))?;
            let items = if src_client == "claude_code" { claude_items(&text) } else { codex_items(&text) };
            if !items.iter().any(|i| matches!(i, Item::User(_))) {
                return Err("nothing was said in the session".into());
            }
            let now = chrono::Utc::now();
            let (id, _) = if dst_client == "codex" { write_codex(&codex_home, &wd, &items, now)? } else { write_claude(&claude_root, &wd, &items, now)? };
            Ok(id)
        })
        .await
        .map_err(|e| e.to_string())??;
        build_session_resume_line(to, &new_id, &group, workdir, aidog_dir)?
    } else {
        build_handoff_line(to, &group, workdir, &source, aidog_dir)?
    };
    let launched = run(&command)?;
    Ok(SessionResumeResult { launched, command })
}

/// PATH 上能找到二进制的客户端码。
pub(crate) fn installed_clients(found: impl Fn(&str) -> bool) -> Vec<String> {
    CLIENT_BINS.iter().filter(|(_, bin)| found(bin)).map(|(c, _)| c.to_string()).collect()
}

crate::tauri_command! {
/// 换客户端继续（扩展-会话页 票 10）：Claude Code ⇄ Codex 转写为目标新会话，其余组合交接原文件路径；
/// 结果同 `session_resume`（macOS 开终端，其它平台返回命令供复制）。无测试直调：真跑会写真实客户端存储并弹终端（测 `session_carry_with`）。
pub async fn session_carry(
    client_code: String,
    session_id: String,
    workdir: String,
    target_client: String) -> Result<SessionResumeResult, String> {
    let home = dirs::home_dir().ok_or("cannot resolve home directory")?;
    let codex_home = std::env::var("CODEX_HOME").ok().map(PathBuf::from).unwrap_or_else(|| home.join(".codex"));
    let homes = CarryHomes { claude_projects: home.join(".claude").join("projects"), codex_home };
    let dir = crate::proxy_cmd::session_cmd::aidog_dir()?;
    session_carry_with(
        aidog_ctx::db(), &client_code, &session_id, &workdir, &target_client, &homes, &dir,
        &crate::proxy_cmd::session_cmd::open_terminal,
    ).await
}
}

crate::tauri_command! {
/// 本机已安装的客户端（PATH 上有二进制；票 10 换客户端继续菜单用）。
pub async fn session_installed_clients() -> Result<Vec<String>, String> {
    tokio::task::spawn_blocking(|| installed_clients(|bin| crate::cli_env::which_first(bin).is_some()))
        .await
        .map_err(|e| e.to_string())
}
}
