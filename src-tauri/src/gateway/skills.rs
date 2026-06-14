//! Agent Skills 管理子系统。
//!
//! **全 npx 化**：list / enable / disable / update 全部 shell out `npx skills`，
//! 禁手动 fs 扫描 / 删除（复用 Vercel Labs 官方生态，单一事实源）。
//! - 列表：`npx skills list --json [-g]` → 统一一条/skill，`agents[]` 含显示名（"Claude Code"/"Codex"）= 该 agent 已启用。
//! - 启用：用 skill 本地 path（list json `path`）作 add package → `npx skills add <path> -a <slug> [-g] -y`（对所有 skill 通用，不依赖锁文件 source）。
//! - 关闭：`npx skills remove -s <name> -a <slug> [-g] -y`。
//! - 更新：`npx skills update [-g] -y`。
//!
//! 所有变更操作均 shell out npx，不违反"全 npx"约束。
//!
//! shell out 模式参考 `gateway/notification.rs`（`std::process::Command`）。
//!
//! Scope 语义：
//! - `Global` → 用户级全局，命令带 `-g`。
//! - `Project { path }` → 项目级，命令在项目目录内执行（不带 `-g`）。
//!
//! Agent 语义：target agent 决定 `-a <slug>` 参数（claude → `claude-code`、codex → `codex`）
//! 与 list json `agents[]` 显示名映射（claude → "Claude Code"、codex → "Codex"）。

use super::models::ProxyClientSettings;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::{Mutex, OnceLock};

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
    /// 缓存键：`Global` → `"global"`；`Project{path}` → `"project:<path>"`。
    /// 不同项目 path 不串；trim 后比较（与命令 cwd 一致）。
    fn cache_key(&self) -> String {
        match self {
            SkillScope::Global => "global".to_string(),
            SkillScope::Project { path } => format!("project:{}", path.trim()),
        }
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
    /// 已在哪些目标 agent（claude/codex 子集）启用 —— 从 list json `agents[]` 显示名映射。
    pub enabled_agents: Vec<SkillAgent>,
    /// 所属 scope。
    pub scope: SkillScope,
    /// 规范存储路径（list json `path`），读不到为 null。
    pub installed_path: Option<String>,
    /// 简介（list json 暂无，预留；读不到为 null）。
    pub description: Option<String>,
    /// 来源 owner/repo（锁文件 `source` 字段）。第三方/手动 symlink skill（锁文件无条目）→ None。
    pub source: Option<String>,
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

/// 进程内 env 探测缓存：node/npx 可用性一会话不变，仅首次真探测。
static ENV_CACHE: OnceLock<SkillsEnv> = OnceLock::new();

/// 探测 npx / node 可用性（进程内缓存，仅首次 spawn 子进程）。
/// 后续调用直接返回缓存值，开页 0 子进程。
pub fn check_env() -> SkillsEnv {
    ENV_CACHE.get_or_init(probe_env).clone()
}

/// 真探测 npx / node 可用性（spawn 子进程）。任一探测失败均不 panic，对应字段降级。
fn probe_env() -> SkillsEnv {
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

/// 列指定 scope 下已装 skills（统一一条/skill，不分 agent）。**直跑 npx**（无缓存）。
///
/// 走 `npx skills list --json [-g]`，解析 `[{name, path, scope, agents:[...]}]`。
/// `agents[]` 含某 agent 显示名 = 该 agent 已启用 → 映射为 `enabled_agents`（仅 claude/codex）。
/// Project scope 在项目目录内执行（不带 `-g`）。命令失败 / 解析失败 → 返回空 vec（不 panic）。
///
/// 注：SWR 链路用 [`list_cached`]（即时缓存）+ [`list_refresh`]（强制刷新）；
/// 内部聚合算子（`align_agents` / `enable_all`）仍走本函数取实时态。
pub fn list_installed(scope: &SkillScope, proxy_url: Option<&str>) -> Vec<SkillInfo> {
    let mut args = vec!["list".to_string(), "--json".to_string()];
    apply_scope(&mut args, scope);
    let res = run_npx_in_scope(&args, scope, proxy_url);
    if !res.success {
        return Vec::new();
    }
    let mut items = parse_list_json(&res.stdout, scope);
    enrich_with_sources(&mut items, scope);
    items
}

// ─── SWR 缓存（list 提速）─────────────────────────────────────
//
// 双层：进程内 `SKILLS_CACHE`（首访从磁盘 lazy load）+ 磁盘 `~/.aidog/skills-cache.json`。
// - `list_cached(scope)` → 立即返回缓存（命中即 0 子进程）；冷启动无缓存 → 空 + stale=true。
// - `list_refresh(scope)` → 强制跑 npx、更新内存+磁盘、返回 fresh（stale=false）。
// - 写操作后 `invalidate(scope)` 失效对应 scope（内存 + 磁盘），下次 refresh 重填。
// 容错：磁盘损坏 / 缺失 → 当冷启动（空缓存）。原子写（temp + rename）防半文件。

/// list 缓存返回：数据 + 是否为陈旧/冷启动（true = 调用方应触发 refresh）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedSkills {
    /// 缓存的 skill 列表（冷启动为空）。
    pub items: Vec<SkillInfo>,
    /// true = 无缓存命中（冷启动），调用方应显加载态 + 强制 refresh。
    pub stale: bool,
}

/// 单 scope 的磁盘缓存条目。
#[derive(Debug, Clone, Serialize, Deserialize)]
struct ScopeCacheEntry {
    /// 写入时刻（毫秒 Unix 戳，仅诊断用，不做 TTL 过期）。
    cached_at: i64,
    items: Vec<SkillInfo>,
}

/// 磁盘缓存根结构（`~/.aidog/skills-cache.json`）。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct SkillsCacheFile {
    /// per-scope（key = `SkillScope::cache_key`）。
    #[serde(default)]
    scopes: HashMap<String, ScopeCacheEntry>,
}

/// 进程内缓存（首访从磁盘 lazy load，之后内存为准 + 写时同步落盘）。
static SKILLS_CACHE: OnceLock<Mutex<SkillsCacheFile>> = OnceLock::new();

/// 磁盘缓存文件路径：`~/.aidog/skills-cache.json`。
/// home 不可解析 → None（降级为纯内存缓存，不落盘）。
fn cache_file_path() -> Option<std::path::PathBuf> {
    let home = dirs::home_dir()?;
    let dir = home.join(".aidog");
    // best-effort 建目录；失败仍返回路径（写时再失败即降级）。
    let _ = std::fs::create_dir_all(&dir);
    Some(dir.join("skills-cache.json"))
}

/// 从磁盘读缓存文件。缺失 / 损坏 / 解析失败 → 默认空（当冷启动）。
fn load_cache_from_disk() -> SkillsCacheFile {
    let Some(p) = cache_file_path() else {
        return SkillsCacheFile::default();
    };
    let Ok(text) = std::fs::read_to_string(&p) else {
        return SkillsCacheFile::default();
    };
    serde_json::from_str(&text).unwrap_or_default()
}

/// 原子落盘：写临时文件 → rename 覆盖，防并发/中断产生半文件。
/// 任一步失败仅记日志（缓存以内存为准，落盘是优化非必需）。
fn persist_cache_to_disk(cache: &SkillsCacheFile) {
    let Some(p) = cache_file_path() else {
        return;
    };
    let Ok(json) = serde_json::to_string(cache) else {
        return;
    };
    // 同目录临时文件（确保 rename 在同一文件系统，原子生效）。
    let tmp = p.with_extension("json.tmp");
    if std::fs::write(&tmp, json.as_bytes()).is_err() {
        return;
    }
    if let Err(e) = std::fs::rename(&tmp, &p) {
        tracing::warn!(error = %e, "skills cache atomic write failed");
        let _ = std::fs::remove_file(&tmp);
    }
}

/// 取进程内缓存（首访从磁盘 load）。
fn cache_store() -> &'static Mutex<SkillsCacheFile> {
    SKILLS_CACHE.get_or_init(|| Mutex::new(load_cache_from_disk()))
}

/// 立即返回缓存（内存→磁盘，命中即 0 子进程）；无缓存返回空 + stale=true。
///
/// SWR 的 "stale" 半：调用方应立即渲染 `items`，再后台 `list_refresh`。
pub fn list_cached(scope: &SkillScope) -> CachedSkills {
    let key = scope.cache_key();
    let guard = match cache_store().lock() {
        Ok(g) => g,
        Err(poisoned) => poisoned.into_inner(),
    };
    match guard.scopes.get(&key) {
        Some(entry) => {
            // 向后兼容：旧缓存 items 无 source 字段（source-grouping task 前写入）。
            // 命中缓存后 enrich_with_sources 读锁文件补 source（0 npx，cheap）。
            // 旧 None + 锁文件有 → 补；已有 source → 幂等重赋；第三方 symlink → 保持 None。
            let mut items = entry.items.clone();
            enrich_with_sources(&mut items, scope);
            CachedSkills { items, stale: false }
        }
        None => CachedSkills {
            items: Vec::new(),
            stale: true,
        },
    }
}

/// 强制跑 npx 取最新，更新内存+磁盘缓存，返回 fresh（stale=false）。
///
/// SWR 的 "revalidate" 半。npx 失败 → 返回空 fresh（不污染已有缓存？这里仍写空覆盖，
/// 与直跑 `list_installed` 失败语义一致：上游列表真为空 vs 命令失败不可区分，保持简单）。
pub fn list_refresh(scope: &SkillScope, proxy_url: Option<&str>) -> CachedSkills {
    let items = list_installed(scope, proxy_url);
    write_cache(scope, items.clone());
    CachedSkills { items, stale: false }
}

/// 写入某 scope 缓存（内存 + 落盘）。
fn write_cache(scope: &SkillScope, items: Vec<SkillInfo>) {
    let key = scope.cache_key();
    let snapshot = {
        let mut guard = match cache_store().lock() {
            Ok(g) => g,
            Err(poisoned) => poisoned.into_inner(),
        };
        guard.scopes.insert(
            key,
            ScopeCacheEntry {
                cached_at: chrono::Utc::now().timestamp_millis(),
                items,
            },
        );
        guard.clone()
    };
    persist_cache_to_disk(&snapshot);
}

/// 失效某 scope 缓存（内存 + 落盘）。写操作成功后调用，下次 refresh 重填。
pub fn invalidate(scope: &SkillScope) {
    let key = scope.cache_key();
    let snapshot = {
        let mut guard = match cache_store().lock() {
            Ok(g) => g,
            Err(poisoned) => poisoned.into_inner(),
        };
        guard.scopes.remove(&key);
        guard.clone()
    };
    persist_cache_to_disk(&snapshot);
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
                .map(|s| s.to_string())
                .or_else(|| installed_path.as_deref().and_then(read_skill_description));
            Some(SkillInfo {
                name,
                enabled_agents,
                scope: scope.clone(),
                installed_path,
                description,
                source: None,
            })
        })
        .collect();
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

/// 从 SKILL.md 文本 frontmatter 解析 `description:` 单行值。
/// 规则: 首行 `---` 起, 到下一个 `---` 止; 行 `description: <value>`, 去首尾引号 (单/双)。
/// 无 frontmatter / 无 description 行 / 空值 → None。多行折叠 (`>-`) 不支持。
fn parse_skill_description_from_frontmatter(content: &str) -> Option<String> {
    let mut lines = content.lines();
    let first = lines.next()?;
    if first.trim() != "---" {
        return None;
    }
    for line in lines {
        let t = line.trim();
        if t == "---" {
            break;
        }
        if let Some(rest) = t.strip_prefix("description:") {
            let v = rest.trim().trim_matches('"').trim_matches('\'');
            if !v.is_empty() {
                return Some(v.to_string());
            }
        }
    }
    None
}

/// 读 `<skill_path>/SKILL.md` frontmatter 的 description 字段。
/// 文件缺失 / 读失败 → None。
fn read_skill_description(skill_path: &str) -> Option<String> {
    let p = std::path::Path::new(skill_path).join("SKILL.md");
    let content = std::fs::read_to_string(p).ok()?;
    parse_skill_description_from_frontmatter(&content)
}

/// 锁文件路径：global → `~/.agents/.skill-lock.json`；project → `<path>/.agents/.skill-lock.json`。
/// home 不可解析（global）→ None。
fn lock_file_path(scope: &SkillScope) -> Option<std::path::PathBuf> {
    match scope {
        SkillScope::Global => {
            dirs::home_dir().map(|h| h.join(".agents").join(".skill-lock.json"))
        }
        SkillScope::Project { path } => Some(
            std::path::Path::new(path)
                .join(".agents")
                .join(".skill-lock.json"),
        ),
    }
}

/// 读锁文件 `skills` map → `name → source`（owner/repo）。
/// 文件缺失 / 损坏 / 无 skills 对象 → 空 map（等价所有 skill source=None，归「其他」组）。
/// source 空 → 不入 map（同样归 None）。
fn read_skill_sources(scope: &SkillScope) -> HashMap<String, String> {
    let Some(p) = lock_file_path(scope) else {
        return HashMap::new();
    };
    let Ok(text) = std::fs::read_to_string(&p) else {
        return HashMap::new();
    };
    parse_skill_sources_json(&text)
}

/// 纯逻辑：解析锁文件文本 → `name → source` map（供单测，不耦合 fs）。
/// 损坏 JSON / 无 skills 对象 / source 空 → 不入 map。
fn parse_skill_sources_json(text: &str) -> HashMap<String, String> {
    let Ok(raw) = serde_json::from_str::<serde_json::Value>(text) else {
        return HashMap::new();
    };
    let Some(skills) = raw.get("skills").and_then(|v| v.as_object()) else {
        return HashMap::new();
    };
    skills
        .iter()
        .filter_map(|(name, meta)| {
            let src = meta.get("source").and_then(|v| v.as_str())?;
            if src.trim().is_empty() {
                return None;
            }
            Some((name.clone(), src.to_string()))
        })
        .collect()
}

/// 用锁文件 source map 填充已解析 items 的 `source` 字段（就地修改）。
/// 命中 → Some(owner/repo)；未命中（第三方/手动 symlink）→ None。
fn enrich_with_sources(items: &mut [SkillInfo], scope: &SkillScope) {
    let sources = read_skill_sources(scope);
    if sources.is_empty() {
        return;
    }
    for s in items.iter_mut() {
        s.source = sources.get(&s.name).cloned();
    }
}

/// catalog 抓取地址（skills.sh 的 JSON 索引）。
const CATALOG_URL: &str = "https://skills.sh/api/skills";

/// 浏览 catalog：优先 HTTP 抓 skills.sh，失败回退 `npx skills find`（空 kw 列全部）。
pub async fn browse_catalog(proxy_url: Option<&str>) -> Vec<CatalogEntry> {
    if let Some(list) = fetch_catalog_http(proxy_url).await {
        if !list.is_empty() {
            return list;
        }
    }
    // 回退：npx find 无关键词。
    npx_find("", proxy_url)
}

/// 搜索 catalog：HTTP 抓后本地按 kw 过滤；HTTP 空则走 `npx skills find <kw>`。
pub async fn search(kw: &str, proxy_url: Option<&str>) -> Vec<CatalogEntry> {
    let kw_lower = kw.trim().to_lowercase();
    if let Some(list) = fetch_catalog_http(proxy_url).await {
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
    npx_find(kw, proxy_url)
}

/// HTTP 抓 skills.sh catalog JSON。失败（网络 / 解析）返回 None。
/// `proxy_url` 为 `Some` 时经上游代理抓取（与 npx 子进程一致尊重代理）。
async fn fetch_catalog_http(proxy_url: Option<&str>) -> Option<Vec<CatalogEntry>> {
    let mut builder =
        reqwest::Client::builder().timeout(std::time::Duration::from_secs(10));
    if let Some(url) = proxy_url {
        if let Ok(proxy) = reqwest::Proxy::all(url) {
            builder = builder.proxy(proxy);
        }
    }
    let client = builder.build().ok()?;
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
fn npx_find(kw: &str, proxy_url: Option<&str>) -> Vec<CatalogEntry> {
    let mut args = vec!["--yes", "skills", "find"];
    let kw = kw.trim();
    if !kw.is_empty() {
        args.push(kw);
    }
    let mut cmd = Command::new("npx");
    cmd.args(&args);
    apply_proxy_env(&mut cmd, proxy_url);
    let output = match cmd.output() {
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
/// `proxy_url` 为 `Some` 时注入代理 env（见 `apply_proxy_env`），`None` 直连。
fn run_npx(extra_args: &[String], proxy_url: Option<&str>) -> SkillsOpResult {
    let mut args: Vec<String> = vec!["--yes".to_string(), "skills".to_string()];
    args.extend(extra_args.iter().cloned());
    let mut cmd = Command::new("npx");
    cmd.args(&args);
    apply_proxy_env(&mut cmd, proxy_url);
    match cmd.output() {
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

/// 构造 enable（启用）命令 args：`add <path> -a <slug> [-g] -y`。
/// 用 skill 本地 path 作 add package（list json `path`），对所有 skill 通用，不依赖锁文件 source。
/// 单 skill 目录 add 无需 `-s <name>`。抽出便于单测断言（不真跑 npx）。
fn enable_args(path: &str, agent: SkillAgent, scope: &SkillScope) -> Vec<String> {
    let mut args = vec![
        "add".to_string(),
        path.to_string(),
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

/// 为某 agent 启用 skill：用 skill 本地 path 作 add package → `npx skills add <path> -a <slug> [-g] -y`。
/// path 为空 → 明确错误。Project scope 在项目目录内执行（不带 `-g`）。
pub fn enable(
    name: &str,
    path: &str,
    agent: SkillAgent,
    scope: &SkillScope,
    proxy_url: Option<&str>,
) -> SkillsOpResult {
    let name = name.trim();
    if name.is_empty() {
        return SkillsOpResult {
            success: false,
            stdout: String::new(),
            stderr: "skill name is empty".to_string(),
        };
    }
    let path = path.trim();
    if path.is_empty() {
        return SkillsOpResult {
            success: false,
            stdout: String::new(),
            stderr: format!("skill '{name}' has no installed path; cannot enable"),
        };
    }
    let args = enable_args(path, agent, scope);
    run_npx_in_scope(&args, scope, proxy_url)
}

/// 为某 agent 关闭 skill：`npx skills remove -s <name> -a <slug> [-g] -y`。
pub fn disable(
    name: &str,
    agent: SkillAgent,
    scope: &SkillScope,
    proxy_url: Option<&str>,
) -> SkillsOpResult {
    let name = name.trim();
    if name.is_empty() {
        return SkillsOpResult {
            success: false,
            stdout: String::new(),
            stderr: "skill name is empty".to_string(),
        };
    }
    let args = disable_args(name, agent, scope);
    run_npx_in_scope(&args, scope, proxy_url)
}

/// 组级 agent 批量：对某 source 组（`group_source=None` = 「其他」组，匹配 source=None 的 skill）
/// 内所有 skill 统一启用/禁用某 agent。仅对需变更的 skill 跑 npx（已处目标态跳过）。
/// 完成后 invalidate(scope)。返回汇总（stdout "ok/total"，stderr 聚合失败明细）。
pub fn set_group_agent(
    group_source: Option<&str>,
    agent: SkillAgent,
    should_enable: bool,
    scope: &SkillScope,
    proxy_url: Option<&str>,
) -> SkillsOpResult {
    let items = list_installed(scope, proxy_url);
    let targets: Vec<&SkillInfo> = items
        .iter()
        .filter(|s| match (group_source, &s.source) {
            (Some(g), Some(src)) => src == g,
            (None, None) => true,
            _ => false,
        })
        .collect();
    if targets.is_empty() {
        return SkillsOpResult {
            success: true,
            stdout: "no skills in group".to_string(),
            stderr: String::new(),
        };
    }
    let mut ok: u32 = 0;
    let mut skipped: u32 = 0;
    let mut fail: u32 = 0;
    let mut errs: Vec<String> = Vec::new();
    for s in &targets {
        let already = s.enabled_agents.contains(&agent);
        let should_act = if should_enable { !already } else { already };
        if !should_act {
            skipped += 1;
            continue;
        }
        let res = if should_enable {
            enable(
                &s.name,
                s.installed_path.as_deref().unwrap_or(""),
                agent,
                scope,
                proxy_url,
            )
        } else {
            disable(&s.name, agent, scope, proxy_url)
        };
        if res.success {
            ok += 1;
        } else {
            fail += 1;
            let detail = if res.stderr.trim().is_empty() {
                res.stdout.trim()
            } else {
                res.stderr.trim()
            };
            errs.push(format!("{}: {}", s.name, detail));
        }
    }
    invalidate(scope);
    SkillsOpResult {
        success: fail == 0,
        stdout: format!(
            "{}/{} updated, {} skipped, {} failed",
            ok,
            targets.len(),
            skipped,
            fail
        ),
        stderr: errs.join("\n"),
    }
}

/// 更新已装 skills：`npx skills update [-g] -y`。
pub fn update(scope: &SkillScope, proxy_url: Option<&str>) -> SkillsOpResult {
    let mut args = vec!["update".to_string()];
    apply_scope(&mut args, scope);
    args.push("-y".to_string());
    run_npx_in_scope(&args, scope, proxy_url)
}

/// 一键卸载当前 scope 下所有平台所有 skills：`npx skills remove --all [-g]`。
/// `--all` = `--skill '*' --agent '*' -y`（删规范存储 + 所有 agent symlink）。
pub fn uninstall_all(scope: &SkillScope, proxy_url: Option<&str>) -> SkillsOpResult {
    let mut args = vec!["remove".to_string(), "--all".to_string()];
    apply_scope(&mut args, scope);
    run_npx_in_scope(&args, scope, proxy_url)
}

/// 构造单一 skill 卸载 args：`remove -s <name> [-g] -y`。
/// **不带 `-a`** = 删该 skill 在所有 agent 的启用配置 + 规范存储（实测验证）。
/// ⚠️ `-a` 不接受通配（`-a '*'` 报 `Invalid agents: *` exit 1）；仅 `--all` 简写内部展开。
/// 故单 skill 全卸载只能省略 `-a`，等效 `--all` 但限定单个 skill。
fn uninstall_args(name: &str, scope: &SkillScope) -> Vec<String> {
    let mut args = vec![
        "remove".to_string(),
        "-s".to_string(),
        name.to_string(),
    ];
    apply_scope(&mut args, scope);
    args.push("-y".to_string());
    args
}

/// 卸载单一 skill：`npx skills remove -s <name> [-g] -y`（破坏性，前端二次确认）。
/// 删规范存储目录 + 所有 agent 的启用配置（symlink / 锁文件项）。
///
/// **fs 兜底**：第三方/手动 symlink skill（如 understand-*，非 npx 装、不在锁文件）
/// npx remove 返回 "No matching skills found"（exit 0 但没删）。检测到此输出 → fs 兜底
/// 删规范存储 symlink + 各 agent 目录 symlink（用户决策 A，突破"全 npx"约束，对称于
/// enable 用 path 绕锁文件）。
pub fn uninstall(name: &str, scope: &SkillScope, proxy_url: Option<&str>) -> SkillsOpResult {
    let name = name.trim();
    if name.is_empty() {
        return SkillsOpResult {
            success: false,
            stdout: String::new(),
            stderr: "skill name is empty".to_string(),
        };
    }
    let args = uninstall_args(name, scope);
    let res = run_npx_in_scope(&args, scope, proxy_url);
    // 检测 npx 不认该 skill（第三方/手动 symlink，非锁文件注册）→ fs 兜底删。
    let no_match = res
        .stdout
        .contains("No matching skills found")
        || res.stderr.contains("No matching skills found");
    if no_match {
        let (removed, errs) = fs_fallback_remove(name, scope);
        let success = !removed.is_empty() && errs.is_empty();
        return SkillsOpResult {
            success,
            stdout: format!(
                "fs fallback removed {} path(s): [{}]",
                removed.len(),
                removed.join(", ")
            ),
            stderr: if errs.is_empty() {
                String::new()
            } else {
                format!("fs fallback errors: {}", errs.join("; "))
            },
        };
    }
    res
}

/// 校验 skill name 安全（防路径遍历：禁 `..` / `/` / `\` / 空 / `.`）。
fn is_safe_skill_name(name: &str) -> bool {
    !name.is_empty()
        && name != "."
        && name != ".."
        && !name.contains('/')
        && !name.contains('\\')
        && !name.contains("..")
}

/// 删除单个路径（symlink → remove_file 不跟随；目录 → remove_dir_all；不存在 → skip）。
/// 返回 Some(()) 表示删成功，None 表示不存在，Some(Err) 转 errs。
fn remove_path(p: &PathBuf, removed: &mut Vec<String>, errs: &mut Vec<String>) {
    let meta = match fs::symlink_metadata(p) {
        Ok(m) => m,
        Err(_) => return, // 不存在 → skip
    };
    let r = if meta.is_dir() && !meta.file_type().is_symlink() {
        fs::remove_dir_all(p)
    } else {
        fs::remove_file(p) // symlink 或文件：不跟随 symlink target
    };
    match r {
        Ok(()) => removed.push(p.display().to_string()),
        Err(e) => errs.push(format!("remove {}: {e}", p.display())),
    }
}

/// fs 兜底删第三方/手动 symlink skill。返回 (已删路径, 错误)。
///
/// - **规范存储**：global `~/.agents/skills/<name>`，project `<project>/.agents/skills/<name>`。
/// - **各 agent symlink**（仅 global）：扫 `~/` 下 `.` 开头目录（.claude/.codex/.trae-cn/...），
///   若 `<dir>/skills/<name>` 存在则删。不硬编码 agent 列表，通配扫。
///
/// 安全：name 经 `is_safe_skill_name` 校验，防路径遍历。
fn fs_fallback_remove(name: &str, scope: &SkillScope) -> (Vec<String>, Vec<String>) {
    let mut removed: Vec<String> = Vec::new();
    let mut errs: Vec<String> = Vec::new();

    if !is_safe_skill_name(name) {
        return (removed, vec![format!("unsafe skill name: '{name}'")]);
    }

    match scope {
        SkillScope::Global => {
            if let Some(home) = dirs::home_dir() {
                // 规范存储
                let canon = home.join(".agents").join("skills").join(name);
                remove_path(&canon, &mut removed, &mut errs);
                // 各 agent 目录（home 下 . 开头目录，排除 .agents 本身）
                if let Ok(entries) = fs::read_dir(&home) {
                    for e in entries.flatten() {
                        let s = e.file_name();
                        let dir_name = s.to_string_lossy();
                        if !dir_name.starts_with('.') || dir_name == ".agents" {
                            continue;
                        }
                        let agent_skill = home.join(dir_name.as_ref()).join("skills").join(name);
                        remove_path(&agent_skill, &mut removed, &mut errs);
                    }
                }
            } else {
                errs.push("cannot resolve home directory".to_string());
            }
        }
        SkillScope::Project { path } => {
            let canon = PathBuf::from(path).join(".agents").join("skills").join(name);
            remove_path(&canon, &mut removed, &mut errs);
        }
    }

    (removed, errs)
}

/// 对齐决策：以 source 启用态决定 target 应做何操作。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AlignAction {
    /// source 启用 + target 未启用 → target 需 enable。
    Enable,
    /// source 未启用 + target 启用 → target 需 disable。
    Disable,
    /// 其余（两者一致）→ 不变。
    Keep,
}

fn plan_align_action(from_on: bool, to_on: bool) -> AlignAction {
    match (from_on, to_on) {
        (true, false) => AlignAction::Enable,
        (false, true) => AlignAction::Disable,
        _ => AlignAction::Keep,
    }
}

/// 使 `to` 的启用配置与 `from` 完全一致（逐 skill 比对 → enable/disable 凑齐）。
/// `from == to` → noop。逐 skill shell out `npx skills enable/disable`，N 小可接受。
pub fn align_agents(
    from: SkillAgent,
    to: SkillAgent,
    scope: &SkillScope,
    proxy_url: Option<&str>,
) -> SkillsOpResult {
    if from == to {
        return SkillsOpResult {
            success: true,
            stdout: "noop: source equals target".to_string(),
            stderr: String::new(),
        };
    }
    let skills = list_installed(scope, proxy_url);
    let mut enabled_n = 0usize;
    let mut disabled_n = 0usize;
    let mut errs: Vec<String> = Vec::new();
    for s in &skills {
        let from_on = s.enabled_agents.contains(&from);
        let to_on = s.enabled_agents.contains(&to);
        match plan_align_action(from_on, to_on) {
            AlignAction::Enable => {
                let path = s.installed_path.as_deref().unwrap_or("");
                let r = enable(&s.name, path, to, scope, proxy_url);
                if r.success {
                    enabled_n += 1;
                } else {
                    errs.push(format!(
                        "enable {} on {}: {}",
                        s.name,
                        to.cli_slug(),
                        r.stderr.trim()
                    ));
                }
            }
            AlignAction::Disable => {
                let r = disable(&s.name, to, scope, proxy_url);
                if r.success {
                    disabled_n += 1;
                } else {
                    errs.push(format!(
                        "disable {} on {}: {}",
                        s.name,
                        to.cli_slug(),
                        r.stderr.trim()
                    ));
                }
            }
            AlignAction::Keep => {}
        }
    }
    let total = enabled_n + disabled_n;
    SkillsOpResult {
        success: errs.is_empty(),
        stdout: format!(
            "aligned {total} changes ({enabled_n} enabled, {disabled_n} disabled)"
        ),
        stderr: errs.join("; "),
    }
}

/// 为某 agent 启用当前 scope 下全部已装 skills（只增不减，非破坏性）。
/// 逐 skill：agent 未启用则 `enable()`，已启用跳过。
pub fn enable_all(agent: SkillAgent, scope: &SkillScope, proxy_url: Option<&str>) -> SkillsOpResult {
    let skills = list_installed(scope, proxy_url);
    let mut enabled_n = 0usize;
    let mut errs: Vec<String> = Vec::new();
    for s in &skills {
        if s.enabled_agents.contains(&agent) {
            continue;
        }
        let path = s.installed_path.as_deref().unwrap_or("");
        let r = enable(&s.name, path, agent, scope, proxy_url);
        if r.success {
            enabled_n += 1;
        } else {
            errs.push(format!("enable {} on {}: {}", s.name, agent.cli_slug(), r.stderr.trim()));
        }
    }
    SkillsOpResult {
        success: errs.is_empty(),
        stdout: format!("enabled {enabled_n} skills"),
        stderr: errs.join("; "),
    }
}

/// 按 scope 追加 `-g`（仅 Global）。
fn apply_scope(args: &mut Vec<String>, scope: &SkillScope) {
    if matches!(scope, SkillScope::Global) {
        args.push("-g".to_string());
    }
}

/// 由上游代理设置构造 npm/npx 用的代理 URL。
///
/// - 未启用（`enabled == false`）→ `None`（保持直连，不注入 env）。
/// - 启用 → `Some("{scheme}://[user:pass@]host:port")`。
/// - scheme：`socks5` 且 `dns_over_proxy` → `socks5h`（DNS 走代理）；否则按 proxy_type
///   映射（`socks5`/`https`/其余 → `http`），与 `ProxyClientSettings::to_reqwest_proxy` 一致。
///
/// ⚠️ socks5 限制：npm/npx 原生对 socks5 支持有限，依赖底层（如 undici / global-agent）的
/// `ALL_PROXY` 识别，未必所有 npm 版本生效；http/https 代理走 `HTTP_PROXY`/`HTTPS_PROXY` 最稳。
///
/// ⚠️ 认证编码：user/pass 原样嵌入 URL，不做 percent-encode。若凭证含 `@` `:` `/` 等保留字符，
/// 生成的 URL 可能被 npm/node 解析歧义（同 npm 自身约定：env 代理 URL 的凭证需调用方自行编码）。
/// 与 `to_reqwest_proxy`（用 `proxy.basic_auth` 内部处理）的差异仅在此边界场景显现。
pub fn proxy_env_url(settings: &ProxyClientSettings) -> Option<String> {
    if !settings.enabled {
        return None;
    }
    let scheme = match settings.proxy_type.as_str() {
        "socks5" if settings.dns_over_proxy => "socks5h",
        "socks5" => "socks5",
        "https" => "https",
        _ => "http",
    };
    let auth = if settings.username.is_empty() {
        String::new()
    } else {
        format!("{}:{}@", settings.username, settings.password)
    };
    Some(format!(
        "{}://{}{}:{}",
        scheme, auth, settings.host, settings.port
    ))
}

/// 为 npx `Command` 注入代理 env（若 `proxy_url` 为 `Some`）。
///
/// 设大小写两组 `HTTP_PROXY`/`HTTPS_PROXY`（兼容不同 npm/node 读法）；socks5(h) 时额外设
/// `ALL_PROXY`（npm 对 socks5 仅经此识别）。`None` → 不注入，保持直连行为不变。
fn apply_proxy_env(cmd: &mut Command, proxy_url: Option<&str>) {
    let Some(url) = proxy_url else {
        return;
    };
    cmd.env("HTTP_PROXY", url)
        .env("HTTPS_PROXY", url)
        .env("http_proxy", url)
        .env("https_proxy", url);
    if url.starts_with("socks5") {
        cmd.env("ALL_PROXY", url).env("all_proxy", url);
    }
}

/// 在 scope 对应的 cwd 执行 npx：Project → 项目目录；Global → 默认 cwd。
/// `proxy_url` 为 `Some` 时给 npx 子进程注入代理 env（突破网络限制），`None` 直连。
fn run_npx_in_scope(
    extra_args: &[String],
    scope: &SkillScope,
    proxy_url: Option<&str>,
) -> SkillsOpResult {
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
        let mut cmd = Command::new("npx");
        cmd.args(&full).current_dir(p);
        apply_proxy_env(&mut cmd, proxy_url);
        return match cmd.output() {
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
    run_npx(extra_args, proxy_url)
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn uninstall_args_global() {
        let args = uninstall_args("my-skill", &SkillScope::Global);
        assert_eq!(args[0], "remove");
        assert_eq!(args[1], "-s");
        assert_eq!(args[2], "my-skill");
        // 不带 -a（实测：删所有 agent；-a '*' 会 invalid exit 1）。
        assert!(!args.iter().any(|a| a == "-a"));
        assert!(args.contains(&"-g".to_string()));
        assert!(args.contains(&"-y".to_string()));
    }

    #[test]
    fn uninstall_args_project_no_g() {
        let args = uninstall_args(
            "my-skill",
            &SkillScope::Project {
                path: "/tmp".to_string(),
            },
        );
        assert!(!args.contains(&"-g".to_string()));
        assert!(args.contains(&"-y".to_string()));
        assert!(!args.iter().any(|a| a == "-a"));
    }

    #[test]
    fn is_safe_skill_name_rejects_traversal() {
        // 合法
        assert!(is_safe_skill_name("understand-onboard"));
        assert!(is_safe_skill_name("my_skill"));
        assert!(is_safe_skill_name("a"));
        // 路径遍历 / 非法
        assert!(!is_safe_skill_name(""));
        assert!(!is_safe_skill_name("."));
        assert!(!is_safe_skill_name(".."));
        assert!(!is_safe_skill_name("../etc"));
        assert!(!is_safe_skill_name("foo/bar"));
        assert!(!is_safe_skill_name("foo\\bar"));
        assert!(!is_safe_skill_name("a..b"));
    }

    #[test]
    fn parse_skill_sources_json_handles_cases() {
        // 正常：foo 有 source，bar 空 source 不入，baz 无 source 字段不入。
        let m = parse_skill_sources_json(
            r#"{"version":1,"skills":{"foo":{"source":"owner/repo","sourceType":"github"},"bar":{"source":"   "},"baz":{"sourceType":"github"}}}"#,
        );
        assert_eq!(m.len(), 1);
        assert_eq!(m.get("foo").map(String::as_str), Some("owner/repo"));

        // 损坏 JSON → 空。
        assert!(parse_skill_sources_json("not json {{{").is_empty());
        // 无 skills 对象 → 空。
        assert!(parse_skill_sources_json(r#"{"version":1}"#).is_empty());
        // 多条合法 source。
        let m2 = parse_skill_sources_json(
            r#"{"skills":{"a":{"source":"x/y"},"b":{"source":"p/q"}}}"#,
        );
        assert_eq!(m2.len(), 2);
        assert_eq!(m2.get("a").map(String::as_str), Some("x/y"));
        assert_eq!(m2.get("b").map(String::as_str), Some("p/q"));
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
    fn frontmatter_description_plain() {
        let md = "---\nname: foo\ndescription: A great skill for stuff.\n---\nbody\n";
        assert_eq!(
            parse_skill_description_from_frontmatter(md).as_deref(),
            Some("A great skill for stuff.")
        );
    }

    #[test]
    fn frontmatter_description_quoted() {
        let md = "---\nname: foo\ndescription: \"Quoted desc\"\n---\n";
        assert_eq!(
            parse_skill_description_from_frontmatter(md).as_deref(),
            Some("Quoted desc")
        );
    }

    #[test]
    fn frontmatter_description_single_quoted() {
        let md = "---\ndescription: 'single'\n---\n";
        assert_eq!(
            parse_skill_description_from_frontmatter(md).as_deref(),
            Some("single")
        );
    }

    #[test]
    fn frontmatter_no_frontmatter() {
        let md = "just plain markdown\nno frontmatter\n";
        assert!(parse_skill_description_from_frontmatter(md).is_none());
    }

    #[test]
    fn frontmatter_no_description_field() {
        let md = "---\nname: foo\n---\nbody\n";
        assert!(parse_skill_description_from_frontmatter(md).is_none());
    }

    #[test]
    fn frontmatter_empty_description() {
        let md = "---\ndescription: \"\"\n---\n";
        assert!(parse_skill_description_from_frontmatter(md).is_none());
    }

    #[test]
    fn frontmatter_desc_only_inside_frontmatter() {
        // description 行在正文 (非 frontmatter) 不应被解析。
        let md = "---\nname: foo\n---\ndescription: fake in body\n";
        assert!(parse_skill_description_from_frontmatter(md).is_none());
    }

    #[test]
    fn plan_align_action_matrix() {
        assert_eq!(plan_align_action(true, false), AlignAction::Enable);
        assert_eq!(plan_align_action(false, true), AlignAction::Disable);
        assert_eq!(plan_align_action(true, true), AlignAction::Keep);
        assert_eq!(plan_align_action(false, false), AlignAction::Keep);
    }

    #[test]
    fn enable_args_global_claude() {
        // path 作 add package，无 -s；global 带 -g。
        let args = enable_args("/p/foo", SkillAgent::Claude, &SkillScope::Global);
        assert_eq!(
            args,
            vec!["add", "/p/foo", "-a", "claude-code", "-g", "-y"]
        );
        assert!(!args.contains(&"-s".to_string()));
    }

    #[test]
    fn enable_args_project_codex_no_g() {
        let args = enable_args(
            "/p/bar",
            SkillAgent::Codex,
            &SkillScope::Project { path: "/proj".to_string() },
        );
        assert_eq!(args, vec!["add", "/p/bar", "-a", "codex", "-y"]);
        assert!(!args.contains(&"-g".to_string()));
        assert!(!args.contains(&"-s".to_string()));
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
    fn enable_empty_path_fails() {
        // path 为空 → 明确错误，不真跑 npx。
        let r = enable("whatever", "   ", SkillAgent::Claude, &SkillScope::Global, None);
        assert!(!r.success);
        assert!(r.stderr.contains("no installed path"));
    }

    #[test]
    fn enable_empty_name_fails() {
        let r = enable("  ", "/p/foo", SkillAgent::Claude, &SkillScope::Global, None);
        assert!(!r.success);
    }

    #[test]
    fn disable_empty_name_fails() {
        let r = disable("  ", SkillAgent::Claude, &SkillScope::Global, None);
        assert!(!r.success);
    }

    // ── 代理 URL 构造 ──

    fn proxy_settings(
        enabled: bool,
        ty: &str,
        user: &str,
        pass: &str,
        dns_over_proxy: bool,
    ) -> ProxyClientSettings {
        ProxyClientSettings {
            enabled,
            proxy_type: ty.to_string(),
            host: "1.2.3.4".to_string(),
            port: 7890,
            username: user.to_string(),
            password: pass.to_string(),
            dns_over_proxy,
        }
    }

    #[test]
    fn proxy_env_url_disabled_is_none() {
        let s = proxy_settings(false, "http", "", "", true);
        assert_eq!(proxy_env_url(&s), None);
    }

    #[test]
    fn proxy_env_url_http_no_auth() {
        let s = proxy_settings(true, "http", "", "", true);
        assert_eq!(proxy_env_url(&s).as_deref(), Some("http://1.2.3.4:7890"));
    }

    #[test]
    fn proxy_env_url_https_with_auth() {
        let s = proxy_settings(true, "https", "u", "p", true);
        assert_eq!(
            proxy_env_url(&s).as_deref(),
            Some("https://u:p@1.2.3.4:7890")
        );
    }

    #[test]
    fn proxy_env_url_socks5_dns_over_proxy_is_socks5h() {
        let s = proxy_settings(true, "socks5", "", "", true);
        assert_eq!(proxy_env_url(&s).as_deref(), Some("socks5h://1.2.3.4:7890"));
    }

    #[test]
    fn proxy_env_url_socks5_no_dns_is_socks5() {
        let s = proxy_settings(true, "socks5", "", "", false);
        assert_eq!(proxy_env_url(&s).as_deref(), Some("socks5://1.2.3.4:7890"));
    }

    #[test]
    fn proxy_env_url_socks5_with_auth() {
        let s = proxy_settings(true, "socks5", "u", "p", false);
        assert_eq!(
            proxy_env_url(&s).as_deref(),
            Some("socks5://u:p@1.2.3.4:7890")
        );
    }

    #[test]
    fn proxy_env_url_unknown_type_falls_back_http() {
        let s = proxy_settings(true, "weird", "", "", true);
        assert_eq!(proxy_env_url(&s).as_deref(), Some("http://1.2.3.4:7890"));
    }

    // ── env 注入（构造 Command 断言 env，不真跑 npx）──

    fn env_of<'a>(cmd: &'a Command, key: &str) -> Option<&'a std::ffi::OsStr> {
        cmd.get_envs()
            .find(|(k, _)| *k == std::ffi::OsStr::new(key))
            .and_then(|(_, v)| v)
    }

    #[test]
    fn apply_proxy_env_none_injects_nothing() {
        let mut cmd = Command::new("npx");
        apply_proxy_env(&mut cmd, None);
        assert_eq!(cmd.get_envs().count(), 0);
    }

    #[test]
    fn apply_proxy_env_http_sets_http_https_not_all() {
        let mut cmd = Command::new("npx");
        apply_proxy_env(&mut cmd, Some("http://1.2.3.4:7890"));
        assert_eq!(
            env_of(&cmd, "HTTP_PROXY"),
            Some(std::ffi::OsStr::new("http://1.2.3.4:7890"))
        );
        assert_eq!(
            env_of(&cmd, "HTTPS_PROXY"),
            Some(std::ffi::OsStr::new("http://1.2.3.4:7890"))
        );
        assert_eq!(
            env_of(&cmd, "http_proxy"),
            Some(std::ffi::OsStr::new("http://1.2.3.4:7890"))
        );
        // 非 socks5 → 不设 ALL_PROXY。
        assert_eq!(env_of(&cmd, "ALL_PROXY"), None);
    }

    #[test]
    fn apply_proxy_env_socks5_also_sets_all_proxy() {
        let mut cmd = Command::new("npx");
        apply_proxy_env(&mut cmd, Some("socks5h://1.2.3.4:7890"));
        assert_eq!(
            env_of(&cmd, "ALL_PROXY"),
            Some(std::ffi::OsStr::new("socks5h://1.2.3.4:7890"))
        );
        assert_eq!(
            env_of(&cmd, "all_proxy"),
            Some(std::ffi::OsStr::new("socks5h://1.2.3.4:7890"))
        );
        assert_eq!(
            env_of(&cmd, "HTTP_PROXY"),
            Some(std::ffi::OsStr::new("socks5h://1.2.3.4:7890"))
        );
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
    fn cache_key_global_and_project_distinct() {
        assert_eq!(SkillScope::Global.cache_key(), "global");
        let p = SkillScope::Project { path: "/proj/a".to_string() };
        assert_eq!(p.cache_key(), "project:/proj/a");
        // 不同项目 path 不串。
        let q = SkillScope::Project { path: "/proj/b".to_string() };
        assert_ne!(p.cache_key(), q.cache_key());
        // global ≠ 任意 project。
        assert_ne!(SkillScope::Global.cache_key(), p.cache_key());
    }

    #[test]
    fn cache_key_trims_project_path() {
        let p = SkillScope::Project { path: "  /proj/a  ".to_string() };
        assert_eq!(p.cache_key(), "project:/proj/a");
    }

    #[test]
    fn cache_file_roundtrip_serde() {
        // 缓存文件结构可序列化/反序列化往返。
        let mut file = SkillsCacheFile::default();
        file.scopes.insert(
            "global".to_string(),
            ScopeCacheEntry {
                cached_at: 123,
                items: vec![SkillInfo {
                    name: "foo".to_string(),
                    enabled_agents: vec![SkillAgent::Claude],
                    scope: SkillScope::Global,
                    installed_path: Some("/p/foo".to_string()),
                    description: None,
                    source: None,
                }],
            },
        );
        let json = serde_json::to_string(&file).unwrap();
        let back: SkillsCacheFile = serde_json::from_str(&json).unwrap();
        assert_eq!(back.scopes.len(), 1);
        let entry = back.scopes.get("global").unwrap();
        assert_eq!(entry.cached_at, 123);
        assert_eq!(entry.items[0].name, "foo");
    }

    #[test]
    fn cache_file_corrupt_json_defaults_empty() {
        // 损坏 JSON → 默认空（当冷启动），不 panic。
        let back: SkillsCacheFile = serde_json::from_str("not json {{{").unwrap_or_default();
        assert!(back.scopes.is_empty());
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
