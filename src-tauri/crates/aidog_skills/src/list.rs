//! 列已装 skills：**直接读 `~/.agents/.skill-lock.json` + 探测本地 agent symlink**（免 npx spawn）。
//!
//! 数据源（2026-06-26 重构，用户明确要求）：
//! - **锁文件** `~/.agents/.skill-lock.json`（global）或 `<project>/.agents/.skill-lock.json`（project）
//!   提供 name + 7 个独有字段（source/sourceType/sourceUrl/skillFolderHash/pluginName/installedAt/updatedAt）
//! - **agent 启用态** 探测 `~/.<agent>/skills/<name>`（claude→`~/.claude/skills/`、codex→`~/.codex/skills/`）
//!   存在 = 该 agent 已启用（与 npx skills CLI 判定一致，见 vercel-labs/skills dist/cli.mjs
//!   `agents[agent].globalSkillsDir` + `access(agentSkillDir)`）
//! - **规范存储路径** `<scope_skills_dir>/<name>`（`~/.agents/skills/<name>` 或 `<project>/.agents/skills/<name>`）
//! - **简介** 读 `<path>/SKILL.md` frontmatter `description`
//!
//! 失败信号：`list_installed` 返回 `(items, ok)`，`ok=false` 表示 HOME 缺失 / 锁文件损坏 / 非预期
//! schema version → 调用方不应覆盖已有缓存（见 `cache::list_refresh`）。

use super::env::resolve_home_env;
use super::types::{SkillAgent, SkillInfo, SkillScope};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// 锁文件 schema 兼容版本（v3 当前唯一支持）。未来 v4 引入时按需扩展。
const LOCK_SCHEMA_VERSION: i64 = 3;

/// 列指定 scope 下已装 skills（统一一条/skill，不分 agent）。**零 npx spawn**（纯文件读 + symlink 探测）。
///
/// 走 `~/.agents/.skill-lock.json`（global）或 `<project>/.agents/.skill-lock.json`（project），
/// 解析 `skills` map → per-skill `SkillInfo`（含 7 个锁文件独有字段），再探测
/// `~/.<agent>/skills/<name>` 存在性填 `enabled_agents`。
///
/// 返回 `(items, ok)`：`ok=true` = 锁文件读取/解析成功（`items` 可能为空 = 真无 skill）；
/// `ok=false` = HOME 缺失 / 锁文件路径不可解析 / JSON 损坏 / 非预期 version（`items` 为空 vec，
/// **不可写覆盖已有缓存**，防假空缓存）。让调用方区分「真无 skill」vs「加载失败」。
///
/// 注：SWR 链路用 [`super::cache::list_cached`]（即时缓存）+ [`super::cache::list_refresh`]
/// （强制刷新）；内部聚合算子（`align_agents` / `enable_all`）仍走本函数取实时态（`ok` 忽略，
/// 仅用 `items`）。
pub fn list_installed(scope: &SkillScope, _proxy_url: Option<&str>) -> (Vec<SkillInfo>, bool) {
    // HOME env 防御（memory [[skills-claude-code-home-env]]）：dirs::home_dir() 与 HOME env 均失败
    // 时 codex/claude 启用探测 + global 锁文件路径均不可解析 → 返失败信号保留旧缓存。
    // （注：即便锁文件是纯文件读，global 路径走 dirs::home_dir()，HOME 异常场景仍漏检。）
    if resolve_home_env().0.is_none() && matches!(scope, SkillScope::Global) {
        tracing::warn!(
            "list_installed: HOME 无法解析（dirs::home_dir() 和 HOME env 均缺失），skills list 失败"
        );
        return (Vec::new(), false);
    }
    let Some(lock_path) = lock_file_path(scope) else {
        tracing::warn!(scope = ?scope, "list_installed: 锁文件路径不可解析（HOME 缺失或 project path 空）");
        return (Vec::new(), false);
    };
    let Ok(text) = std::fs::read_to_string(&lock_path) else {
        // 锁文件不存在 = 真空（用户从未装过），不是加载失败。返 ok=true + 空 vec（覆盖缓存合法）。
        tracing::debug!(
            scope = ?scope,
            path = %lock_path.display(),
            "list_installed: 锁文件不存在（真空，非失败）"
        );
        return (Vec::new(), true);
    };
    // JSON 损坏 / version 非预期 → ok=false（防假空缓存覆盖真数据）。
    let Ok(parsed) = parse_lock_file(&text) else {
        tracing::warn!(
            scope = ?scope,
            path = %lock_path.display(),
            "list_installed: 锁文件 JSON 损坏或 version 非预期，视为失败保守处理"
        );
        return (Vec::new(), false);
    };
    let mut items = build_skill_infos(parsed, scope);
    // 排序保证 deterministic 输出（与旧 npx list 行为一致）。
    items.sort_by(|a, b| a.name.cmp(&b.name));
    (items, true)
}

/// 规范 skills 存储目录：global → `~/.agents/skills`；project → `<path>/.agents/skills`。
/// 用于 installed_path 构造。home 不可解析（global）→ None。
fn canonical_skills_dir(scope: &SkillScope) -> Option<PathBuf> {
    match scope {
        SkillScope::Global => dirs::home_dir().map(|h| h.join(".agents").join("skills")),
        SkillScope::Project { path } => Some(
            Path::new(path)
                .join(".agents")
                .join("skills"),
        ),
    }
}

/// 锁文件路径：global → `~/.agents/.skill-lock.json`；project → `<path>/.agents/.skill-lock.json`。
/// home 不可解析（global）→ None。
fn lock_file_path(scope: &SkillScope) -> Option<PathBuf> {
    match scope {
        SkillScope::Global => dirs::home_dir().map(|h| h.join(".agents").join(".skill-lock.json")),
        SkillScope::Project { path } => Some(
            Path::new(path)
                .join(".agents")
                .join(".skill-lock.json"),
        ),
    }
}

/// 某 agent 的全局 skills 目录：claude → `$CLAUDE_CONFIG_DIR/skills || ~/.claude/skills`；
/// codex → `$CODEX_HOME/skills || ~/.codex/skills`。home 不可解析 → None。
///
/// 与 npx skills CLI 的 `agents[agent].globalSkillsDir` + `codexHome`/`claudeHome` 解析一致
/// （见 vercel-labs/skills dist/cli.mjs L938/L1035/L1099）。
fn agent_global_skills_dir(agent: SkillAgent) -> Option<PathBuf> {
    match agent {
        SkillAgent::Claude => {
            // CLAUDE_CONFIG_DIR 优先（透传用户自定义），fallback ~/.claude。
            let cfg = std::env::var("CLAUDE_CONFIG_DIR")
                .ok()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty());
            if let Some(c) = cfg {
                return Some(PathBuf::from(c).join("skills"));
            }
            dirs::home_dir().map(|h| h.join(".claude").join("skills"))
        }
        SkillAgent::Codex => {
            // CODEX_HOME 优先（透传），fallback ~/.codex。
            let cfg = std::env::var("CODEX_HOME")
                .ok()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty());
            if let Some(c) = cfg {
                return Some(PathBuf::from(c).join("skills"));
            }
            dirs::home_dir().map(|h| h.join(".codex").join("skills"))
        }
    }
}

/// 某 agent 的 project skills 目录：`<project>/.<dir>/skills`（统一 `.agents/skills` ——
/// vercel-labs/skills CLI 的 universal agent 模式，所有 agent 共享 `.agents/skills` 目录）。
fn agent_project_skills_dir(agent: SkillAgent, project_path: &str) -> Option<PathBuf> {
    // 注：npx skills CLI codex 在 project scope 用 `.agents/skills`（universal）而非 `.codex/skills`
    // （仅 global 才是 agent-specific），与 listInstalledSkills 中 `agent.skillsDir` 一致。
    // 简化：所有 agent 在 project scope 都查 `.agents/skills/<name>`（与 canonical_skills_dir 同路径）。
    // 这里 _agent 仅保留签名对称性，实际路径忽略（universal 模式）。
    let _ = agent;
    if project_path.trim().is_empty() {
        return None;
    }
    Some(
        Path::new(project_path)
            .join(".agents")
            .join("skills"),
    )
}

/// 探测某 skill 在某 agent 是否已启用（目录或 symlink 存在）。
/// global → `~/.<agent>/skills/<name>`；project → `<project>/.agents/skills/<name>`。
///
/// 用 `path.exists()` 判断（symlink 指向已删目录时 exists() 返 false，符合语义）。
fn is_skill_enabled_for_agent(
    name: &str,
    agent: SkillAgent,
    scope: &SkillScope,
) -> bool {
    if name.is_empty() {
        return false;
    }
    let dir = match scope {
        SkillScope::Global => agent_global_skills_dir(agent),
        SkillScope::Project { path } => agent_project_skills_dir(agent, path),
    };
    let Some(base) = dir else {
        return false;
    };
    // 路径遍历防御（memory [[pathbuf-starts-with-traversal]]）：禁 `..` / `/`。
    // skill name 来自锁文件 key，合法值不含路径分隔符，但纵深防御防注入。
    if name.contains('/') || name.contains('\\') || name.contains("..") {
        return false;
    }
    base.join(name).exists()
}

/// 解析锁文件文本 → `LockFile` 结构（含 schema version 守卫）。
/// 损坏 JSON / version 非预期 → Err。
pub(super) fn parse_lock_file(text: &str) -> Result<LockFile, serde_json::Error> {
    let lf: LockFile = serde_json::from_str(text)?;
    // version 守卫：v3 当前唯一支持。未来 v4 等需扩展（字段缺失不破坏，但主版本变可能改 schema）。
    // 非预期 version 视为损坏保守返 Err（让 list_installed 返 ok=false 保旧缓存）。
    if lf.version != LOCK_SCHEMA_VERSION {
        return Err(serde::de::Error::custom(format!(
            "unexpected lockfile version: got {}, expected {}",
            lf.version, LOCK_SCHEMA_VERSION
        )));
    }
    Ok(lf)
}

/// 锁文件根结构（`~/.agents/.skill-lock.json`）。
#[derive(Debug, Clone, serde::Deserialize)]
pub(super) struct LockFile {
    pub version: i64,
    #[serde(default)]
    pub skills: HashMap<String, LockSkill>,
}

/// 锁文件单 skill 条目。
#[derive(Debug, Clone, serde::Deserialize)]
pub(super) struct LockSkill {
    pub source: Option<String>,
    #[serde(rename = "sourceType")]
    pub source_type: Option<String>,
    #[serde(rename = "sourceUrl")]
    pub source_url: Option<String>,
    /// 锁文件记录的 SKILL.md 相对路径（如 `skills/foo/SKILL.md`）。
    /// 当前未透出到 SkillInfo（规范存储路径用 canonical_skills_dir 推导），
    /// 保留字段为 schema 完整性 + 未来 v4 兼容性。
    #[serde(rename = "skillPath")]
    #[allow(dead_code)]
    pub skill_path: Option<String>,
    #[serde(rename = "skillFolderHash")]
    pub skill_folder_hash: Option<String>,
    #[serde(rename = "pluginName")]
    pub plugin_name: Option<String>,
    #[serde(rename = "installedAt")]
    pub installed_at: Option<String>,
    #[serde(rename = "updatedAt")]
    pub updated_at: Option<String>,
}

/// 从解析的锁文件 + scope 构造 `Vec<SkillInfo>`（含 enabled_agents 探测 + installed_path + description）。
/// 内部辅助（pub(super) 供单测）。
pub(super) fn build_skill_infos(lock: LockFile, scope: &SkillScope) -> Vec<SkillInfo> {
    let canonical = canonical_skills_dir(scope);
    lock.skills
        .into_iter()
        .map(|(name, meta)| {
            // enabled_agents：探测 claude/codex 各自 skills 目录是否存在该 name。
            let enabled_agents: Vec<SkillAgent> = SkillAgent::all()
                .into_iter()
                .filter(|a| is_skill_enabled_for_agent(&name, *a, scope))
                .collect();
            // installed_path：规范存储目录 + name（与 npx list 的 path 字段语义一致）。
            let installed_path = canonical
                .as_ref()
                .map(|c| c.join(&name).to_string_lossy().into_owned());
            // description：读 SKILL.md frontmatter（installed_path 存在才尝试）。
            let description = installed_path
                .as_deref()
                .and_then(read_skill_description);
            // 空白归一化：trim 后空 → None（避免 "  " 等无效 source 显示）。
            let non_empty = |s: Option<String>| -> Option<String> {
                s.and_then(|v| {
                    let t = v.trim();
                    if t.is_empty() {
                        None
                    } else {
                        Some(t.to_string())
                    }
                })
            };
            SkillInfo {
                name,
                enabled_agents,
                scope: scope.clone(),
                installed_path,
                description,
                source: non_empty(meta.source),
                source_type: non_empty(meta.source_type),
                source_url: non_empty(meta.source_url),
                skill_folder_hash: non_empty(meta.skill_folder_hash),
                plugin_name: non_empty(meta.plugin_name),
                installed_at: non_empty(meta.installed_at),
                updated_at: non_empty(meta.updated_at),
            }
        })
        .collect()
}

/// 从 SKILL.md 文本 frontmatter 解析 `description:` 单行值。
/// 规则: 首行 `---` 起, 到下一个 `---` 止; 行 `description: <value>`, 去首尾引号 (单/双)。
/// 无 frontmatter / 无 description 行 / 空值 → None。多行折叠 (`>-`) 不支持。
pub(super) fn parse_skill_description_from_frontmatter(content: &str) -> Option<String> {
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
    let p = Path::new(skill_path).join("SKILL.md");
    let content = std::fs::read_to_string(p).ok()?;
    parse_skill_description_from_frontmatter(&content)
}

#[cfg(test)]
#[path = "test_list.rs"]
mod test_list;
