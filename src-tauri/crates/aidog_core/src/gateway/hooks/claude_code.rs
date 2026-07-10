//! Claude Code hook 注入 / 移除 + per_event 配置 + `_aidog_hooks` 标记。
//!
//! 职责：把 `hooks.<Event>`（按 settings.per_event 启用集）注入 `claude_code` 基线配置
//! （经 `do_sync_group_settings` 物化到每分组 settings.{group}.json）；按命令路径识别并移除
//! aidog 注入项，保留用户自定义 hook。共享 helper `references_aidog_script`/`ensure_object`
//! 供 codex 子模块复用（pub(crate)）。

use super::scripts::{
    ScriptPaths, LEGACY_SCRIPT_COMPLETE, LEGACY_SCRIPT_WAITING, SCRIPT_COMPLETE, SCRIPT_EVENT_NOTIFY,
    SCRIPT_WAITING,
};
use serde_json::{json, Value};

/// Claude Code 内部标记键（UI 状态；禁止写入 settings.{group}.json）。
pub const MARKER_HOOKS: &str = "_aidog_hooks";

/// 读取基线配置中 `_aidog_hooks.enabled` 标记（缺失/非布尔时返回 false）。
/// 用于 `do_sync_group_settings` 决定是否默认物化通知 hook。
pub fn hooks_marker_enabled(config: &Value) -> bool {
    config
        .get(MARKER_HOOKS)
        .and_then(|m| m.get("enabled"))
        .and_then(|e| e.as_bool())
        .unwrap_or(false)
}

/// 注入 Claude Code hooks 到基线配置（`claude_code` 全局 setting）。
///
/// N2 hook 事件通知：遍历 `enabled_events`（settings.per_event 中 enabled 的事件名）→
/// 每个 `set_event_hook(event, scripts.event_notify)`（全部指向同一通用脚本 command）。
/// 先 `remove_claude_code_hooks` 清掉旧 aidog 注入（避免改配置后残留旧事件 hook）。
///
/// 结构遵循 Claude Code hooks schema：
/// `{ "<Event>": [ { "hooks": [ { "type":"command", "command": "<path>" } ] } ] }`。
/// 仅覆盖 aidog 注入的命令项（按脚本文件名识别），保留用户其他 hook。
/// 同时打 `_aidog_hooks` 标记（UI 状态，sync 时 strip）。
///
/// `enabled_events` 为空时仅打 marker、清旧 aidog hook（不注入任何事件）。
pub fn inject_claude_code_hooks(config: &mut Value, scripts: &ScriptPaths, enabled_events: &[String]) {
    // 先清掉旧 aidog 注入（全量事件目录遍历），保用户项；随后按 enabled_events 重新注入。
    // remove 会顺带删 marker，下面再补回。
    remove_claude_code_hooks(config);

    let obj = ensure_object(config);
    obj.insert(MARKER_HOOKS.to_string(), json!({ "enabled": true }));

    if enabled_events.is_empty() {
        return;
    }

    let hooks = obj
        .entry("hooks".to_string())
        .or_insert_with(|| Value::Object(Default::default()));
    let hooks_obj = match hooks.as_object_mut() {
        Some(o) => o,
        None => {
            *hooks = Value::Object(Default::default());
            hooks.as_object_mut().unwrap()
        }
    };

    for event in enabled_events {
        set_event_hook(hooks_obj, event, &scripts.event_notify);
    }
    // 若清旧后无任何注入（理论不至，enabled 非空），保险删空 hooks。
    if hooks_obj.is_empty() {
        obj.remove("hooks");
    }
}

/// 移除 Claude Code 中 aidog 注入的 hooks（按命令路径识别），并去掉 `_aidog_hooks` 标记。
/// 保留用户自定义 hook；清空后空数组的 Event / 空 hooks 对象一并删除。
///
/// N2：遍历**全量事件目录** `CC_HOOK_EVENTS` 移除 aidog 项（确保改配置后旧事件 hook 不残留）；
/// `references_aidog_script` 靠单脚本名 `aidog-notify.py`（+ 旧 complete/waiting/.sh）识别，全匹配。
pub fn remove_claude_code_hooks(config: &mut Value) {
    let Some(obj) = config.as_object_mut() else { return };
    obj.remove(MARKER_HOOKS);
    let Some(hooks) = obj.get_mut("hooks").and_then(|v| v.as_object_mut()) else { return };
    for event in crate::gateway::models::CC_HOOK_EVENTS {
        remove_event_hook(hooks, event);
    }
    if hooks.is_empty() {
        obj.remove("hooks");
    }
}

/// 设置某 Event 的 aidog hook 命令项（移除旧的 aidog 项后追加，保留用户项）。
fn set_event_hook(hooks: &mut serde_json::Map<String, Value>, event: &str, script_path: &str) {
    // 先移除该 event 下已有的 aidog 注入项
    remove_event_hook(hooks, event);

    let entry = json!({
        "hooks": [ { "type": "command", "command": script_path } ]
    });

    let arr = hooks
        .entry(event.to_string())
        .or_insert_with(|| Value::Array(Vec::new()));
    if let Some(a) = arr.as_array_mut() {
        a.push(entry);
    } else {
        *arr = Value::Array(vec![entry]);
    }
}

/// 从某 Event 数组移除所有指向 aidog notify 脚本的命令项；清理空匹配组与空 Event。
/// 混合组（aidog + 用户命令）只剔除 aidog 命令项，保留用户项；纯 aidog 组整组丢弃。
fn remove_event_hook(hooks: &mut serde_json::Map<String, Value>, event: &str) {
    let Some(arr) = hooks.get_mut(event).and_then(|v| v.as_array_mut()) else { return };
    for matcher in arr.iter_mut() {
        if let Some(inner) = matcher.get_mut("hooks").and_then(|v| v.as_array_mut()) {
            inner.retain(|h| !is_aidog_command(h));
        }
    }
    // 丢弃 hooks 数组被清空的匹配组
    arr.retain(|matcher| {
        matcher
            .get("hooks")
            .and_then(|v| v.as_array())
            .map(|inner| !inner.is_empty())
            .unwrap_or(true)
    });
    if arr.is_empty() {
        hooks.remove(event);
    }
}

/// 判断命令串是否指向 aidog notify 脚本（含当前 `.py` 与旧版 `.sh` 文件名，
/// 以便移除时也能清掉历史 bash 注入；command 串可能为 `uv run <path>` / `python3 <path>` / 裸路径）。
pub(crate) fn references_aidog_script(cmd: &str) -> bool {
    // 各脚本名互不为子串（aidog-notify.py / -complete.py / -waiting.py / .sh），任一命中即 true。
    cmd.contains(SCRIPT_EVENT_NOTIFY)
        || cmd.contains(SCRIPT_COMPLETE)
        || cmd.contains(SCRIPT_WAITING)
        || cmd.contains(LEGACY_SCRIPT_COMPLETE)
        || cmd.contains(LEGACY_SCRIPT_WAITING)
}

/// 判断一个 hook 命令项是否为 aidog notify 脚本（按命令字符串含脚本文件名识别）。
fn is_aidog_command(h: &Value) -> bool {
    h.get("command")
        .and_then(|c| c.as_str())
        .map(references_aidog_script)
        .unwrap_or(false)
}

/// 确保 config 为对象并返回可变引用（非对象时重置为空对象）。
pub(crate) fn ensure_object(config: &mut Value) -> &mut serde_json::Map<String, Value> {
    if !config.is_object() {
        *config = Value::Object(Default::default());
    }
    config.as_object_mut().unwrap()
}

#[cfg(test)]
mod test_claude_code;
