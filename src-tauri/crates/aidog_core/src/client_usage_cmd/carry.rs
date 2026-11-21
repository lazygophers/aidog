//! 换客户端继续（扩展-会话页 票 10，spec D5）。
//!
//! - Claude Code ⇄ Codex：转写。用户点选时才读源会话原文，按目标格式在目标客户端自己的存储写一个新会话，
//!   再用目标的继续命令在原工作目录打开。规则对齐 Magpie（yetone/magpie:internal/sessions/carry_convert.go）：
//!   Claude 源取最后一条主线行沿 `parentUuid` 回溯（compact 边界无 parent，回溯自然停在那里），丢弃
//!   sidechain / `isMeta` / 斜杠命令回显 / `<system-reminder>`；Codex 源取 `response_item`，遇 `compacted`
//!   用其 `replacement_history`，丢弃 `<environment_context>` / `<user_instructions>` / AGENTS.md / developer 消息。
//!   搬用户与助手的话、用户贴的图（data: URL）、工具调用与结果（shell 类成目标自己的 shell 工具，其余工具
//!   成助手的一段话；单个输出最多 64 KiB）；thinking / reasoning 不搬。原文不进 AiDog 任何数据库表与日志。
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

/// 一次工具调用（结果已配上；`shell` 非空 = 跑了一条 shell 命令）。
#[derive(Debug, Clone, PartialEq, Default)]
pub(crate) struct ToolCall {
    pub name: String,
    pub input: String,
    pub shell: String,
    pub workdir: String,
    pub id: String,
    /// None = 没记到输出
    pub result: Option<String>,
    pub is_error: bool,
}

/// 一条要搬过去的话。
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Item {
    /// 用户的话与贴的图（data: URL）
    User(String, Vec<String>),
    Assistant(String),
    Call(ToolCall),
}

/// 单个工具输出最多搬这么多字节。
pub(crate) const CARRY_RESULT_MAX: usize = 64 << 10;

/// 超出部分截掉（退到 UTF-8 字符边界），同 Magpie `cutResult`。
pub(crate) fn cut_result(s: &str) -> String {
    if s.len() <= CARRY_RESULT_MAX {
        return s.to_string();
    }
    let mut cut = CARRY_RESULT_MAX;
    while !s.is_char_boundary(cut) {
        cut -= 1;
    }
    format!("{}\n[…]", &s[..cut])
}

/// 问本机二进制 `--version` 的执行器（生产起进程；测试注入假实现）。返回原始输出，失败 None。
pub(crate) type VersionOf = std::sync::Arc<dyn Fn(&str) -> Option<String> + Send + Sync>;

/// `--version` 输出里第一个形如 `数字.…` 的词（`2.1.319 (Claude Code)` / `codex-cli 0.46.0`）。
pub(crate) fn parse_version(out: &str) -> Option<String> {
    out.split_whitespace()
        .find(|w| w.starts_with(|c: char| c.is_ascii_digit()) && w.contains('.'))
        .map(str::to_string)
}

/// 生产执行器：PATH 上找到二进制再跑 `--version`。
fn local_version(bin: &str) -> Option<String> {
    let path = crate::cli_env::which_first(bin)?;
    let out = std::process::Command::new(path).arg("--version").output().ok()?;
    out.status.success().then(|| String::from_utf8_lossy(&out.stdout).into_owned())
}

/// JSON 排版成易读形式（短的才排），同 Magpie `pretty`。
fn pretty(raw: &str) -> String {
    if raw.is_empty() || raw.len() > 4000 {
        return raw.to_string();
    }
    serde_json::from_str::<Value>(raw).ok().and_then(|v| serde_json::to_string_pretty(&v).ok()).unwrap_or_else(|| raw.to_string())
}

/// Claude tool_result 的正文：字符串，或文字块（图片记 `[image]`），同 Magpie `resultText`。
fn result_text(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Array(bs) => bs
            .iter()
            .filter_map(|b| match str_of(b, "type") {
                "text" => Some(str_of(b, "text").to_string()),
                "image" => Some("[image]".to_string()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n"),
        Value::Null => String::new(),
        other => other.to_string(),
    }
}

/// Codex 工具输出：字符串，或带 content / output 的对象，同 Magpie `codexOutput`。
fn codex_output(v: &Value) -> String {
    if let Some(s) = v.as_str() {
        return s.to_string();
    }
    for k in ["content", "output"] {
        let s = str_of(v, k);
        if !s.is_empty() {
            return s.to_string();
        }
    }
    if v.is_null() { String::new() } else { v.to_string() }
}

/// argv 还原成 shell 行：`bash -lc <脚本>` 取脚本，否则带特殊字符的词单引号，同 Magpie `shellLine`。
fn shell_line(argv: &[String]) -> String {
    if argv.len() == 3 && (argv[1] == "-lc" || argv[1] == "-c") {
        return argv[2].clone();
    }
    argv.iter()
        .map(|a| {
            if a.is_empty() || a.chars().any(|c| " \t\n'\"\\$`*?[]{}()<>|&;#~!".contains(c)) {
                shell_squote(a)
            } else {
                a.clone()
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// 目标客户端没有同名工具时，把调用写成助手的一段话，同 Magpie `note`。
fn note(c: &ToolCall) -> String {
    let mut b = format!("[{}]\n", c.name);
    if !c.input.trim().is_empty() {
        b.push_str(&fence(&c.input));
    }
    if let Some(r) = &c.result {
        b.push_str(if c.is_error { "\n[error]\n" } else { "\n[result]\n" });
        b.push_str(&fence(r));
    }
    b
}

/// 放进任何一行都关不掉的代码块。
fn fence(s: &str) -> String {
    let mut f = "```".to_string();
    while s.contains(&f) {
        f.push('`');
    }
    format!("{f}\n{}\n{f}\n", s.trim_end_matches('\n'))
}

/// data: URL → (媒体类型, base64 数据)。
fn split_data_url(u: &str) -> Option<(&str, &str)> {
    let (head, data) = u.strip_prefix("data:")?.split_once(',')?;
    let mt = head.strip_suffix(";base64")?;
    (!mt.is_empty()).then_some((mt, data))
}

/// 读出来的中间形态：结果按 id 配到它的调用上（结果不单独搬）。
enum Raw {
    Item(Item),
    Result { id: String, text: String, is_error: bool },
}

/// 同 Magpie `pairCarry`：每个结果放回它的调用处。
fn pair(raw: Vec<Raw>) -> Vec<Item> {
    let mut at = std::collections::HashMap::new();
    let mut out: Vec<Item> = Vec::new();
    for r in raw {
        match r {
            Raw::Item(Item::Call(c)) => {
                if !c.id.is_empty() {
                    at.insert(c.id.clone(), out.len());
                }
                out.push(Item::Call(c));
            }
            Raw::Item(i) => out.push(i),
            Raw::Result { id, text, is_error } => {
                if let Some(Item::Call(c)) = at.get(&id).and_then(|&i| out.get_mut(i))
                    && c.result.is_none()
                {
                    c.result = Some(cut_result(&text));
                    c.is_error = is_error;
                }
            }
        }
    }
    out
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
    let mut raw = Vec::new();
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
            raw.push(Raw::Item(if user { Item::User(s.to_string(), vec![]) } else { Item::Assistant(s.to_string()) }));
            continue;
        }
        // 同 Magpie `claudeLineItems`：用户的话与图合成一条放最前；thinking 不搬。
        let (mut words, mut images, mut rest) = (Vec::new(), Vec::new(), Vec::new());
        for b in content.as_array().into_iter().flatten() {
            match str_of(b, "type") {
                "text" if user => {
                    if !claude_own_words(str_of(b, "text")) {
                        words.push(str_of(b, "text"));
                    }
                }
                "text" if !str_of(b, "text").trim().is_empty() => rest.push(Raw::Item(Item::Assistant(str_of(b, "text").to_string()))),
                "image" if user && str_of(&b["source"], "type") == "base64" && !str_of(&b["source"], "data").is_empty() => {
                    images.push(format!("data:{};base64,{}", str_of(&b["source"], "media_type"), str_of(&b["source"], "data")));
                }
                "tool_use" => {
                    let input = b.get("input").map(Value::to_string).unwrap_or_default();
                    let shell = if str_of(b, "name") == "Bash" { str_of(&b["input"], "command").to_string() } else { String::new() };
                    rest.push(Raw::Item(Item::Call(ToolCall {
                        name: str_of(b, "name").to_string(),
                        id: str_of(b, "id").to_string(),
                        input: pretty(&input),
                        shell,
                        ..Default::default()
                    })));
                }
                "tool_result" => rest.push(Raw::Result {
                    id: str_of(b, "tool_use_id").to_string(),
                    text: result_text(&b["content"]),
                    is_error: b.get("is_error").and_then(Value::as_bool) == Some(true),
                }),
                _ => {}
            }
        }
        if user && (!words.is_empty() || !images.is_empty()) {
            raw.push(Raw::Item(Item::User(words.join("\n\n"), images)));
        }
        raw.extend(rest);
    }
    pair(raw)
}

/// Codex 自己写进用户消息的上下文（`<environment_context>` / `<user_instructions>` 等以 `<` 开头；AGENTS.md 注入以标题开头）。
fn codex_own_words(t: &str) -> bool {
    let t = t.trim();
    t.is_empty() || t.starts_with('<') || t.starts_with("# AGENTS.md")
}

/// 一条 Codex response item → 话 / 调用 / 结果，同 Magpie `codexItem`；developer、reasoning、加密摘要丢弃。
fn codex_item(p: &Value) -> Option<Raw> {
    let call = |name: &str, input: String| ToolCall { name: name.to_string(), id: str_of(p, "call_id").to_string(), input, ..Default::default() };
    match str_of(p, "type") {
        "message" => {
            let (mut words, mut images) = (Vec::new(), Vec::new());
            for c in p["content"].as_array().into_iter().flatten() {
                match str_of(c, "type") {
                    "input_text" | "output_text" | "text" => words.push(str_of(c, "text")),
                    "input_image" if str_of(c, "image_url").starts_with("data:") => images.push(str_of(c, "image_url").to_string()),
                    _ => {}
                }
            }
            let text = words.join("\n");
            match str_of(p, "role") {
                "user" if codex_own_words(&text) && images.is_empty() => None,
                "user" => Some(Raw::Item(Item::User(if codex_own_words(&text) { String::new() } else { text }, images))),
                "assistant" if !text.trim().is_empty() => Some(Raw::Item(Item::Assistant(text))),
                _ => None,
            }
        }
        "function_call" => {
            let args_raw = str_of(p, "arguments");
            let mut c = call(str_of(p, "name"), pretty(args_raw));
            if let Ok(a) = serde_json::from_str::<Value>(args_raw) {
                c.shell = match c.name.as_str() {
                    "exec_command" => str_of(&a, "cmd").to_string(),
                    "shell" | "container.exec" => a["command"]
                        .as_array()
                        .map(|v| shell_line(&v.iter().filter_map(|s| s.as_str().map(str::to_string)).collect::<Vec<_>>()))
                        .unwrap_or_default(),
                    "shell_command" => str_of(&a, "command").to_string(),
                    _ => String::new(),
                };
                c.workdir = str_of(&a, "workdir").to_string();
            }
            Some(Raw::Item(Item::Call(c)))
        }
        "custom_tool_call" => Some(Raw::Item(Item::Call(call(str_of(p, "name"), str_of(p, "input").to_string())))),
        "local_shell_call" => {
            let argv: Vec<String> = p["action"]["command"].as_array().into_iter().flatten().filter_map(|s| s.as_str().map(str::to_string)).collect();
            let mut c = call("shell", argv.join(" "));
            c.shell = shell_line(&argv);
            c.workdir = str_of(&p["action"], "working_directory").to_string();
            Some(Raw::Item(Item::Call(c)))
        }
        "function_call_output" | "custom_tool_call_output" => {
            Some(Raw::Result { id: str_of(p, "call_id").to_string(), text: codex_output(&p["output"]), is_error: false })
        }
        _ => None,
    }
}

/// 读 Codex rollout 原文 → 话与调用。
pub(crate) fn codex_items(text: &str) -> Vec<Item> {
    let mut raw = Vec::new();
    for l in text.lines() {
        let Ok(v) = serde_json::from_str::<Value>(l) else { continue };
        let p = &v["payload"];
        match str_of(&v, "type") {
            "response_item" => raw.extend(codex_item(p)),
            "compacted" => {
                if let Some(h) = p.get("replacement_history").and_then(Value::as_array) {
                    raw = h.iter().filter_map(codex_item).collect();
                } else if !str_of(p, "message").is_empty() {
                    raw.retain(|i| matches!(i, Raw::Item(Item::User(..))));
                    raw.push(Raw::Item(Item::User(str_of(p, "message").to_string(), vec![])));
                }
            }
            _ => {}
        }
    }
    pair(raw)
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
    version: &str,
) -> Result<(String, PathBuf), String> {
    let slug = claude_slug(cwd);
    // shortcut: Claude Code 对超 200 字符的目录名另加哈希，算法无一手出处（Magpie relocate_claude.go
    // `claudeProjectName` 同样直接报错），未实现；拿到官方算法再补。
    if slug.len() > 200 {
        return Err("workdir path too long for a Claude Code project".into());
    }
    let id = uuid::Uuid::new_v4().to_string();
    let path = projects_root.join(slug).join(format!("{id}.jsonl"));
    let ts = stamp(now);
    let mut parent = Value::Null;
    let mut msg_id = String::new();
    let mut lines: Vec<Value> = Vec::new();
    // 一行（字段顺序同 Claude Code 自己的文件，同 Magpie `writeClaudeCarry`），返回它的 uuid。
    let line = |lines: &mut Vec<Value>, parent: &mut Value, ty: &str, message: Value, extra: Value| -> String {
        let u = uuid::Uuid::new_v4().to_string();
        let mut l = serde_json::Map::new();
        l.insert("parentUuid".into(), parent.clone());
        l.insert("isSidechain".into(), false.into());
        l.insert("type".into(), ty.into());
        l.insert("message".into(), message);
        if let Value::Object(m) = extra {
            l.extend(m);
        }
        for (k, v) in [("uuid", u.as_str()), ("timestamp", ts.as_str()), ("userType", "external"), ("entrypoint", "cli"),
            ("cwd", cwd), ("sessionId", id.as_str()), ("version", version)]
        {
            l.insert(k.into(), v.into());
        }
        lines.push(Value::Object(l));
        *parent = Value::String(u.clone());
        u
    };
    let assistant = |lines: &mut Vec<Value>, parent: &mut Value, msg_id: &mut String, block: Value, stop: &str| -> String {
        if msg_id.is_empty() {
            *msg_id = format!("msg_{}", uuid::Uuid::new_v4().simple());
        }
        let message = json!({
            "id": msg_id, "type": "message", "role": "assistant", "model": "<synthetic>",
            "content": [block], "stop_reason": stop, "stop_sequence": null,
            "usage": {"input_tokens": 0, "cache_creation_input_tokens": 0, "cache_read_input_tokens": 0, "output_tokens": 0},
        });
        line(lines, parent, "assistant", message, Value::Null)
    };
    for it in items {
        match it {
            Item::User(t, images) => {
                msg_id.clear();
                let content = if images.is_empty() {
                    json!(t)
                } else {
                    let mut blocks = Vec::new();
                    if !t.is_empty() {
                        blocks.push(json!({"type": "text", "text": t}));
                    }
                    for (mt, data) in images.iter().filter_map(|i| split_data_url(i)) {
                        blocks.push(json!({"type": "image", "source": {"type": "base64", "media_type": mt, "data": data}}));
                    }
                    Value::Array(blocks)
                };
                line(&mut lines, &mut parent, "user", json!({"role": "user", "content": content}), Value::Null);
            }
            Item::Assistant(t) => {
                assistant(&mut lines, &mut parent, &mut msg_id, json!({"type": "text", "text": t}), "end_turn");
            }
            Item::Call(c) if c.shell.is_empty() => {
                assistant(&mut lines, &mut parent, &mut msg_id, json!({"type": "text", "text": note(c)}), "end_turn");
            }
            Item::Call(c) => {
                let mut command = c.shell.clone();
                if !c.workdir.is_empty() && c.workdir != cwd {
                    command = format!("cd {} && {command}", shell_squote(&c.workdir));
                }
                let call_id = format!("toolu_{}", &uuid::Uuid::new_v4().simple().to_string()[..24]);
                let by = assistant(&mut lines, &mut parent, &mut msg_id,
                    json!({"type": "tool_use", "id": call_id, "name": "Bash", "input": {"command": command}}), "tool_use");
                msg_id.clear();
                let out = c.result.clone().unwrap_or_else(|| "[no output was recorded]".to_string());
                line(&mut lines, &mut parent, "user",
                    json!({"role": "user", "content": [{"tool_use_id": call_id, "type": "tool_result", "content": out, "is_error": c.is_error}]}),
                    json!({"toolUseResult": {"stdout": out, "stderr": "", "interrupted": false, "isImage": false}, "sourceToolAssistantUUID": by}));
            }
        }
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
    version: &str,
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
        "cli_version": version, "source": "cli", "model_provider": codex_provider(codex_home),
    }})];
    let add = |lines: &mut Vec<Value>, ty: &str, payload: Value| lines.push(json!({"timestamp": ts, "type": ty, "payload": payload}));
    // 每句话同时写成 Codex 自己的 event_msg：其 TUI 续会话时据此显示历史。
    let say = |lines: &mut Vec<Value>, t: &str| {
        add(lines, "response_item", json!({"type": "message", "role": "assistant", "content": [{"type": "output_text", "text": t}]}));
        add(lines, "event_msg", json!({"type": "agent_message", "message": t}));
    };
    // 同 Magpie `writeCodexCarry`：shell 调用写成 exec_command + function_call_output，其余工具成助手的话。
    for it in items {
        match it {
            Item::User(t, imgs) => {
                let images: Vec<&String> = imgs.iter().filter(|i| split_data_url(i).is_some()).collect();
                let mut content = Vec::new();
                if !t.is_empty() {
                    content.push(json!({"type": "input_text", "text": t}));
                }
                content.extend(images.iter().map(|i| json!({"type": "input_image", "image_url": i})));
                add(&mut lines, "response_item", json!({"type": "message", "role": "user", "content": content}));
                add(&mut lines, "event_msg", json!({"type": "user_message", "message": t, "images": images, "local_images": [], "text_elements": []}));
            }
            Item::Assistant(t) => say(&mut lines, t),
            Item::Call(c) if c.shell.is_empty() => say(&mut lines, &note(c)),
            Item::Call(c) => {
                let call_id = format!("call_{}", &uuid::Uuid::new_v4().simple().to_string()[..24]);
                let workdir = if c.workdir.is_empty() { cwd } else { &c.workdir };
                let args = json!({"cmd": c.shell, "workdir": workdir}).to_string();
                add(&mut lines, "response_item", json!({"type": "function_call", "name": "exec_command", "arguments": args, "call_id": call_id}));
                let out = c.result.clone().unwrap_or_else(|| "[no output was recorded]".to_string());
                add(&mut lines, "response_item", json!({"type": "function_call_output", "call_id": call_id, "output": out}));
            }
        }
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
    version_of: VersionOf,
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
            if !items.iter().any(|i| matches!(i, Item::User(..))) {
                return Err("nothing was said in the session".into());
            }
            let now = chrono::Utc::now();
            // 写本机客户端版本号（同 Magpie 写目标自己的版本）；问不出回落固定值。
            let (bin, fallback) = if dst_client == "codex" { ("codex", "0.0.0") } else { ("claude", "2.0.0") };
            let version = version_of(bin).as_deref().and_then(parse_version).unwrap_or_else(|| fallback.to_string());
            let (id, _) = if dst_client == "codex" { write_codex(&codex_home, &wd, &items, now, &version)? } else { write_claude(&claude_root, &wd, &items, now, &version)? };
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
        std::sync::Arc::new(local_version),
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
