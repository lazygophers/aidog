use super::*;
use std::process::Command;

fn env_of<'a>(cmd: &'a Command, key: &str) -> Option<&'a std::ffi::OsStr> {
    cmd.get_envs()
        .find(|(k, _)| *k == std::ffi::OsStr::new(key))
        .and_then(|(_, v)| v)
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
