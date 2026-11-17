//! 扩展 → 子代理（magpie 资料库 Subagents 对齐，Claude Code + Codex）。
//!
//! 条目存 DB `setting`（scope="global", key="subagents"，JSON 数组）。按 agent 写出独立文件：
//! - Claude Code：`$CLAUDE_CONFIG_DIR/agents/<name>.md`（缺省 `~/.claude/agents`），YAML front matter
//!   （name / description）+ 正文 prompt。
//! - Codex：`$CODEX_HOME/agents/<name>.toml`（缺省 `~/.codex/agents`），键 name / description /
//!   model（有才写）/ developer_instructions。依据 openai/codex:codex-rs/agent-roles/src/loader.rs
//!   （`config_folder.join("agents")` 自动发现）+ discovery.rs（只收 `.toml`）+ agent_role_config.rs
//!   （name / description / developer_instructions）。
//!
//! 只撤自己写的：`written[agent]` 记写出内容 sha256；关闭 / 删除时磁盘内容哈希等于记录才删，
//! 用户改过（哈希不等）不删、标 modified；目标已有同名且无记录 → 不覆盖、标 conflict。

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use aidog_db::models::SetSettingInput;
use aidog_db::Db;
use serde::{Deserialize, Serialize};

pub const AGENT_CLAUDE: &str = "claude-code";
pub const AGENT_CODEX: &str = "codex";
const AGENTS: [&str; 2] = [AGENT_CLAUDE, AGENT_CODEX];
const SETTING_KEY: &str = "subagents";

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct Subagent {
    pub name: String,
    pub description: String,
    pub prompt: String,
    #[serde(default)]
    pub agents: Vec<String>,
    /// 绑定库内技能名（Claude front matter `skills`；Codex 角色文件无此字段不写）。
    #[serde(default)]
    pub skills: Vec<String>,
    /// 绑定库内 MCP server 名（Claude front matter `mcpServers`；Codex 不写）。
    #[serde(default)]
    pub mcp_servers: Vec<String>,
    /// 只写给 Codex 角色文件的 `model`。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub codex_model: Option<String>,
    /// 导入来源：`<source>#<相对路径>`。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from: Option<String>,
    /// 导入时 description + prompt 的哈希。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from_hash: Option<String>,
    /// agent → aidog 写出内容的 sha256。
    #[serde(default)]
    pub written: BTreeMap<String, String>,
}

/// 列表视图：条目 + 每个 agent 的文件状态（on / off / modified / conflict）
/// + 每 agent 能力（skills / mcpServers / model 是否可写，前端灰显不支持项）。
#[derive(Serialize, Clone, Debug)]
pub struct SubagentView {
    #[serde(flatten)]
    pub sub: Subagent,
    pub status: BTreeMap<String, String>,
    /// agent → 支持的能力名（"skills" / "mcpServers" / "model"）。
    pub capabilities: BTreeMap<String, Vec<String>>,
}

/// 每 agent 能力（对齐 magpie subagentSpecOf：只有 Claude Code 的文件能写
/// skills 与 mcpServers；只有 Codex 角色文件能写 model）。
pub(crate) fn agent_capabilities(agent: &str) -> Vec<&'static str> {
    match agent {
        AGENT_CLAUDE => vec!["skills", "mcpServers"],
        AGENT_CODEX => vec!["model"],
        _ => vec![],
    }
}

#[derive(Serialize, Clone, Debug, Default, PartialEq)]
pub struct SubagentImportReport {
    pub added: Vec<String>,
    pub updated: Vec<String>,
    pub have: Vec<String>,
    pub edited: Vec<String>,
    /// 同源内被浅层同名压掉的深层文件（相对路径）。
    pub conflicts: Vec<String>,
}

/// 两个 agent 的配置根目录（其下 `agents/`）。
pub struct Dirs {
    pub claude: PathBuf,
    pub codex: PathBuf,
}

impl Dirs {
    pub fn resolve() -> Result<Self, String> {
        let claude = match std::env::var("CLAUDE_CONFIG_DIR") {
            Ok(v) if !v.trim().is_empty() => PathBuf::from(v),
            _ => dirs::home_dir()
                .ok_or("cannot resolve home directory")?
                .join(".claude"),
        };
        Ok(Self {
            claude,
            codex: aidog_db::codex_home()?,
        })
    }
}

fn sha(s: &str) -> String {
    crate::gateway::import_export::container::sha256_hex(s.as_bytes())
}

fn content_hash(desc: &str, prompt: &str) -> String {
    sha(&format!("{desc}\n\0{prompt}"))
}

pub(crate) fn target_path(dirs: &Dirs, agent: &str, name: &str) -> PathBuf {
    if agent == AGENT_CODEX {
        dirs.codex.join("agents").join(format!("{name}.toml"))
    } else {
        dirs.claude.join("agents").join(format!("{name}.md"))
    }
}

fn render(sub: &Subagent, agent: &str) -> String {
    if agent == AGENT_CODEX {
        // Codex 角色文件（RawAgentRoleFileToml）无 skills / mcpServers 键，不写。
        let mut t = toml::Table::new();
        t.insert("name".into(), sub.name.clone().into());
        t.insert("description".into(), sub.description.clone().into());
        if let Some(m) = &sub.codex_model {
            t.insert("model".into(), m.clone().into());
        }
        t.insert("developer_instructions".into(), sub.prompt.clone().into());
        toml::to_string(&t).unwrap_or_default()
    } else {
        // JSON 字符串即合法 YAML 双引号标量，免手写转义；列表同理用 JSON 数组。
        let desc = serde_json::to_string(&sub.description).unwrap_or_default();
        let yaml_list = |xs: &[String]| serde_json::to_string(&xs).unwrap_or_default();
        let mut fm = format!("---\nname: {}\ndescription: {desc}", sub.name);
        if !sub.skills.is_empty() {
            fm.push_str(&format!("\nskills: {}", yaml_list(&sub.skills)));
        }
        if !sub.mcp_servers.is_empty() {
            fm.push_str(&format!("\nmcpServers: {}", yaml_list(&sub.mcp_servers)));
        }
        format!("{fm}\n---\n\n{}", sub.prompt)
    }
}

/// 按 `sub.agents` 写出 / 撤回文件，返回失败信息（单面失败不中断其余）。
pub(crate) fn sync_entry(sub: &mut Subagent, dirs: &Dirs) -> Vec<String> {
    let mut errs = Vec::new();
    for agent in AGENTS {
        let path = target_path(dirs, agent, &sub.name);
        let disk = std::fs::read_to_string(&path).ok().map(|d| sha(&d));
        let rec = sub.written.get(agent).cloned();
        let ours = disk.is_some() && disk == rec;
        if sub.agents.iter().any(|a| a == agent) {
            // 外来文件（无记录）或用户改过（哈希不等）→ 不覆盖
            if disk.is_some() && !ours {
                continue;
            }
            let content = render(sub, agent);
            let res = path
                .parent()
                .map_or(Ok(()), std::fs::create_dir_all)
                .and_then(|_| std::fs::write(&path, &content));
            match res {
                Ok(()) => {
                    sub.written.insert(agent.to_string(), sha(&content));
                }
                Err(e) => errs.push(format!("write {}: {e}", path.display())),
            }
        } else if ours {
            match std::fs::remove_file(&path) {
                Ok(()) => {
                    sub.written.remove(agent);
                }
                Err(e) => errs.push(format!("remove {}: {e}", path.display())),
            }
        } else if disk.is_none() {
            sub.written.remove(agent);
        }
    }
    errs
}

pub(crate) fn agent_status(sub: &Subagent, agent: &str, dirs: &Dirs) -> &'static str {
    let Ok(d) = std::fs::read_to_string(target_path(dirs, agent, &sub.name)) else {
        return "off";
    };
    match sub.written.get(agent) {
        Some(r) if *r == sha(&d) => "on",
        Some(_) => "modified",
        None if sub.agents.iter().any(|a| a == agent) => "conflict",
        None => "off",
    }
}

fn valid_name(n: &str) -> bool {
    !n.is_empty()
        && !n.starts_with('-')
        && n.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

fn slug(s: &str) -> String {
    let mut out = String::new();
    for c in s.chars().flat_map(char::to_lowercase) {
        if c.is_ascii_lowercase() || c.is_ascii_digit() {
            out.push(c);
        } else if !out.is_empty() && !out.ends_with('-') {
            out.push('-');
        }
    }
    out.trim_end_matches('-').to_string()
}

fn one_line(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn unique_name(list: &[Subagent], base: &str) -> String {
    let taken = |n: &str| list.iter().any(|s| s.name == n);
    if !taken(base) {
        return base.to_string();
    }
    (2..)
        .map(|i| format!("{base}-{i}"))
        .find(|n| !taken(n))
        .unwrap_or_default()
}

/// (相对路径, name, description, prompt)
fn scan(root: &Path) -> Vec<(String, String, String, String)> {
    let mut found = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        if dir.join("SKILL.md").exists() {
            continue;
        }
        let Ok(rd) = std::fs::read_dir(&dir) else { continue };
        for p in rd.flatten().map(|e| e.path()) {
            let fname = p.file_name().unwrap_or_default().to_string_lossy().to_string();
            if p.is_dir() {
                if ![".git", ".github", "node_modules"].contains(&fname.as_str()) {
                    stack.push(p);
                }
                continue;
            }
            if p.extension().is_none_or(|x| x != "md") {
                continue;
            }
            let text = std::fs::read_to_string(&p).unwrap_or_default();
            let Some((fm, body)) = text.strip_prefix("---").and_then(|r| r.split_once("\n---")) else {
                continue;
            };
            let Ok(v) = serde_yaml::from_str::<serde_yaml::Value>(fm) else { continue };
            let desc = one_line(v.get("description").and_then(|d| d.as_str()).unwrap_or(""));
            if desc.is_empty() {
                continue;
            }
            let name = v
                .get("name")
                .and_then(|n| n.as_str())
                .map(slug)
                .filter(|n| !n.is_empty())
                .unwrap_or_else(|| slug(p.file_stem().unwrap_or_default().to_string_lossy().as_ref()));
            if name.is_empty() {
                continue;
            }
            // 闭合行 `---` 之后的正文，去掉紧随的换行
            let body = body.split_once('\n').map_or("", |(_, b)| b).to_string();
            let rel = p
                .strip_prefix(root)
                .unwrap_or(&p)
                .components()
                .map(|c| c.as_os_str().to_string_lossy())
                .collect::<Vec<_>>()
                .join("/");
            found.push((rel, name, desc, body));
        }
    }
    // 浅层优先：同名冲突时浅层文件胜出（import_into 依赖此顺序）
    found.sort_by(|a, b| {
        a.0.split('/').count().cmp(&b.0.split('/').count()).then(a.0.cmp(&b.0))
    });
    found
}

/// 扫描 `root` 导入到 `list`（来源键 `source`），新条目勾选 `agents` 并写出。
/// `selected` = 只导入这些相对路径；None = 全部。
/// 同来源再导入：内容同 = have；上游变且本地未改 = update；本地改过 = edited（不动）。
/// 同名冲突：本源内浅层文件优先，深层计 `conflicts`；与既有条目同名仍走改名。
pub(crate) fn import_into(
    list: &mut Vec<Subagent>,
    root: &Path,
    source: &str,
    agents: &[String],
    selected: Option<&[String]>,
    dirs: &Dirs,
) -> SubagentImportReport {
    let mut report = SubagentImportReport::default();
    let mut claimed: std::collections::HashSet<String> = Default::default();
    for (rel, name, desc, prompt) in scan(root) {
        if selected.is_some_and(|s| !s.iter().any(|r| r == &rel)) {
            continue;
        }
        let from = format!("{source}#{rel}");
        let upstream = content_hash(&desc, &prompt);
        if let Some(s) = list.iter_mut().find(|s| s.from.as_deref() == Some(&from)) {
            let local = content_hash(&s.description, &s.prompt);
            if s.from_hash.as_deref() != Some(&local) {
                report.edited.push(s.name.clone());
            } else if local == upstream {
                report.have.push(s.name.clone());
            } else {
                s.description = desc;
                s.prompt = prompt;
                s.from_hash = Some(upstream);
                sync_entry(s, dirs);
                report.updated.push(s.name.clone());
            }
            claimed.insert(s.name.clone());
            continue;
        }
        if claimed.contains(&name) {
            report.conflicts.push(rel);
            continue;
        }
        let assigned = unique_name(list, &name);
        claimed.insert(assigned.clone());
        let mut s = Subagent {
            name: assigned,
            description: desc,
            prompt,
            agents: agents.iter().filter(|a| AGENTS.contains(&a.as_str())).cloned().collect(),
            from: Some(from),
            from_hash: Some(upstream),
            ..Default::default()
        };
        sync_entry(&mut s, dirs);
        report.added.push(s.name.clone());
        list.push(s);
    }
    report
}

async fn load(db: &Db) -> Vec<Subagent> {
    match aidog_db::get_setting(db, "global", SETTING_KEY).await {
        Ok(Some(v)) => serde_json::from_value(v).unwrap_or_default(),
        _ => Vec::new(),
    }
}

async fn store(db: &Db, list: &[Subagent]) -> Result<(), String> {
    let value = serde_json::to_value(list).map_err(|e| e.to_string())?;
    aidog_db::set_setting(
        db,
        SetSettingInput {
            scope: "global".to_string(),
            key: SETTING_KEY.to_string(),
            value,
        },
    )
    .await
    .map(|_| ())
}

fn views(list: &[Subagent], dirs: &Dirs) -> Vec<SubagentView> {
    list.iter()
        .map(|s| SubagentView {
            sub: s.clone(),
            status: AGENTS
                .iter()
                .map(|a| (a.to_string(), agent_status(s, a, dirs).to_string()))
                .collect(),
            capabilities: AGENTS
                .iter()
                .map(|a| (a.to_string(), agent_capabilities(a).iter().map(|s| s.to_string()).collect()))
                .collect(),
        })
        .collect()
}

crate::tauri_command! {
    pub async fn subagents_list() -> Result<Vec<SubagentView>, String> {
        let db = aidog_ctx::db();
        let dirs = Dirs::resolve()?;
        Ok(views(&load(db).await, &dirs))
    }
}

crate::tauri_command! {
    /// 新建（`old_name` 空）或编辑。导入来源与写出记录沿用旧条目；改名先撤回旧名文件。
    pub async fn subagents_save(old_name: Option<String>, subagent: Subagent) -> Result<Vec<SubagentView>, String> {
        let db = aidog_ctx::db();
        let dirs = Dirs::resolve()?;
        let mut list = load(db).await;
        let mut s = subagent;
        s.name = s.name.trim().to_string();
        if !valid_name(&s.name) {
            return Err(format!("invalid name: {}", s.name));
        }
        s.description = one_line(&s.description);
        if s.description.is_empty() {
            return Err("description is required".into());
        }
        s.agents.retain(|a| AGENTS.contains(&a.as_str()));
        s.skills.retain(|n| !n.trim().is_empty());
        s.skills = s.skills.iter().map(|n| n.trim().to_string()).collect();
        s.mcp_servers.retain(|n| !n.trim().is_empty());
        s.mcp_servers = s.mcp_servers.iter().map(|n| n.trim().to_string()).collect();
        s.codex_model = s.codex_model.map(|m| m.trim().to_string()).filter(|m| !m.is_empty());
        let old_idx = old_name.as_deref().and_then(|n| list.iter().position(|x| x.name == n));
        if list.iter().enumerate().any(|(i, x)| x.name == s.name && Some(i) != old_idx) {
            return Err(format!("name already exists: {}", s.name));
        }
        let mut errs = Vec::new();
        (s.from, s.from_hash, s.written) = (None, None, BTreeMap::new());
        if let Some(i) = old_idx {
            let mut old = list.remove(i);
            (s.from, s.from_hash) = (old.from.clone(), old.from_hash.clone());
            if old.name == s.name {
                s.written = old.written;
            } else {
                old.agents.clear();
                errs.extend(sync_entry(&mut old, &dirs));
            }
            list.insert(i, s);
        } else {
            list.push(s);
        }
        let i = old_idx.unwrap_or(list.len() - 1);
        errs.extend(sync_entry(&mut list[i], &dirs));
        store(db, &list).await?;
        if !errs.is_empty() {
            return Err(errs.join("; "));
        }
        Ok(views(&list, &dirs))
    }
}

crate::tauri_command! {
    /// 删除条目：撤回 aidog 写出且未被改过的文件；返回因被用户修改而保留的文件路径。
    pub async fn subagents_delete(name: String) -> Result<Vec<String>, String> {
        let db = aidog_ctx::db();
        let dirs = Dirs::resolve()?;
        let mut list = load(db).await;
        let Some(i) = list.iter().position(|x| x.name == name) else {
            return Ok(Vec::new());
        };
        let mut s = list.remove(i);
        s.agents.clear();
        let errs = sync_entry(&mut s, &dirs);
        if !errs.is_empty() {
            return Err(errs.join("; "));
        }
        store(db, &list).await?;
        Ok(s.written
            .keys()
            .map(|a| target_path(&dirs, a, &s.name).display().to_string())
            .collect())
    }
}

/// 非本地目录来源浅克隆到 `~/.aidog/tmp/` 临时目录；本地目录原样返回。
async fn stage_source(source: &str) -> Result<(PathBuf, Option<tempfile::TempDir>), String> {
    let local = PathBuf::from(source);
    if local.is_dir() {
        return Ok((local, None));
    }
    let base = crate::shared::aidog_data_dir()?.join("tmp");
    std::fs::create_dir_all(&base).map_err(|e| format!("create {}: {e}", base.display()))?;
    let tmp = tempfile::Builder::new()
        .prefix("subagents-import-")
        .tempdir_in(&base)
        .map_err(|e| e.to_string())?;
    let dest = tmp.path().join("repo");
    let out = tokio::process::Command::new("git")
        .args(["clone", "--depth", "1", "--"])
        .arg(source)
        .arg(&dest)
        .output()
        .await
        .map_err(|e| format!("git clone: {e}"))?;
    if !out.status.success() {
        return Err(format!("git clone failed: {}", String::from_utf8_lossy(&out.stderr).trim()));
    }
    Ok((dest, Some(tmp)))
}

/// (相对路径, name, description, prompt)
#[derive(Serialize, Clone, Debug)]
pub struct SubagentImportCandidate {
    pub rel: String,
    pub name: String,
    pub description: String,
    pub prompt: String,
}

crate::tauri_command! {
    /// 导入前预扫（只读），返回候选清单供 UI 勾选。
    pub async fn subagents_import_preview(source: String) -> Result<Vec<SubagentImportCandidate>, String> {
        let (root, tmp) = stage_source(source.trim()).await?;
        let cands = scan(&root)
            .into_iter()
            .map(|(rel, name, description, prompt)| SubagentImportCandidate { rel, name, description, prompt })
            .collect();
        drop(tmp);
        Ok(cands)
    }
}

crate::tauri_command! {
    /// 从本地文件夹或 git 地址导入（git 浅克隆到 `~/.aidog/tmp/`，用完删）。
    /// `selected` = 只导入这些相对路径；缺省 = 全部。
    pub async fn subagents_import(
        source: String,
        agents: Vec<String>,
        selected: Option<Vec<String>>,
    ) -> Result<SubagentImportReport, String> {
        let db = aidog_ctx::db();
        let dirs = Dirs::resolve()?;
        let source = source.trim().to_string();
        let (root, tmp) = stage_source(&source).await?;
        let mut list = load(db).await;
        let report = import_into(&mut list, &root, &source, &agents, selected.as_deref(), &dirs);
        store(db, &list).await?;
        drop(tmp);
        Ok(report)
    }
}

#[cfg(test)]
#[path = "test_subagents.rs"]
mod test_subagents;
