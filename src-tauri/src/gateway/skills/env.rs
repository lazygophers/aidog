//! 环境探测：npx / node 可用性 + spawn 子进程的 home 相关 env 注入。

use super::types::SkillsEnv;
use std::process::Command;
use std::sync::OnceLock;

/// 进程内 env 探测缓存：node/npx 可用性一会话不变，仅首次真探测。
static ENV_CACHE: OnceLock<SkillsEnv> = OnceLock::new();

/// `ensure_runtime_path` 幂等守卫：仅首次真合并 PATH。
static PATH_FIXED: OnceLock<()> = OnceLock::new();

/// 把登录 shell 的交互式 PATH 并入当前进程 PATH（幂等，启动时调一次即可）。
///
/// **真因**: GUI 经 launchd / Finder 启动，env 极简（PATH 仅 `/usr/bin:/bin:...`）。
/// 用户用 brew(`/opt/homebrew/bin`)/nvm/pyenv/asdf 装的 node/npx/python/uv 不在此 PATH →
/// `Command::new("npx")` 直接 spawn 失败，连 `check_env` 探测都误报「未安装」。
/// 这是 skills 安装 / 导入「环境缺失」的最常见真因：**已装但找不到**，非真没装。
///
/// **修复**: spawn 登录 shell 读其交互式 PATH（会 source 用户 rc，含 nvm/pyenv shim 注入），
/// 合并进进程 PATH（登录 PATH 在前优先），覆盖全部后续子进程（检测 / npx / uv / 导入）。
/// 静默自动修：失败仅 warn，不打断任何流程。Windows GUI 继承用户 PATH，无此问题 → 跳过。
pub fn ensure_runtime_path() {
    PATH_FIXED.get_or_init(|| {
        if let Some(merged) = probe_login_path() {
            // edition 2021：set_var 为安全 API，启动早期单线程调用，无并发 env 读写竞争。
            std::env::set_var("PATH", &merged);
            tracing::info!("runtime PATH 已并入登录 shell PATH（修 GUI 极简 PATH 致 node/npx/python 找不到）");
        }
    });
}

/// 探测登录 shell 的交互式 PATH 并与当前 PATH 合并（登录在前、去重、保序）。
/// 返回 None 表示无需 / 无法修（Windows、shell 失败、空、与现状一致）。
#[cfg(not(target_os = "windows"))]
fn probe_login_path() -> Option<String> {
    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string());
    let out = Command::new(&shell)
        .args(["-ilc", "echo $PATH"])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let login = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if login.is_empty() {
        return None;
    }
    let current = std::env::var("PATH").unwrap_or_default();
    let merged = merge_path(&login, &current);
    (merged != current).then_some(merged)
}

#[cfg(target_os = "windows")]
fn probe_login_path() -> Option<String> {
    None
}

/// 合并两段 `:` 分隔 PATH：`first` 优先，追加 `rest` 中未出现的项，去重保序，丢空段。
#[cfg(not(target_os = "windows"))]
fn merge_path(first: &str, rest: &str) -> String {
    let mut seen = std::collections::HashSet::new();
    first
        .split(':')
        .chain(rest.split(':'))
        .filter(|s| !s.is_empty())
        .filter(|s| seen.insert(s.to_string()))
        .collect::<Vec<_>>()
        .join(":")
}

/// 探测 npx / node 可用性（进程内缓存，仅首次 spawn 子进程）。
/// 后续调用直接返回缓存值，开页 0 子进程。
pub fn check_env() -> SkillsEnv {
    ENV_CACHE.get_or_init(probe_env).clone()
}

/// 真探测 npx / node 可用性（spawn 子进程）。任一探测失败均不 panic，对应字段降级。
fn probe_env() -> SkillsEnv {
    let node_version = Command::new("node")
        .arg("--version")
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .filter(|s| !s.is_empty());

    // npx 仅探测可执行性（--version 在所有平台稳定）。
    let npx_available = Command::new("npx")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);

    SkillsEnv {
        npx_available,
        node_version,
    }
}

/// 解析 spawn npx 子进程所需的 home 相关 env。
///
/// skills CLI 的 claude-code agent 检测依赖 `claudeHome = CLAUDE_CONFIG_DIR || ~/.claude`
/// （仅看 `HOME` env，无 getpwuid 兜底），codex 则有 `/etc/codex` 兜底容错更强。
/// 打包版 GUI（launchd 启动）env 极简，`HOME` 可能缺失或异常 → claude-code 漏检 → list
/// `agents[]` 不含 Claude Code → UI 显示该 skill 未启用 claude。
///
/// 修复：用 `dirs::home_dir()`（getpwuid 解析）显式注入 `HOME`，比继承父 env 可靠；
/// `CLAUDE_CONFIG_DIR` 若父 env 已设则透传（保留用户自定义配置目录），未设不强制注入。
///
/// 返回 `(HOME 值, 可选 CLAUDE_CONFIG_DIR)`，纯函数便于单测。
///
/// pub(super)：供 `list::list_installed` 在入口预检 HOME 是否可解析（不可解析即视为 npx 失败，
/// 避免写空缓存；见 F1 缓存写空防御）。
pub(super) fn resolve_home_env() -> (Option<String>, Option<String>) {
    let home = dirs::home_dir()
        .map(|h| h.to_string_lossy().into_owned())
        .or_else(|| std::env::var("HOME").ok().filter(|h| !h.is_empty()));
    let claude_config = std::env::var("CLAUDE_CONFIG_DIR")
        .ok()
        .filter(|v| !v.trim().is_empty());
    (home, claude_config)
}

/// 给 npx 子进程注入 home 相关 env（见 `resolve_home_env`）。
///
/// `dirs::home_dir()` 返 None（极罕见，如 launchd 极简 env）时静默跳过 + warn 日志。
/// **list 路径** (`list_installed`) 在入口预检 `resolve_home_env().0.is_none()` 并返失败信号
/// （见 F1 缓存写空防御），避免假空缓存。**install/enable 等写路径** 仍继续执行（即便 HOME
/// 缺失 npx 也可能用默认 cwd 跑通），失败由 npx 自身 stderr 报。
pub(super) fn apply_home_env(cmd: &mut Command) {
    let (home, claude_config) = resolve_home_env();
    if let Some(h) = home {
        cmd.env("HOME", h);
    } else {
        tracing::warn!("apply_home_env: dirs::home_dir() 返 None 且 HOME env 缺失，skills claude-code 检测可能漏");
    }
    if let Some(c) = claude_config {
        cmd.env("CLAUDE_CONFIG_DIR", c);
    }
}

#[cfg(test)]
#[path = "test_env.rs"]
mod test_env;
