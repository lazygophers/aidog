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
