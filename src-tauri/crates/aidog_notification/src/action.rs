//! 通知动作文案派生：按 `tool_name` / `notification_type` 生成 `{action}` 变量（zh 硬编码，同默认模板口径）。
use std::collections::HashMap;

use aidog_db::models::{DEFAULT_ON_EVENTS, NotificationSettings};

fn var<'a>(vars: &'a HashMap<String, String>, k: &str) -> &'a str {
    vars.get(k).map(|s| s.trim()).unwrap_or("")
}

/// PermissionRequest / Notification 事件的 `{action}`；其它事件或缺关键字段返回 None。
pub(crate) fn derive_action(event: &str, vars: &HashMap<String, String>) -> Option<String> {
    match event {
        "PermissionRequest" => {
            let tool = var(vars, "tool_name");
            if tool.is_empty() {
                return None;
            }
            let fixed = match tool {
                "AskUserQuestion" => "有问题请你做选择",
                "ExitPlanMode" => "计划已就绪，请批准",
                "Bash" | "PowerShell" => "请求运行命令",
                "Edit" | "NotebookEdit" => "请求修改文件",
                "Write" => "请求写入文件",
                "WebFetch" => "请求访问网页",
                "WebSearch" => "请求联网搜索",
                "Agent" => "请求启动子代理",
                _ => "",
            };
            if !fixed.is_empty() {
                return Some(fixed.to_string());
            }
            // MCP 工具名 `mcp__<server>__<tool>`：按第一个 `__` 切 server / tool。
            if let Some((server, t)) = tool.strip_prefix("mcp__").and_then(|r| r.split_once("__"))
                && !server.is_empty()
                && !t.is_empty()
            {
                return Some(format!("请求调用 {server} 的 {t}"));
            }
            Some(format!("请求授权使用 {tool}"))
        }
        "Notification" => {
            let fixed = match var(vars, "notification_type") {
                "permission_prompt" => "还在等你授权",
                "idle_prompt" => "空闲一分钟了，在等你回话",
                "elicitation_dialog" => "MCP 服务请你填写信息",
                "elicitation_url_dialog" => "请你打开网页完成操作",
                "elicitation_complete" | "elicitation_response" | "auth_success" => "操作已完成",
                "agent_needs_input" => "后台会话在等你输入",
                "agent_completed" => "后台会话已结束",
                "quota_auto_resume_fired" => "额度已恢复，继续执行",
                "quota_auto_resume_stale" => "额度已恢复，请按回车继续",
                "quota_auto_resume_disabled" => "额度等待结束，未自动继续",
                _ => "",
            };
            if !fixed.is_empty() {
                return Some(fixed.to_string());
            }
            // plugin_notification / 未知 / 缺失：回落 message 原文，再空则兜底句。
            let m = var(vars, "message");
            Some(if m.is_empty() { "Claude Code 有新通知" } else { m }.to_string())
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
