//! `scripts.rs` 单测（1:1）：脚本内容生成 + 运行时不崩溃。

use super::*;

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
    // Stop 过滤从真实 transcript 判断未终止的后台 Agent/Bash 调用。
    assert!(s.contains("def has_active_background_task(payload):"));
    assert!(s.contains("run_in_background"));
    assert!(s.contains("<task-notification>"));
    assert!(s.contains("if event == \"Stop\" and has_active_background_task(payload):"));
}

/// 实跑生成脚本（spawn python3）的运行时测试已删除：依赖宿主装 python3，且仅验证
/// 「能解析、不崩溃」，已被上面的纯字符串断言（event_notify_script_reads_stdin_and_posts_event）
/// 覆盖等价语义。删后该文件不再触任何外部进程。

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
fn stop_filter_uses_anonymized_real_transcript_events() {
    let temp = std::env::temp_dir().join(format!("aidog-notify-{}", std::process::id()));
    let transcript_dir = temp.join(".claude/projects");
    std::fs::create_dir_all(&transcript_dir).unwrap();
    let script_path = temp.join("aidog-notify.py");
    std::fs::write(&script_path, build_event_notify_script()).unwrap();
    let runner = r#"import io,json,runpy,sys,urllib.request
calls=[]
def post(*args,**kwargs):
    calls.append(1)
    return io.BytesIO()
urllib.request.urlopen=post
sys.stdin=io.StringIO(sys.argv[2])
runpy.run_path(sys.argv[1],run_name="notify_test")["main"]()
print(len(calls))"#;
    let cases = [
        (
            "ordinary-stop",
            r#"{"type":"assistant","sessionId":"anonymous-session","message":{"role":"assistant","content":[{"type":"text","text":"<redacted>"}]}}"#,
            "1",
        ),
        (
            "foreground-agent",
            r#"{"type":"assistant","sessionId":"anonymous-session","message":{"role":"assistant","content":[{"type":"tool_use","id":"toolu-foreground-agent","name":"Agent","input":{"description":"<redacted>","run_in_background":false}}]}}"#,
            "1",
        ),
        (
            "background-task-still-running-across-turns",
            concat!(
                r#"{"type":"assistant","sessionId":"anonymous-session","message":{"role":"assistant","content":[{"type":"tool_use","id":"toolu-old-agent","name":"Agent","input":{"description":"<redacted>","run_in_background":true}}]}}"#,
                "\n",
                r#"{"type":"user","sessionId":"anonymous-session","message":{"role":"user","content":"new user message"}}"#,
                "\n",
                r#"{"type":"assistant","sessionId":"anonymous-session","message":{"role":"assistant","content":[{"type":"text","text":"<redacted>"}]}}"#
            ),
            "0",
        ),
        (
            "background-task-finished-after-new-user-message",
            concat!(
                r#"{"type":"assistant","sessionId":"anonymous-session","message":{"role":"assistant","content":[{"type":"tool_use","id":"toolu-old-agent","name":"Agent","input":{"description":"<redacted>","run_in_background":true}}]}}"#,
                "\n",
                r#"{"type":"user","sessionId":"anonymous-session","message":{"role":"user","content":"new user message"}}"#,
                "\n",
                r#"{"type":"user","sessionId":"anonymous-session","message":{"role":"user","content":"<task-notification><task-id>task-anon</task-id><tool-use-id>toolu-old-agent</tool-use-id><status>completed</status></task-notification>"}}"#
            ),
            "1",
        ),
        (
            "background-agent-running",
            r#"{"type":"assistant","sessionId":"anonymous-session","message":{"role":"assistant","content":[{"type":"tool_use","id":"toolu-anon-agent","name":"Agent","input":{"description":"<redacted>","run_in_background":true}}]}}"#,
            "0",
        ),
        (
            "background-bash-running",
            r#"{"type":"assistant","sessionId":"anonymous-session","message":{"role":"assistant","content":[{"type":"tool_use","id":"toolu-anon-bash","name":"Bash","input":{"command":"<redacted>","description":"<redacted>","run_in_background":true}}]}}"#,
            "0",
        ),
        (
            "agent-terminal-notification",
            concat!(
                r#"{"type":"assistant","sessionId":"anonymous-session","message":{"role":"assistant","content":[{"type":"tool_use","id":"toolu-anon-agent","name":"Agent","input":{"description":"<redacted>","run_in_background":true}}]}}"#,
                "\n",
                r#"{"type":"user","sessionId":"anonymous-session","message":{"role":"user","content":"<task-notification><task-id>task-anon</task-id><tool-use-id>toolu-anon-agent</tool-use-id><output-file><redacted></output-file><status>completed</status><summary><redacted></summary></task-notification>"}}"#,
                "\n"
            ),
            "1",
        ),
        (
            "bash-terminal-notification",
            concat!(
                r#"{"type":"assistant","sessionId":"anonymous-session","message":{"role":"assistant","content":[{"type":"tool_use","id":"toolu-anon-bash","name":"Bash","input":{"command":"<redacted>","description":"<redacted>","run_in_background":true}}]}}"#,
                "\n",
                r#"{"type":"user","sessionId":"anonymous-session","message":{"role":"user","content":"<task-notification><task-id>task-anon</task-id><tool-use-id>toolu-anon-bash</tool-use-id><output-file><redacted></output-file><status>failed</status><summary><redacted></summary></task-notification>"}}"#,
                "\n"
            ),
            "1",
        ),
        (
            "unknown-transcript-session",
            r#"{"type":"assistant","sessionId":"other-session","message":{"role":"assistant","content":[{"type":"text","text":"<redacted>"}]}}"#,
            "0",
        ),
        ("missing-transcript", "", "0"),
    ];

    for (name, transcript, expected) in cases {
        let transcript_path = transcript_dir.join(format!("{name}.jsonl"));
        if name != "missing-transcript" {
            let transcript = if transcript.ends_with('\n') {
                transcript.to_string()
            } else {
                format!("{transcript}\n")
            };
            std::fs::write(&transcript_path, transcript).unwrap();
        }
        let payload = format!(
            r#"{{"hook_event_name":"Stop","session_id":"anonymous-session","transcript_path":"~/.claude/projects/{name}.jsonl"}}"#
        );
        let output = std::process::Command::new("python3")
            .arg("-c")
            .arg(runner)
            .arg(&script_path)
            .arg(&payload)
            .env("HOME", &temp)
            .env("ANTHROPIC_BASE_URL", "http://127.0.0.1")
            .env("ANTHROPIC_AUTH_TOKEN", "test")
            .output()
            .expect("python3 must be available to test the generated Python hook");
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            String::from_utf8_lossy(&output.stdout).trim(),
            expected,
            "case: {name}"
        );
    }
    let official_cases = [
        ("registry-empty", "background-agent-running", "[]", "1"),
        (
            "registry-running",
            "ordinary-stop",
            r#"[{"id":"task-anon","type":"shell","status":"running","description":"<redacted>","command":"<redacted>"}]"#,
            "0",
        ),
        (
            "registry-terminal",
            "background-agent-running",
            r#"[{"id":"task-anon","type":"subagent","status":"completed","description":"<redacted>","agent_type":"Explore"}]"#,
            "1",
        ),
        (
            "registry-unknown-status",
            "ordinary-stop",
            r#"[{"id":"task-anon","type":"shell","status":"unknown","description":"<redacted>"}]"#,
            "0",
        ),
    ];
    for (name, transcript_name, tasks, expected) in official_cases {
        let payload = format!(
            r#"{{"hook_event_name":"Stop","session_id":"anonymous-session","transcript_path":"~/.claude/projects/{transcript_name}.jsonl","background_tasks":{tasks}}}"#
        );
        let output = std::process::Command::new("python3")
            .arg("-c")
            .arg(runner)
            .arg(&script_path)
            .arg(&payload)
            .env("HOME", &temp)
            .env("ANTHROPIC_BASE_URL", "http://127.0.0.1")
            .env("ANTHROPIC_AUTH_TOKEN", "test")
            .output()
            .expect("python3 must be available to test the generated Python hook");
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            String::from_utf8_lossy(&output.stdout).trim(),
            expected,
            "case: {name}"
        );
    }
    std::fs::remove_dir_all(temp).unwrap();
}

#[test]
fn event_script_does_not_truncate_path_vars() {
    let s = build_event_notify_script();
    // 仅 cwd / transcript_path 豁免 200 字符截断（内核要读文件）；其余 *_path（如 file_path）与字符串仍截断。
    assert!(s.contains(r#"key not in ("cwd", "transcript_path")"#));
    assert!(!s.contains(r#"endswith("_path")"#));
    assert!(s.contains("len(text) > max_len and key not in"));
}

#[test]
fn complete_script_forwards_codex_thread_id() {
    let s = build_hook_script("task_complete");
    // Codex notify 把事件 JSON 放在 argv[1]；thread-id → vars.thread_id。
    assert!(s.contains("sys.argv[1]"));
    assert!(s.contains(r#"get("thread-id")"#));
    assert!(s.contains(r#"vars_map["thread_id"]"#));
}

#[test]
fn subagent_cap_script_is_bundled() {
    let s = build_subagent_cap_script();
    assert!(s.contains("CLAUDE_CODE_SUBAGENT_HARD_CAP"));
    assert!(s.contains("--reset"));
    assert!(s.contains("permissionDecision"));
}
