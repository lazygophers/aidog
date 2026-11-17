//! Claude Code plugin bundles: safe source staging, unpacking, distribution and own-write cleanup.
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::{
    context_compress,
    subagents::{self, Subagent},
};
use crate::gateway;

const KEY: &str = "claude_plugins";
const AGENTS: [&str; 2] = [subagents::AGENT_CLAUDE, subagents::AGENT_CODEX];

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
struct SkillPart {
    name: String,
    source: String,
    #[serde(default)]
    written: BTreeMap<String, String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
struct HookPart {
    event: String,
    matcher: Option<String>,
    command: String,
    #[serde(default)]
    written: BTreeMap<String, String>,
    #[serde(default)]
    preexisting: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
struct ServerPart {
    name: String,
    #[serde(default)]
    agents: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
struct Plugin {
    name: String,
    source: String,
    pick: Option<String>,
    agents: Vec<String>,
    #[serde(default)]
    off: bool,
    #[serde(default)]
    skills: Vec<SkillPart>,
    #[serde(default)]
    subagents: Vec<String>,
    #[serde(default)]
    hooks: Vec<HookPart>,
    #[serde(default)]
    servers: Vec<ServerPart>,
    #[serde(default)]
    left: Vec<String>,
}

#[derive(Serialize, Clone, Debug, Default)]
pub struct PluginView {
    pub name: String,
    pub source: String,
    pub pick: Option<String>,
    pub agents: Vec<String>,
    pub on: bool,
    pub skills: Vec<String>,
    pub subagents: Vec<String>,
    pub hooks: Vec<String>,
    pub servers: Vec<String>,
    pub left: Vec<String>,
}

#[derive(Serialize, Clone, Debug, Default)]
pub struct PluginViewList {
    pub plugins: Vec<PluginView>,
    pub warnings: Vec<String>,
}

#[derive(Serialize, Clone, Debug, Default)]
pub struct PluginCandidate {
    pub name: String,
    pub description: String,
    pub version: String,
    /// Folder path in the marketplace source.
    pub rel: String,
    /// Optional separate git repository for a marketplace plugin.
    pub repository: Option<String>,
    pub installed: bool,
}

fn view(p: &Plugin) -> PluginView {
    PluginView {
        name: p.name.clone(),
        source: p.source.clone(),
        pick: p.pick.clone(),
        agents: p.agents.clone(),
        on: !p.off,
        skills: p.skills.iter().map(|x| x.name.clone()).collect(),
        subagents: p.subagents.clone(),
        hooks: p
            .hooks
            .iter()
            .map(|h| format!("{}: {}", h.event, h.command))
            .collect(),
        servers: p.servers.iter().map(|s| s.name.clone()).collect(),
        left: p.left.clone(),
    }
}

async fn load(db: &aidog_db::Db) -> Vec<Plugin> {
    aidog_db::get_setting(db, "global", KEY)
        .await
        .ok()
        .flatten()
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or_default()
}

async fn save(db: &aidog_db::Db, rows: &[Plugin]) -> Result<(), String> {
    aidog_db::set_setting(
        db,
        aidog_db::models::SetSettingInput {
            scope: "global".into(),
            key: KEY.into(),
            value: serde_json::to_value(rows).map_err(|e| e.to_string())?,
        },
    )
    .await
    .map(|_| ())
}

fn sha(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

fn root_dir() -> Result<PathBuf, String> {
    Ok(crate::shared::aidog_data_dir()?.join("plugins"))
}
fn plugin_dir(name: &str) -> Result<PathBuf, String> {
    Ok(root_dir()?.join(name))
}
fn plugin_data(name: &str) -> Result<PathBuf, String> {
    Ok(crate::shared::aidog_data_dir()?
        .join("plugin-data")
        .join(name))
}
fn dirs() -> Result<subagents::Dirs, String> {
    subagents::Dirs::resolve()
}

fn slug(s: &str) -> String {
    let mut out = String::new();
    for c in s.trim().chars() {
        if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
            out.push(c);
        } else if !out.is_empty() && !out.ends_with('-') {
            out.push('-');
        }
    }
    let out = out.trim_matches(['-', '_']).to_string();
    out.chars()
        .take(40)
        .collect::<String>()
        .trim_matches(['-', '_'])
        .to_string()
}

fn copy_tree(src: &Path, dst: &Path) -> Result<(), String> {
    std::fs::create_dir_all(dst).map_err(|e| e.to_string())?;
    let mut entries = std::fs::read_dir(src)
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    entries.sort_by_key(|e| e.file_name());
    for e in entries {
        let from = e.path();
        let to = dst.join(e.file_name());
        let meta = std::fs::symlink_metadata(&from).map_err(|e| e.to_string())?;
        if meta.file_type().is_symlink() {
            return Err(format!(
                "plugin source contains symlink: {}",
                from.display()
            ));
        }
        if meta.is_dir() {
            copy_tree(&from, &to)?;
        } else if meta.is_file() {
            std::fs::copy(&from, &to).map_err(|e| format!("copy {}: {e}", from.display()))?;
        }
    }
    Ok(())
}

/// Removes a distributed folder (outside the library) file by file after its tree hash matched;
/// never recursive-deletes: a file that appears after the check keeps its folder.
fn remove_owned_tree(dir: &Path) -> Result<(), String> {
    for e in std::fs::read_dir(dir).map_err(|e| e.to_string())? {
        let p = e.map_err(|e| e.to_string())?.path();
        let meta = std::fs::symlink_metadata(&p).map_err(|e| e.to_string())?;
        if meta.is_dir() {
            remove_owned_tree(&p)?;
        } else {
            std::fs::remove_file(&p).map_err(|e| format!("remove {}: {e}", p.display()))?;
        }
    }
    std::fs::remove_dir(dir).map_err(|e| format!("remove {}: {e}", dir.display()))
}

/// Recursive delete only for a direct child of the library's plugins folder, checked after
/// canonicalization (no symlink or `..` can point it elsewhere).
fn remove_library_dir(name: &str) -> Result<(), String> {
    let root = root_dir()?;
    let dir = root.join(name);
    if !dir.exists() {
        return Ok(());
    }
    let (root, dir) = (
        root.canonicalize().map_err(|e| e.to_string())?,
        dir.canonicalize().map_err(|e| e.to_string())?,
    );
    if dir.parent() != Some(root.as_path()) {
        return Err(format!("refuse to delete outside library: {}", dir.display()));
    }
    std::fs::remove_dir_all(&dir).map_err(|e| format!("remove {}: {e}", dir.display()))
}

fn tree_hash(root: &Path) -> Result<String, String> {
    fn walk(root: &Path, dir: &Path, out: &mut Vec<u8>) -> Result<(), String> {
        let mut es = std::fs::read_dir(dir)
            .map_err(|e| e.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;
        es.sort_by_key(|e| e.file_name());
        for e in es {
            let p = e.path();
            let meta = std::fs::symlink_metadata(&p).map_err(|e| e.to_string())?;
            if meta.file_type().is_symlink() {
                return Err(format!("skill contains symlink: {}", p.display()));
            }
            if meta.is_dir() {
                walk(root, &p, out)?;
            } else if meta.is_file() {
                out.extend_from_slice(
                    p.strip_prefix(root)
                        .map_err(|e| e.to_string())?
                        .to_string_lossy()
                        .as_bytes(),
                );
                out.push(0);
                out.extend_from_slice(&std::fs::read(&p).map_err(|e| e.to_string())?);
                out.push(0);
            }
        }
        Ok(())
    }
    let mut bytes = Vec::new();
    walk(root, root, &mut bytes)?;
    Ok(sha(&bytes))
}

fn resolve(s: &str, root: &Path, data: &Path) -> String {
    let root = root.to_string_lossy().replace('\\', "/");
    let data = data.to_string_lossy().replace('\\', "/");
    s.replace("${CLAUDE_PLUGIN_ROOT}", &root)
        .replace("$CLAUDE_PLUGIN_ROOT", &root)
        .replace("%CLAUDE_PLUGIN_ROOT%", &root)
        .replace("${CLAUDE_PLUGIN_DATA}", &data)
        .replace("$CLAUDE_PLUGIN_DATA", &data)
        .replace("%CLAUDE_PLUGIN_DATA%", &data)
}

/// process.env.NAME resolves to a quoted literal. Keep this small and target the documented JS spelling.
fn resolve_hook(command: &str, root: &Path, data: &Path) -> String {
    let mut s = resolve(command, root, data);
    for (name, path) in [("CLAUDE_PLUGIN_ROOT", root), ("CLAUDE_PLUGIN_DATA", data)] {
        let p = path.to_string_lossy().replace('\\', "/");
        s = s.replace(
            &format!("process.env.{name}"),
            &format!("'{}'", p.replace('\'', "\\'")),
        );
    }
    s
}

fn read_json(path: &Path) -> Result<Value, String> {
    let text =
        std::fs::read_to_string(path).map_err(|e| format!("read {}: {e}", path.display()))?;
    serde_json::from_str(text.trim_start_matches('\u{feff}'))
        .map_err(|e| format!("parse {}: {e}", path.display()))
}

fn marketplace(root: &Path) -> Result<Vec<PluginCandidate>, String> {
    let path = [
        root.join(".claude-plugin/marketplace.json"),
        root.join("marketplace.json"),
    ]
    .into_iter()
    .find(|p| p.exists())
    .ok_or_else(|| format!("marketplace.json not found in {}", root.display()))?;
    let v = read_json(&path)?;
    let mut out = Vec::new();
    for p in v
        .get("plugins")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let name = p.get("name").and_then(Value::as_str).unwrap_or_default();
        let (source, repository) = match p.get("source") {
            Some(Value::String(path)) => (path.trim_start_matches("./").to_string(), None),
            Some(Value::Object(source)) => {
                let kind = source
                    .get("source")
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                let path = source
                    .get("path")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .trim_start_matches("./")
                    .to_string();
                let repo = source
                    .get("repo")
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                let repository = match kind {
                    "github" if !repo.is_empty() => Some(if repo.starts_with("http") {
                        repo.to_string()
                    } else {
                        format!("https://github.com/{repo}.git")
                    }),
                    "git" if !repo.is_empty() => Some(repo.to_string()),
                    _ => None,
                };
                (path, repository)
            }
            _ => (String::new(), None),
        };
        if name.is_empty()
            || (source.is_empty() && repository.is_none())
            || Path::new(&source).is_absolute()
            || Path::new(&source)
                .components()
                .any(|c| c == std::path::Component::ParentDir)
        {
            continue;
        }
        out.push(PluginCandidate {
            name: name.into(),
            description: p
                .get("description")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .into(),
            version: p
                .get("version")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .into(),
            rel: source,
            repository,
            installed: false,
        });
    }
    Ok(out)
}

fn plugin_name(path: &Path, hint: &str) -> Result<(String, String, String), String> {
    let p = path.join(".claude-plugin/plugin.json");
    let v = if p.exists() {
        read_json(&p)?
    } else {
        json!({})
    };
    let name = slug(v.get("name").and_then(Value::as_str).unwrap_or(hint));
    if name.is_empty() {
        return Err("plugin name is empty".into());
    }
    Ok((
        name,
        v.get("description")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .into(),
        v.get("version")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .into(),
    ))
}

fn candidates(root: &Path, installed: &[Plugin]) -> Result<Vec<PluginCandidate>, String> {
    let m = root.join(".claude-plugin/marketplace.json").exists()
        || root.join("marketplace.json").exists();
    if m {
        let mut c = marketplace(root)?;
        for x in &mut c {
            x.installed = installed.iter().any(|p| p.name == x.name);
        }
        return Ok(c);
    }
    if !root.join(".claude-plugin/plugin.json").exists()
        && !root.join("skills").exists()
        && !root.join("agents").exists()
        && !root.join(".mcp.json").exists()
        && !root.join("hooks/hooks.json").exists()
    {
        return Err(format!("no Claude plugin in {}", root.display()));
    }
    let (name, description, version) = plugin_name(
        root,
        &root.file_name().unwrap_or_default().to_string_lossy(),
    )?;
    Ok(vec![PluginCandidate {
        installed: installed.iter().any(|p| p.name == name),
        name,
        description,
        version,
        rel: String::new(),
        repository: None,
    }])
}

async fn stage(source: &str) -> Result<(PathBuf, Option<tempfile::TempDir>), String> {
    let path = PathBuf::from(source);
    if path.is_dir() {
        return Ok((path, None));
    }
    if path.is_file() {
        if path.extension().is_some_and(|e| e == "json") {
            let mut root = path
                .parent()
                .ok_or("marketplace file has no parent")?
                .to_path_buf();
            if root.file_name().is_some_and(|n| n == ".claude-plugin") {
                root.pop();
            }
            return Ok((root, None));
        }
        return Err(format!(
            "source is not a folder or marketplace.json: {}",
            path.display()
        ));
    }
    if source.trim().is_empty()
        || source.starts_with('-')
        || source.starts_with("ext::")
        || source.starts_with("file::")
    {
        return Err("invalid git source".into());
    }
    let tmpbase = crate::shared::aidog_data_dir()?.join("tmp");
    std::fs::create_dir_all(&tmpbase).map_err(|e| e.to_string())?;
    let tmp = tempfile::Builder::new()
        .prefix("plugin-source-")
        .tempdir_in(tmpbase)
        .map_err(|e| e.to_string())?;
    let dest = tmp.path().join("repo");
    let out = tokio::process::Command::new("git")
        .args([
            "-c",
            "protocol.ext.allow=never",
            "clone",
            "--depth",
            "1",
            "--",
            source,
        ])
        .arg(&dest)
        .output()
        .await
        .map_err(|e| format!("git clone: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "git clone failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    Ok((dest, Some(tmp)))
}

fn skill_entries(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let d = root.join("skills");
    if let Ok(entries) = std::fs::read_dir(&d) {
        for e in entries.flatten() {
            if e.path().is_dir() && e.path().join("SKILL.md").is_file() {
                out.push(e.path());
            }
        }
    }
    out.sort();
    out
}

fn free_name(base: &str, plugin: &str, taken: impl Fn(&str) -> bool) -> String {
    if !taken(base) {
        return base.to_string();
    }
    let prefix = format!("{plugin}-{base}");
    if !taken(&prefix) {
        return prefix;
    }
    (2..)
        .map(|i| format!("{prefix}-{i}"))
        .find(|n| !taken(n))
        .unwrap_or(prefix)
}

async fn choose_server_name(db: &aidog_db::Db, base: &str, plugin: &str) -> Result<String, String> {
    let prefix = format!("{plugin}-{base}");
    for candidate in [base.to_string(), prefix.clone()] {
        let in_db = gateway::mcp::store::get_mcp_server(db, &candidate)
            .await?
            .is_some();
        let in_files = !gateway::mcp::agents_with_entry(&candidate).is_empty();
        if !in_db && !in_files {
            return Ok(candidate);
        }
    }
    for i in 2.. {
        let candidate = format!("{prefix}-{i}");
        let in_db = gateway::mcp::store::get_mcp_server(db, &candidate)
            .await?
            .is_some();
        let in_files = !gateway::mcp::agents_with_entry(&candidate).is_empty();
        if !in_db && !in_files {
            return Ok(candidate);
        }
    }
    unreachable!()
}

/// Copy a plugin source into its owned folder, unpack normal skills/subagents/hooks/MCP,
/// and record each distributed part for exact cleanup.
async fn install_impl(
    db: &aidog_db::Db,
    source: String,
    agents: Vec<String>,
    selected: Option<Vec<String>>,
) -> Result<PluginViewList, String> {
    let source = source.trim().to_string();
    let (staged, tmp) = stage(&source).await?;
    let mut plugins = load(db).await;
    let cs = candidates(&staged, &plugins)?;
    let picked: Vec<_> = cs
        .iter()
        .filter(|c| {
            selected
                .as_ref()
                .is_none_or(|names| names.contains(&c.name))
        })
        .cloned()
        .collect();
    if picked.is_empty() {
        return Err("no plugin selected".into());
    }
    let mut warnings = Vec::new();
    for c in picked {
        let (candidate_root, _candidate_tmp) = if let Some(repo) = &c.repository {
            let (root, temporary) = stage(repo).await?;
            (root, temporary)
        } else {
            (staged.clone(), None)
        };
        let src = if c.rel.is_empty() {
            candidate_root.clone()
        } else {
            candidate_root.join(&c.rel)
        };
        let (name, _, _) = plugin_name(&src, &c.name)?;
        let dest = plugin_dir(&name)?;
        if dest.exists() && !plugins.iter().any(|p| p.name == name) {
            return Err(format!("plugin folder already exists: {}", dest.display()));
        }
        let rootdir = crate::shared::aidog_data_dir()?.join("plugins");
        std::fs::create_dir_all(&rootdir).map_err(|e| e.to_string())?;
        let staged_plugin = tempfile::Builder::new()
            .prefix("plugin-install-")
            .tempdir_in(&rootdir)
            .map_err(|e| e.to_string())?;
        let copied = staged_plugin.path().join("bundle");
        copy_tree(&src, &copied)?;
        // Validate package manifests before retracting an existing version.
        plugin_name(&copied, &c.name)?;
        read_hooks(&copied)?;
        if copied.join(".mcp.json").exists() {
            mcp_servers(&read_json(&copied.join(".mcp.json"))?)?;
        }
        let old_idx = plugins.iter().position(|p| p.name == name);
        let old = old_idx.map(|i| plugins[i].clone());
        if let Some(mut previous) = old.clone() {
            previous.off = true;
            sync_parts(db, &mut previous, &mut warnings).await?;
            remove_parts_db(db, &previous).await?;
        }
        let backup = staged_plugin.path().join("old");
        if dest.exists() {
            std::fs::rename(&dest, &backup).map_err(|e| format!("stage old plugin: {e}"))?;
        }
        if let Err(e) = std::fs::rename(&copied, &dest) {
            if backup.exists() {
                let _ = std::fs::rename(&backup, &dest);
            }
            return Err(format!("replace plugin folder: {e}"));
        }
        let data = plugin_data(&name)?;
        std::fs::create_dir_all(&data).map_err(|e| e.to_string())?;
        let (name2, _, _) = plugin_name(&dest, &c.name)?;
        let mut p = old.unwrap_or_default();
        p.name = name2.clone();
        p.source = source.clone();
        p.pick = (cs.len() > 1).then(|| c.name.clone());
        if p.agents.is_empty() {
            p.agents = agents
                .iter()
                .filter(|a| AGENTS.contains(&a.as_str()))
                .cloned()
                .collect();
        }
        p.skills.clear();
        p.subagents.clear();
        p.hooks.clear();
        p.servers.clear();
        p.left = unsupported_parts(&dest)?;

        // Skills keep a conflict-free name here; sync_parts copies them and records tree hashes.
        for sdir in skill_entries(&dest) {
            let sname = sdir
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            let target_name = free_name(&sname, &name, |n| {
                AGENTS
                    .iter()
                    .any(|a| skills_dir(a).map(|d| d.join(n).exists()).unwrap_or(false))
            });
            p.skills.push(SkillPart {
                name: target_name,
                source: sname,
                written: BTreeMap::new(),
            });
        }

        // Plugin agent files become normal subagents in the shared DB list.
        let mut subs = subagents::load_list(db).await;
        let mut agent_files = Vec::new();
        if let Ok(rd) = std::fs::read_dir(dest.join("agents")) {
            for e in rd.flatten() {
                if e.path().extension().is_some_and(|x| x == "md") {
                    agent_files.push(e.path());
                }
            }
        }
        agent_files.sort();
        for f in agent_files {
            let tmpdir = f.parent().unwrap();
            for (_rel, sname, description, prompt) in subagents::scan(tmpdir) {
                if sname != f.file_stem().unwrap_or_default().to_string_lossy() {
                    continue;
                }
                let assigned = free_name(&sname, &name, |n| subs.iter().any(|s| s.name == n));
                let mut s = Subagent {
                    name: assigned.clone(),
                    description,
                    prompt: resolve(&prompt, &dest, &data),
                    agents: p.agents.clone(),
                    bundle: Some(name.clone()),
                    ..Default::default()
                };
                subagents::sync_entry(&mut s, &subagents::Dirs::resolve()?);
                p.subagents.push(assigned);
                subs.push(s);
            }
        }
        subagents::store_list(db, &subs).await?;

        // Hook command groups; record only groups this bundle adds. Commands already present are user-owned.
        for (event, groups) in read_hooks(&dest)? {
            for (matcher, command) in groups {
                let command = resolve_hook(&command, &dest, &data);
                p.hooks.push(HookPart {
                    event: event.clone(),
                    matcher,
                    command,
                    written: BTreeMap::new(),
                    preexisting: Vec::new(),
                });
            }
        }

        // MCP servers become ordinary DB entries; the mcp crate records own-write hashes per agent.
        let mcpfile = dest.join(".mcp.json");
        if mcpfile.exists() {
            let v = read_json(&mcpfile)?;
            for (server_name, entry) in mcp_servers(&v)? {
                // Same transport inference as Claude Code: url without type = http.
                let transport = match entry.get("type").and_then(Value::as_str) {
                    Some("sse") => "sse",
                    Some("http" | "streamable-http") => "http",
                    None if entry.get("url").and_then(Value::as_str).is_some() => "http",
                    None | Some("stdio") => "stdio",
                    Some(other) => {
                        p.left.push(format!(
                            "mcp {server_name}: transport {other} is not carried"
                        ));
                        continue;
                    }
                };
                let mcp_name = choose_server_name(db, server_name, &name).await?;
                let mut env: BTreeMap<String, String> = entry
                    .get("env")
                    .and_then(Value::as_object)
                    .into_iter()
                    .flatten()
                    .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), resolve(s, &dest, &data))))
                    .collect();
                env.insert(
                    "CLAUDE_PLUGIN_ROOT".into(),
                    dest.to_string_lossy().into_owned(),
                );
                env.insert(
                    "CLAUDE_PLUGIN_DATA".into(),
                    data.to_string_lossy().into_owned(),
                );
                let payload = gateway::mcp::McpUpdatePayload {
                    name: mcp_name.clone(),
                    transport: transport.into(),
                    command: resolve(
                        entry
                            .get("command")
                            .and_then(Value::as_str)
                            .unwrap_or_default(),
                        &dest,
                        &data,
                    ),
                    args: entry
                        .get("args")
                        .and_then(Value::as_array)
                        .into_iter()
                        .flatten()
                        .filter_map(Value::as_str)
                        .map(|s| resolve(s, &dest, &data))
                        .collect(),
                    env,
                    url: resolve(
                        entry.get("url").and_then(Value::as_str).unwrap_or_default(),
                        &dest,
                        &data,
                    ),
                    headers: entry
                        .get("headers")
                        .and_then(Value::as_object)
                        .into_iter()
                        .flatten()
                        .filter_map(|(k, v)| {
                            v.as_str().map(|s| (k.clone(), resolve(s, &dest, &data)))
                        })
                        .collect(),
                };
                gateway::mcp::add_server(db, payload).await?;
                for agent in AGENTS {
                    let target = if agent == subagents::AGENT_CLAUDE {
                        gateway::mcp::McpAgent::ClaudeCode
                    } else {
                        gateway::mcp::McpAgent::Codex
                    };
                    gateway::mcp::mark_bundle_entry(db, &mcp_name, target, &name).await?;
                }
                p.servers.push(ServerPart {
                    name: mcp_name,
                    agents: p.agents.clone(),
                });
            }
        }
        sync_parts(db, &mut p, &mut warnings).await?;
        if let Some(i) = old_idx {
            plugins.remove(i);
        }
        plugins.push(p);
        let _ = staged_plugin;
    }
    save(db, &plugins).await?;
    drop(tmp);
    Ok(PluginViewList {
        plugins: plugins.iter().map(view).collect(),
        warnings,
    })
}

fn skills_dir(agent: &str) -> Result<PathBuf, String> {
    let d = dirs()?;
    Ok(if agent == subagents::AGENT_CODEX {
        d.codex.join("skills")
    } else {
        d.claude.join("skills")
    })
}

type HookGroup = (String, Vec<(Option<String>, String)>);

fn unsupported_parts(root: &Path) -> Result<Vec<String>, String> {
    let mut left = Vec::new();
    let manifest_path = root.join(".claude-plugin/plugin.json");
    let manifest = if manifest_path.exists() {
        read_json(&manifest_path)?
    } else {
        json!({})
    };
    if root.join("commands").is_dir() || manifest.get("commands").is_some() {
        left.push("commands: slash commands are not carried".into());
    }
    if manifest.get("lspServers").is_some() || manifest.get("lsp").is_some() {
        left.push("lsp: LSP servers are not carried".into());
    }
    // Only the default folders are read; parts declared inline / at custom paths in plugin.json are not.
    for key in ["agents", "hooks", "mcpServers", "outputStyles"] {
        if manifest.get(key).is_some() {
            left.push(format!("plugin.json {key}: declared parts are not carried"));
        }
    }
    let hooks_path = root.join("hooks/hooks.json");
    if hooks_path.exists() {
        let hooks = read_json(&hooks_path)?;
        for (event, groups) in hooks
            .get("hooks")
            .and_then(Value::as_object)
            .into_iter()
            .flatten()
        {
            for hook in groups.as_array().into_iter().flatten().flat_map(|g| {
                g.get("hooks")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
            }) {
                if hook
                    .get("type")
                    .and_then(Value::as_str)
                    .unwrap_or("command")
                    != "command"
                {
                    left.push(format!("hook {event}: only command hooks are carried"));
                }
            }
        }
    }
    Ok(left)
}

/// `.mcp.json` is `{"mcpServers": {...}}` or the server map itself; non-object entries are skipped.
fn mcp_servers(v: &Value) -> Result<Vec<(&String, &Value)>, String> {
    let map = v
        .get("mcpServers")
        .and_then(Value::as_object)
        .or_else(|| v.as_object())
        .ok_or(".mcp.json is not an object")?;
    Ok(map.iter().filter(|(_, e)| e.is_object()).collect())
}

fn read_hooks(root: &Path) -> Result<Vec<HookGroup>, String> {
    let path = root.join("hooks/hooks.json");
    if !path.exists() {
        return Ok(Vec::new());
    }
    let value = read_json(&path)?;
    let mut result = Vec::new();
    for (event, groups) in value
        .get("hooks")
        .and_then(Value::as_object)
        .into_iter()
        .flatten()
    {
        let mut rows = Vec::new();
        for group in groups.as_array().into_iter().flatten() {
            let matcher = group
                .get("matcher")
                .and_then(Value::as_str)
                .map(str::to_string);
            for hook in group
                .get("hooks")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
            {
                if hook
                    .get("type")
                    .and_then(Value::as_str)
                    .unwrap_or("command")
                    != "command"
                {
                    continue;
                }
                if let Some(command) = hook.get("command").and_then(Value::as_str) {
                    rows.push((matcher.clone(), command.to_string()));
                }
            }
        }
        result.push((event.clone(), rows));
    }
    Ok(result)
}

fn sync_hooks_value(
    cfg: &mut Value,
    plugin: &mut Plugin,
    agent: &str,
    enabled: bool,
    warnings: &mut Vec<String>,
) -> Result<(), String> {
    for hook in &mut plugin.hooks {
        let existed_before = hook.preexisting.iter().any(|a| a == agent);
        if enabled {
            if existed_before {
                continue;
            }
            if let Some(expected) = hook.written.get(agent) {
                let current_hash = cfg
                    .get("hooks")
                    .and_then(|v| v.get(&hook.event))
                    .and_then(Value::as_array)
                    .and_then(|groups| {
                        groups.iter().find(|group| {
                            group
                                .get("hooks")
                                .and_then(Value::as_array)
                                .is_some_and(|items| {
                                    items.len() == 1
                                        && items[0].get("command").and_then(Value::as_str)
                                            == Some(&hook.command)
                                })
                        })
                    })
                    .and_then(|group| serde_json::to_vec(group).ok())
                    .map(|bytes| sha(&bytes));
                if current_hash.as_deref() == Some(expected.as_str()) {
                    continue;
                }
                hook.preexisting.push(agent.to_string());
                hook.written.remove(agent);
                warnings.push(format!("hook {}: modified, kept", hook.command));
                continue;
            }
            if context_compress::has_hook(cfg, &hook.event, &hook.command) {
                hook.preexisting.push(agent.to_string());
                continue;
            }
            context_compress::add_hook(cfg, &hook.event, hook.matcher.as_deref(), &hook.command)?;
            if let Some(group) = cfg
                .get("hooks")
                .and_then(|v| v.get(&hook.event))
                .and_then(Value::as_array)
                .and_then(|groups| {
                    groups.iter().find(|group| {
                        group
                            .get("hooks")
                            .and_then(Value::as_array)
                            .is_some_and(|items| {
                                items.len() == 1
                                    && items[0].get("command").and_then(Value::as_str)
                                        == Some(&hook.command)
                            })
                    })
                })
            {
                let bytes = serde_json::to_vec(group).map_err(|e| e.to_string())?;
                hook.written.insert(agent.to_string(), sha(&bytes));
            }
        } else if !existed_before && let Some(expected) = hook.written.get(agent).cloned() {
            let current = cfg
                .get("hooks")
                .and_then(|v| v.get(&hook.event))
                .and_then(Value::as_array)
                .and_then(|groups| {
                    groups.iter().position(|group| {
                        group
                            .get("hooks")
                            .and_then(Value::as_array)
                            .is_some_and(|items| {
                                items.len() == 1
                                    && items[0].get("command").and_then(Value::as_str)
                                        == Some(&hook.command)
                                    && serde_json::to_vec(group)
                                        .ok()
                                        .is_some_and(|bytes| sha(&bytes) == expected)
                            })
                    })
                });
            if let Some(index) = current {
                if let Some(groups) = cfg
                    .get_mut("hooks")
                    .and_then(|v| v.get_mut(&hook.event))
                    .and_then(Value::as_array_mut)
                {
                    groups.remove(index);
                    if groups.is_empty() {
                        cfg["hooks"].as_object_mut().unwrap().remove(&hook.event);
                    }
                }
                hook.written.remove(agent);
            } else {
                warnings.push(format!("hook {}: modified, kept", hook.command));
            }
        }
    }
    Ok(())
}

async fn sync_hooks(
    db: &aidog_db::Db,
    p: &mut Plugin,
    warnings: &mut Vec<String>,
) -> Result<(), String> {
    if p.hooks.is_empty() {
        return Ok(());
    }
    let on = |agent: &str| !p.off && p.agents.iter().any(|a| a == agent);
    let (on_claude, on_codex) = (on(subagents::AGENT_CLAUDE), on(subagents::AGENT_CODEX));
    let mut claude = context_compress::load_claude_cfg_pub(db).await;
    let claude_before = claude.clone();
    sync_hooks_value(&mut claude, p, subagents::AGENT_CLAUDE, on_claude, warnings)?;
    if claude != claude_before {
        context_compress::store_claude_cfg_pub(db, claude).await?;
    }
    // Codex file untouched unless this bundle targets Codex or still owns a hook there.
    if !on_codex
        && !p
            .hooks
            .iter()
            .any(|h| h.written.contains_key(subagents::AGENT_CODEX))
    {
        return Ok(());
    }
    let path = dirs()?.codex.join("hooks.json");
    let mut codex = match std::fs::read_to_string(&path) {
        Ok(s) => serde_json::from_str(&s).map_err(|e| format!("parse {}: {e}", path.display()))?,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => json!({}),
        Err(e) => return Err(e.to_string()),
    };
    let codex_before = codex.clone();
    sync_hooks_value(&mut codex, p, subagents::AGENT_CODEX, on_codex, warnings)?;
    if codex != codex_before {
        std::fs::create_dir_all(path.parent().unwrap()).map_err(|e| e.to_string())?;
        std::fs::write(
            &path,
            serde_json::to_vec_pretty(&codex).map_err(|e| e.to_string())?,
        )
        .map_err(|e| format!("write {}: {e}", path.display()))?;
    }
    Ok(())
}

async fn sync_parts(
    db: &aidog_db::Db,
    p: &mut Plugin,
    warnings: &mut Vec<String>,
) -> Result<(), String> {
    let on = !p.off;
    let mut subs = subagents::load_list(db).await;
    for s in subs
        .iter_mut()
        .filter(|s| s.bundle.as_deref() == Some(&p.name))
    {
        s.agents = if on { p.agents.clone() } else { Vec::new() };
        for e in subagents::sync_entry(s, &dirs()?) {
            warnings.push(format!("subagent {}: {e}", s.name));
        }
    }
    subagents::store_list(db, &subs).await?;
    for skill in &mut p.skills {
        for agent in AGENTS {
            let target = skills_dir(agent)?.join(&skill.name);
            let current = if target.exists() {
                tree_hash(&target).ok()
            } else {
                None
            };
            let expected = skill.written.get(agent).cloned();
            if on && p.agents.contains(&agent.to_string()) {
                if current.is_some() {
                    if current != expected {
                        warnings.push(format!("skill {} ({agent}): modified, kept", skill.name));
                    }
                    continue;
                }
                let src = plugin_dir(&p.name)?.join("skills").join(&skill.source);
                copy_tree(&src, &target)?;
                let md = target.join("SKILL.md");
                let text = std::fs::read_to_string(&md).map_err(|e| e.to_string())?;
                std::fs::write(
                    &md,
                    resolve(&text, &plugin_dir(&p.name)?, &plugin_data(&p.name)?),
                )
                .map_err(|e| e.to_string())?;
                skill.written.insert(agent.into(), tree_hash(&target)?);
            } else if let Some(expected) = expected {
                if current.as_deref() == Some(&expected) {
                    remove_owned_tree(&target)?;
                    skill.written.remove(agent);
                } else if current.is_some() {
                    warnings.push(format!("skill {} ({agent}): modified, kept", skill.name));
                }
            }
        }
    }
    sync_hooks(db, p, warnings).await?;
    for s in &p.servers {
        for agent in AGENTS {
            let a = if agent == subagents::AGENT_CLAUDE {
                gateway::mcp::McpAgent::ClaudeCode
            } else {
                gateway::mcp::McpAgent::Codex
            };
            if let Err(e) = gateway::mcp::set_agent_enabled(
                db,
                &s.name,
                a,
                on && p.agents.contains(&agent.to_string()),
            )
            .await
            {
                warnings.push(format!("mcp {} ({agent}): {e}", s.name));
            }
        }
    }
    Ok(())
}

async fn remove_parts_db(db: &aidog_db::Db, p: &Plugin) -> Result<(), String> {
    let mut subs = subagents::load_list(db).await;
    subs.retain(|s| s.bundle.as_deref() != Some(&p.name));
    subagents::store_list(db, &subs).await?;
    for s in &p.servers {
        gateway::mcp::delete_server(db, &s.name).await?;
    }
    Ok(())
}

async fn list_impl(db: &aidog_db::Db) -> PluginViewList {
    PluginViewList {
        plugins: load(db).await.iter().map(view).collect(),
        warnings: Vec::new(),
    }
}

crate::tauri_command! {
    pub async fn plugins_bundle_list() -> Result<PluginViewList, String> {
        Ok(list_impl(aidog_ctx::db()).await)
    }
}

crate::tauri_command! {
    pub async fn plugins_bundle_probe(source: String) -> Result<Vec<PluginCandidate>, String> {
        let (root, _tmp) = stage(source.trim()).await?;
        candidates(&root, &load(aidog_ctx::db()).await)
    }
}

crate::tauri_command! {
    pub async fn plugins_bundle_install(source: String, agents: Vec<String>, selected: Option<Vec<String>>) -> Result<PluginViewList, String> {
        install_impl(aidog_ctx::db(), source, agents, selected).await
    }
}

crate::tauri_command! {
    pub async fn plugins_bundle_set(name: String, on: bool) -> Result<PluginViewList, String> {
        let db = aidog_ctx::db();
        let mut rows = load(db).await;
        let Some(p) = rows.iter_mut().find(|p| p.name == name) else { return Err(format!("no plugin called {name}")); };
        p.off = !on;
        let mut warnings = Vec::new();
        sync_parts(db,p,&mut warnings).await?;
        save(db,&rows).await?;
        Ok(PluginViewList{plugins:rows.iter().map(view).collect(),warnings})
    }
}

crate::tauri_command! {
    pub async fn plugins_bundle_set_agents(name: String, agents: Vec<String>) -> Result<PluginViewList, String> {
        let db=aidog_ctx::db();let mut rows=load(db).await;
        let Some(p)=rows.iter_mut().find(|p|p.name==name) else{return Err(format!("no plugin called {name}"));};
        p.agents=agents.into_iter().filter(|a|AGENTS.contains(&a.as_str())).collect();
        let mut warnings=Vec::new();sync_parts(db,p,&mut warnings).await?;save(db,&rows).await?;
        Ok(PluginViewList{plugins:rows.iter().map(view).collect(),warnings})
    }
}

crate::tauri_command! {
    pub async fn plugins_bundle_remove(name: String) -> Result<PluginViewList, String> {
        plugins_remove_impl(aidog_ctx::db(), &name).await
    }
}

async fn plugins_remove_impl(db: &aidog_db::Db, name: &str) -> Result<PluginViewList, String> {
    let mut rows = load(db).await;
    let Some(mut p) = rows.iter().find(|p| p.name == name).cloned() else {
        return Err(format!("no plugin called {name}"));
    };
    let mut warnings = Vec::new();
    p.off = true;
    sync_parts(db, &mut p, &mut warnings).await?;
    remove_parts_db(db, &p).await?;
    rows.retain(|x| x.name != name);
    save(db, &rows).await?;
    if let Err(e) = remove_library_dir(name) {
        warnings.push(e);
    }
    Ok(PluginViewList {
        plugins: rows.iter().map(view).collect(),
        warnings,
    })
}

crate::tauri_command! {
    pub async fn plugins_bundle_update(name: String) -> Result<PluginViewList, String> {
        let db=aidog_ctx::db();
        let p=load(db).await.into_iter().find(|p|p.name==name).ok_or_else(||format!("no plugin called {name}"))?;
        install_impl(db, p.source.clone(), Vec::new(), p.pick.clone().map(|x| vec![x])).await
    }
}

#[cfg(test)]
#[path = "test_plugin_bundle.rs"]
mod test_plugin_bundle;
