//! 批量写操作：align_agents / enable_all。

use super::list::list_installed;
use super::ops::{disable, enable};
use super::types::{SkillAgent, SkillScope, SkillsOpResult};

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
        stdout: format!("aligned {total} changes ({enabled_n} enabled, {disabled_n} disabled)"),
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

#[cfg(test)]
#[path = "test_bulk.rs"]
mod test_bulk;
