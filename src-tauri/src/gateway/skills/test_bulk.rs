use super::*;
use crate::gateway::skills::types::SkillAgent;

#[test]
fn plan_align_action_matrix() {
    assert_eq!(plan_align_action(true, false), AlignAction::Enable);
    assert_eq!(plan_align_action(false, true), AlignAction::Disable);
    assert_eq!(plan_align_action(true, true), AlignAction::Keep);
    assert_eq!(plan_align_action(false, false), AlignAction::Keep);
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
