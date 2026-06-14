//! Agent Skills 管理子系统。
//!
//! 混合方案：
//! - 读操作（环境探测 / 扫已装 / 浏览 catalog / 搜索）走 Rust 原生（HTTP + 文件系统）。
//! - 写操作（安装 / 更新 / 卸载）shell out `npx skills`（复用 Vercel Labs 官方生态）。
//!
//! shell out 模式参考 `gateway/notification.rs`（`std::process::Command`）。
//!
//! Scope 语义：
//! - `Global` → 用户级全局，安装走 `npx skills add -g`，扫 `~/.<agent-dir>/skills`。
//! - `Project { path }` → 项目级，安装不带 `-g`，扫 `<path>/.<agent-dir>/skills`。
//!
//! Agent 语义：target agent 决定 `--agent <a>` 参数与扫描目录名（claude → `.claude`）。

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::process::Command;

/// 安装目标 scope。`Global` = 用户级全局；`Project` = 指定项目目录。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum SkillScope {
    /// 用户级全局（`-g`）。
    Global,
    /// 项目级，path 为项目根目录绝对路径。
    Project { path: String },
}

impl SkillScope {
    /// scope 对应的扫描基目录。Global → home；Project → 项目根。
    fn base_dir(&self) -> Option<PathBuf> {
        match self {
            SkillScope::Global => dirs_home(),
            SkillScope::Project { path } => {
                let p = path.trim();
                if p.is_empty() {
                    None
                } else {
                    Some(PathBuf::from(p))
                }
            }
        }
    }
}

/// 目标 agent。决定 `--agent` 参数与本地配置目录名。
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum SkillAgent {
    Claude,
    Codex,
    Cursor,
}

impl SkillAgent {
    /// `npx skills --agent <a>` 的 agent 名。
    fn cli_name(self) -> &'static str {
        match self {
            SkillAgent::Claude => "claude",
            SkillAgent::Codex => "codex",
            SkillAgent::Cursor => "cursor",
        }
    }

    /// agent 本地配置目录名（含点）。skills 装到 `<base>/<dir>/skills`。
    fn config_dir(self) -> &'static str {
        match self {
            SkillAgent::Claude => ".claude",
            SkillAgent::Codex => ".codex",
            SkillAgent::Cursor => ".cursor",
        }
    }
}

/// 环境探测结果。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillsEnv {
    /// `npx` 是否可用（写操作前置）。
    pub npx_available: bool,
    /// `node --version` 输出（如 "v20.11.0"），不可用为 null。
    pub node_version: Option<String>,
}

/// 已装 skill 描述（原生扫描产出）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillInfo {
    /// skill 名（目录名）。
    pub name: String,
    /// 来源（owner/repo），从 SKILL 元数据读不到时为 null。
    pub source: Option<String>,
    /// 所属 agent。
    pub agent: SkillAgent,
    /// 所属 scope。
    pub scope: SkillScope,
    /// 已装目录绝对路径。
    pub installed_path: String,
    /// 简介（从 SKILL.md frontmatter description 读，读不到为 null）。
    pub description: Option<String>,
}

/// catalog 条目（可装 skill）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CatalogEntry {
    /// 安装标识（owner/repo 或 skill slug）。
    pub id: String,
    /// 展示名。
    pub name: String,
    /// 简介。
    pub description: Option<String>,
    /// 来源仓库 URL。
    pub repo_url: Option<String>,
}

/// 写操作（install/update/remove）结果。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillsOpResult {
    /// 退出码为 0 视为成功。
    pub success: bool,
    /// 合并的 stdout。
    pub stdout: String,
    /// 合并的 stderr。
    pub stderr: String,
}

/// home 目录。封装便于 scope 复用。
fn dirs_home() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
}

/// 探测 npx / node 可用性。任一探测失败均不 panic，对应字段降级。
pub fn check_env() -> SkillsEnv {
    let node_version = Command::new("node")
        .arg("--version")
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .filter(|s| !s.is_empty());

    // npx 仅探测可执行性（--version 在所有平台稳定）。
    let npx_available = Command::new("npx")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);

    SkillsEnv {
        npx_available,
        node_version,
    }
}

/// 原生扫描指定 scope + agent 下的已装 skills。
///
/// 扫描目录 = `<base>/<agent-config-dir>/skills/*`，每个子目录视为一个 skill。
/// base 无法解析（如 Project 路径空）或目录不存在 → 返回空 vec（不报错）。
pub fn scan_installed(scope: &SkillScope, agent: SkillAgent) -> Vec<SkillInfo> {
    let Some(base) = scope.base_dir() else {
        return Vec::new();
    };
    let skills_dir = base.join(agent.config_dir()).join("skills");
    let Ok(entries) = std::fs::read_dir(&skills_dir) else {
        return Vec::new();
    };

    let mut out = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        // 跳过隐藏 / 元数据目录。
        if name.starts_with('.') {
            continue;
        }
        let description = read_skill_description(&path);
        out.push(SkillInfo {
            name: name.to_string(),
            source: None,
            agent,
            scope: scope.clone(),
            installed_path: path.to_string_lossy().to_string(),
            description,
        });
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

/// 从 skill 目录的 `SKILL.md` frontmatter 读 `description`（best-effort）。
fn read_skill_description(dir: &std::path::Path) -> Option<String> {
    let md = dir.join("SKILL.md");
    let content = std::fs::read_to_string(&md).ok()?;
    // 仅在 frontmatter（首个 `---` 围栏）内找 `description:`。
    let mut in_front = false;
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed == "---" {
            if in_front {
                break;
            }
            in_front = true;
            continue;
        }
        if in_front {
            if let Some(rest) = trimmed.strip_prefix("description:") {
                let val = rest.trim().trim_matches('"').trim_matches('\'').to_string();
                if !val.is_empty() {
                    return Some(val);
                }
            }
        }
    }
    None
}

/// catalog 抓取地址（skills.sh 的 JSON 索引）。
const CATALOG_URL: &str = "https://skills.sh/api/skills";

/// 浏览 catalog：优先 HTTP 抓 skills.sh，失败回退 `npx skills find`（空 kw 列全部）。
pub async fn browse_catalog() -> Vec<CatalogEntry> {
    if let Some(list) = fetch_catalog_http().await {
        if !list.is_empty() {
            return list;
        }
    }
    // 回退：npx find 无关键词。
    npx_find("")
}

/// 搜索 catalog：HTTP 抓后本地按 kw 过滤；HTTP 空则走 `npx skills find <kw>`。
pub async fn search(kw: &str) -> Vec<CatalogEntry> {
    let kw_lower = kw.trim().to_lowercase();
    if let Some(list) = fetch_catalog_http().await {
        if !list.is_empty() {
            if kw_lower.is_empty() {
                return list;
            }
            return list
                .into_iter()
                .filter(|e| {
                    e.id.to_lowercase().contains(&kw_lower)
                        || e.name.to_lowercase().contains(&kw_lower)
                        || e
                            .description
                            .as_deref()
                            .map(|d| d.to_lowercase().contains(&kw_lower))
                            .unwrap_or(false)
                })
                .collect();
        }
    }
    npx_find(kw)
}

/// HTTP 抓 skills.sh catalog JSON。失败（网络 / 解析）返回 None。
async fn fetch_catalog_http() -> Option<Vec<CatalogEntry>> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .ok()?;
    let resp = client.get(CATALOG_URL).send().await.ok()?;
    if !resp.status().is_success() {
        return None;
    }
    let raw: serde_json::Value = resp.json().await.ok()?;
    Some(parse_catalog_json(&raw))
}

/// 解析 skills.sh 返回的 JSON 到 CatalogEntry 列表。
///
/// 容错：接受 `{ "skills": [...] }` 或裸数组；每项尽量从常见字段名提取。
fn parse_catalog_json(raw: &serde_json::Value) -> Vec<CatalogEntry> {
    let arr = raw
        .get("skills")
        .and_then(|v| v.as_array())
        .or_else(|| raw.as_array());
    let Some(items) = arr else {
        return Vec::new();
    };
    items
        .iter()
        .filter_map(|item| {
            let id = item
                .get("id")
                .or_else(|| item.get("slug"))
                .or_else(|| item.get("repo"))
                .or_else(|| item.get("fullName"))
                .and_then(|v| v.as_str())?
                .to_string();
            if id.is_empty() {
                return None;
            }
            let name = item
                .get("name")
                .or_else(|| item.get("title"))
                .and_then(|v| v.as_str())
                .unwrap_or(&id)
                .to_string();
            let description = item
                .get("description")
                .or_else(|| item.get("summary"))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let repo_url = item
                .get("repoUrl")
                .or_else(|| item.get("url"))
                .or_else(|| item.get("html_url"))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            Some(CatalogEntry {
                id,
                name,
                description,
                repo_url,
            })
        })
        .collect()
}

/// `npx skills find <kw>` 回退：解析 stdout 每行为一个条目（best-effort）。
fn npx_find(kw: &str) -> Vec<CatalogEntry> {
    let mut args = vec!["--yes", "skills", "find"];
    let kw = kw.trim();
    if !kw.is_empty() {
        args.push(kw);
    }
    let output = match Command::new("npx").args(&args).output() {
        Ok(o) => o,
        Err(_) => return Vec::new(),
    };
    let stdout = String::from_utf8_lossy(&output.stdout);
    stdout
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
        // 过滤明显的 npm 噪声行。
        .filter(|l| !l.starts_with("npm") && !l.starts_with('>'))
        .map(|l| {
            // 取首 token 作 id（owner/repo），整行作 name。
            let id = l.split_whitespace().next().unwrap_or(l).to_string();
            CatalogEntry {
                id,
                name: l.to_string(),
                description: None,
                repo_url: None,
            }
        })
        .collect()
}

/// 封装 `npx skills <args...>`，捕获 stdout/stderr/退出码。
fn run_npx(extra_args: &[String]) -> SkillsOpResult {
    let mut args: Vec<String> = vec!["--yes".to_string(), "skills".to_string()];
    args.extend(extra_args.iter().cloned());
    match Command::new("npx").args(&args).output() {
        Ok(o) => SkillsOpResult {
            success: o.status.success(),
            stdout: String::from_utf8_lossy(&o.stdout).to_string(),
            stderr: String::from_utf8_lossy(&o.stderr).to_string(),
        },
        Err(e) => SkillsOpResult {
            success: false,
            stdout: String::new(),
            stderr: format!("failed to spawn npx: {e}"),
        },
    }
}

/// 安装 skill：`npx skills add <id> --agent <a> [-g]`。
/// Project scope 在该项目目录下执行（cwd），不带 `-g`。
pub fn install(id: &str, agent: SkillAgent, scope: &SkillScope) -> SkillsOpResult {
    let id = id.trim();
    if id.is_empty() {
        return SkillsOpResult {
            success: false,
            stdout: String::new(),
            stderr: "skill id is empty".to_string(),
        };
    }
    let mut args = vec![
        "add".to_string(),
        id.to_string(),
        "--agent".to_string(),
        agent.cli_name().to_string(),
    ];
    apply_scope(&mut args, scope);
    run_npx_in_scope(&args, scope)
}

/// 更新已装 skills：`npx skills update [--agent <a>] [-g]`。
pub fn update(agent: SkillAgent, scope: &SkillScope) -> SkillsOpResult {
    let mut args = vec![
        "update".to_string(),
        "--agent".to_string(),
        agent.cli_name().to_string(),
    ];
    apply_scope(&mut args, scope);
    run_npx_in_scope(&args, scope)
}

/// 卸载 skill：原生删除已装目录（npx skills 无稳定 remove 子命令）。
/// 删除前校验目录在预期 skills 根下，防越权删除。
pub fn remove(name: &str, agent: SkillAgent, scope: &SkillScope) -> SkillsOpResult {
    let name = name.trim();
    if name.is_empty() {
        return SkillsOpResult {
            success: false,
            stdout: String::new(),
            stderr: "skill name is empty".to_string(),
        };
    }
    // Path-traversal 防护：name 必须是单个目录段，禁路径分隔符 / `..` / 绝对路径。
    // `PathBuf::starts_with` 是纯词法比较，对 `..` 不归一化，故下方 starts_with 不足以挡越权，
    // 必须在此先拒绝任何含分隔符或父目录引用的 name。
    if name == ".."
        || name.contains('/')
        || name.contains('\\')
        || std::path::Path::new(name).components().count() != 1
    {
        return SkillsOpResult {
            success: false,
            stdout: String::new(),
            stderr: format!("invalid skill name: {name}"),
        };
    }
    let Some(base) = scope.base_dir() else {
        return SkillsOpResult {
            success: false,
            stdout: String::new(),
            stderr: "cannot resolve scope base directory".to_string(),
        };
    };
    let skills_root = base.join(agent.config_dir()).join("skills");
    let target = skills_root.join(name);
    // 越权保护：target 必须确实在 skills_root 下且为目录。
    if !target.starts_with(&skills_root) || !target.is_dir() {
        return SkillsOpResult {
            success: false,
            stdout: String::new(),
            stderr: format!("skill directory not found: {}", target.display()),
        };
    }
    match std::fs::remove_dir_all(&target) {
        Ok(()) => SkillsOpResult {
            success: true,
            stdout: format!("removed {}", target.display()),
            stderr: String::new(),
        },
        Err(e) => SkillsOpResult {
            success: false,
            stdout: String::new(),
            stderr: format!("remove failed: {e}"),
        },
    }
}

/// 按 scope 追加 `-g`（仅 Global）。
fn apply_scope(args: &mut Vec<String>, scope: &SkillScope) {
    if matches!(scope, SkillScope::Global) {
        args.push("-g".to_string());
    }
}

/// 在 scope 对应的 cwd 执行 npx：Project → 项目目录；Global → 默认 cwd。
fn run_npx_in_scope(extra_args: &[String], scope: &SkillScope) -> SkillsOpResult {
    if let SkillScope::Project { path } = scope {
        let p = path.trim();
        if p.is_empty() {
            return SkillsOpResult {
                success: false,
                stdout: String::new(),
                stderr: "project path is empty".to_string(),
            };
        }
        let mut full: Vec<String> = vec!["--yes".to_string(), "skills".to_string()];
        full.extend(extra_args.iter().cloned());
        return match Command::new("npx").args(&full).current_dir(p).output() {
            Ok(o) => SkillsOpResult {
                success: o.status.success(),
                stdout: String::from_utf8_lossy(&o.stdout).to_string(),
                stderr: String::from_utf8_lossy(&o.stderr).to_string(),
            },
            Err(e) => SkillsOpResult {
                success: false,
                stdout: String::new(),
                stderr: format!("failed to spawn npx: {e}"),
            },
        };
    }
    run_npx(extra_args)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scope_base_dir_project_empty_is_none() {
        let s = SkillScope::Project {
            path: "   ".to_string(),
        };
        assert!(s.base_dir().is_none());
    }

    #[test]
    fn scope_base_dir_project_path() {
        let s = SkillScope::Project {
            path: "/tmp/proj".to_string(),
        };
        assert_eq!(s.base_dir(), Some(PathBuf::from("/tmp/proj")));
    }

    #[test]
    fn agent_cli_and_dir() {
        assert_eq!(SkillAgent::Claude.cli_name(), "claude");
        assert_eq!(SkillAgent::Claude.config_dir(), ".claude");
        assert_eq!(SkillAgent::Codex.config_dir(), ".codex");
        assert_eq!(SkillAgent::Cursor.cli_name(), "cursor");
    }

    #[test]
    fn apply_scope_global_adds_g() {
        let mut args = vec!["add".to_string(), "owner/repo".to_string()];
        apply_scope(&mut args, &SkillScope::Global);
        assert!(args.contains(&"-g".to_string()));
    }

    #[test]
    fn apply_scope_project_no_g() {
        let mut args = vec!["add".to_string()];
        apply_scope(
            &mut args,
            &SkillScope::Project {
                path: "/tmp".to_string(),
            },
        );
        assert!(!args.contains(&"-g".to_string()));
    }

    #[test]
    fn scan_installed_missing_dir_is_empty() {
        let s = SkillScope::Project {
            path: "/nonexistent/path/xyz123".to_string(),
        };
        assert!(scan_installed(&s, SkillAgent::Claude).is_empty());
    }

    #[test]
    fn parse_catalog_wrapped_object() {
        let raw = serde_json::json!({
            "skills": [
                { "id": "vercel-labs/foo", "name": "Foo", "description": "a foo skill" },
                { "slug": "bar/baz", "title": "Baz" }
            ]
        });
        let out = parse_catalog_json(&raw);
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].id, "vercel-labs/foo");
        assert_eq!(out[0].name, "Foo");
        assert_eq!(out[1].id, "bar/baz");
        assert_eq!(out[1].name, "Baz");
    }

    #[test]
    fn parse_catalog_bare_array() {
        let raw = serde_json::json!([
            { "repo": "a/b" }
        ]);
        let out = parse_catalog_json(&raw);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].id, "a/b");
        // name 回退到 id。
        assert_eq!(out[0].name, "a/b");
    }

    #[test]
    fn remove_empty_name_fails() {
        let r = remove("  ", SkillAgent::Claude, &SkillScope::Global);
        assert!(!r.success);
    }

    #[test]
    fn remove_rejects_path_traversal() {
        for bad in ["..", "../evil", "../../etc", "foo/../../bar", "a/b", "/etc/passwd"] {
            let r = remove(bad, SkillAgent::Claude, &SkillScope::Global);
            assert!(!r.success, "traversal name should be rejected: {bad}");
            assert!(
                r.stderr.contains("invalid skill name"),
                "expected invalid-name error for {bad}, got: {}",
                r.stderr
            );
        }
    }
}
