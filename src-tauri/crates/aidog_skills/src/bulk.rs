//! 批量写操作：align_agents / enable_all / install_batch / uninstall_batch。

use super::list::list_installed;
use super::npx::{apply_scope, run_npx_in_scope};
use super::ops::{disable, enable};
use super::types::{SkillAgent, SkillScope, SkillsOpResult};
use std::collections::BTreeMap;

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
    // `list_installed` 新签名返 (items, ok)；align 取实时态忽略失败信号（ok=false 时 items 空等价 noop）。
    let (skills, _ok) = list_installed(scope, proxy_url);
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
        stdout: format!("aligned {total} changes ({enabled_n} enabled, {disabled_n} disabled)"),
        stderr: errs.join("; "),
    }
}

/// 为某 agent 启用当前 scope 下全部已装 skills（只增不减，非破坏性）。
/// 逐 skill：agent 未启用则 `enable()`，已启用跳过。
pub fn enable_all(
    agent: SkillAgent,
    scope: &SkillScope,
    proxy_url: Option<&str>,
) -> SkillsOpResult {
    // `list_installed` 新签名返 (items, ok)；enable_all 取实时态忽略失败信号（ok=false 时 items 空等价 noop）。
    let (skills, _ok) = list_installed(scope, proxy_url);
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
            errs.push(format!(
                "enable {} on {}: {}",
                s.name,
                agent.cli_slug(),
                r.stderr.trim()
            ));
        }
    }
    SkillsOpResult {
        success: errs.is_empty(),
        stdout: format!("enabled {enabled_n} skills"),
        stderr: errs.join("; "),
    }
}

/// 按 repo 分组 ids（`owner/repo@skill` → repo → skills），保序（BTreeMap key 排序）。
/// 含裸 repo（无 `@`）→ 该组 skills 为空 = 装整仓库。
pub(super) fn group_ids_by_repo(ids: &[String]) -> BTreeMap<String, Vec<String>> {
    let mut groups: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for id in ids {
        let id = id.trim();
        if id.is_empty() {
            continue;
        }
        match id.rsplit_once('@') {
            Some((repo, skill)) if !repo.is_empty() && !skill.is_empty() => {
                groups
                    .entry(repo.to_string())
                    .or_default()
                    .push(skill.to_string());
            }
            _ => {
                groups.entry(id.to_string()).or_default();
            }
        }
    }
    groups
}

/// 构造单组批量安装 args：`add <repo> -a <a1> <a2> -s <s1> <s2> [-g] -y`。
/// skills 为空（裸 repo）→ 无 `-s`（装整仓库）。抽出便于单测断言（不真跑 npx）。
pub(super) fn install_batch_args(
    repo: &str,
    skills: &[String],
    agents: &[SkillAgent],
    scope: &SkillScope,
) -> Vec<String> {
    let mut args = vec!["add".to_string(), repo.to_string(), "-a".to_string()];
    args.extend(agents.iter().map(|a| a.cli_slug().to_string()));
    if !skills.is_empty() {
        args.push("-s".to_string());
        args.extend(skills.iter().cloned());
    }
    apply_scope(&mut args, scope);
    args.push("-y".to_string());
    args
}

/// 批量安装：ids（`owner/repo@skill`）按 repo 分组，**同仓库合并一次 npx 调用**
/// （`add <repo> -a <agents...> -s <skills...> [-g] -y`，CLI 原生支持多 agent / 多 skill），
/// 跨仓库逐组调用。任一组失败 → success=false，stderr 聚合各组失败明细。
/// 成功后调用方负责 `invalidate(scope)`。
pub fn install_batch(
    ids: &[String],
    agents: &[SkillAgent],
    scope: &SkillScope,
    proxy_url: Option<&str>,
) -> SkillsOpResult {
    if ids.is_empty() {
        return SkillsOpResult {
            success: false,
            stdout: String::new(),
            stderr: "no skill ids provided".to_string(),
        };
    }
    if agents.is_empty() {
        return SkillsOpResult {
            success: false,
            stdout: String::new(),
            stderr: "no agent selected".to_string(),
        };
    }
    let groups = group_ids_by_repo(ids);
    let mut success = true;
    let mut stdout = String::new();
    let mut stderr = String::new();
    for (repo, skills) in &groups {
        let args = install_batch_args(repo, skills, agents, scope);
        let res = run_npx_in_scope(&args, scope, proxy_url);
        if !res.success {
            success = false;
            if !stderr.is_empty() {
                stderr.push_str("\n---\n");
            }
            stderr.push_str(&format!("[{repo}] {}", res.stderr.trim()));
        }
        if !res.stdout.trim().is_empty() {
            if !stdout.is_empty() {
                stdout.push_str("\n---\n");
            }
            stdout.push_str(&format!("[{repo}] {}", res.stdout.trim()));
        }
    }
    SkillsOpResult {
        success,
        stdout,
        stderr,
    }
}

/// 构造批量卸载 args：`remove <n1> <n2> [-g] -y`（CLI 原生支持多 skill 位置参数）。
pub(super) fn uninstall_batch_args(names: &[String], scope: &SkillScope) -> Vec<String> {
    let mut args = vec!["remove".to_string()];
    args.extend(names.iter().map(|n| n.trim().to_string()));
    apply_scope(&mut args, scope);
    args.push("-y".to_string());
    args
}

/// 批量卸载（破坏性，前端二次确认）：`remove <names...> [-g] -y` 一次 npx 调用，
/// 删规范存储 + 所有 agent 启用配置。names 为空 → 错误。
pub fn uninstall_batch(
    names: &[String],
    scope: &SkillScope,
    proxy_url: Option<&str>,
) -> SkillsOpResult {
    let names: Vec<String> = names
        .iter()
        .map(|n| n.trim().to_string())
        .filter(|n| !n.is_empty())
        .collect();
    if names.is_empty() {
        return SkillsOpResult {
            success: false,
            stdout: String::new(),
            stderr: "no skill names provided".to_string(),
        };
    }
    let args = uninstall_batch_args(&names, scope);
    tracing::warn!(
        names = ?names,
        scope = ?scope,
        args = ?args,
        trigger = "skills_uninstall_batch",
        "物理删除 skills：npx skills remove <names...>（真物理删：规范存储 + 所有 agent symlink，不可恢复）"
    );
    run_npx_in_scope(&args, scope, proxy_url)
}

#[cfg(test)]
#[path = "test_bulk.rs"]
mod test_bulk;
