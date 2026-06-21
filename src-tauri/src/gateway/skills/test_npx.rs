use super::*;
use crate::gateway::skills::types::SkillScope;

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
