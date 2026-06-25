//! 环境探测：npx / node 可用性 + spawn 子进程的 home 相关 env 注入。

use super::types::SkillsEnv;
use std::process::Command;
use std::sync::OnceLock;

/// 进程内 env 探测缓存：node/npx 可用性一会话不变，仅首次真探测。
static ENV_CACHE: OnceLock<SkillsEnv> = OnceLock::new();

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
