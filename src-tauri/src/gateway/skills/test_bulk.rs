use super::*;
use crate::gateway::skills::types::SkillAgent;

#[test]
fn plan_align_action_matrix() {
    assert_eq!(plan_align_action(true, false), AlignAction::Enable);
    assert_eq!(plan_align_action(false, true), AlignAction::Disable);
    assert_eq!(plan_align_action(true, true), AlignAction::Keep);
    assert_eq!(plan_align_action(false, false), AlignAction::Keep);
}

/// set_group_agent: does not panic, returns a result.
#[test]
fn set_group_agent_source_does_not_panic() {
    // list_installed may return empty or real skills; either way it must not panic.
    let r = set_group_agent(Some("nonexistent-source-xyz"), SkillAgent::Claude, true, &SkillScope::Global, None);
    // No skills match "nonexistent-source-xyz" → "no skills in group".
    assert!(r.success, "empty group should succeed: {}", r.stdout);
    assert_eq!(r.stdout, "no skills in group");
}

/// uninstall_group: nonexistent source → no skills in group.
#[test]
fn uninstall_group_nonexistent_source_no_skills() {
    let r = uninstall_group(Some("nonexistent-source-xyz"), &SkillScope::Global, None);
    assert!(r.success);
    assert_eq!(r.stdout, "no skills in group");
}

/// align_agents: from == to → noop immediately.
#[test]
fn align_agents_same_agent_noop() {
    let r = align_agents(SkillAgent::Claude, SkillAgent::Claude, &SkillScope::Global, None);
    assert!(r.success);
    assert_eq!(r.stdout, "noop: source equals target");
}

/// align_agents: from != to — does not panic, returns result.
#[test]
fn align_agents_different_agents_does_not_panic() {
    let r = align_agents(SkillAgent::Claude, SkillAgent::Codex, &SkillScope::Global, None);
    // May succeed or fail depending on installed skills; must not panic.
    let _ = r;
}

/// enable_all: does not panic, returns result.
#[test]
fn enable_all_does_not_panic() {
    let r = enable_all(SkillAgent::Claude, &SkillScope::Global, None);
    // May succeed or fail depending on installed skills; must not panic.
    let _ = r;
}

/// set_group_agent group_source=None (「其他」组): does not panic.
#[test]
fn set_group_agent_none_source_does_not_panic() {
    let r = set_group_agent(None, SkillAgent::Codex, false, &SkillScope::Global, None);
    // May or may not have skills with source=None; must not panic.
    let _ = r;
}
