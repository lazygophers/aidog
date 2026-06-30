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

/// run_npx_in_scope Global + 变更类命令 在测试中必须 panic — 测试硬拦防删用户 ~/.agents。
/// 见 npx.rs::run_npx_in_scope 的 #[cfg(test)] 守卫注释。
/// 历史教训: test_ops.rs:160 uninstall_all(&Global) 真实 npx skills remove --all -g
/// 删空用户 ~/.agents。Global scope + 变更命令 测试必须 panic 而非 spawn。
#[test]
#[should_panic(expected = "run_npx_in_scope(Global, 变更类命令) 在测试中被调用")]
fn run_npx_in_scope_global_mutating_panics_in_test() {
    // remove --all 是历史删空 ~/.agents 的真凶；任何 mutating verb + Global 都应 panic。
    let args = vec!["remove".to_string(), "--all".to_string()];
    let _ = run_npx_in_scope(&args, &SkillScope::Global, None);
}

/// run_npx_in_scope Global + 只读命令（list）不 panic — 验证拦截器不误伤合法只读路径。
/// export_skills() 生产路径经 list_installed → run_npx_in_scope(Global, list) 被测试调用。
#[test]
fn run_npx_in_scope_global_readonly_does_not_panic() {
    let args = vec!["list".to_string()];
    let result = run_npx_in_scope(&args, &SkillScope::Global, None);
    // 只读 list 合法 (即便读用户 ~/.agents 也无害); 成功/失败均可, 关键不 panic。
    let _ = result;
}

/// run_npx_in_scope Project scope（合法 tempdir）+ 变更命令 不 panic — Project 隔离安全。
/// `#[ignore]`：真 spawn npx 二进制，依赖宿主装 npx；默认跳过，仅 `--ignored` 显式跑。
#[test]
#[ignore = "needs host npx"]
fn run_npx_in_scope_project_mutating_does_not_panic() {
    let args = vec!["add".to_string(), "owner/repo".to_string()];
    let temp = tempfile::tempdir().expect("tempdir");
    let result = run_npx_in_scope(
        &args,
        &SkillScope::Project {
            path: temp.path().to_string_lossy().to_string(),
        },
        None,
    );
    // Project scope 合法隔离, 不 panic; npx add 应成功或失败, 均可接受。
    let _ = result;
}

/// run_npx_in_scope Project scope（合法 tempdir）+ 只读命令 不 panic。
/// `#[ignore]`：真 spawn npx 二进制（即便只读 --version），依赖宿主装 npx；默认跳过。
#[test]
#[ignore = "needs host npx"]
fn run_npx_in_scope_project_readonly_does_not_panic() {
    let args = vec!["--version".to_string()];
    let temp = tempfile::tempdir().expect("tempdir");
    let result = run_npx_in_scope(
        &args,
        &SkillScope::Project {
            path: temp.path().to_string_lossy().to_string(),
        },
        None,
    );
    let _ = result;
}

/// run_npx 含 -g + 变更类命令 在测试中必须 panic — 测试硬拦防删用户 ~/.agents。
/// 见 npx.rs::run_npx 的 #[cfg(test)] 守卫。
#[test]
#[should_panic(expected = "run_npx(args 含 -g + 变更类命令) 在测试中被调用")]
fn run_npx_with_global_flag_mutating_panics_in_test() {
    let args = vec!["remove".to_string(), "--all".to_string(), "-g".to_string()];
    let _ = run_npx(&args, None);
}

/// run_npx 不含 -g args 不 panic — 验证拦截器只针对 -g + 变更命令。
#[test]
fn run_npx_without_global_flag_does_not_panic() {
    let args = vec!["--version".to_string()];
    let result = run_npx(&args, None);
    let _ = result;
}

/// run_npx 含 -g + 只读命令 不 panic — 验证不误伤只读路径。
#[test]
fn run_npx_with_global_flag_readonly_does_not_panic() {
    let args = vec!["list".to_string(), "-g".to_string()];
    let result = run_npx(&args, None);
    let _ = result;
}

/// run_npx_in_scope project with valid path attempts spawn.
/// `#[ignore]`：真 spawn npx 二进制，依赖宿主装 npx；默认跳过，仅 `--ignored` 显式跑。
#[test]
#[ignore = "needs host npx"]
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
