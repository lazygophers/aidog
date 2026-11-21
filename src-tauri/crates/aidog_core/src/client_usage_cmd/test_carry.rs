//! 票 10：换客户端继续。夹具全是演示文本、落 tempdir；HomeGuard 隔离 HOME，不碰真实 ~/.claude / ~/.codex；
//! 执行器只记录命令，不真跑 osascript。
use super::carry::*;
use super::*;
use aidog_db::test_support::{HomeGuard, four_file_db, sample_log};
use crate::proxy_cmd::session_cmd::SessionResumeResult;

const AD: &str = "/Users/demo/.aidog";
fn ad() -> &'static Path {
    Path::new(AD)
}
use std::path::{Path, PathBuf};

fn now() -> chrono::DateTime<chrono::Utc> {
    chrono::DateTime::parse_from_rfc3339("2026-11-21T08:30:00Z").unwrap().with_timezone(&chrono::Utc)
}

/// Claude 源：compact 边界前的旧话、isMeta、斜杠命令回显、system-reminder、sidechain、被回退的分支、
/// thinking / tool_use / tool_result 都不搬，只剩主线文字。
const CLAUDE_SRC: &str = r#"{"type":"user","uuid":"a0","parentUuid":null,"message":{"role":"user","content":"压缩前的旧话"}}
{"type":"assistant","uuid":"a1","parentUuid":"a0","message":{"role":"assistant","content":[{"type":"text","text":"旧回答"}]}}
{"type":"system","subtype":"compact_boundary","uuid":"b0","parentUuid":null,"logicalParentUuid":"a1"}
{"type":"user","uuid":"u1","parentUuid":"b0","isMeta":true,"message":{"role":"user","content":"演示 meta 行"}}
{"type":"user","uuid":"u2","parentUuid":"u1","isCompactSummary":true,"message":{"role":"user","content":"摘要：之前做了演示"}}
{"type":"user","uuid":"r1","parentUuid":"u2","message":{"role":"user","content":"被回退的分支"}}
{"type":"user","uuid":"u3","parentUuid":"u2","message":{"role":"user","content":"<command-name>/model</command-name>"}}
{"type":"user","uuid":"u4","parentUuid":"u3","message":{"role":"user","content":[{"type":"text","text":"<system-reminder>演示提醒</system-reminder>"},{"type":"text","text":"请修复登录"}]}}
{"type":"assistant","uuid":"x1","parentUuid":"u4","isSidechain":true,"message":{"role":"assistant","content":[{"type":"text","text":"子代理的话"}]}}
{"type":"assistant","uuid":"u5","parentUuid":"u4","message":{"role":"assistant","content":[{"type":"thinking","thinking":"想"},{"type":"text","text":"好的，已修复"},{"type":"tool_use","id":"t1","name":"Bash","input":{"command":"ls"}}]}}
{"type":"user","uuid":"u6","parentUuid":"u5","message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"t1","content":"ok"}]}}
not json
{"type":"assistant","uuid":"u7","parentUuid":"u6","message":{"role":"assistant","content":[{"type":"text","text":"完成"}]}}
"#;

fn claude_expected() -> Vec<Item> {
    vec![
        Item::User("摘要：之前做了演示".into()),
        Item::User("请修复登录".into()),
        Item::Assistant("好的，已修复".into()),
        Item::Assistant("完成".into()),
    ]
}

/// Codex 源：developer / user_instructions / AGENTS.md / environment_context 不搬；compacted 用 replacement_history
/// 取代之前的历史；reasoning / function_call / event_msg 不搬。
const CODEX_SRC: &str = r##"{"timestamp":"2026-11-20T01:00:00.000Z","type":"session_meta","payload":{"id":"c1","cwd":"/w"}}
{"type":"response_item","payload":{"type":"message","role":"developer","content":[{"type":"input_text","text":"演示开发者指令"}]}}
{"type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"<user_instructions>演示</user_instructions>"}]}}
{"type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"# AGENTS.md instructions for /w"}]}}
{"type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"<environment_context>cwd</environment_context>"}]}}
{"type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"压缩前的话"}]}}
{"type":"compacted","payload":{"message":"","replacement_history":[{"type":"message","role":"user","content":[{"type":"input_text","text":"保留的问题"}]},{"type":"message","role":"assistant","content":[{"type":"output_text","text":"保留的回答"}]}]}}
{"type":"response_item","payload":{"type":"reasoning","summary":[]}}
{"type":"event_msg","payload":{"type":"user_message","message":"重复"}}
{"type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"继续写测试"}]}}
{"type":"response_item","payload":{"type":"function_call","name":"shell","arguments":"{}","call_id":"c"}}
{"type":"response_item","payload":{"type":"message","role":"assistant","content":[{"type":"output_text","text":"测试已写好"}]}}
"##;

fn codex_expected() -> Vec<Item> {
    vec![
        Item::User("保留的问题".into()),
        Item::Assistant("保留的回答".into()),
        Item::User("继续写测试".into()),
        Item::Assistant("测试已写好".into()),
    ]
}

#[test]
fn claude_items_walk_main_line_and_filter() {
    assert_eq!(claude_items(CLAUDE_SRC), claude_expected());
}

#[test]
fn codex_items_use_replacement_history_and_filter() {
    assert_eq!(codex_items(CODEX_SRC), codex_expected());
}

/// compacted 无 replacement_history：保留之前的用户话 + 摘要作一条用户话。
#[test]
fn codex_compacted_message_keeps_user_words() {
    let src = r#"{"type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"问一"}]}}
{"type":"response_item","payload":{"type":"message","role":"assistant","content":[{"type":"output_text","text":"答一"}]}}
{"type":"compacted","payload":{"message":"演示摘要"}}
"#;
    assert_eq!(codex_items(src), vec![Item::User("问一".into()), Item::User("演示摘要".into())]);
}

#[test]
fn claude_slug_matches_claude_code() {
    assert_eq!(claude_slug("/Users/me/my.proj_x"), "-Users-me-my-proj-x");
    assert_eq!(claude_slug("/a/中"), "-a--");
    assert_eq!(claude_slug("/a/😀"), "-a---", "emoji = 2 个 UTF-16 单元");
}

fn lines(p: &Path) -> Vec<serde_json::Value> {
    std::fs::read_to_string(p).unwrap().lines().map(|l| serde_json::from_str(l).unwrap()).collect()
}

/// Claude 目标：路径 = projects/<slug>/<id>.jsonl；parentUuid 串成链；读回同一组话。
#[test]
fn write_claude_format_and_roundtrip() {
    let d = tempfile::tempdir().unwrap();
    let (id, path) = write_claude(d.path(), "/w/p", &codex_expected(), now()).unwrap();
    assert_eq!(path, d.path().join("-w-p").join(format!("{id}.jsonl")));
    let ls = lines(&path);
    assert_eq!(ls.len(), 4);
    let mut parent = serde_json::Value::Null;
    for l in &ls {
        assert_eq!(l["parentUuid"], parent);
        assert_eq!(l["sessionId"], id.as_str());
        assert_eq!(l["cwd"], "/w/p");
        assert_eq!(l["isSidechain"], false);
        parent = l["uuid"].clone();
    }
    assert_eq!(ls[0]["type"], "user");
    assert_eq!(ls[0]["message"]["content"], "保留的问题");
    assert_eq!(ls[1]["type"], "assistant");
    assert_eq!(ls[1]["message"]["content"][0]["text"], "保留的回答");
    assert_eq!(ls[1]["timestamp"], "2026-11-21T08:30:00.000Z");
    assert_eq!(claude_items(&std::fs::read_to_string(&path).unwrap()), codex_expected());
    // 已有同名文件不覆盖：再写一次是新 id 新文件。
    let (id2, _) = write_claude(d.path(), "/w/p", &codex_expected(), now()).unwrap();
    assert_ne!(id, id2);
}

/// Codex 目标：sessions/YYYY/MM/DD/rollout-*-<id>.jsonl；首行 session_meta；provider 取 config.toml；读回同一组话。
#[test]
fn write_codex_format_and_roundtrip() {
    let d = tempfile::tempdir().unwrap();
    std::fs::write(d.path().join("config.toml"), "model_provider = \"demo_provider\"\n[profiles.x]\nmodel_provider = \"other\"\n").unwrap();
    let (id, path) = write_codex(d.path(), "/w", &claude_expected(), now()).unwrap();
    let rel = path.strip_prefix(d.path().join("sessions")).unwrap().to_string_lossy().to_string();
    let local = now().with_timezone(&chrono::Local);
    assert!(rel.starts_with(&local.format("%Y/%m/%d/rollout-%Y-%m-%dT%H-%M-%S-").to_string()), "{rel}");
    assert!(rel.ends_with(&format!("{id}.jsonl")), "{rel}");
    let ls = lines(&path);
    assert_eq!(ls[0]["type"], "session_meta");
    assert_eq!(ls[0]["payload"]["id"], id.as_str());
    assert_eq!(ls[0]["payload"]["cwd"], "/w");
    assert_eq!(ls[0]["payload"]["model_provider"], "demo_provider");
    assert_eq!(codex_items(&std::fs::read_to_string(&path).unwrap()), claude_expected());
    // 无 config.toml → openai。
    let d2 = tempfile::tempdir().unwrap();
    let (_, p2) = write_codex(d2.path(), "/w", &claude_expected(), now()).unwrap();
    assert_eq!(lines(&p2)[0]["payload"]["model_provider"], "openai");
}

/// 交接命令：四个目标各自的启动参数；提示与路径单引号困住；无目录 / 控制字符 / 未知目标 → Err。
#[test]
fn handoff_line_per_target_and_injection() {
    let b = |t: &str, g: &str, w: &str, f: &str| build_handoff_line(t, g, w, f, ad());
    let p = "'请阅读 /s/a.jsonl 了解上一会话的上下文'";
    assert_eq!(
        b("claude_code", "teamA", "/w", "/s/a.jsonl").unwrap(),
        format!("cd -- '/w' && claude --settings '/Users/demo/.aidog/settings.teamA.json' {p}")
    );
    assert_eq!(b("claude_code", "", "/w", "/s/a.jsonl").unwrap(), format!("cd -- '/w' && claude {p}"));
    assert_eq!(b("codex", "g", "/w", "/s/a.jsonl").unwrap(), format!("cd -- '/w' && AIDOG_KEY='g' codex -p 'g' {p}"));
    assert_eq!(b("codex", "", "/w", "/s/a.jsonl").unwrap(), format!("cd -- '/w' && codex {p}"));
    assert_eq!(b("gemini_cli", "g", "/w", "/s/a.jsonl").unwrap(), format!("cd -- '/w' && gemini -i {p}"));
    assert_eq!(b("opencode", "g", "/w", "/s/a.jsonl").unwrap(), format!("cd -- '/w' && opencode --prompt {p}"));
    assert_eq!(
        b("gemini_cli", "", "/my dir'; x", "/s/a'; rm -rf ~.jsonl").unwrap(),
        "cd -- '/my dir'\\''; x' && gemini -i '请阅读 /s/a'\\''; rm -rf ~.jsonl 了解上一会话的上下文'"
    );
    assert!(b("gemini_cli", "", "", "/s/a.jsonl").is_err());
    assert!(b("gemini_cli", "", "/w", "/s/a\n.jsonl").is_err());
    assert!(b("gemini_cli", "", "/w", "").is_err());
    assert!(b("codex", "-g", "/w", "/s/a.jsonl").is_err());
    assert!(b("pi", "", "/w", "/s/a.jsonl").is_err());
}

#[test]
fn installed_clients_follow_path_lookup() {
    assert_eq!(installed_clients(|b| b == "codex" || b == "opencode"), vec!["codex", "opencode"]);
}

fn call(client: &str, sid: &str, file: &str, side: bool, pos: i64) -> aidog_db::ClientCallRow {
    aidog_db::ClientCallRow {
        client_code: client.into(),
        session_id: sid.into(),
        is_sidechain: side,
        call_time_ms: 1_000 + pos,
        model: "demo-model".into(),
        input_tokens: 1,
        output_tokens: 1,
        cache_read_tokens: 0,
        cache_write_tokens: 0,
        reasoning_tokens: 0,
        cost: None,
        source_file_id: file.into(),
        file_pos: pos,
    }
}

async fn counts(db: &aidog_db::Db) -> (i64, usize) {
    let calls = aidog_db::client_usage_sessions(db, 0, i64::MAX, None, 1000).await.unwrap().iter().map(|r| r.calls).sum();
    (calls, aidog_logs::list_proxy_logs(db, 1000, 0).await.unwrap().len())
}

fn only_file(root: &Path) -> PathBuf {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).into_iter().flatten().flatten() {
            if e.path().is_dir() { stack.push(e.path()) } else { out.push(e.path()) }
        }
    }
    assert_eq!(out.len(), 1, "{out:?}");
    out.remove(0)
}

/// 端到端：闸门先于读文件；转写写到目标存储并用目标继续命令打开；不写 client_usage.db / proxy_log；
/// 交接不读原文（源文件不存在也能拼出命令）。
#[tokio::test]
async fn carry_gates_transcribes_and_hands_off() {
    let _home = HomeGuard::new();
    let (db, _d) = four_file_db().await;
    let src = tempfile::tempdir().unwrap();
    let cfile = src.path().join("s1.jsonl");
    let xfile = src.path().join("rollout-c1.jsonl");
    std::fs::write(&cfile, CLAUDE_SRC).unwrap();
    std::fs::write(&xfile, CODEX_SRC).unwrap();
    let cf = cfile.to_string_lossy().to_string();
    let xf = xfile.to_string_lossy().to_string();
    aidog_db::insert_client_calls(&db, &[
        call("claude_code", "s1", "/demo/sub/agent-a.jsonl", true, 1),
        call("claude_code", "s1", &cf, false, 2),
        call("codex", "c1", &xf, false, 3),
        call("claude_code", "s2", "/demo/missing.jsonl", false, 4),
    ]).await.unwrap();
    let mut log = sample_log("l1", "teamA", 1_000);
    log.session_id = "s1".into();
    aidog_logs::upsert_proxy_log(&db, log).await.unwrap();
    let homes_dir = tempfile::tempdir().unwrap();
    let homes = CarryHomes { claude_projects: homes_dir.path().join("projects"), codex_home: homes_dir.path().join("codex") };
    let ran = std::sync::Mutex::new(Vec::<String>::new());
    let run = |l: &str| { ran.lock().unwrap().push(l.to_string()); Ok(true) };
    let before = counts(&db).await;

    // 原文开关关：Err，不执行、不写目标存储。
    let e = session_carry_with(&db, "claude_code", "s1", "/w", "codex", &homes, ad(), &run).await.unwrap_err();
    assert!(e.contains("transcript"), "{e}");
    // 原文开关开但源客户端读取开关关：同样拒。
    save_transcript_consent(&db, true).await.unwrap();
    assert!(session_carry_with(&db, "claude_code", "s1", "/w", "codex", &homes, ad(), &run).await.is_err());
    assert!(ran.lock().unwrap().is_empty());
    assert!(!homes_dir.path().join("codex").exists() && !homes_dir.path().join("projects").exists());

    let mut enabled = std::collections::HashMap::new();
    enabled.insert("claude_code".to_string(), true);
    enabled.insert("codex".to_string(), true);
    save_consent(&db, &ClientUsageConsent { asked: true, enabled }).await.unwrap();

    // Claude → Codex：沿用源会话分组。
    let r = session_carry_with(&db, "claude_code", "s1", "/w", "codex", &homes, ad(), &run).await.unwrap();
    let out = only_file(&homes_dir.path().join("codex").join("sessions"));
    let id = lines(&out)[0]["payload"]["id"].as_str().unwrap().to_string();
    assert_eq!(r, SessionResumeResult { launched: true, command: format!("cd -- '/w' && AIDOG_KEY='teamA' codex -p 'teamA' resume '{id}'") });
    assert_eq!(codex_items(&std::fs::read_to_string(&out).unwrap()), claude_expected());

    // Codex → Claude：无分组。
    let r = session_carry_with(&db, "codex", "c1", "/w", "claude_code", &homes, ad(), &run).await.unwrap();
    let out = only_file(&homes.claude_projects);
    assert_eq!(out.parent().unwrap().file_name().unwrap(), "-w");
    let id = out.file_stem().unwrap().to_string_lossy().to_string();
    assert_eq!(r.command, format!("cd -- '/w' && claude --resume '{id}'"));
    assert_eq!(claude_items(&std::fs::read_to_string(&out).unwrap()), codex_expected());

    // 交接：原文开关关也可用，且不读源文件（s2 的文件根本不存在）。
    save_transcript_consent(&db, false).await.unwrap();
    let r = session_carry_with(&db, "claude_code", "s2", "/w", "gemini_cli", &homes, ad(), &run).await.unwrap();
    assert_eq!(r.command, "cd -- '/w' && gemini -i '请阅读 /demo/missing.jsonl 了解上一会话的上下文'");
    let r = session_carry_with(&db, "claude_code", "s1", "/w", "opencode", &homes, ad(), &run).await.unwrap();
    assert_eq!(r.command, format!("cd -- '/w' && opencode --prompt '请阅读 {cf} 了解上一会话的上下文'"), "取主线文件，不取子代理文件");

    // 同一客户端 / 无目录 / 无记录 → Err。
    assert!(session_carry_with(&db, "codex", "c1", "/w", "codex", &homes, ad(), &run).await.is_err());
    assert!(session_carry_with(&db, "codex", "c1", "", "gemini_cli", &homes, ad(), &run).await.is_err());
    assert!(session_carry_with(&db, "codex", "nope", "/w", "gemini_cli", &homes, ad(), &run).await.is_err());
    assert_eq!(ran.lock().unwrap().len(), 4);
    assert_eq!(counts(&db).await, before, "不写 client_usage.db 与 proxy_log");
}
