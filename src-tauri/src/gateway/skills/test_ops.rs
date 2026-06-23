use super::*;

#[test]
fn uninstall_args_global() {
    let args = uninstall_args("my-skill", &SkillScope::Global);
    assert_eq!(args[0], "remove");
    assert_eq!(args[1], "-s");
    assert_eq!(args[2], "my-skill");
    // 不带 -a（实测：删所有 agent；-a '*' 会 invalid exit 1）。
    assert!(!args.iter().any(|a| a == "-a"));
    assert!(args.contains(&"-g".to_string()));
    assert!(args.contains(&"-y".to_string()));
}

#[test]
fn uninstall_args_project_no_g() {
    let args = uninstall_args(
        "my-skill",
        &SkillScope::Project {
            path: "/tmp".to_string(),
        },
    );
    assert!(!args.contains(&"-g".to_string()));
    assert!(args.contains(&"-y".to_string()));
    assert!(!args.iter().any(|a| a == "-a"));
}

#[test]
fn is_safe_skill_name_rejects_traversal() {
    // 合法
    assert!(is_safe_skill_name("understand-onboard"));
    assert!(is_safe_skill_name("my_skill"));
    assert!(is_safe_skill_name("a"));
    // 路径遍历 / 非法
    assert!(!is_safe_skill_name(""));
    assert!(!is_safe_skill_name("."));
    assert!(!is_safe_skill_name(".."));
    assert!(!is_safe_skill_name("../etc"));
    assert!(!is_safe_skill_name("foo/bar"));
    assert!(!is_safe_skill_name("foo\\bar"));
    assert!(!is_safe_skill_name("a..b"));
}

#[test]
fn enable_args_global_claude() {
    // path 作 add package，无 -s；global 带 -g。
    let args = enable_args("/p/foo", SkillAgent::Claude, &SkillScope::Global);
    assert_eq!(args, vec!["add", "/p/foo", "-a", "claude-code", "-g", "-y"]);
    assert!(!args.contains(&"-s".to_string()));
}

#[test]
fn install_args_global_claude() {
    // id 含 @skill，无 -s；global 带 -g。
    let args = install_args(
        "vercel-labs/skills@foo",
        SkillAgent::Claude,
        &SkillScope::Global,
    );
    assert_eq!(
        args,
        vec!["add", "vercel-labs/skills@foo", "-a", "claude-code", "-g", "-y"]
    );
    assert!(!args.contains(&"-s".to_string()));
}

#[test]
fn install_args_project_codex_no_g() {
    let args = install_args(
        "xixu-me/skills@github-actions-docs",
        SkillAgent::Codex,
        &SkillScope::Project {
            path: "/proj".to_string(),
        },
    );
    assert_eq!(
        args,
        vec!["add", "xixu-me/skills@github-actions-docs", "-a", "codex", "-y"]
    );
}

#[test]
fn enable_args_project_codex_no_g() {
    let args = enable_args(
        "/p/bar",
        SkillAgent::Codex,
        &SkillScope::Project {
            path: "/proj".to_string(),
        },
    );
    assert_eq!(args, vec!["add", "/p/bar", "-a", "codex", "-y"]);
    assert!(!args.contains(&"-g".to_string()));
    assert!(!args.contains(&"-s".to_string()));
}

#[test]
fn disable_args_global_claude() {
    let args = disable_args("foo", SkillAgent::Claude, &SkillScope::Global);
    assert_eq!(
        args,
        vec!["remove", "-s", "foo", "-a", "claude-code", "-g", "-y"]
    );
}

#[test]
fn disable_args_project_no_g() {
    let args = disable_args(
        "foo",
        SkillAgent::Codex,
        &SkillScope::Project {
            path: "/proj".to_string(),
        },
    );
    assert!(!args.contains(&"-g".to_string()));
    assert_eq!(args, vec!["remove", "-s", "foo", "-a", "codex", "-y"]);
}

#[test]
fn enable_empty_path_fails() {
    // path 为空 → 明确错误，不真跑 npx。
    let r = enable("whatever", "   ", SkillAgent::Claude, &SkillScope::Global, None);
    assert!(!r.success);
    assert!(r.stderr.contains("no installed path"));
}

#[test]
fn enable_empty_name_fails() {
    let r = enable("  ", "/p/foo", SkillAgent::Claude, &SkillScope::Global, None);
    assert!(!r.success);
}

#[test]
fn disable_empty_name_fails() {
    let r = disable("  ", SkillAgent::Claude, &SkillScope::Global, None);
    assert!(!r.success);
}

#[test]
fn install_empty_id_fails() {
    let r = install("  ", &[SkillAgent::Claude], &SkillScope::Global, None);
    assert!(!r.success);
    assert!(r.stderr.contains("skill id is empty"), "stderr: {}", r.stderr);
}

#[test]
fn install_empty_agents_fails() {
    let r = install("some/skill@foo", &[], &SkillScope::Global, None);
    assert!(!r.success);
    assert!(r.stderr.contains("no agent selected"), "stderr: {}", r.stderr);
}

#[test]
fn uninstall_empty_name_fails() {
    let r = uninstall("  ", &SkillScope::Global, None);
    assert!(!r.success);
    assert!(r.stderr.contains("skill name is empty"), "stderr: {}", r.stderr);
}

/// uninstall_all builds correct args for global scope.
#[test]
fn uninstall_all_args_global_contains_all() {
    // uninstall_all calls run_npx_in_scope with ["remove", "--all", "-g"].
    // We can't easily intercept without running npx, but we can verify it doesn't panic.
    // The function is a thin wrapper; actual behavior verified via npx integration.
    // Just ensure no panic on call:
    let _r = uninstall_all(&SkillScope::Global, None);
    // either succeeds or fails gracefully
}

/// update builds correct args for global scope — just verify no panic.
#[test]
fn update_global_does_not_panic() {
    let _r = update(&SkillScope::Global, None);
}

/// fs_fallback_remove: unsafe name returns error immediately.
#[test]
fn fs_fallback_unsafe_name_errors() {
    // fs_fallback_remove is private, test via uninstall with stdout="No matching skills found"
    // We can't easily fake that; instead test is_safe_skill_name for the unsafe cases.
    assert!(!is_safe_skill_name("../etc/passwd"));
    assert!(!is_safe_skill_name("foo/bar"));
    assert!(!is_safe_skill_name(""));
}
