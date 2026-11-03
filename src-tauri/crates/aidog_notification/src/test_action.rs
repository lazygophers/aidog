use super::action::*;
use aidog_db::models::NotificationSettings;
use std::collections::HashMap;

fn v(pairs: &[(&str, &str)]) -> HashMap<String, String> {
    pairs.iter().map(|(k, x)| (k.to_string(), x.to_string())).collect()
}

fn perm(tool: &str) -> Option<String> {
    derive_action("PermissionRequest", &v(&[("tool_name", tool)]))
}

#[test]
fn permission_action_per_tool() {
    let cases = [
        ("AskUserQuestion", "有问题请你做选择"),
        ("ExitPlanMode", "计划已就绪，请批准"),
        ("Bash", "请求运行命令"),
        ("PowerShell", "请求运行命令"),
        ("Edit", "请求修改文件"),
        ("NotebookEdit", "请求修改文件"),
        ("Write", "请求写入文件"),
        ("WebFetch", "请求访问网页"),
        ("WebSearch", "请求联网搜索"),
        ("Agent", "请求启动子代理"),
        ("mcp__github__create_issue", "请求调用 github 的 create_issue"),
        ("mcp__plugin_x_y__do__it", "请求调用 plugin_x_y 的 do__it"),
        ("MultiEdit", "请求授权使用 MultiEdit"),
    ];
    for (tool, want) in cases {
        assert_eq!(perm(tool).as_deref(), Some(want), "{tool}");
    }
    // 缺 tool_name → 不派生（模板 fill_empty 兜底）
    assert_eq!(derive_action("PermissionRequest", &v(&[])), None);
    // 非目标事件不派生
    assert_eq!(derive_action("Stop", &v(&[("tool_name", "Bash")])), None);
}

#[test]
fn notification_action_per_type() {
    let a = |t: &str, m: &str| derive_action("Notification", &v(&[("notification_type", t), ("message", m)]));
    assert_eq!(a("idle_prompt", "x").as_deref(), Some("空闲一分钟了，在等你回话"));
    assert_eq!(a("elicitation_dialog", "x").as_deref(), Some("MCP 服务请你填写信息"));
    assert_eq!(a("permission_prompt", "x").as_deref(), Some("还在等你授权"));
    assert_eq!(a("plugin_notification", "raw msg").as_deref(), Some("raw msg"));
    assert_eq!(a("weird", "raw msg").as_deref(), Some("raw msg"));
    assert_eq!(a("weird", "").as_deref(), Some("Claude Code 有新通知"));
}

#[test]
fn duplicate_permission_prompt_branches() {
    let n = v(&[("notification_type", "permission_prompt")]);
    let mut s = NotificationSettings::default();
    // per_event 缺省 → PermissionRequest 默认 ON
    assert!(is_duplicate_permission_prompt("Notification", &n, &s));
    // 其它类型 / 其它事件不去重
    assert!(!is_duplicate_permission_prompt("Notification", &v(&[("notification_type", "idle_prompt")]), &s));
    assert!(!is_duplicate_permission_prompt("Stop", &n, &s));
    // 显式关闭 PermissionRequest → 照常提醒
    s.per_event.insert(
        "PermissionRequest".into(),
        serde_json::from_value(serde_json::json!({ "enabled": false })).unwrap(),
    );
    assert!(!is_duplicate_permission_prompt("Notification", &n, &s));
}
