//! 通知 hook 集成（N2 — 系统通知模块）。
//!
//! 职责（按子模块拆分）：
//! - [`scripts`]：生成 hook 脚本到 `~/.aidog/scripts/`（`aidog-notify-complete.py` /
//!   `aidog-notify.py` 通用事件脚本）。脚本用 `ANTHROPIC_BASE_URL` 推导 `/api/notify` 端点 +
//!   `ANTHROPIC_AUTH_TOKEN`（=group_key）Bearer 鉴权，project=cwd basename。脚本为 Python
//!   （stdlib only，PEP723 内联依赖头），由 `uv run --script` / `python3` 执行。
//! - [`claude_code`]：Claude Code 一键注入。把 `hooks.<Event>`（按 per_event 启用集）注入
//!   `claude_code` 基线配置（经 `do_sync_group_settings` 物化到每分组 settings.{group}.json）。
//!   strip 内部标记 `_aidog_hooks`（防回写污染，仿 `_aidog_statusline`）。
//! - [`codex`]：Codex 一键注入。写 `config.toml` 顶层 `notify = ["<脚本>"]`（Codex notify 机制）。
//!
//! 纯逻辑（脚本内容生成 / settings JSON 改写 / TOML 改写）抽为纯函数便于单测；
//! 副作用（写文件 / chmod）在 command 层（lib.rs）调用。
//!
//! 内置两类默认模板「{project} 完成」「{project} 等待用户输入」，存
//! `NotificationSettings.per_type[task_complete/waiting_input].template`，用户可改。

mod claude_code;
mod codex;
mod scripts;

// 对外路径保持不变（`gateway::hooks::X`）：re-export 各子模块的公开项。
pub use claude_code::{
    hooks_marker_enabled, inject_claude_code_hooks, remove_claude_code_hooks, MARKER_HOOKS,
};
pub use codex::{inject_codex_notify, remove_codex_notify};
pub use scripts::{
    build_event_notify_script, build_hook_script, ScriptPaths, LEGACY_SCRIPT_COMPLETE,
    LEGACY_SCRIPT_WAITING, SCRIPT_COMPLETE, SCRIPT_EVENT_NOTIFY,
};

/// hook 客户端类型。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HookClient {
    ClaudeCode,
    Codex,
}

impl HookClient {
    pub fn from_str(s: &str) -> Result<Self, String> {
        match s {
            "claude_code" => Ok(HookClient::ClaudeCode),
            "codex" => Ok(HookClient::Codex),
            other => Err(format!("unknown hook client: {other}")),
        }
    }
}
