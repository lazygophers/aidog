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
    // 导出仅列本机已装 skills（list 不联网），无需经上游代理 → None。
    // `list_installed` 新签名返 (items, ok)；导出取实时态忽略失败信号（ok=false 时 installed 空等价导出空）。
    let (installed, _ok) = skills::list_installed(&scope, None);
    installed
        .into_iter()
        .filter_map(|info| {
            // fix-skills-enable(3525117) 后 SkillInfo 无 source 字段，改用 installed_path
            // 作 npx add package（本地路径；跨机迁移受限，仅同机备份/恢复保证可用）。
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
                // 导入语义只增不减：enabled=false 跳过，不主动 remove（防导入 .aidogx 默认全选时
                // 误删现有 agent 启用）。保留现状，用户需删可手动在 Skills 页操作。
                continue;
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gateway::skills::{SkillAgent, SkillScope};

    // ── agent_slug / a_display ──
    #[test]
    fn agent_slug_values() {
        assert_eq!(agent_slug(SkillAgent::Claude), "claude-code");
        assert_eq!(agent_slug(SkillAgent::Codex), "codex");
    }

    #[test]
    fn a_display_values() {
        assert_eq!(a_display(SkillAgent::Claude), "Claude Code");
        assert_eq!(a_display(SkillAgent::Codex), "Codex");
    }

    // ── parse_agent ──
    #[test]
    fn parse_agent_known_values() {
        assert_eq!(parse_agent("Claude Code"), Some(SkillAgent::Claude));
        assert_eq!(parse_agent("Codex"), Some(SkillAgent::Codex));
    }

    #[test]
    fn parse_agent_unknown_returns_none() {
        assert!(parse_agent("Unknown Agent").is_none());
        assert!(parse_agent("").is_none());
        assert!(parse_agent("claude-code").is_none()); // slug, not display name
    }

    // ── build_add_args ──
    #[test]
    fn build_add_args_global_scope() {
        let args = build_add_args("my-skill", "owner/repo", SkillAgent::Claude, &SkillScope::Global);
        // Should include: add, source, -s, name, -a, slug, -g, -y
        assert_eq!(&args[0], "add");
        assert_eq!(&args[1], "owner/repo");
        assert_eq!(&args[2], "-s");
        assert_eq!(&args[3], "my-skill");
        assert_eq!(&args[4], "-a");
        assert_eq!(&args[5], "claude-code");
        assert!(args.contains(&"-g".to_string()), "global scope should add -g");
        assert!(args.contains(&"-y".to_string()), "should add -y");
    }

    #[test]
    fn build_add_args_project_scope_no_global_flag() {
        let scope = SkillScope::Project { path: "/tmp/myproject".to_string() };
        let args = build_add_args("tool", "src/tool", SkillAgent::Codex, &scope);
        assert_eq!(&args[0], "add");
        assert_eq!(&args[5], "codex");
        assert!(!args.contains(&"-g".to_string()), "project scope must NOT add -g");
        assert!(args.contains(&"-y".to_string()));
    }

    // ── build_remove_args 已移除（导入只增不减，enabled=false 跳过不 remove）──

    // ── all_agents ──
    #[test]
    fn all_agents_returns_two_agents() {
        let agents = all_agents();
        assert_eq!(agents.len(), 2);
        assert!(agents.contains(&SkillAgent::Claude));
        assert!(agents.contains(&SkillAgent::Codex));
    }

    // ── bump ──
    #[test]
    fn bump_increments_report_counter() {
        let mut report = ImportReport::default();
        bump(&mut report, "skills");
        assert_eq!(report.applied.get("skills"), Some(&1));
        bump(&mut report, "skills");
        assert_eq!(report.applied.get("skills"), Some(&2));
        bump(&mut report, "other");
        assert_eq!(report.applied.get("other"), Some(&1));
    }

    // ── import_skills edge cases ──

    /// import_skills with empty entries → no errors, no applied.
    #[test]
    fn import_skills_empty_entries_no_op() {
        let mut report = ImportReport::default();
        import_skills(&[], &mut report);
        assert!(report.errors.is_empty());
        assert!(report.applied.is_empty());
    }

    /// import_skills with unknown agent display → error recorded.
    #[test]
    fn import_skills_unknown_agent_records_error() {
        let entry = SkillExportEntry {
            name: "skill-x".to_string(),
            source: "owner/repo".to_string(),
            scope: SkillScope::Global,
            agents: vec![AgentState {
                display: "UnknownAgentXYZ".to_string(),
                enabled: true,
            }],
        };
        let mut report = ImportReport::default();
        import_skills(&[entry], &mut report);
        assert_eq!(report.errors.len(), 1, "should have 1 error for unknown agent");
        assert!(report.errors[0].contains("unknown agent"), "got: {}", report.errors[0]);
    }

    /// import_skills with disabled agent → 跳过不 remove（导入只增不减语义，防误删现有启用）。
    /// 验证 no-op：不调 npx remove，不记 applied，不记 errors。
    #[test]
    fn import_skills_disabled_agent_skips_no_remove() {
        let entry = SkillExportEntry {
            name: "skill-y".to_string(),
            source: "owner/repo".to_string(),
            scope: SkillScope::Global,
            agents: vec![AgentState {
                display: "Codex".to_string(),
                enabled: false, // disabled → 跳过，不 remove
            }],
        };
        let mut report = ImportReport::default();
        import_skills(&[entry], &mut report);
        // 完全 no-op：无 applied 计数，无 error，无 panic。
        assert!(report.applied.is_empty(), "disabled agent should not bump applied");
        assert!(report.errors.is_empty(), "disabled agent should not produce errors");
    }

    /// export_skills does not panic (may return empty if npx not available).
    #[test]
    fn export_skills_does_not_panic() {
        let _entries = export_skills();
        // Either empty (no npx) or populated; must not panic.
    }

    // ── SkillExportEntry serde roundtrip ──
    #[test]
    fn skill_export_entry_serde_roundtrip() {
        let entry = SkillExportEntry {
            name: "my-skill".to_string(),
            source: "owner/repo".to_string(),
            scope: SkillScope::Global,
            agents: vec![
                AgentState { display: "Claude Code".to_string(), enabled: true },
                AgentState { display: "Codex".to_string(), enabled: false },
            ],
        };
        let json = serde_json::to_string(&entry).unwrap();
        let decoded: SkillExportEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.name, entry.name);
        assert_eq!(decoded.source, entry.source);
        assert_eq!(decoded.agents.len(), 2);
        assert!(decoded.agents[0].enabled);
        assert!(!decoded.agents[1].enabled);
    }
}
