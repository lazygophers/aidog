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

/// 判定 `npx skills` args 是否为变更类命令（会修改用户 ~/.agents）。
/// Global scope + 变更命令 = 测试禁操作；只读命令（list 等）允许。
/// 首个非选项 token（不以 `-` 开头）视为子命令名。
#[cfg(test)]
fn is_mutating_args(extra_args: &[String]) -> bool {
    const MUTATING: &[&str] = &[
        "install", "add", "remove", "uninstall", "enable", "disable", "update",
    ];
    extra_args
        .iter()
        .find(|a| !a.is_empty() && !a.starts_with('-'))
        .map(|cmd| MUTATING.iter().any(|m| m == cmd))
        .unwrap_or(false)
}

/// 封装 `npx skills <args...>`，捕获 stdout/stderr/退出码。
/// `proxy_url` 为 `Some` 时注入代理 env（见 `apply_proxy_env`），`None` 直连。
///
/// **注意**: 此函数在默认 cwd（用户 HOME）执行，若 args 含 `-g` 会操作用户
/// ~/.agents（install/uninstall/enable/disable）。测试不得直接调用 — 用
/// `run_npx_in_scope`（Project scope + tempdir）或 `*_args` 纯函数断言。
/// Global scope 拦截见 `run_npx_in_scope`。
pub(super) fn run_npx(extra_args: &[String], proxy_url: Option<&str>) -> SkillsOpResult {
    // 测试编译期硬拦: run_npx 是 Global scope 的最终 spawn 点，含 -g + 变更类命令
    // 会修改用户 ~/.agents。任何测试误调直接 panic 暴露，禁静默破坏用户数据。
    #[cfg(test)]
    if extra_args.iter().any(|a| a == "-g") && is_mutating_args(extra_args) {
        panic!(
            "run_npx(args 含 -g + 变更类命令) 在测试中被调用 — 会真实修改用户 ~/.agents。\
             测试改用 run_npx_in_scope(Project {{ tempdir }}) 或调 *_args 纯函数断言。args={:?}",
            extra_args
        );
    }
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
    // 测试编译期硬拦: Global scope + 变更类命令（install/add/remove/uninstall/
    // enable/disable/update）会真实 spawn `npx skills <verb> ... -g` 直接修改用户
    // ~/.agents。任何测试误调将静默删用户数据。
    // 历史教训: agent 跑全 cargo test 二分时 test_ops.rs:160 uninstall_all(&Global)
    // 真实 `npx skills remove --all -g` 删空 ~/.agents。
    // 测试必须用 SkillScope::Project { tempdir } 或调 *_args 纯函数断言。
    // 只读命令（list）经 export_skills 等生产路径被测试间接调用，属正常 → 不拦。
    // 详见 memory [[skills-test-isolation]] / 本任务 prd。
    #[cfg(test)]
    if matches!(scope, SkillScope::Global) && is_mutating_args(extra_args) {
        panic!(
            "run_npx_in_scope(Global, 变更类命令) 在测试中被调用 — 会真实修改用户 ~/.agents \
             (install/add/remove/uninstall/enable/disable/update --global)。测试改用 \
             SkillScope::Project {{ tempdir }} 或调 *_args 纯函数断言。args={:?}",
            extra_args
        );
    }
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
