//! Agent Skills 管理子系统。
//!
//! **全 npx 化**：list / enable / disable / update 全部 shell out `npx skills`，
//! 禁手动 fs 扫描 / 删除（复用 Vercel Labs 官方生态，单一事实源）。
//! - 列表：`npx skills list --json [-g]` → 统一一条/skill，`agents[]` 含显示名（"Claude Code"/"Codex"）= 该 agent 已启用。
//! - 启用：从锁文件 `.agents/.skill-lock.json` 读 `skills[<name>].source` → `npx skills add <source> -s <name> -a <slug> [-g] -y`。
//! - 关闭：`npx skills remove -s <name> -a <slug> [-g] -y`。
//! - 更新：`npx skills update [-g] -y`。
//!
//! 锁文件仅作**只读元数据**喂给 npx 命令（取 source），所有变更操作仍是 npx，不违反"全 npx"约束。
//!
//! shell out 模式参考 `gateway/notification.rs`（`std::process::Command`）。
//!
//! Scope 语义：
//! - `Global` → 用户级全局，命令带 `-g`，读 `~/.agents/.skill-lock.json`。
//! - `Project { path }` → 项目级，命令在项目目录内执行（不带 `-g`），读 `<path>/.agents/.skill-lock.json`。
//!
//! Agent 语义：target agent 决定 `-a <slug>` 参数（claude → `claude-code`、codex → `codex`）
//! 与 list json `agents[]` 显示名映射（claude → "Claude Code"、codex → "Codex"）。

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
    /// scope 对应的锁文件基目录。Global → home；Project → 项目根。
    /// 锁文件为 `<base>/.agents/.skill-lock.json`。
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

    /// scope 对应的锁文件路径 `<base>/.agents/.skill-lock.json`。
    fn lock_file(&self) -> Option<PathBuf> {
        Some(self.base_dir()?.join(".agents").join(".skill-lock.json"))
    }
}

/// 目标 agent。决定 `--agent` 参数与本地配置目录名。
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum SkillAgent {
    Claude,
    Codex,
}

impl SkillAgent {
    /// `npx skills ... -a <slug>` 的 agent slug。
    /// claude → `claude-code`（修正旧 "claude"）；codex → `codex`。
    fn cli_slug(self) -> &'static str {
        match self {
            SkillAgent::Claude => "claude-code",
            SkillAgent::Codex => "codex",
        }
    }

    /// `npx skills list --json` 的 `agents[]` 显示名。用于解析某 agent 是否启用。
    fn display_name(self) -> &'static str {
        match self {
            SkillAgent::Claude => "Claude Code",
            SkillAgent::Codex => "Codex",
        }
    }

    /// 目标 agent 全集（UI 仅显示 claude/codex 两个）。
    fn all() -> [SkillAgent; 2] {
        [SkillAgent::Claude, SkillAgent::Codex]
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

/// 已装 skill 描述（`npx skills list --json` 解析产出，一条/skill，不分 agent）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillInfo {
    /// skill 名。
    pub name: String,
    /// 来源（owner/repo），从锁文件 source 读不到时为 null。
    pub source: Option<String>,
    /// 已在哪些目标 agent（claude/codex 子集）启用 —— 从 list json `agents[]` 显示名映射。
    pub enabled_agents: Vec<SkillAgent>,
    /// 所属 scope。
    pub scope: SkillScope,
    /// 规范存储路径（list json `path`），读不到为 null。
    pub installed_path: Option<String>,
    /// 简介（list json 暂无，预留；读不到为 null）。
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

/// 列指定 scope 下已装 skills（统一一条/skill，不分 agent）。
///
/// 走 `npx skills list --json [-g]`，解析 `[{name, path, scope, agents:[...]}]`。
/// `agents[]` 含某 agent 显示名 = 该 agent 已启用 → 映射为 `enabled_agents`（仅 claude/codex）。
/// Project scope 在项目目录内执行（不带 `-g`）。命令失败 / 解析失败 → 返回空 vec（不 panic）。
pub fn list_installed(scope: &SkillScope) -> Vec<SkillInfo> {
    let mut args = vec!["list".to_string(), "--json".to_string()];
    apply_scope(&mut args, scope);
    let res = run_npx_in_scope(&args, scope);
    if !res.success {
        return Vec::new();
    }
    parse_list_json(&res.stdout, scope)
}

/// 解析 `npx skills list --json` 输出为 `Vec<SkillInfo>`。
/// 容错：接受裸数组或 `{ "skills": [...] }`；非法 JSON → 空 vec。
fn parse_list_json(stdout: &str, scope: &SkillScope) -> Vec<SkillInfo> {
    let Ok(raw) = serde_json::from_str::<serde_json::Value>(stdout.trim()) else {
        return Vec::new();
    };
    let arr = raw
        .get("skills")
        .and_then(|v| v.as_array())
        .or_else(|| raw.as_array());
    let Some(items) = arr else {
        return Vec::new();
    };
    let mut out: Vec<SkillInfo> = items
        .iter()
        .filter_map(|item| {
            let name = item.get("name").and_then(|v| v.as_str())?.to_string();
            if name.is_empty() {
                return None;
            }
            let agent_names: Vec<&str> = item
                .get("agents")
                .and_then(|v| v.as_array())
                .map(|a| a.iter().filter_map(|x| x.as_str()).collect())
                .unwrap_or_default();
            // 映射 claude/codex 显示名 → SkillAgent，保持 claude 优先 codex 次序。
            let enabled_agents: Vec<SkillAgent> = SkillAgent::all()
                .into_iter()
                .filter(|a| agent_names.contains(&a.display_name()))
                .collect();
            let installed_path = item
                .get("path")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let description = item
                .get("description")
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())
                .map(|s| s.to_string());
            Some(SkillInfo {
                name,
                source: None,
                enabled_agents,
                scope: scope.clone(),
                installed_path,
                description,
            })
        })
        .collect();
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

/// 从 scope 对应锁文件读 `skills[<name>].source`（enable 需要）。读不到 → None。
fn read_skill_source(name: &str, scope: &SkillScope) -> Option<String> {
    let lock = scope.lock_file()?;
    let content = std::fs::read_to_string(&lock).ok()?;
    let raw: serde_json::Value = serde_json::from_str(&content).ok()?;
    raw.get("skills")?
        .get(name)?
        .get("source")?
        .as_str()
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
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

/// 构造 enable（启用）命令 args：`add <source> -s <name> -a <slug> [-g] -y`。
/// 抽出便于单测断言（不真跑 npx）。
fn enable_args(name: &str, source: &str, agent: SkillAgent, scope: &SkillScope) -> Vec<String> {
    let mut args = vec![
        "add".to_string(),
        source.to_string(),
        "-s".to_string(),
        name.to_string(),
        "-a".to_string(),
        agent.cli_slug().to_string(),
    ];
    apply_scope(&mut args, scope);
    args.push("-y".to_string());
    args
}

/// 构造 disable（关闭）命令 args：`remove -s <name> -a <slug> [-g] -y`。
fn disable_args(name: &str, agent: SkillAgent, scope: &SkillScope) -> Vec<String> {
    let mut args = vec![
        "remove".to_string(),
        "-s".to_string(),
        name.to_string(),
        "-a".to_string(),
        agent.cli_slug().to_string(),
    ];
    apply_scope(&mut args, scope);
    args.push("-y".to_string());
    args
}

/// 为某 agent 启用 skill：从锁文件取 source → `npx skills add <source> -s <name> -a <slug> [-g] -y`。
/// source 缺失 → 明确错误。Project scope 在项目目录内执行（不带 `-g`）。
pub fn enable(name: &str, agent: SkillAgent, scope: &SkillScope) -> SkillsOpResult {
    let name = name.trim();
    if name.is_empty() {
        return SkillsOpResult {
            success: false,
            stdout: String::new(),
            stderr: "skill name is empty".to_string(),
        };
    }
    let Some(source) = read_skill_source(name, scope) else {
        return SkillsOpResult {
            success: false,
            stdout: String::new(),
            stderr: format!("cannot resolve source for skill '{name}' from .skill-lock.json"),
        };
    };
    let args = enable_args(name, &source, agent, scope);
    run_npx_in_scope(&args, scope)
}

/// 为某 agent 关闭 skill：`npx skills remove -s <name> -a <slug> [-g] -y`。
pub fn disable(name: &str, agent: SkillAgent, scope: &SkillScope) -> SkillsOpResult {
    let name = name.trim();
    if name.is_empty() {
        return SkillsOpResult {
            success: false,
            stdout: String::new(),
            stderr: "skill name is empty".to_string(),
        };
    }
    let args = disable_args(name, agent, scope);
    run_npx_in_scope(&args, scope)
}

/// 更新已装 skills：`npx skills update [-g] -y`。
pub fn update(scope: &SkillScope) -> SkillsOpResult {
    let mut args = vec!["update".to_string()];
    apply_scope(&mut args, scope);
    args.push("-y".to_string());
    run_npx_in_scope(&args, scope)
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
    fn agent_slug_and_display() {
        // 关键修正：claude slug 必须 "claude-code"（旧值 "claude" 是错的）。
        assert_eq!(SkillAgent::Claude.cli_slug(), "claude-code");
        assert_eq!(SkillAgent::Codex.cli_slug(), "codex");
        assert_eq!(SkillAgent::Claude.display_name(), "Claude Code");
        assert_eq!(SkillAgent::Codex.display_name(), "Codex");
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
    fn parse_list_json_maps_enabled_agents() {
        let stdout = r#"[
            {"name":"alpha","path":"/p/alpha","scope":"global","agents":["Claude Code","Codex","Zed"]},
            {"name":"beta","path":"/p/beta","scope":"global","agents":["Codex"]},
            {"name":"gamma","path":"/p/gamma","scope":"global","agents":["Gemini CLI"]}
        ]"#;
        let out = parse_list_json(stdout, &SkillScope::Global);
        assert_eq!(out.len(), 3);
        // 排序后 alpha/beta/gamma。
        assert_eq!(out[0].name, "alpha");
        assert_eq!(out[0].enabled_agents, vec![SkillAgent::Claude, SkillAgent::Codex]);
        assert_eq!(out[0].installed_path.as_deref(), Some("/p/alpha"));
        assert_eq!(out[1].enabled_agents, vec![SkillAgent::Codex]);
        // gamma 无 claude/codex → 空。
        assert!(out[2].enabled_agents.is_empty());
    }

    #[test]
    fn parse_list_json_bad_json_is_empty() {
        assert!(parse_list_json("not json", &SkillScope::Global).is_empty());
    }

    #[test]
    fn parse_list_json_wrapped_object() {
        let stdout = r#"{"skills":[{"name":"x","agents":["Claude Code"]}]}"#;
        let out = parse_list_json(stdout, &SkillScope::Global);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].enabled_agents, vec![SkillAgent::Claude]);
    }

    #[test]
    fn enable_args_global_claude() {
        let args = enable_args("foo", "owner/repo", SkillAgent::Claude, &SkillScope::Global);
        assert_eq!(
            args,
            vec!["add", "owner/repo", "-s", "foo", "-a", "claude-code", "-g", "-y"]
        );
    }

    #[test]
    fn enable_args_project_codex_no_g() {
        let args = enable_args(
            "bar",
            "a/b",
            SkillAgent::Codex,
            &SkillScope::Project { path: "/proj".to_string() },
        );
        assert_eq!(args, vec!["add", "a/b", "-s", "bar", "-a", "codex", "-y"]);
        assert!(!args.contains(&"-g".to_string()));
    }

    #[test]
    fn disable_args_global_claude() {
        let args = disable_args("foo", SkillAgent::Claude, &SkillScope::Global);
        assert_eq!(args, vec!["remove", "-s", "foo", "-a", "claude-code", "-g", "-y"]);
    }

    #[test]
    fn disable_args_project_no_g() {
        let args = disable_args(
            "foo",
            SkillAgent::Codex,
            &SkillScope::Project { path: "/proj".to_string() },
        );
        assert!(!args.contains(&"-g".to_string()));
        assert_eq!(args, vec!["remove", "-s", "foo", "-a", "codex", "-y"]);
    }

    #[test]
    fn enable_missing_source_fails() {
        // 不存在的 scope 锁文件 → source 解析失败 → 明确错误，不真跑 npx。
        let s = SkillScope::Project {
            path: "/nonexistent/path/xyz123".to_string(),
        };
        let r = enable("whatever", SkillAgent::Claude, &s);
        assert!(!r.success);
        assert!(r.stderr.contains("cannot resolve source"));
    }

    #[test]
    fn disable_empty_name_fails() {
        let r = disable("  ", SkillAgent::Claude, &SkillScope::Global);
        assert!(!r.success);
    }

    #[test]
    fn lock_file_path() {
        let p = SkillScope::Project {
            path: "/proj".to_string(),
        }
        .lock_file();
        assert_eq!(p, Some(PathBuf::from("/proj/.agents/.skill-lock.json")));
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
}
