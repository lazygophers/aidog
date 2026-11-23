//! 通知动作文案派生：按 `tool_name` / `notification_type` 生成 `{action}` 变量（按界面语言取 `notif.action.*`）。
use std::collections::HashMap;

use aidog_db::models::{DEFAULT_ON_EVENTS, NotificationSettings};
use aidog_i18n::{Lang, tr};

fn var<'a>(vars: &'a HashMap<String, String>, k: &str) -> &'a str {
    vars.get(k).map(|s| s.trim()).unwrap_or("")
}

/// PermissionRequest / Notification 事件的 `{action}`；其它事件或缺关键字段返回 None。
pub(crate) fn derive_action(event: &str, vars: &HashMap<String, String>, lang: Lang) -> Option<String> {
    match event {
        "PermissionRequest" => {
            let tool = var(vars, "tool_name");
            if tool.is_empty() {
                return None;
            }
            let fixed = match tool {
                "AskUserQuestion" => "notif.action.askUserQuestion",
                "ExitPlanMode" => "notif.action.exitPlanMode",
                "Bash" | "PowerShell" => "notif.action.runCommand",
                "Edit" | "NotebookEdit" => "notif.action.editFile",
                "Write" => "notif.action.writeFile",
                "WebFetch" => "notif.action.webFetch",
                "WebSearch" => "notif.action.webSearch",
                "Agent" => "notif.action.agent",
                _ => "",
            };
            if !fixed.is_empty() {
                return Some(tr(lang, fixed).to_string());
            }
            // MCP 工具名 `mcp__<server>__<tool>`：按第一个 `__` 切 server / tool。
            if let Some((server, t)) = tool.strip_prefix("mcp__").and_then(|r| r.split_once("__"))
                && !server.is_empty()
                && !t.is_empty()
            {
                return Some(
                    tr(lang, "notif.action.mcpCall")
                        .replace("{{server}}", server)
                        .replace("{{tool}}", t),
                );
            }
            Some(tr(lang, "notif.action.useTool").replace("{{tool}}", tool))
        }
        "Notification" => {
            let fixed = match var(vars, "notification_type") {
                "permission_prompt" => "notif.action.permissionPrompt",
                "idle_prompt" => "notif.action.idlePrompt",
                "elicitation_dialog" => "notif.action.elicitationDialog",
                "elicitation_url_dialog" => "notif.action.elicitationUrlDialog",
                "elicitation_complete" | "elicitation_response" | "auth_success" => {
                    "notif.action.completed"
                }
                "agent_needs_input" => "notif.action.agentNeedsInput",
                "agent_completed" => "notif.action.agentCompleted",
                "quota_auto_resume_fired" => "notif.action.quotaResumeFired",
                "quota_auto_resume_stale" => "notif.action.quotaResumeStale",
                "quota_auto_resume_disabled" => "notif.action.quotaResumeDisabled",
                _ => "",
            };
            if !fixed.is_empty() {
                return Some(tr(lang, fixed).to_string());
            }
            // plugin_notification / 未知 / 缺失：回落 message 原文，再空则兜底句。
            let m = var(vars, "message");
            Some(if m.is_empty() { tr(lang, "notif.action.fallback") } else { m }.to_string())
        }
        _ => None,
    }
}

/// Notification(permission_prompt) 与已启用的 PermissionRequest 对同一次授权重复：
/// 命中时调用方只落收件箱，不弹窗 / 播报。PermissionRequest 未显式配置时按默认 ON 集判定。
pub(crate) fn is_duplicate_permission_prompt(
    event: &str,
    vars: &HashMap<String, String>,
    settings: &NotificationSettings,
) -> bool {
    event == "Notification"
        && var(vars, "notification_type") == "permission_prompt"
        && settings
            .event_setting("PermissionRequest")
            .map(|es| es.enabled)
            .unwrap_or_else(|| DEFAULT_ON_EVENTS.contains(&"PermissionRequest"))
}
