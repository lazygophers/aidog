//! 列已装 skills：`npx skills list --json` 解析 + SKILL.md 描述 + 锁文件 source 富化。
//!
//! 失败信号：`list_installed` 返回 `(items, ok)`，`ok=false` 表示 npx 失败 / HOME 缺失 /
//! JSON 解析失败，调用方不应覆盖已有缓存（见 `cache::list_refresh`）。

use super::env::resolve_home_env;
use super::npx::{apply_scope, run_npx_in_scope};
use super::types::{SkillAgent, SkillInfo, SkillScope};
use std::collections::HashMap;

/// 列指定 scope 下已装 skills（统一一条/skill，不分 agent）。**直跑 npx**（无缓存）。
///
/// 走 `npx skills list --json [-g]`，解析 `[{name, path, scope, agents:[...]}]`。
/// `agents[]` 含某 agent 显示名 = 该 agent 已启用 → 映射为 `enabled_agents`（仅 claude/codex）。
/// Project scope 在项目目录内执行（不带 `-g`）。
///
/// 返回 `(items, ok)`：`ok=true` = npx 成功且 JSON 解析成功（`items` 为真实列表，可能真空）；
/// `ok=false` = npx 失败 / HOME 缺失 / JSON 解析失败 / stdout 空（`items` 为空 vec，**不可写覆盖
/// 已有缓存**，避免假空缓存）。让调用方区分「真无 skill」vs「加载失败」。
///
/// 注：SWR 链路用 [`super::cache::list_cached`]（即时缓存）+ [`super::cache::list_refresh`]
/// （强制刷新）；内部聚合算子（`align_agents` / `enable_all`）仍走本函数取实时态（`ok` 忽略，
/// 仅用 `items`）。
pub fn list_installed(scope: &SkillScope, proxy_url: Option<&str>) -> (Vec<SkillInfo>, bool) {
    // HOME env 防御：home 无法解析（极罕见，launchd 极简 env）时 npx skills 必然漏 agent 检测
    // 甚至整体失败 → 不如显式失败信号，让上层保留旧缓存而非写空。
    if resolve_home_env().0.is_none() {
        tracing::warn!(
            "list_installed: HOME 无法解析（dirs::home_dir() 和 HOME env 均缺失），skills list 失败"
        );
        return (Vec::new(), false);
    }
    let mut args = vec!["list".to_string(), "--json".to_string()];
    apply_scope(&mut args, scope);
    let res = run_npx_in_scope(&args, scope, proxy_url);
    if !res.success {
        tracing::warn!(
            scope = ?scope,
            stderr = %truncate_str(&res.stderr, 200),
            "list_installed: npx skills list 失败"
        );
        return (Vec::new(), false);
    }
    // npx 成功但 stdout 空 = 异常（list --json 至少返 `[]`）→ 视为失败保守处理（防假空）。
    if res.stdout.trim().is_empty() {
        tracing::warn!(
            scope = ?scope,
            stderr = %truncate_str(&res.stderr, 200),
            "list_installed: npx 成功但 stdout 空，视为失败保守处理"
        );
        return (Vec::new(), false);
    }
    // JSON 解析失败（npx 成功但输出非 JSON）→ 同样视为失败保守（防假空缓存）。
    // parse_list_json 内部 from_str 失败即返空 vec，这里需在调用前先校验。
    if serde_json::from_str::<serde_json::Value>(res.stdout.trim()).is_err() {
        tracing::warn!(
            scope = ?scope,
            stdout_preview = %truncate_str(&res.stdout, 200),
            "list_installed: npx 成功但 stdout 非 JSON，视为失败保守处理"
        );
        return (Vec::new(), false);
    }
    let mut items = parse_list_json(&res.stdout, scope);
    enrich_with_sources(&mut items, scope);
    (items, true)
}

/// 截断字符串到 max_chars 字符（多出 `…`），用于日志 stderr 摘要。
fn truncate_str(s: &str, max_chars: usize) -> String {
    if s.chars().count() <= max_chars {
        return s.to_string();
    }
    let truncated: String = s.chars().take(max_chars).collect();
    format!("{truncated}…")
}

/// 解析 `npx skills list --json` 输出为 `Vec<SkillInfo>`。
/// 容错：接受裸数组或 `{ "skills": [...] }`；非法 JSON → 空 vec。
pub(super) fn parse_list_json(stdout: &str, scope: &SkillScope) -> Vec<SkillInfo> {
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
    let p = std::path::Path::new(skill_path).join("SKILL.md");
    let content = std::fs::read_to_string(p).ok()?;
    parse_skill_description_from_frontmatter(&content)
}

/// 锁文件路径：global → `~/.agents/.skill-lock.json`；project → `<path>/.agents/.skill-lock.json`。
/// home 不可解析（global）→ None。
fn lock_file_path(scope: &SkillScope) -> Option<std::path::PathBuf> {
    match scope {
        SkillScope::Global => dirs::home_dir().map(|h| h.join(".agents").join(".skill-lock.json")),
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
pub(super) fn parse_skill_sources_json(text: &str) -> HashMap<String, String> {
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
pub(super) fn enrich_with_sources(items: &mut [SkillInfo], scope: &SkillScope) {
    let sources = read_skill_sources(scope);
    if sources.is_empty() {
        return;
    }
    for s in items.iter_mut() {
        s.source = sources.get(&s.name).cloned();
    }
}

#[cfg(test)]
#[path = "test_list.rs"]
mod test_list;
