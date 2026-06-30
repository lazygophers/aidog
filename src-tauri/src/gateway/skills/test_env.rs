use super::*;
use std::process::Command;

fn env_of<'a>(cmd: &'a Command, key: &str) -> Option<&'a std::ffi::OsStr> {
    cmd.get_envs()
        .find(|(k, _)| *k == std::ffi::OsStr::new(key))
        .and_then(|(_, v)| v)
}

#[cfg(not(target_os = "windows"))]
#[test]
fn merge_path_dedups_preserves_order_and_priority() {
    // login 在前优先；rest 中重复项丢弃；空段丢弃；保序。
    assert_eq!(
        merge_path("/opt/homebrew/bin:/usr/bin", "/usr/bin:/bin::"),
        "/opt/homebrew/bin:/usr/bin:/bin"
    );
    // 完全相同 → 输出等于规范化后的同串（调用方据此判 no-op）。
    assert_eq!(merge_path("/usr/bin:/bin", "/usr/bin:/bin"), "/usr/bin:/bin");
    // 空 login → 退化为 rest 去重。
    assert_eq!(merge_path("", "/bin:/bin"), "/bin");
}

#[test]
fn resolve_home_env_returns_dirs_home() {
    let (home, _) = resolve_home_env();
    // 测试环境总有可解析的 home（dirs::home_dir 或 HOME env）。
    let expected = dirs::home_dir()
        .map(|h| h.to_string_lossy().into_owned())
        .or_else(|| std::env::var("HOME").ok().filter(|h| !h.is_empty()));
    assert_eq!(home, expected);
}

#[test]
fn apply_home_env_sets_home_on_command() {
    let mut cmd = Command::new("npx");
    apply_home_env(&mut cmd);
    let (home, _) = resolve_home_env();
    if let Some(h) = home {
        assert_eq!(env_of(&cmd, "HOME"), Some(std::ffi::OsStr::new(&h)));
    }
}

/// check_env exercises probe_env (OnceLock init path) and returns valid SkillsEnv.
/// Since node/npx are present in the test environment, npx_available should be true.
#[test]
fn check_env_does_not_panic_and_is_consistent() {
    let env1 = check_env();
    let env2 = check_env(); // second call returns cached value
    // Both calls should return the same values (OnceLock)
    assert_eq!(env1.npx_available, env2.npx_available);
    assert_eq!(env1.node_version, env2.node_version);
    // node_version, if present, should start with 'v'
    if let Some(ver) = &env1.node_version {
        assert!(ver.starts_with('v'), "node version should start with 'v': {ver}");
    }
}
