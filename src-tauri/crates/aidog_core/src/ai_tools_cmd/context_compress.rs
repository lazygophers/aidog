//! 扩展 → 上下文压缩（RTK / sqz）。
//!
//! - Claude 侧钩子一律改「设置 → Claude Code」全局配置（DB `setting` global/claude_code，空时同
//!   `do_sync_group_settings` 回落内置 oss 预设），改完触发同一分组同步；不直接写
//!   `~/.claude/settings.json`（默认分组同步会全量覆盖，`sync_settings.rs:109`）。开关状态 = 配置里有该命令。
//! - RTK Codex：跑 `rtk init -g --codex`（rtk 1.2.0 无 `--agent codex`，隔离 HOME 实测），写入前后快照
//!   `$CODEX_HOME/{RTK.md,hooks.json,AGENTS.md}` 记入 DB（global/context_compress）。关闭：文件仍等于
//!   记录的写后内容 → 原样恢复写前内容；被改过 → 只删 rtk 的那部分。不用 `rtk init --uninstall`：
//!   实测会吞掉 AGENTS.md 结尾换行、留下空 hooks.json 与 hooks.json.bak。
//! - sqz：单一总开关。开 = 两个 Claude 钩子 + 所有 stdio MCP 包成 `sqz-mcp proxy -- <原命令>`
//!   （~/.claude.json 顶层与各 project、`$CODEX_HOME/config.toml`）+ 各加一个 `sqz-mcp --transport stdio`；
//!   关 = 反向。HTTP/SSE 不动；命令已是 sqz-mcp 的不再包。任一项缺 = 未启用。
//!   文件解析失败一律拒写；写走原子替换并保留原权限位。

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use aidog_db::Db;
use aidog_db::models::SetSettingInput;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

pub const RTK_CLAUDE_CMD: &str = "rtk hook claude";
pub const RTK_CODEX_CMD: &str = "rtk hook codex";
/// (事件, matcher, 命令)：形状对齐用户 ~/.claude/settings.json 与 lazygophers 预设。
const SQZ_HOOKS: [(&str, Option<&str>, &str); 2] = [
    ("PostToolUse", Some("Bash"), "sqz hook claude-post"),
    ("PreCompact", None, "sqz hook precompact"),
];
const SQZ_BIN: &str = "sqz-mcp";
const SQZ_SELF_ARGS: [&str; 2] = ["--transport", "stdio"];
/// `sqz-mcp proxy [--no-desc] [--no-cache] [--lazy-tools] -- <upstream...>`（sqz-mcp 2.2.0 usage）。
const PROXY_FLAGS: [&str; 3] = ["--no-desc", "--no-cache", "--lazy-tools"];
const RTK_CODEX_FILES: [&str; 3] = ["RTK.md", "hooks.json", "AGENTS.md"];
const SETTING_KEY: &str = "context_compress";

pub struct Paths {
    pub claude_json: PathBuf,
    pub codex: PathBuf,
}

impl Paths {
    pub fn resolve() -> Result<Self, String> {
        Ok(Self {
            claude_json: dirs::home_dir()
                .ok_or("cannot resolve home directory")?
                .join(".claude.json"),
            codex: aidog_db::codex_home()?,
        })
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct FileRec {
    pub before: Option<String>,
    pub after: Option<String>,
}
/// 文件名 → aidog 开 RTK Codex 时该文件的写前 / 写后内容。
pub type RtkCodexRecord = BTreeMap<String, FileRec>;

#[derive(Serialize, Clone, Debug, Default)]
pub struct ContextCompressStatus {
    pub rtk_version: Option<String>,
    pub rtk_claude: bool,
    pub rtk_codex: bool,
    pub codex_present: bool,
    pub sqz_version: Option<String>,
    pub sqz_mcp_version: Option<String>,
    pub sqz_enabled: bool,
    pub sqz_missing: Vec<String>,
}

// ─── Claude 钩子（全局配置 JSON）────────────────────────────────

fn group_cmds(g: &Value) -> impl Iterator<Item = &str> {
    g.get("hooks")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|h| h.get("command").and_then(Value::as_str))
}

pub(crate) fn has_hook(cfg: &Value, event: &str, cmd: &str) -> bool {
    cfg.get("hooks")
        .and_then(|h| h.get(event))
        .and_then(Value::as_array)
        .is_some_and(|gs| gs.iter().any(|g| group_cmds(g).any(|c| c == cmd)))
}

pub(crate) fn add_hook(
    cfg: &mut Value,
    event: &str,
    matcher: Option<&str>,
    cmd: &str,
) -> Result<(), String> {
    if has_hook(cfg, event, cmd) {
        return Ok(());
    }
    let root = cfg.as_object_mut().ok_or("config is not an object")?;
    let hooks = root
        .entry("hooks")
        .or_insert_with(|| json!({}))
        .as_object_mut()
        .ok_or("hooks is not an object")?;
    let groups = hooks
        .entry(event)
        .or_insert_with(|| json!([]))
        .as_array_mut()
        .ok_or_else(|| format!("hooks.{event} is not an array"))?;
    let mut group = json!({"hooks": [{"type": "command", "command": cmd}]});
    if let Some(m) = matcher {
        group["matcher"] = m.into();
    }
    groups.push(group);
    Ok(())
}

/// 只删命令等于 `cmd` 的钩子项；只剪本次删空的组 / 事件 / hooks 对象。
pub(crate) fn remove_hook(cfg: &mut Value, event: &str, cmd: &str) {
    let Some(hooks) = cfg.get_mut("hooks").and_then(Value::as_object_mut) else {
        return;
    };
    let Some(groups) = hooks.get_mut(event).and_then(Value::as_array_mut) else {
        return;
    };
    let before = groups.len();
    groups.retain_mut(|g| {
        let Some(hs) = g.get_mut("hooks").and_then(Value::as_array_mut) else {
            return true;
        };
        let n = hs.len();
        hs.retain(|h| h.get("command").and_then(Value::as_str) != Some(cmd));
        !(hs.is_empty() && n > 0)
    });
    if groups.is_empty() && before > 0 {
        hooks.remove(event);
        if hooks.is_empty()
            && let Some(o) = cfg.as_object_mut()
        {
            o.remove("hooks");
        }
    }
}

pub(crate) fn sqz_apply_hooks(cfg: &mut Value, on: bool) -> Result<(), String> {
    for (ev, matcher, cmd) in SQZ_HOOKS {
        if on {
            add_hook(cfg, ev, matcher, cmd)?;
        } else {
            remove_hook(cfg, ev, cmd);
        }
    }
    Ok(())
}

// ─── sqz MCP 包装 ──────────────────────────────────────────────

#[derive(PartialEq)]
enum Kind {
    Plain,
    Wrapped,
    SelfServer,
}

/// `—`（em dash）宽容归一：用户手写配置经排版常把 `--` 排成 `—` / `——`，前导连字符
/// 一律归成 `--`（`—transport` / `-transport` / `——transport` → `--transport`）。
fn dash_norm(s: &str) -> String {
    match s.trim_start_matches(['-', '—']) {
        t if t.len() < s.len() => format!("--{t}"),
        _ => s.to_string(),
    }
}

fn kind(cmd: &str, args: &[String]) -> Kind {
    if Path::new(cmd).file_name().is_none_or(|n| n != SQZ_BIN) {
        Kind::Plain
    } else if is_known_proxy_shape(args) {
        Kind::Wrapped
    } else if args.len() == SQZ_SELF_ARGS.len()
        && args
            .iter()
            .zip(SQZ_SELF_ARGS)
            .all(|(a, s)| dash_norm(a) == *s)
    {
        Kind::SelfServer
    } else {
        Kind::Plain
    }
}

fn wrap(cmd: &str, args: &[String]) -> (String, Vec<String>) {
    let mut a = vec!["proxy".to_string(), "--".to_string(), cmd.to_string()];
    a.extend_from_slice(args);
    (SQZ_BIN.to_string(), a)
}

/// 只有 aidog 自己写的已知形状 `proxy [已知旗标] -- <上游...>` 才判 Wrapped：
/// 用户自装的 `sqz-mcp proxy --custom x` 等未知旗标组合不还原（会撕坏用户配置）、
/// 开启时照普通命令再包一层（可还原）。
fn is_known_proxy_shape(args: &[String]) -> bool {
    if args.first().map(String::as_str) != Some("proxy") {
        return false;
    }
    let mut i = 1;
    while args
        .get(i)
        .is_some_and(|a| PROXY_FLAGS.contains(&dash_norm(a).as_str()))
    {
        i += 1;
    }
    args.get(i)
        .is_some_and(|a| !a.is_empty() && a.chars().all(|c| c == '-' || c == '—'))
}

/// 还原 `proxy [flags] -- <cmd> <args...>`。用户手写配置里分隔符与旗标有 em dash 变体，
/// 经 dash_norm 归一后认；其后各参数原样保留。
fn unwrap(args: &[String]) -> Option<(String, Vec<String>)> {
    let mut i = 1;
    while args
        .get(i)
        .is_some_and(|a| PROXY_FLAGS.contains(&dash_norm(a).as_str()))
    {
        i += 1;
    }
    if args
        .get(i)
        .is_some_and(|a| !a.is_empty() && a.chars().all(|c| c == '-' || c == '—'))
    {
        i += 1;
    }
    let (cmd, rest) = args.get(i..)?.split_first()?;
    Some((cmd.clone(), rest.to_vec()))
}

fn next_cmd(cmd: &str, args: &[String], on: bool) -> Option<(String, Vec<String>)> {
    match (kind(cmd, args), on) {
        (Kind::Plain, true) => Some(wrap(cmd, args)),
        (Kind::Wrapped, false) => unwrap(args),
        _ => None,
    }
}

fn free_name(taken: impl Fn(&str) -> bool) -> String {
    ["sqz", "sqz-mcp", "sqz-mcp-self"]
        .into_iter()
        .find(|n| !taken(n))
        .map(str::to_string)
        .unwrap_or_else(|| {
            (2..)
                .map(|n| format!("sqz-mcp-self-{n}"))
                .find(|n| !taken(n))
                .expect("an unused sqz server name exists")
        })
}

/// stdio 条目的 (command, args)；HTTP/SSE、args 非字符串数组 → None（不碰）。
fn json_stdio(e: &Value) -> Option<(String, Vec<String>)> {
    let o = e.as_object()?;
    if o.contains_key("url")
        || o.get("type")
            .and_then(Value::as_str)
            .is_some_and(|t| t != "stdio")
    {
        return None;
    }
    let cmd = o.get("command")?.as_str()?.to_string();
    let args = match o.get("args") {
        None => Vec::new(),
        Some(a) => a
            .as_array()?
            .iter()
            .map(|x| x.as_str().map(String::from))
            .collect::<Option<Vec<_>>>()?,
    };
    Some((cmd, args))
}

fn json_is(e: &Value, k: Kind) -> bool {
    json_stdio(e).is_some_and(|(c, a)| kind(&c, &a) == k)
}

/// ~/.claude.json 里所有 mcpServers：(前缀, map)，顶层前缀为空，project 为 `<路径>:`。
fn claude_maps(root: &mut Value) -> Vec<(String, &mut serde_json::Map<String, Value>)> {
    let mut out = Vec::new();
    let Some(obj) = root.as_object_mut() else {
        return out;
    };
    for (k, v) in obj.iter_mut() {
        if k == "mcpServers" {
            if let Some(m) = v.as_object_mut() {
                out.push((String::new(), m));
            }
        } else if k == "projects"
            && let Some(ps) = v.as_object_mut()
        {
            for (p, pv) in ps.iter_mut() {
                if let Some(m) = pv.get_mut("mcpServers").and_then(Value::as_object_mut) {
                    out.push((format!("{p}:"), m));
                }
            }
        }
    }
    out
}

/// 返回是否有改动。
pub(crate) fn sqz_apply_claude_json(root: &mut Value, on: bool) -> Result<bool, String> {
    let obj = root
        .as_object_mut()
        .ok_or("~/.claude.json root is not an object")?;
    if on {
        obj.entry("mcpServers")
            .or_insert_with(|| json!({}))
            .as_object()
            .ok_or("mcpServers is not an object")?;
    }
    let mut changed = false;
    for (_, m) in claude_maps(root) {
        for e in m.values_mut() {
            let Some((c, a)) = json_stdio(e) else {
                continue;
            };
            if let Some((c, a)) = next_cmd(&c, &a, on) {
                e["command"] = c.into();
                e["args"] = a.into();
                changed = true;
            }
        }
    }
    if let Some(top) = root.get_mut("mcpServers").and_then(Value::as_object_mut) {
        if on && !top.values().any(|e| json_is(e, Kind::SelfServer)) {
            let name = free_name(|n| top.contains_key(n));
            top.insert(
                name,
                json!({"type": "stdio", "command": SQZ_BIN, "args": SQZ_SELF_ARGS}),
            );
            changed = true;
        } else if !on {
            let n = top.len();
            top.retain(|_, e| !json_is(e, Kind::SelfServer));
            changed |= top.len() != n;
        }
    }
    Ok(changed)
}

fn toml_stdio(e: &toml::Value) -> Option<(String, Vec<String>)> {
    let t = e.as_table()?;
    if t.contains_key("url") {
        return None;
    }
    let cmd = t.get("command")?.as_str()?.to_string();
    let args = match t.get("args") {
        None => Vec::new(),
        Some(a) => a
            .as_array()?
            .iter()
            .map(|x| x.as_str().map(String::from))
            .collect::<Option<Vec<_>>>()?,
    };
    Some((cmd, args))
}

fn toml_is(e: &toml::Value, k: Kind) -> bool {
    toml_stdio(e).is_some_and(|(c, a)| kind(&c, &a) == k)
}

fn toml_args(a: Vec<String>) -> toml::Value {
    toml::Value::Array(a.into_iter().map(toml::Value::String).collect())
}

pub(crate) fn sqz_apply_codex(root: &mut toml::Table, on: bool) -> Result<bool, String> {
    if on {
        root.entry("mcp_servers")
            .or_insert_with(|| toml::Value::Table(toml::Table::new()));
    }
    let Some(servers) = root.get_mut("mcp_servers") else {
        return Ok(false);
    };
    let servers = servers.as_table_mut().ok_or("mcp_servers is not a table")?;
    let mut changed = false;
    for (_, e) in servers.iter_mut() {
        let Some((c, a)) = toml_stdio(e) else {
            continue;
        };
        if let Some((c, a)) = next_cmd(&c, &a, on)
            && let Some(t) = e.as_table_mut()
        {
            t.insert("command".into(), c.into());
            t.insert("args".into(), toml_args(a));
            changed = true;
        }
    }
    if on && !servers.values().any(|e| toml_is(e, Kind::SelfServer)) {
        let name = free_name(|n| servers.contains_key(n));
        let mut t = toml::Table::new();
        t.insert("command".into(), SQZ_BIN.into());
        t.insert(
            "args".into(),
            toml_args(SQZ_SELF_ARGS.map(String::from).to_vec()),
        );
        servers.insert(name, toml::Value::Table(t));
        changed = true;
    } else if !on {
        let n = servers.len();
        servers.retain(|_, e| !toml_is(e, Kind::SelfServer));
        changed |= servers.len() != n;
    }
    Ok(changed)
}

/// 缺的项（空 = 已启用）：`hook:<事件>`、`wrap:claude:<前缀><名>`、`self:claude`、`wrap:codex:<名>`、`self:codex`。
pub(crate) fn sqz_missing(
    cfg: &Value,
    claude_json: &Value,
    codex: Option<&toml::Table>,
) -> Vec<String> {
    let mut out = Vec::new();
    for (ev, _, cmd) in SQZ_HOOKS {
        if !has_hook(cfg, ev, cmd) {
            out.push(format!("hook:{ev}"));
        }
    }
    let mut cj = claude_json.clone();
    for (p, m) in claude_maps(&mut cj) {
        out.extend(
            m.iter()
                .filter(|(_, e)| json_is(e, Kind::Plain))
                .map(|(n, _)| format!("wrap:claude:{p}{n}")),
        );
    }
    let top_self = claude_json
        .get("mcpServers")
        .and_then(Value::as_object)
        .is_some_and(|m| m.values().any(|e| json_is(e, Kind::SelfServer)));
    if !top_self {
        out.push("self:claude".into());
    }
    if let Some(t) = codex {
        let servers = t.get("mcp_servers").and_then(toml::Value::as_table);
        if let Some(s) = servers {
            out.extend(
                s.iter()
                    .filter(|(_, e)| toml_is(e, Kind::Plain))
                    .map(|(n, _)| format!("wrap:codex:{n}")),
            );
        }
        if !servers.is_some_and(|s| s.values().any(|e| toml_is(e, Kind::SelfServer))) {
            out.push("self:codex".into());
        }
    } else {
        out.push("codex:unavailable".into());
    }
    out
}

// ─── 文件读写 ──────────────────────────────────────────────────

fn read_original(path: &Path) -> Result<Option<String>, String> {
    match std::fs::read_to_string(path) {
        Ok(s) => Ok(Some(s)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(format!("read {}: {e}", path.display())),
    }
}

fn read_opt(path: &Path) -> Result<Option<String>, String> {
    match std::fs::read_to_string(path) {
        Ok(s) if s.trim().is_empty() => Ok(None),
        Ok(s) => Ok(Some(s)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(format!("read {}: {e}", path.display())),
    }
}

fn read_json_file(path: &Path) -> Result<Value, String> {
    read_opt(path)?
        .map_or(Ok(json!({})), |s| serde_json::from_str(&s))
        .map_err(|e| format!("parse {}: {e}", path.display()))
}

fn read_toml_file(path: &Path) -> Result<toml::Table, String> {
    read_opt(path)?
        .map_or(Ok(toml::Table::new()), |s| toml::from_str(&s))
        .map_err(|e| format!("parse {}: {e}", path.display()))
}

/// 原子写并保留原文件权限位。
fn write_keep_mode(path: &Path, content: &str) -> Result<(), String> {
    #[cfg(unix)]
    let mode = {
        use std::os::unix::fs::PermissionsExt;
        std::fs::metadata(path).ok().map(|m| m.permissions().mode())
    };
    #[cfg(not(unix))]
    let mode = None;
    super::profile::atomic_write(path, content, mode)
}

fn remove_if_exists(path: &Path) -> Result<(), String> {
    match std::fs::remove_file(path) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => {
            Err(format!("remove {}: {e}", path.display()))
        }
        _ => Ok(()),
    }
}

fn write_transaction(
    updates: Vec<(PathBuf, String, Option<String>)>,
    mut write: impl FnMut(&Path, &str) -> Result<(), String>,
    mut remove: impl FnMut(&Path) -> Result<(), String>,
) -> Result<(), String> {
    let mut completed: Vec<(PathBuf, Option<String>)> = Vec::new();
    for (path, content, before) in updates {
        if let Err(error) = write(&path, &content) {
            let rollback_errors: Vec<_> = completed
                .iter()
                .rev()
                .filter_map(|(path, before)| {
                    let result = match before {
                        Some(content) => write(path, content),
                        None => remove(path),
                    };
                    result.err().map(|e| format!("{}: {e}", path.display()))
                })
                .collect();
            return if rollback_errors.is_empty() {
                Err(error)
            } else {
                Err(format!(
                    "{error}; rollback failed for {}",
                    rollback_errors.join("; ")
                ))
            };
        }
        completed.push((path, before));
    }
    Ok(())
}

fn pretty(v: &Value) -> Result<String, String> {
    serde_json::to_string_pretty(v).map_err(|e| e.to_string())
}

/// Codex 只在 `$CODEX_HOME` 目录存在时参与。
fn codex_cfg(p: &Paths) -> Option<PathBuf> {
    p.codex.is_dir().then(|| p.codex.join("config.toml"))
}

/// 两个文件都先解析，任一非法则一个都不写。
pub(crate) fn sqz_apply_files(p: &Paths, on: bool) -> Result<(), String> {
    let claude_before = read_original(&p.claude_json)?;
    let mut cj = claude_before
        .as_deref()
        .filter(|s| !s.trim().is_empty())
        .map(serde_json::from_str)
        .transpose()
        .map_err(|e| format!("parse {}: {e}", p.claude_json.display()))?
        .unwrap_or_else(|| json!({}));
    let cx_path = (p.codex.is_dir() || on).then(|| p.codex.join("config.toml"));
    let cx_before = cx_path.as_deref().map(read_original).transpose()?.flatten();
    let mut cx = cx_path
        .as_deref()
        .map(|path| {
            cx_before
                .as_deref()
                .filter(|s| !s.trim().is_empty())
                .map(toml::from_str)
                .transpose()
                .map_err(|e| format!("parse {}: {e}", path.display()))
                .map(|table| table.unwrap_or_default())
        })
        .transpose()?;
    let claude_changed = sqz_apply_claude_json(&mut cj, on)?;
    let codex_changed = if let Some(t) = cx.as_mut() {
        sqz_apply_codex(t, on)?
    } else {
        false
    };
    if on {
        std::fs::create_dir_all(&p.codex)
            .map_err(|e| format!("create {}: {e}", p.codex.display()))?;
    }

    let mut updates = Vec::new();
    if claude_changed {
        updates.push((p.claude_json.clone(), pretty(&cj)?, claude_before));
    }
    if let (Some(path), Some(t)) = (cx_path, cx.as_ref())
        && codex_changed
    {
        let content =
            toml::to_string_pretty(t).map_err(|e| format!("serialize {}: {e}", path.display()))?;
        updates.push((path, content, cx_before));
    }
    write_transaction(updates, write_keep_mode, remove_if_exists)
}

pub(crate) fn sqz_file_missing(p: &Paths, cfg: &Value) -> Result<Vec<String>, String> {
    let cj = read_json_file(&p.claude_json)?;
    let cx = codex_cfg(p).as_deref().map(read_toml_file).transpose()?;
    Ok(sqz_missing(cfg, &cj, cx.as_ref()))
}

// ─── RTK Codex ────────────────────────────────────────────────

pub(crate) fn snapshot(codex: &Path) -> BTreeMap<&'static str, Option<String>> {
    RTK_CODEX_FILES
        .iter()
        .map(|f| (*f, std::fs::read_to_string(codex.join(f)).ok()))
        .collect()
}

/// 记下变化的文件；已有记录保留最早的 `before`（重复开启不丢用户原文）。
pub(crate) fn record_changes(
    rec: &mut RtkCodexRecord,
    before: BTreeMap<&'static str, Option<String>>,
    after: BTreeMap<&'static str, Option<String>>,
) {
    for (f, a) in after {
        let b = before.get(f).cloned().flatten();
        if b == a {
            continue;
        }
        rec.entry(f.to_string())
            .or_insert_with(|| FileRec {
                before: b,
                after: None,
            })
            .after = a;
    }
}

pub(crate) fn rtk_codex_on(codex: &Path) -> bool {
    std::fs::read_to_string(codex.join("hooks.json"))
        .ok()
        .and_then(|s| serde_json::from_str::<Value>(&s).ok())
        .is_some_and(|v| has_hook(&v, "PreToolUse", RTK_CODEX_CMD))
}

/// 与记录写后内容一致 → 恢复写前；否则仅删除可精确识别的 RTK hook / 引用。
pub(crate) fn rtk_codex_disable(codex: &Path, rec: &RtkCodexRecord) -> Result<(), String> {
    for f in RTK_CODEX_FILES {
        let path = codex.join(f);
        let Ok(cur) = std::fs::read_to_string(&path) else {
            continue;
        };
        let Some(r) = rec.get(f) else {
            continue;
        };
        if r.after.as_deref() == Some(cur.as_str()) {
            match &r.before {
                Some(b) => write_keep_mode(&path, b)?,
                None => remove_if_exists(&path)?,
            }
            continue;
        }
        if r.after.is_none() {
            continue;
        }
        match f {
            "hooks.json" => {
                let originally_had_hook = r
                    .before
                    .as_deref()
                    .and_then(|s| serde_json::from_str::<Value>(s).ok())
                    .is_some_and(|v| has_hook(&v, "PreToolUse", RTK_CODEX_CMD));
                if originally_had_hook {
                    continue;
                }
                let mut v: Value = serde_json::from_str(&cur)
                    .map_err(|e| format!("parse {}: {e}", path.display()))?;
                if !has_hook(&v, "PreToolUse", RTK_CODEX_CMD) {
                    continue;
                }
                remove_hook(&mut v, "PreToolUse", RTK_CODEX_CMD);
                if v.as_object().is_some_and(|o| o.is_empty()) {
                    remove_if_exists(&path)?;
                } else {
                    write_keep_mode(&path, &pretty(&v)?)?;
                }
            }
            "AGENTS.md" => {
                let line = format!("@{}", codex.join("RTK.md").display());
                if r.before
                    .as_deref()
                    .is_some_and(|before| before.lines().any(|l| l == line))
                {
                    continue;
                }
                if cur.lines().any(|l| l == line) {
                    let kept: String = cur
                        .split_inclusive('\n')
                        .filter(|l| l.trim_end_matches(['\n', '\r']) != line)
                        .collect();
                    write_keep_mode(&path, &kept)?;
                }
            }
            "RTK.md" => {
                let Some(rtk_content) = rec.get(f).and_then(|r| r.after.as_deref()) else {
                    continue;
                };
                if let Some(user_content) = cur.strip_prefix(rtk_content) {
                    if user_content.is_empty() {
                        remove_if_exists(&path)?;
                    } else {
                        write_keep_mode(&path, user_content)?;
                    }
                    continue;
                }
                // 前插 / 中插：按段边界（行首行尾）定位 rtk 段后精确删除。
                let removed = cur.find(rtk_content).and_then(|pos| {
                    let end = pos + rtk_content.len();
                    let (before, after) = (&cur[..pos], &cur[end..]);
                    let aligned = (pos == 0 || before.ends_with('\n'))
                        && (end == cur.len() || rtk_content.ends_with('\n') || after.starts_with('\n'));
                    aligned.then(|| {
                        let mut out = String::with_capacity(cur.len() - rtk_content.len());
                        out.push_str(before);
                        if out.ends_with('\n') && after.starts_with('\n') {
                            out.pop();
                        }
                        out.push_str(after);
                        out
                    })
                });
                match removed {
                    Some(s) if s.trim().is_empty() => remove_if_exists(&path)?,
                    Some(s) => write_keep_mode(&path, &s)?,
                    None => {
                        return Err(
                            "RTK.md: cannot locate the recorded rtk block; re-run `rtk init -g --codex` or restore it manually"
                                .to_string(),
                        );
                    }
                }
            }
            _ => {}
        }
    }
    Ok(())
}

// ─── 命令 ─────────────────────────────────────────────────────

async fn run_cmd(program: &str, args: &[&str], envs: &[(&str, &Path)]) -> Result<String, String> {
    let mut c = tokio::process::Command::new(program);
    c.args(args).stdin(std::process::Stdio::null());
    // GUI 进程 PATH 极简，补登录 shell PATH（同 skills 的 npx 探测）
    if let Some(p) = aidog_skills::runtime_path() {
        c.env("PATH", p);
    }
    for (k, v) in envs {
        c.env(k, v);
    }
    let out = c.output().await.map_err(|e| format!("{program}: {e}"))?;
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    if out.status.success() {
        Ok(text)
    } else {
        Err(format!("{program} {}: {}", args.join(" "), text.trim()))
    }
}

async fn version(bin: &str) -> Option<String> {
    let out = run_cmd(bin, &["--version"], &[]).await.ok()?;
    out.split_whitespace().last().map(str::to_string)
}

/// 安装命令（一手来源）：
/// - rtk：https://github.com/rtk-ai/rtk README「brew install rtk」/
///   `curl -fsSL https://raw.githubusercontent.com/rtk-ai/rtk/refs/heads/master/install.sh | sh`
/// - sqz：https://github.com/ojuschugh1/sqz README「brew install ojuschugh1/sqz/sqz」/
///   `curl -fsSL https://raw.githubusercontent.com/ojuschugh1/sqz/main/install.sh | sh`（含 sqz-mcp）
fn install_script(tool: &str, brew: bool) -> Option<&'static str> {
    Some(match (tool, brew) {
        ("rtk", true) => "brew install rtk",
        ("rtk", false) => {
            "curl -fsSL https://raw.githubusercontent.com/rtk-ai/rtk/refs/heads/master/install.sh | sh"
        }
        ("sqz", true) => "brew install ojuschugh1/sqz/sqz",
        ("sqz", false) => {
            "curl -fsSL https://raw.githubusercontent.com/ojuschugh1/sqz/main/install.sh | sh"
        }
        _ => return None,
    })
}

/// 有效全局配置：同 `do_sync_group_settings`，DB 空时回落内置 oss 预设。
async fn load_claude_cfg(db: &Db) -> Value {
    aidog_db::get_setting(db, "global", "claude_code")
        .await
        .ok()
        .flatten()
        .filter(|v| v.as_object().is_some_and(|o| !o.is_empty()))
        .unwrap_or_else(crate::sync_settings::bundled_oss_preset)
}

async fn set_setting(db: &Db, key: &str, value: Value) -> Result<(), String> {
    aidog_db::set_setting(
        db,
        SetSettingInput {
            scope: "global".to_string(),
            key: key.to_string(),
            value,
        },
    )
    .await
    .map(|_| ())
}

/// 存全局配置 + 触发与设置页保存相同的分组同步。
async fn store_claude_cfg(db: &Db, cfg: Value) -> Result<(), String> {
    set_setting(db, "claude_code", cfg).await?;
    crate::sync_settings::try_sync_settings(db).await;
    Ok(())
}

async fn load_record(db: &Db) -> RtkCodexRecord {
    aidog_db::get_setting(db, "global", SETTING_KEY)
        .await
        .ok()
        .flatten()
        .and_then(|v| v.get("rtk_codex").cloned())
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or_default()
}

async fn store_record(db: &Db, rec: &RtkCodexRecord) -> Result<(), String> {
    set_setting(db, SETTING_KEY, json!({ "rtk_codex": rec })).await
}

async fn status(db: &Db) -> Result<ContextCompressStatus, String> {
    let p = Paths::resolve()?;
    let cfg = load_claude_cfg(db).await;
    let sqz_missing = sqz_file_missing(&p, &cfg)?;
    Ok(ContextCompressStatus {
        rtk_version: version("rtk").await,
        rtk_claude: has_hook(&cfg, "PreToolUse", RTK_CLAUDE_CMD),
        rtk_codex: rtk_codex_on(&p.codex),
        codex_present: p.codex.is_dir(),
        sqz_version: version("sqz").await,
        sqz_mcp_version: version(SQZ_BIN).await,
        sqz_enabled: sqz_missing.is_empty(),
        sqz_missing,
    })
}

crate::tauri_command! {
    pub async fn context_compress_status() -> Result<ContextCompressStatus, String> {
        status(aidog_ctx::db()).await
    }
}

crate::tauri_command! {
    /// 一键安装 rtk / sqz（有 brew 用 brew，否则官方 install.sh）。
    pub async fn context_compress_install(tool: String) -> Result<ContextCompressStatus, String> {
        let brew = run_cmd("brew", &["--version"], &[]).await.is_ok();
        let script = install_script(&tool, brew).ok_or_else(|| format!("unknown tool: {tool}"))?;
        run_cmd("sh", &["-c", script], &[]).await?;
        status(aidog_ctx::db()).await
    }
}

crate::tauri_command! {
    /// RTK 开关：`claude-code` 改全局配置 PreToolUse 钩子；`codex` 跑 `rtk init -g --codex` 并记录写入。
    pub async fn context_compress_rtk_set(agent: String, on: bool) -> Result<ContextCompressStatus, String> {
        let db = aidog_ctx::db();
        if on && version("rtk").await.is_none() {
            return Err("rtk is not installed".into());
        }
        match agent.as_str() {
            "claude-code" => {
                let mut cfg = load_claude_cfg(db).await;
                if on {
                    add_hook(&mut cfg, "PreToolUse", None, RTK_CLAUDE_CMD)?;
                } else {
                    remove_hook(&mut cfg, "PreToolUse", RTK_CLAUDE_CMD);
                }
                store_claude_cfg(db, cfg).await?;
            }
            "codex" => {
                let codex = Paths::resolve()?.codex;
                let mut rec = load_record(db).await;
                if on {
                    if !codex.is_dir() {
                        return Err(format!("{} does not exist", codex.display()));
                    }
                    let before = snapshot(&codex);
                    let res = run_cmd("rtk", &["init", "-g", "--codex"], &[("CODEX_HOME", &codex)]).await;
                    // 失败也记下已写的部分，关闭时能撤
                    record_changes(&mut rec, before, snapshot(&codex));
                    store_record(db, &rec).await?;
                    res?;
                } else {
                    rtk_codex_disable(&codex, &rec)?;
                    store_record(db, &RtkCodexRecord::default()).await?;
                }
            }
            _ => return Err(format!("unknown agent: {agent}")),
        }
        status(db).await
    }
}

crate::tauri_command! {
    /// sqz 总开关：开 = 补齐全部缺项；关 = 撤掉全部 sqz 项（其余钩子 / 服务器不动）。
    pub async fn context_compress_sqz_set(on: bool) -> Result<ContextCompressStatus, String> {
        let db = aidog_ctx::db();
        if on && (version("sqz").await.is_none() || version(SQZ_BIN).await.is_none()) {
            return Err("sqz / sqz-mcp is not installed".into());
        }
        sqz_apply_files(&Paths::resolve()?, on)?;
        let mut cfg = load_claude_cfg(db).await;
        let before = cfg.clone();
        sqz_apply_hooks(&mut cfg, on)?;
        if cfg != before {
            store_claude_cfg(db, cfg).await?;
        }
        status(db).await
    }
}

#[cfg(test)]
#[path = "test_context_compress.rs"]
mod test_context_compress;
