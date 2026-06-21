//! `npx skills <args>` 执行封装：scope→cwd 路由、`-g` 追加、子进程 spawn。

use super::env::apply_home_env;
use super::proxy_env::apply_proxy_env;
use super::types::{SkillScope, SkillsOpResult};
use std::process::Command;

/// 按 scope 追加 `-g`（仅 Global）。
pub(super) fn apply_scope(args: &mut Vec<String>, scope: &SkillScope) {
    if matches!(scope, SkillScope::Global) {
        args.push("-g".to_string());
    }
}

/// 封装 `npx skills <args...>`，捕获 stdout/stderr/退出码。
/// `proxy_url` 为 `Some` 时注入代理 env（见 `apply_proxy_env`），`None` 直连。
pub(super) fn run_npx(extra_args: &[String], proxy_url: Option<&str>) -> SkillsOpResult {
    let mut args: Vec<String> = vec!["--yes".to_string(), "skills".to_string()];
    args.extend(extra_args.iter().cloned());
    let mut cmd = Command::new("npx");
    cmd.args(&args);
    apply_home_env(&mut cmd);
    apply_proxy_env(&mut cmd, proxy_url);
    match cmd.output() {
        Ok(o) => SkillsOpResult {
            success: o.status.success(),
            stdout: String::from_utf8_lossy(&o.stdout).to_string(),
            stderr: String::from_utf8_lossy(&o.stderr).to_string(),
        },
        Err(e) => SkillsOpResult {
            success: false,
            stdout: String::new(),
            stderr: format!("failed to spawn npx: {e}"),
        },
    }
}

/// 在 scope 对应的 cwd 执行 npx：Project → 项目目录；Global → 默认 cwd。
/// `proxy_url` 为 `Some` 时给 npx 子进程注入代理 env（突破网络限制），`None` 直连。
pub(super) fn run_npx_in_scope(
    extra_args: &[String],
    scope: &SkillScope,
    proxy_url: Option<&str>,
) -> SkillsOpResult {
    if let SkillScope::Project { path } = scope {
        let p = path.trim();
        if p.is_empty() {
            return SkillsOpResult {
                success: false,
                stdout: String::new(),
                stderr: "project path is empty".to_string(),
            };
        }
        let mut full: Vec<String> = vec!["--yes".to_string(), "skills".to_string()];
        full.extend(extra_args.iter().cloned());
        let mut cmd = Command::new("npx");
        cmd.args(&full).current_dir(p);
        apply_home_env(&mut cmd);
        apply_proxy_env(&mut cmd, proxy_url);
        return match cmd.output() {
            Ok(o) => SkillsOpResult {
                success: o.status.success(),
                stdout: String::from_utf8_lossy(&o.stdout).to_string(),
                stderr: String::from_utf8_lossy(&o.stderr).to_string(),
            },
            Err(e) => SkillsOpResult {
                success: false,
                stdout: String::new(),
                stderr: format!("failed to spawn npx: {e}"),
            },
        };
    }
    run_npx(extra_args, proxy_url)
}

#[cfg(test)]
#[path = "test_npx.rs"]
mod test_npx;
