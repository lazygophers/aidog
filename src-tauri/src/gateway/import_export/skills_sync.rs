//! Skills 自动化导入导出。
//!
//! 导出：从 `npx skills list --json` 收集每条 skill 的 source + per-agent enable +
//! scope，序列化为 [`SkillExportEntry`]。
//!
//! 导入：对每条执行 `npx skills add <source> -s <name> -a <slug> [-g] -y`（每个
//! enabled agent 一次），再对原本 disabled 的 agent 调 `remove`，**安装权限
//! （scope + agent 列表 + enable 状态）与原完全一致**。

use serde::{Deserialize, Serialize};

use super::ImportReport;
use crate::gateway::skills::{self, SkillAgent, SkillScope};

/// 单条 skill 导出条目。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillExportEntry {
    pub name: String,
    /// owner/repo 来源。
    pub source: String,
    /// 安装 scope（序列化 `SkillScope`）。
    pub scope: SkillScope,
    /// 每个 agent 的目标 enable 状态。
    pub agents: Vec<AgentState>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentState {
    /// agent 显示名（"Claude Code" / "Codex"）。
    pub display: String,
    /// 是否启用。
    pub enabled: bool,
}

/// 导出当前 Global scope 下全部已装 skills。
///
/// 注：仅导出 Global scope（用户级）；Project scope skills 跨机迁移意义不大
/// （绑定特定项目路径），留待后续按需扩展。
pub fn export_skills() -> Vec<SkillExportEntry> {
    let scope = SkillScope::Global;
    let installed = skills::list_installed(&scope);
    installed
        .into_iter()
        .filter_map(|info| {
            // skills 重构后（3525117）以 installed_path 作 npx add package，不再有独立 source。
            let source = info.installed_path?;
            if source.is_empty() {
                return None;
            }
            let agents = info
                .enabled_agents
                .iter()
                .map(|a| AgentState {
                    display: a_display(*a).to_string(),
                    enabled: true,
                })
                .collect::<Vec<_>>();
            // 补齐未启用的 agent（记录为 enabled=false，导入时保持一致）。
            let mut agents_full = agents;
            for a in all_agents() {
                if !agents_full.iter().any(|x| x.display == a_display(a)) {
                    agents_full.push(AgentState {
                        display: a_display(a).to_string(),
                        enabled: false,
                    });
                }
            }
            Some(SkillExportEntry {
                name: info.name,
                source,
                scope: scope.clone(),
                agents: agents_full,
            })
        })
        .collect()
}

/// 导入：对每条 skill 按 enable 状态执行 npx add/remove。
/// 单条失败收集到 report.errors，不阻塞其他条目。
pub fn import_skills(entries: &[SkillExportEntry], report: &mut ImportReport) {
    for entry in entries {
        let scope = &entry.scope;
        for agent in &entry.agents {
            let Some(agent_enum) = parse_agent(&agent.display) else {
                report.errors.push(format!(
                    "skill「{}」unknown agent「{}」",
                    entry.name, agent.display
                ));
                continue;
            };
            if agent.enabled {
                // add（安装 + 启用）：用导出的 source，不经锁文件。
                let args = build_add_args(&entry.name, &entry.source, agent_enum, scope);
                let res = run_npx(&args, scope);
                if res.success {
                    bump(report, super::SCOPE_SKILLS);
                } else {
                    report.errors.push(format!(
                        "skill「{}」add for {}: {}",
                        entry.name,
                        agent.display,
                        res.stderr.trim()
                    ));
                }
            } else {
                // 确保未启用：remove（幂等，未装则 no-op）。
                let args = build_remove_args(&entry.name, agent_enum, scope);
                let _ = run_npx(&args, scope);
            }
        }
    }
}

fn build_add_args(
    name: &str,
    source: &str,
    agent: SkillAgent,
    scope: &SkillScope,
) -> Vec<String> {
    let mut args = vec![
        "add".to_string(),
        source.to_string(),
        "-s".to_string(),
        name.to_string(),
        "-a".to_string(),
        agent_slug(agent).to_string(),
    ];
    if matches!(scope, SkillScope::Global) {
        args.push("-g".to_string());
    }
    args.push("-y".to_string());
    args
}

fn build_remove_args(name: &str, agent: SkillAgent, scope: &SkillScope) -> Vec<String> {
    let mut args = vec![
        "remove".to_string(),
        "-s".to_string(),
        name.to_string(),
        "-a".to_string(),
        agent_slug(agent).to_string(),
    ];
    if matches!(scope, SkillScope::Global) {
        args.push("-g".to_string());
    }
    args.push("-y".to_string());
    args
}

fn run_npx(args: &[String], scope: &SkillScope) -> skills::SkillsOpResult {
    let mut full: Vec<String> = vec!["--yes".to_string(), "skills".to_string()];
    full.extend(args.iter().cloned());
    let mut cmd = std::process::Command::new("npx");
    cmd.args(&full);
    if let SkillScope::Project { path } = scope {
        if !path.trim().is_empty() {
            cmd.current_dir(path);
        }
    }
    match cmd.output() {
        Ok(o) => skills::SkillsOpResult {
            success: o.status.success(),
            stdout: String::from_utf8_lossy(&o.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&o.stderr).into_owned(),
        },
        Err(e) => skills::SkillsOpResult {
            success: false,
            stdout: String::new(),
            stderr: format!("spawn npx: {e}"),
        },
    }
}

fn all_agents() -> [SkillAgent; 2] {
    [SkillAgent::Claude, SkillAgent::Codex]
}

fn agent_slug(a: SkillAgent) -> &'static str {
    match a {
        SkillAgent::Claude => "claude-code",
        SkillAgent::Codex => "codex",
    }
}

fn a_display(a: SkillAgent) -> &'static str {
    match a {
        SkillAgent::Claude => "Claude Code",
        SkillAgent::Codex => "Codex",
    }
}

fn parse_agent(display: &str) -> Option<SkillAgent> {
    match display {
        "Claude Code" => Some(SkillAgent::Claude),
        "Codex" => Some(SkillAgent::Codex),
        _ => None,
    }
}

fn bump(report: &mut ImportReport, scope: &str) {
    *report.applied.entry(scope.to_string()).or_insert(0) += 1;
}
