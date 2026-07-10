//! `claude_code.rs` 单测（1:1）：CC hook 注入/移除、per_event、marker 解析。

use super::*;
use crate::gateway::hooks::scripts::ScriptPaths;
use serde_json::json;

/// 测试辅助：构造 ScriptPaths（含 event_notify）。
fn test_scripts(complete: &str, event_notify: &str) -> ScriptPaths {
    ScriptPaths {
        complete: complete.into(),
        event_notify: event_notify.into(),
    }
}

/// N2 默认精选事件集（与 models 的 DEFAULT_ON_EVENTS 对齐），便于测试注入遍历。
fn enabled_events(events: &[&str]) -> Vec<String> {
    events.iter().map(|s| s.to_string()).collect()
}

#[test]
fn remove_strips_legacy_sh_command() {
    // 旧版 bash `.sh` 注入也应被识别并移除（迁移兼容）。
    let mut cfg = json!({
        "hooks": {
            "Stop": [ { "hooks": [
                { "type": "command", "command": "/a/aidog-notify-complete.sh" }
            ] } ]
        },
        "_aidog_hooks": { "enabled": true }
    });
    remove_claude_code_hooks(&mut cfg);
    assert!(cfg["hooks"].get("Stop").is_none());
}

#[test]
fn inject_claude_code_sets_enabled_events() {
    let mut cfg = json!({});
    let scripts = test_scripts(
        "/home/u/.aidog/aidog-notify-complete.py",
        "/home/u/.aidog/scripts/aidog-notify.py",
    );
    let events = enabled_events(&["Stop", "Notification", "PermissionRequest"]);
    inject_claude_code_hooks(&mut cfg, &scripts, &events);

    // 标记存在
    assert!(cfg.get(MARKER_HOOKS).is_some());
    // 每个启用事件均指向通用脚本 command。
    for ev in ["Stop", "Notification", "PermissionRequest"] {
        let h = &cfg["hooks"][ev][0]["hooks"][0];
        assert_eq!(h["type"], "command");
        assert_eq!(h["command"], "/home/u/.aidog/scripts/aidog-notify.py");
    }
    // 未启用事件不注入。
    assert!(cfg["hooks"].get("PreToolUse").is_none());
}

#[test]
fn inject_empty_events_only_sets_marker() {
    let mut cfg = json!({});
    let scripts = test_scripts("/a/c.py", "/a/aidog-notify.py");
    inject_claude_code_hooks(&mut cfg, &scripts, &[]);
    assert!(cfg.get(MARKER_HOOKS).is_some());
    // 无启用事件 → 无 hooks 对象。
    assert!(cfg.get("hooks").is_none());
}

#[test]
fn inject_is_idempotent_no_duplicate() {
    let mut cfg = json!({});
    let scripts = test_scripts("/a/c.py", "/a/aidog-notify.py");
    let events = enabled_events(&["Stop", "Notification"]);
    inject_claude_code_hooks(&mut cfg, &scripts, &events);
    inject_claude_code_hooks(&mut cfg, &scripts, &events);
    assert_eq!(cfg["hooks"]["Stop"].as_array().unwrap().len(), 1);
    assert_eq!(cfg["hooks"]["Notification"].as_array().unwrap().len(), 1);
}

#[test]
fn inject_reconfig_removes_stale_events() {
    // 改配置：先注入 Stop+Notification，再注入仅 SessionEnd → 旧 aidog 项应被清。
    let mut cfg = json!({});
    let scripts = test_scripts("/a/c.py", "/a/aidog-notify.py");
    inject_claude_code_hooks(&mut cfg, &scripts, &enabled_events(&["Stop", "Notification"]));
    inject_claude_code_hooks(&mut cfg, &scripts, &enabled_events(&["SessionEnd"]));
    // 旧 Stop/Notification aidog 项已清。
    assert!(cfg["hooks"].get("Stop").is_none());
    assert!(cfg["hooks"].get("Notification").is_none());
    // 新 SessionEnd 注入。
    assert_eq!(
        cfg["hooks"]["SessionEnd"][0]["hooks"][0]["command"],
        "/a/aidog-notify.py"
    );
}

#[test]
fn inject_preserves_user_hooks() {
    let mut cfg = json!({
        "hooks": {
            "Stop": [ { "hooks": [ { "type": "command", "command": "/usr/bin/my-own.sh" } ] } ],
            "PreToolUse": [ { "hooks": [ { "type": "command", "command": "/x.sh" } ] } ]
        }
    });
    let scripts = test_scripts("/a/c.py", "/a/aidog-notify.py");
    inject_claude_code_hooks(&mut cfg, &scripts, &enabled_events(&["Stop"]));
    // 用户的 Stop 项保留 + aidog 追加
    let stop = cfg["hooks"]["Stop"].as_array().unwrap();
    assert_eq!(stop.len(), 2);
    // PreToolUse 不动
    assert!(cfg["hooks"]["PreToolUse"].is_array());
}

#[test]
fn remove_claude_code_strips_aidog_only() {
    let mut cfg = json!({
        "hooks": {
            "Stop": [
                { "hooks": [ { "type": "command", "command": "/usr/bin/my-own.sh" } ] },
                { "hooks": [ { "type": "command", "command": "/a/aidog-notify.py" } ] }
            ],
            "Notification": [
                { "hooks": [ { "type": "command", "command": "/a/aidog-notify.py" } ] }
            ],
            // 非默认精选事件也应被全量目录遍历清掉。
            "SessionEnd": [
                { "hooks": [ { "type": "command", "command": "/a/aidog-notify.py" } ] }
            ]
        },
        "_aidog_hooks": { "enabled": true }
    });
    remove_claude_code_hooks(&mut cfg);
    assert!(cfg.get(MARKER_HOOKS).is_none());
    // 用户 Stop 项保留，aidog 项移除
    let stop = cfg["hooks"]["Stop"].as_array().unwrap();
    assert_eq!(stop.len(), 1);
    assert_eq!(stop[0]["hooks"][0]["command"], "/usr/bin/my-own.sh");
    // Notification / SessionEnd 全是 aidog → 整个 Event 移除（全量目录遍历）
    assert!(cfg["hooks"].get("Notification").is_none());
    assert!(cfg["hooks"].get("SessionEnd").is_none());
}

#[test]
fn remove_strips_aidog_within_mixed_group() {
    // 同一匹配组内 aidog + 用户命令混合：只剔 aidog，保留用户命令。
    let mut cfg = json!({
        "hooks": {
            "Stop": [ { "hooks": [
                { "type": "command", "command": "/a/aidog-notify-complete.sh" },
                { "type": "command", "command": "/usr/bin/keep.sh" }
            ] } ]
        }
    });
    remove_claude_code_hooks(&mut cfg);
    let inner = cfg["hooks"]["Stop"][0]["hooks"].as_array().unwrap();
    assert_eq!(inner.len(), 1);
    assert_eq!(inner[0]["command"], "/usr/bin/keep.sh");
}

#[test]
fn remove_claude_code_drops_empty_hooks_object() {
    let mut cfg = json!({});
    let scripts = test_scripts("/a/c.py", "/a/aidog-notify.py");
    inject_claude_code_hooks(&mut cfg, &scripts, &enabled_events(&["Stop", "Notification"]));
    remove_claude_code_hooks(&mut cfg);
    // 没有用户 hook → hooks 对象删除
    assert!(cfg.get("hooks").is_none());
    assert!(cfg.get(MARKER_HOOKS).is_none());
}

#[test]
fn notify_hooks_fragment_shape() {
    // 复刻 build_notify_hooks_fragment 取片段逻辑：空对象 inject 后取 hooks 子对象。
    let mut cfg = json!({});
    let scripts = test_scripts(
        "/u/.aidog/scripts/aidog-notify-complete.py",
        "/u/.aidog/scripts/aidog-notify.py",
    );
    inject_claude_code_hooks(&mut cfg, &scripts, &enabled_events(&["Stop", "Notification"]));
    let fragment = cfg.get("hooks").cloned().unwrap();
    // 片段含 Stop / Notification，且不含 _aidog_hooks 标记（标记在外层 config）。
    assert!(fragment.get("Stop").is_some());
    assert!(fragment.get("Notification").is_some());
    assert!(fragment.get(MARKER_HOOKS).is_none());
    // 全部事件指向通用事件脚本 command。
    assert_eq!(
        fragment["Stop"][0]["hooks"][0]["command"],
        "/u/.aidog/scripts/aidog-notify.py"
    );
    assert_eq!(
        fragment["Notification"][0]["hooks"][0]["command"],
        "/u/.aidog/scripts/aidog-notify.py"
    );
    assert_eq!(fragment["Stop"][0]["hooks"][0]["type"], "command");
}

#[test]
fn marker_enabled_parsing() {
    assert!(hooks_marker_enabled(&json!({ "_aidog_hooks": { "enabled": true } })));
    assert!(!hooks_marker_enabled(&json!({ "_aidog_hooks": { "enabled": false } })));
    // marker 缺失 → false
    assert!(!hooks_marker_enabled(&json!({})));
    // enabled 非布尔 → false
    assert!(!hooks_marker_enabled(&json!({ "_aidog_hooks": { "enabled": "yes" } })));
    // marker 非对象 → false
    assert!(!hooks_marker_enabled(&json!({ "_aidog_hooks": true })));
}
