//! 组级 / 批量写操作：set_group_agent / uninstall_group / align_agents / enable_all。

use super::cache::invalidate;
use super::list::list_installed;
use super::ops::{disable, enable, uninstall};
use super::types::{SkillAgent, SkillInfo, SkillScope, SkillsOpResult};

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

/// 组级卸载：对某 source 组（`group_source=None` = 「其他」组）内所有 skill 逐个卸载。
/// 复用 `uninstall`（含 npx remove + fs 兜底删第三方 symlink）。完成后 invalidate(scope)。
/// 返回汇总（stdout "ok/total skipped failed"，stderr 聚合失败明细）。
pub fn uninstall_group(
    group_source: Option<&str>,
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
    let mut fail: u32 = 0;
    let mut errs: Vec<String> = Vec::new();
    for s in &targets {
        let res = uninstall(&s.name, scope, proxy_url);
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
        stdout: format!("{}/{} uninstalled, {} failed", ok, targets.len(), fail),
        stderr: errs.join("\n"),
    }
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
