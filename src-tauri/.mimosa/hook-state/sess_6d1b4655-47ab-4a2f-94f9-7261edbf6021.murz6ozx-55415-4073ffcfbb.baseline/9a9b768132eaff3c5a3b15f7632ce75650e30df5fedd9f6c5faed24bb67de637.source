use super::*;
use crate::types::SkillAgent;
use tempfile::tempdir;

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
    let r = align_agents(
        SkillAgent::Claude,
        SkillAgent::Claude,
        &SkillScope::Global,
        None,
    );
    assert!(r.success);
    assert_eq!(r.stdout, "noop: source equals target");
}

/// 构造 Project scope 指向隔离 tempdir，锁文件与 npx 操作落 tempdir/.agents，
/// 不碰用户 `~/.agents/.skill-lock.json`（Global scope 走 HOME，操作真实环境）。
///
/// 隔离机制：
/// - `SkillScope::Project { path }` → `lock_file_path` 返 `<path>/.agents/.skill-lock.json`（不读 HOME）
/// - `run_npx_in_scope` 对 Project scope 设 `.current_dir(path)`，npx skills 无 `-g` → 写项目内 .agents
/// - `tempfile::tempdir()` + `keep()`：消费 TempDir 句柄为普通 PathBuf，
///   目录保留到测试结束（由系统 tmp 目录清理）；多线程安全（无全局 env 写）
fn isolated_project_scope() -> std::path::PathBuf {
    let dir = tempdir().expect("tempdir creation failed");
    // keep() 消费 TempDir → 目录不再随 Drop 删除，保留到测试进程结束。
    // 测试用例数有限（2 个），残留可接受（系统 tmp 周期清理）；优先保证隔离正确性。
    let path = dir.keep();
    std::fs::create_dir_all(path.join(".agents")).ok();
    path
}

/// align_agents: from != to — does not panic, returns result.
///
/// **隔离**：原测试用 `SkillScope::Global` 真实 shell out `npx skills enable/disable`
/// 操作用户 `~/.agents/.skill-lock.json`（全局副作用，违反测试隔离原则）。
/// 改用 Project scope + tempdir，npx 在临时项目目录内执行，锁文件落 tempdir/.agents，
/// 用户全局环境零影响。仍真实跑 npx + list_installed + enable/disable，覆盖 align 逻辑。
#[test]
fn align_agents_different_agents_does_not_panic() {
    let path = isolated_project_scope();
    let scope = SkillScope::Project {
        path: path.to_string_lossy().into_owned(),
    };
    let r = align_agents(SkillAgent::Claude, SkillAgent::Codex, &scope, None);
    // Project scope 在空 tempdir 内 list_installed 必返空（无 .agents/.skill-lock.json 或为空），
    // 无 skill 需对齐 → success=true + "aligned 0 changes"。但即便 npx 返非空或失败也不应 panic。
    let _ = r;
    // 关键不变量：用户全局 ~/.agents 不受影响（Project scope 隔离），由 cargo test 前后 mtime 检查验证。
}

/// enable_all: does not panic, returns result.
///
/// **隔离**：同 align_agents_different_agents_does_not_panic，改用 Project scope + tempdir，
/// 不操作用户 `~/.agents`。仍覆盖 enable_all 逐 skill 启用逻辑（在空 project 上 noop）。
#[test]
fn enable_all_does_not_panic() {
    let path = isolated_project_scope();
    let scope = SkillScope::Project {
        path: path.to_string_lossy().into_owned(),
    };
    let r = enable_all(SkillAgent::Claude, &scope, None);
    let _ = r;
}

// ─── install_batch / uninstall_batch（纯 args 断言，不真跑 npx）───

#[test]
fn group_ids_by_repo_groups_and_keeps_bare_repo() {
    let groups = group_ids_by_repo(&[
        "a/b@s1".into(),
        "a/b@s2".into(),
        "c/d@s3".into(),
        "e/f".into(),
        "  ".into(),
    ]);
    assert_eq!(groups.len(), 3);
    assert_eq!(groups["a/b"], vec!["s1".to_string(), "s2".to_string()]);
    assert_eq!(groups["c/d"], vec!["s3".to_string()]);
    assert!(groups["e/f"].is_empty()); // 裸 repo → 装整仓库
}

#[test]
fn install_batch_args_merges_skills_and_agents_global() {
    let args = install_batch_args(
        "a/b",
        &["s1".to_string(), "s2".to_string()],
        &[SkillAgent::Claude, SkillAgent::Codex],
        &SkillScope::Global,
    );
    assert_eq!(
        args,
        vec![
            "add",
            "a/b",
            "-a",
            "claude-code",
            "codex",
            "-s",
            "s1",
            "s2",
            "-g",
            "-y",
        ]
    );
}

#[test]
fn install_batch_args_bare_repo_no_s_flag() {
    let args = install_batch_args("e/f", &[], &[SkillAgent::Claude], &SkillScope::Global);
    assert_eq!(args, vec!["add", "e/f", "-a", "claude-code", "-g", "-y"]);
}

#[test]
fn install_batch_empty_inputs_error() {
    let r = install_batch(&[], &[SkillAgent::Claude], &SkillScope::Global, None);
    assert!(!r.success);
    assert_eq!(r.stderr, "no skill ids provided");
    let r = install_batch(&["a/b@s".into()], &[], &SkillScope::Global, None);
    assert!(!r.success);
    assert_eq!(r.stderr, "no agent selected");
}

#[test]
fn uninstall_batch_args_names_then_scope() {
    let args = uninstall_batch_args(
        &["x".into(), "y".into()],
        &SkillScope::Project {
            path: "/tmp/p".into(),
        },
    );
    assert_eq!(args, vec!["remove", "x", "y", "-y"]);
    let args = uninstall_batch_args(&["x".into()], &SkillScope::Global);
    assert_eq!(args, vec!["remove", "x", "-g", "-y"]);
}

#[test]
fn uninstall_batch_empty_names_error() {
    let r = uninstall_batch(&["  ".into()], &SkillScope::Global, None);
    assert!(!r.success);
    assert_eq!(r.stderr, "no skill names provided");
}
