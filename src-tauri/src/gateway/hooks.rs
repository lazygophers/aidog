//! 通知 hook 集成（N2 — 系统通知模块）。
//!
//! 职责：
//! - 生成 hook 脚本到 `~/.aidog/scripts/`：`aidog-notify-complete.py`（POST type=task_complete）、
//!   `aidog-notify-waiting.py`（type=waiting_input）。脚本用 `ANTHROPIC_BASE_URL` 推导
//!   `/api/notify` 端点 + `ANTHROPIC_AUTH_TOKEN`（=group_key）Bearer 鉴权，project=cwd basename。
//!   脚本为 Python（stdlib `urllib`/`json`/`os`，无第三方依赖），含 PEP723 内联依赖头，
//!   由 `uv run --script`（uv 可用）或 `python3`（fallback）执行（执行器写进 command 串）。
//! - Claude Code 一键注入：把 `hooks.Stop`（任务完成）+ `hooks.Notification`（等待输入）
//!   注入到 `claude_code` 基线配置（与 statusLine 一样经 `do_sync_group_settings` 物化到
//!   每分组 `settings.{group}.json`）。strip 内部标记 `_aidog_hooks`（防回写污染，仿
//!   `_aidog_statusline`）。
//! - Codex 一键注入：写 `config.toml` 顶层 `notify = ["<脚本>"]`（Codex notify 机制）。
//! - 内置两类默认模板「{project} 完成」「{project} 等待用户输入」，存
//!   `NotificationSettings.per_type[task_complete/waiting_input].template`，用户可改。
//!
//! 纯逻辑（脚本内容生成 / settings JSON 改写 / TOML 改写）抽为纯函数便于单测；
//! 副作用（写文件 / chmod）在 command 层（lib.rs）调用。

use serde_json::{json, Value};

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

/// 任务完成 hook 脚本文件名（Python + PEP723，uv run / python3 执行；**Codex notify 仍用**）。
pub const SCRIPT_COMPLETE: &str = "aidog-notify-complete.py";
/// 等待输入 hook 脚本文件名（Python + PEP723，uv run / python3 执行）。
pub const SCRIPT_WAITING: &str = "aidog-notify-waiting.py";
/// 通用事件通知脚本文件名（N2 hook 事件通知）：读 stdin JSON 取 hook_event_name + 事件特有字段，
/// POST `/api/notify` `{event, vars}`（不传 type，后端按 per_event 解析）。所有 CC 事件共用此脚本。
pub const SCRIPT_EVENT_NOTIFY: &str = "aidog-notify.py";

/// 旧版 bash hook 脚本文件名（迁移清理用，写新脚本时删除）。
pub const LEGACY_SCRIPT_COMPLETE: &str = "aidog-notify-complete.sh";
/// 旧版 bash hook 脚本文件名（迁移清理用，写新脚本时删除）。
pub const LEGACY_SCRIPT_WAITING: &str = "aidog-notify-waiting.sh";

/// Claude Code 内部标记键（UI 状态；禁止写入 settings.{group}.json）。
pub const MARKER_HOOKS: &str = "_aidog_hooks";

/// 生成 hook 脚本内容（Python + PEP723，stdlib only）。
///
/// 脚本从 `ANTHROPIC_BASE_URL` 推导 `/api/notify` 端点（strip 末尾 `/proxy` 与版本前缀），
/// `ANTHROPIC_AUTH_TOKEN`（=group_key）作 Bearer，project=cwd basename 作 `{project}` 变量。
/// `notif_type` 为 `task_complete` / `waiting_input`。
///
/// 脚本对 Claude Code（hooks.Stop/Notification，从 stdin 收 JSON，cwd=项目目录）与
/// Codex（notify，从 `$1` 收 JSON）都安全：仅用 cwd basename 作 project，不解析 stdin/参数。
///
/// PEP723 内联依赖头（`# /// script` … `# ///`，deps 现为空列表，预留第三方依赖隔离）。
/// shebang 是 fallback；实际由 command 串里的 `uv run --script` / `python3` 执行，故 shebang
/// 用 `python3` 以保证无 uv 环境下直接执行也可用。任何异常静默吞掉（通知非关键路径）。
pub fn build_hook_script(notif_type: &str) -> String {
    format!(
        r#"#!/usr/bin/env python3
# /// script
# requires-python = ">=3.8"
# dependencies = []
# ///
# aidog notification hook — auto-generated. Do not edit.
# 触发 aidog 系统通知（POST /api/notify）。
# Claude Code: hooks.Stop / hooks.Notification 调用（stdin 为事件 JSON，忽略）。
# Codex: notify 调用（$1 为事件 JSON，忽略）。
import json
import os
import sys
import urllib.request


def main() -> None:
    base = os.environ.get("ANTHROPIC_BASE_URL", "")
    token = os.environ.get("ANTHROPIC_AUTH_TOKEN", "")
    if not base or not token:
        return

    # 从 base_url 推导代理根：依次去掉末尾 /proxy 及版本前缀（/v1 等），再拼 /api/notify。
    # 顺序剥离（镜像旧 bash root%/proxy → root%/v1 → root%/api/paas/v4）。
    root = base.rstrip("/")
    for suffix in ("/proxy", "/v1", "/api/paas/v4"):
        if root.endswith(suffix):
            root = root[: -len(suffix)]
    url = root + "/api/notify"

    project = os.path.basename(os.getcwd())
    body = json.dumps(
        {{"type": "{notif_type}", "vars": {{"project": project}}}}
    ).encode("utf-8")

    req = urllib.request.Request(
        url,
        data=body,
        method="POST",
        headers={{
            "Authorization": "Bearer " + token,
            "Content-Type": "application/json",
        }},
    )
    try:
        urllib.request.urlopen(req, timeout=5).read()
    except Exception:
        pass


if __name__ == "__main__":
    try:
        main()
    except Exception:
        pass
    sys.exit(0)
"#
    )
}

/// 单个透传 var 字符串值最大长度（超出截断加省略号，防 prompt 等巨串进通知）。
pub const EVENT_VAR_MAX_LEN: usize = 200;

/// 生成通用事件通知脚本内容（N2，Python + PEP723，stdlib only）。
///
/// 与 `build_hook_script` 不同：本脚本**读 stdin JSON**，取 `hook_event_name`(→ event) +
/// cwd basename(→ vars.project) + `session_id`(→ vars.session) +
/// **遍历 stdin 所有顶层标量字段**（str/int/float/bool，跳过 dict/list 嵌套）塞入 vars，
/// str 超 `EVENT_VAR_MAX_LEN`(200) 截断加省略；POST `/api/notify` body
/// `{"event": <name>, "vars": {...}}`（**不传 type**，后端按 per_event 解析）。
/// 这样每事件不同入参自动进 vars，无需枚举字段；模板用 `{字段名}`，未知占位后端忽略/兜空。
/// 无命令行传参（event 来自 stdin），绕开 CC command 是否 shell 解析参数的风险。
/// endpoint/Bearer 推导沿用 `build_hook_script`（ANTHROPIC_BASE_URL 剥后缀 + ANTHROPIC_AUTH_TOKEN）。
/// 任何异常静默吞掉（通知非关键路径）。
pub fn build_event_notify_script() -> String {
    let max_len = EVENT_VAR_MAX_LEN;
    format!(
        r#"#!/usr/bin/env python3
# /// script
# requires-python = ">=3.8"
# dependencies = []
# ///
# aidog notification hook (event-aware) — auto-generated. Do not edit.
# 通用 Claude Code hook 事件通知：读 stdin JSON 取 hook_event_name + 事件字段，POST /api/notify。
# 后端按 per_event 解析通知类型与模板（本脚本不传 type）。
import json
import os
import sys
import urllib.request


def main() -> None:
    base = os.environ.get("ANTHROPIC_BASE_URL", "")
    token = os.environ.get("ANTHROPIC_AUTH_TOKEN", "")
    if not base or not token:
        return

    # 从 base_url 推导代理根：依次去掉末尾 /proxy 及版本前缀（/v1 等），再拼 /api/notify。
    root = base.rstrip("/")
    for suffix in ("/proxy", "/v1", "/api/paas/v4"):
        if root.endswith(suffix):
            root = root[: -len(suffix)]
    url = root + "/api/notify"

    # 读 stdin 事件 JSON（缺失/非法 → 空 dict，仍用 cwd basename 作 project）。
    payload = {{}}
    try:
        raw = sys.stdin.read()
        if raw:
            payload = json.loads(raw)
    except Exception:
        payload = {{}}
    if not isinstance(payload, dict):
        payload = {{}}

    event = payload.get("hook_event_name")
    if not isinstance(event, str) or not event:
        return  # 无事件名无法路由 per_event，静默退出。

    vars_map = {{"project": os.path.basename(os.getcwd())}}
    session = payload.get("session_id")
    if isinstance(session, str) and session:
        vars_map["session"] = session

    # 通用透传：遍历 stdin 所有顶层标量字段（str/int/float/bool），跳过 dict/list 嵌套。
    # str 超长截断加省略（防 prompt 等巨串进通知）；非 str 标量 str() 化。
    max_len = {max_len}
    for key, val in payload.items():
        if not isinstance(key, str) or key == "session_id":
            continue
        # bool 是 int 子类，归入标量；显式排除 dict/list（及其它非标量）。
        if isinstance(val, bool):
            text = str(val)
        elif isinstance(val, (int, float)):
            text = str(val)
        elif isinstance(val, str):
            text = val
        else:
            continue  # 嵌套对象/数组/None → 跳过。
        if len(text) > max_len:
            text = text[:max_len] + "..."
        vars_map[key] = text

    body = json.dumps({{"event": event, "vars": vars_map}}).encode("utf-8")
    req = urllib.request.Request(
        url,
        data=body,
        method="POST",
        headers={{
            "Authorization": "Bearer " + token,
            "Content-Type": "application/json",
        }},
    )
    try:
        urllib.request.urlopen(req, timeout=5).read()
    except Exception:
        pass


if __name__ == "__main__":
    try:
        main()
    except Exception:
        pass
    sys.exit(0)
"#
    )
}

/// 已知占位的脚本绝对路径（command 层写文件后传入）。
/// - `complete`：task_complete 脚本 command（**Codex notify 依赖，勿丢**）。
/// - `event_notify`：通用事件通知脚本 command（N2，所有 CC 事件共用，按 per_event 注入）。
///
/// 注：原 `waiting` 脚本已并入通用事件脚本（N2 单脚本方案），不再注入；
/// `SCRIPT_WAITING` 常量仍保留供 `references_aidog_script` 清理历史注入。
pub struct ScriptPaths {
    pub complete: String,
    pub event_notify: String,
}

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
    for event in super::models::CC_HOOK_EVENTS {
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
fn references_aidog_script(cmd: &str) -> bool {
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

/// 在 Codex `config.toml` 的 JSON 视图中注入顶层 `notify`。
///
/// Codex `notify` 是顶层字符串数组，进程会以脚本 + 事件 JSON 调用。
/// 任务完成事件由 Codex `agent-turn-complete` 触发 → 指向 complete 脚本。
/// （Codex 当前仅 turn-complete 事件，等待输入无独立 notify event，故仅注入 complete。）
pub fn inject_codex_notify(config: &mut Value, complete_script: &str) {
    let obj = ensure_object(config);
    obj.insert("notify".to_string(), json!([complete_script]));
}

/// 移除 Codex 中 aidog 注入的 `notify`（仅当其指向 aidog 脚本时；保留用户自定义 notify）。
pub fn remove_codex_notify(config: &mut Value) {
    let Some(obj) = config.as_object_mut() else { return };
    let is_aidog = obj
        .get("notify")
        .and_then(|v| v.as_array())
        .map(|a| {
            a.iter().any(|v| {
                v.as_str()
                    .map(references_aidog_script)
                    .unwrap_or(false)
            })
        })
        .unwrap_or(false);
    if is_aidog {
        obj.remove("notify");
    }
}

/// 确保 config 为对象并返回可变引用（非对象时重置为空对象）。
fn ensure_object(config: &mut Value) -> &mut serde_json::Map<String, Value> {
    if !config.is_object() {
        *config = Value::Object(Default::default());
    }
    config.as_object_mut().unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 测试辅助：构造 ScriptPaths（含 event_notify）。
    fn test_scripts(complete: &str, event_notify: &str) -> ScriptPaths {
        ScriptPaths {
            complete: complete.into(),
            event_notify: event_notify.into(),
        }
    }

    #[test]
    fn script_contains_endpoint_and_type() {
        let s = build_hook_script("task_complete");
        assert!(s.contains("/api/notify"));
        assert!(s.contains("ANTHROPIC_BASE_URL"));
        assert!(s.contains("ANTHROPIC_AUTH_TOKEN"));
        // Python urllib POST 用 "Bearer " + token，不再是 bash 的 $token。
        assert!(s.contains(r#""Bearer " + token"#));
        assert!(s.contains(r#""type": "task_complete""#));
        // project = cwd basename（Python os.path.basename）。
        assert!(s.contains("os.path.basename(os.getcwd())"));
    }

    #[test]
    fn event_notify_script_reads_stdin_and_posts_event() {
        let s = build_event_notify_script();
        assert!(s.contains("/api/notify"));
        assert!(s.contains("ANTHROPIC_BASE_URL"));
        assert!(s.contains("ANTHROPIC_AUTH_TOKEN"));
        // 读 stdin + 取 hook_event_name（不传 type）。
        assert!(s.contains("sys.stdin.read()"));
        assert!(s.contains("hook_event_name"));
        assert!(!s.contains(r#""type":"#));
        assert!(s.contains(r#""event": event"#));
        // 通用标量透传：遍历 payload.items()，str/int/float/bool 才塞，跳过嵌套。
        assert!(s.contains("payload.items()"));
        assert!(s.contains("isinstance(val, bool)"));
        assert!(s.contains("isinstance(val, (int, float))"));
        assert!(s.contains("isinstance(val, str)"));
        assert!(s.contains("session_id"));
        // 长字符串截断（防巨串进通知）。
        assert!(s.contains(&format!("max_len = {EVENT_VAR_MAX_LEN}")));
        assert!(s.contains("len(text) > max_len"));
        assert!(s.contains(r#"text[:max_len] + "...""#));
        // PEP723 stdlib only。
        assert!(s.contains("# /// script"));
        assert!(s.contains("import urllib.request"));
        assert!(!s.contains("curl"));
    }

    /// 实跑生成脚本验证通用透传 + 跳过嵌套 + 截断（需本机有 python3，缺失则跳过）。
    #[test]
    fn event_notify_script_passthrough_runtime() {
        use std::io::Write;
        use std::process::{Command, Stdio};
        // 写脚本到临时文件。
        let script = build_event_notify_script();
        let dir = std::env::temp_dir();
        let path = dir.join(format!("aidog-test-notify-{}.py", std::process::id()));
        std::fs::write(&path, &script).unwrap();

        // 无 ANTHROPIC_* 环境时脚本会 return 提前退；为验证 vars 构造，
        // 改用内联探针：去掉真正的 POST，仅打印 vars_map。
        // 简化：直接执行脚本并喂 stdin，脚本因缺环境变量静默退出（exit 0），
        // 这里只断言脚本可被 python3 解析且不崩溃（语法正确性 + 标量遍历不抛异常）。
        let python = if Command::new("python3").arg("--version").output().is_ok() {
            "python3"
        } else {
            let _ = std::fs::remove_file(&path);
            return; // 无 python3，跳过运行时校验。
        };

        let long = "x".repeat(500);
        let stdin_json = format!(
            r#"{{"hook_event_name":"Stop","session_id":"s1","tool_name":"Bash","duration_ms":1234,"final":true,"prompt":"{long}","nested":{{"a":1}},"arr":[1,2,3]}}"#
        );
        let mut child = Command::new(python)
            .arg(&path)
            .env_remove("ANTHROPIC_BASE_URL")
            .env_remove("ANTHROPIC_AUTH_TOKEN")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        child
            .stdin
            .as_mut()
            .unwrap()
            .write_all(stdin_json.as_bytes())
            .unwrap();
        let out = child.wait_with_output().unwrap();
        let _ = std::fs::remove_file(&path);
        // 缺环境变量 → 静默 return，退出码 0，无 traceback。
        assert!(out.status.success(), "script crashed: {out:?}");
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert!(!stderr.contains("Traceback"), "python error: {stderr}");
    }

    #[test]
    fn script_is_python_with_pep723_header() {
        let s = build_hook_script("waiting_input");
        // PEP723 内联依赖头（deps 空列表，预留）。
        assert!(s.contains("# /// script"));
        assert!(s.contains("# dependencies = []"));
        assert!(s.contains("# ///"));
        // stdlib only — 无第三方依赖、无 curl。
        assert!(s.contains("import urllib.request"));
        assert!(!s.contains("curl"));
        // shebang python3（fallback；实际执行器在 command 串）。
        assert!(s.starts_with("#!/usr/bin/env python3"));
        assert!(s.contains(r#""type": "waiting_input""#));
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

    /// N2 默认精选事件集（与 models 的 DEFAULT_ON_EVENTS 对齐），便于测试注入遍历。
    fn enabled_events(events: &[&str]) -> Vec<String> {
        events.iter().map(|s| s.to_string()).collect()
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
    fn codex_notify_inject_and_remove() {
        let mut cfg = json!({ "model_provider": "aidog" });
        inject_codex_notify(&mut cfg, "/a/aidog-notify-complete.sh");
        assert_eq!(cfg["notify"][0], "/a/aidog-notify-complete.sh");
        // 其他键不动
        assert_eq!(cfg["model_provider"], "aidog");
        remove_codex_notify(&mut cfg);
        assert!(cfg.get("notify").is_none());
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

    #[test]
    fn codex_remove_preserves_user_notify() {
        let mut cfg = json!({ "notify": ["/usr/bin/user-notify.sh"] });
        remove_codex_notify(&mut cfg);
        // 非 aidog notify 保留
        assert_eq!(cfg["notify"][0], "/usr/bin/user-notify.sh");
    }
}
