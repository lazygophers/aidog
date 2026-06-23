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

/// run_npx_in_scope: project path empty → returns error immediately (no spawn).
#[test]
fn run_npx_in_scope_empty_project_path_fails() {
    let args = vec!["--version".to_string()];
    let result = run_npx_in_scope(&args, &SkillScope::Project { path: "   ".to_string() }, None);
    assert!(!result.success);
    assert!(result.stderr.contains("project path is empty"), "stderr was: {}", result.stderr);
}

/// run_npx: runs npx with --version, should succeed on machines with npx installed.
/// If npx is missing, gracefully returns success=false.
#[test]
fn run_npx_version_does_not_panic() {
    let args = vec!["--version".to_string()];
    // We use a known-safe arg; either succeeds (npx present) or fails gracefully.
    let result = run_npx(&args, None);
    // Either outcome is acceptable — just must not panic.
    let _ = result;
}

/// run_npx_in_scope global delegates to run_npx.
#[test]
fn run_npx_in_scope_global_delegates() {
    let args = vec!["--version".to_string()];
    let result = run_npx_in_scope(&args, &SkillScope::Global, None);
    let _ = result; // Must not panic.
}

/// run_npx_in_scope project with valid path attempts spawn.
#[test]
fn run_npx_in_scope_project_valid_path_attempts() {
    let args = vec!["--version".to_string()];
    let result = run_npx_in_scope(
        &args,
        &SkillScope::Project { path: "/tmp".to_string() },
        None,
    );
    // Either succeeds or fails gracefully — must not panic.
    let _ = result;
}
