//! Hook 脚本生成（Python + PEP723，stdlib only）。
//!
//! 职责：生成 `~/.aidog/scripts/` 下的通知 hook 脚本内容（纯字符串生成，无副作用，便于单测）。
//! - `build_hook_script`：定型 type（task_complete / waiting_input）脚本，**Codex notify 仍用**。
//! - `build_event_notify_script`：通用事件通知脚本，读 stdin JSON 取 hook_event_name + 标量字段。
//!
//! 副作用（写文件 / chmod）在 command 层（lib.rs）调用。

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

/// 单个透传 var 字符串值最大长度（超出截断加省略号，防 prompt 等巨串进通知）。
pub const EVENT_VAR_MAX_LEN: usize = 200;

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

#[cfg(test)]
mod test_scripts;
