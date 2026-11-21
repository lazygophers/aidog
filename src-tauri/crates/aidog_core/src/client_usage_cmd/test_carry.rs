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

fn u(s: &str) -> Item {
    Item::User(s.into(), vec![])
}
fn a(s: &str) -> Item {
    Item::Assistant(s.into())
}

/// Claude 源：compact 边界前的旧话、isMeta、斜杠命令回显、system-reminder、sidechain、被回退的分支、
/// thinking 都不搬；tool_use 与其 tool_result 配成一次调用（票 13）。
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
        u("摘要：之前做了演示"),
        u("请修复登录"),
        a("好的，已修复"),
        Item::Call(ToolCall {
            name: "Bash".into(),
            id: "t1".into(),
            input: "{\n  \"command\": \"ls\"\n}".into(),
            shell: "ls".into(),
            result: Some("ok".into()),
            ..Default::default()
        }),
        a("完成"),
    ]
}

/// Codex 源：developer / user_instructions / AGENTS.md / environment_context 不搬；compacted 用 replacement_history
/// 取代之前的历史；reasoning / event_msg 不搬；function_call 搬成调用（票 13）。
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
        u("保留的问题"),
        a("保留的回答"),
        u("继续写测试"),
        Item::Call(ToolCall { name: "shell".into(), id: "c".into(), input: "{}".into(), ..Default::default() }),
        a("测试已写好"),
    ]
}

/// 只有文字的一组话（写入往返用）。
fn text_items() -> Vec<Item> {
    vec![u("保留的问题"), a("保留的回答"), u("继续写测试"), a("测试已写好")]
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
    assert_eq!(codex_items(src), vec![u("问一"), u("演示摘要")]);
}

/// 票 13：Claude 源的用户图片、tool_result 分块正文（图片记 [image]）、非 Bash 工具。
#[test]
fn claude_items_carry_images_and_tool_results() {
    let src = r#"{"type":"user","uuid":"u1","parentUuid":null,"message":{"role":"user","content":[{"type":"text","text":"看图"},{"type":"image","source":{"type":"base64","media_type":"image/png","data":"QUJD"}}]}}
{"type":"assistant","uuid":"u2","parentUuid":"u1","message":{"role":"assistant","content":[{"type":"tool_use","id":"t9","name":"Read","input":{"file_path":"/w/a.rs"}}]}}
{"type":"user","uuid":"u3","parentUuid":"u2","message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"t9","is_error":true,"content":[{"type":"text","text":"读不到"},{"type":"image"}]}]}}
{"type":"user","uuid":"u4","parentUuid":"u3","message":{"role":"user","content":[{"type":"image","source":{"type":"base64","media_type":"image/jpeg","data":"WFla"}}]}}
"#;
    assert_eq!(
        claude_items(src),
        vec![
            Item::User("看图".into(), vec!["data:image/png;base64,QUJD".into()]),
            Item::Call(ToolCall {
                name: "Read".into(),
                id: "t9".into(),
                input: "{\n  \"file_path\": \"/w/a.rs\"\n}".into(),
                result: Some("读不到\n[image]".into()),
                is_error: true,
                ..Default::default()
            }),
            Item::User(String::new(), vec!["data:image/jpeg;base64,WFla".into()]),
        ]
    );
}

/// 票 13：Codex 源各类工具调用与输出、用户图片（对齐 yetone/magpie:internal/sessions/carry_convert.go codexItem）。
#[test]
fn codex_items_carry_calls_and_images() {
    let src = r#"{"type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"<environment_context>x</environment_context>"},{"type":"input_image","image_url":"data:image/png;base64,QUJD"},{"type":"input_image","image_url":"https://x/y.png"}]}}
{"type":"response_item","payload":{"type":"function_call","name":"shell","arguments":"{\"command\":[\"bash\",\"-lc\",\"echo hi\"],\"workdir\":\"/w/s\"}","call_id":"c1"}}
{"type":"response_item","payload":{"type":"function_call_output","call_id":"c1","output":"{\"output\":\"hi\"}"}}
{"type":"response_item","payload":{"type":"function_call","name":"shell","arguments":"{\"command\":[\"ls\",\"-a b\"]}","call_id":"c2"}}
{"type":"response_item","payload":{"type":"function_call","name":"exec_command","arguments":"{\"cmd\":\"pwd\"}","call_id":"c3"}}
{"type":"response_item","payload":{"type":"function_call_output","call_id":"c3","output":{"content":"/w"}}}
{"type":"response_item","payload":{"type":"function_call","name":"shell_command","arguments":"{\"command\":\"git status\"}","call_id":"c4"}}
{"type":"response_item","payload":{"type":"custom_tool_call","name":"apply_patch","input":"*** Begin Patch","call_id":"c5"}}
{"type":"response_item","payload":{"type":"custom_tool_call_output","call_id":"c5","output":"done"}}
{"type":"response_item","payload":{"type":"local_shell_call","call_id":"c6","action":{"command":["make","lint"],"working_directory":"/w"}}}
"#;
    let calls: Vec<(String, String, String, Option<String>)> = codex_items(src)
        .into_iter()
        .filter_map(|i| match i {
            Item::Call(c) => Some((c.name, c.shell, c.workdir, c.result)),
            _ => None,
        })
        .collect();
    assert_eq!(
        calls,
        vec![
            ("shell".into(), "echo hi".into(), "/w/s".into(), Some("{\"output\":\"hi\"}".into())),
            ("shell".into(), "ls '-a b'".into(), String::new(), None),
            ("exec_command".into(), "pwd".into(), String::new(), Some("/w".into())),
            ("shell_command".into(), "git status".into(), String::new(), None),
            ("apply_patch".into(), String::new(), String::new(), Some("done".into())),
            ("shell".into(), "make lint".into(), "/w".into(), None),
        ]
    );
    assert_eq!(codex_items(src)[0], Item::User(String::new(), vec!["data:image/png;base64,QUJD".into()]), "只留 data: 图，自写上下文去掉");
}

/// 票 13：单个工具输出超 64 KiB 截断，不切断 UTF-8 字符。
#[test]
fn cut_result_caps_tool_output() {
    let s = "中".repeat(30_000); // 90_000 字节
    let c = cut_result(&s);
    assert!(c.ends_with("\n[…]") && c.len() <= CARRY_RESULT_MAX + "\n[…]".len(), "{}", c.len());
    assert_eq!(cut_result("ok"), "ok");
}

/// 票 13：`--version` 输出取版本号；认不出 → None（调用方回落常量）。
#[test]
fn parse_version_reads_cli_output() {
    assert_eq!(parse_version("2.1.319 (Claude Code)\n").as_deref(), Some("2.1.319"));
    assert_eq!(parse_version("codex-cli 0.46.0\n").as_deref(), Some("0.46.0"));
    assert_eq!(parse_version("error: boom"), None);
    assert_eq!(parse_version(""), None);
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
    let (id, path) = write_claude(d.path(), "/w/p", &text_items(), now(), "2.1.319").unwrap();
    assert_eq!(path, d.path().join("-w-p").join(format!("{id}.jsonl")));
    let ls = lines(&path);
    assert_eq!(ls.len(), 4);
    let mut parent = serde_json::Value::Null;
    for l in &ls {
        assert_eq!(l["parentUuid"], parent);
        assert_eq!(l["sessionId"], id.as_str());
        assert_eq!(l["cwd"], "/w/p");
        assert_eq!(l["isSidechain"], false);
        assert_eq!(l["version"], "2.1.319", "写本机版本号");
        parent = l["uuid"].clone();
    }
    assert_eq!(ls[0]["type"], "user");
    assert_eq!(ls[0]["message"]["content"], "保留的问题");
    assert_eq!(ls[1]["type"], "assistant");
    assert_eq!(ls[1]["message"]["content"][0]["text"], "保留的回答");
    assert_eq!(ls[1]["timestamp"], "2026-11-21T08:30:00.000Z");
    assert_eq!(claude_items(&std::fs::read_to_string(&path).unwrap()), text_items());
    // 已有同名文件不覆盖：再写一次是新 id 新文件。
    let (id2, _) = write_claude(d.path(), "/w/p", &text_items(), now(), "2.1.319").unwrap();
    assert_ne!(id, id2);
}

/// 票 13 写入两端共用的一组：带图用户话、另一目录跑的 shell、非 shell 工具、没输出的 shell。
fn tool_items() -> Vec<Item> {
    vec![
        Item::User("看图".into(), vec!["data:image/png;base64,QUJD".into(), "https://x/no.png".into()]),
        Item::Call(ToolCall { name: "exec_command".into(), shell: "cargo test".into(), workdir: "/w/sub".into(), result: Some("ok".into()), ..Default::default() }),
        Item::Call(ToolCall { name: "Read".into(), input: "{\"file\":\"a\"}".into(), result: Some("内容".into()), ..Default::default() }),
        Item::Call(ToolCall { name: "Bash".into(), shell: "ls".into(), ..Default::default() }),
    ]
}

const NOTE: &str = "[Read]\n```\n{\"file\":\"a\"}\n```\n\n[result]\n```\n内容\n```\n";

/// 票 13：Claude 目标——图片成 image 块；shell 成 Bash tool_use + tool_result（另一目录前缀 cd）；其余工具成助手文字。
#[test]
fn write_claude_carries_tools_and_images() {
    let d = tempfile::tempdir().unwrap();
    let (_, path) = write_claude(d.path(), "/w", &tool_items(), now(), "2.0.0").unwrap();
    let ls = lines(&path);
    assert_eq!(ls.len(), 6, "{ls:?}");
    assert_eq!(ls[0]["message"]["content"][1], serde_json::json!({"type":"image","source":{"type":"base64","media_type":"image/png","data":"QUJD"}}));
    assert_eq!(ls[0]["message"]["content"].as_array().unwrap().len(), 2, "非 data: 图不搬");
    let tu = &ls[1]["message"]["content"][0];
    assert_eq!((tu["type"].as_str(), tu["name"].as_str()), (Some("tool_use"), Some("Bash")));
    assert_eq!(tu["input"]["command"], "cd '/w/sub' && cargo test");
    assert_eq!(ls[1]["message"]["stop_reason"], "tool_use");
    assert_eq!(ls[2]["message"]["content"][0]["tool_use_id"], tu["id"]);
    assert_eq!(ls[2]["message"]["content"][0]["content"], "ok");
    assert_eq!(ls[2]["sourceToolAssistantUUID"], ls[1]["uuid"]);
    assert_eq!(ls[3]["message"]["content"][0]["text"], NOTE);
    assert_eq!(ls[5]["message"]["content"][0]["content"], "[no output was recorded]");
    let back = claude_items(&std::fs::read_to_string(&path).unwrap());
    assert_eq!(back[0], Item::User("看图".into(), vec!["data:image/png;base64,QUJD".into()]));
    assert!(matches!(&back[1], Item::Call(c) if c.shell == "cd '/w/sub' && cargo test" && c.result.as_deref() == Some("ok")));
    assert_eq!(back[2], a(NOTE));
}

/// 票 13：Codex 目标——input_image；shell 成 exec_command + function_call_output（无目录回落 cwd）；其余工具成助手文字。
#[test]
fn write_codex_carries_tools_and_images() {
    let d = tempfile::tempdir().unwrap();
    let (_, path) = write_codex(d.path(), "/w", &tool_items(), now(), "0.46.0").unwrap();
    let ls = lines(&path);
    assert_eq!(ls[0]["payload"]["cli_version"], "0.46.0");
    assert_eq!(ls[1]["payload"]["content"][1], serde_json::json!({"type":"input_image","image_url":"data:image/png;base64,QUJD"}));
    assert_eq!(ls[2]["payload"]["images"], serde_json::json!(["data:image/png;base64,QUJD"]));
    assert_eq!(ls[3]["payload"]["name"], "exec_command");
    assert_eq!(ls[3]["payload"]["arguments"], "{\"cmd\":\"cargo test\",\"workdir\":\"/w/sub\"}");
    assert_eq!(ls[4]["payload"]["call_id"], ls[3]["payload"]["call_id"]);
    assert_eq!(ls[4]["payload"]["output"], "ok");
    assert_eq!(ls[5]["payload"]["content"][0]["text"], NOTE);
    let back = codex_items(&std::fs::read_to_string(&path).unwrap());
    assert_eq!(back[0], Item::User("看图".into(), vec!["data:image/png;base64,QUJD".into()]));
    assert!(matches!(&back[1], Item::Call(c) if c.shell == "cargo test" && c.workdir == "/w/sub" && c.result.as_deref() == Some("ok")));
    assert_eq!(back[2], a(NOTE));
    assert!(matches!(&back[3], Item::Call(c) if c.shell == "ls" && c.workdir == "/w" && c.result.as_deref() == Some("[no output was recorded]")));
}

/// Codex 目标：sessions/YYYY/MM/DD/rollout-*-<id>.jsonl；首行 session_meta；provider 取 config.toml；读回同一组话。
#[test]
fn write_codex_format_and_roundtrip() {
    let d = tempfile::tempdir().unwrap();
    std::fs::write(d.path().join("config.toml"), "model_provider = \"demo_provider\"\n[profiles.x]\nmodel_provider = \"other\"\n").unwrap();
    let (id, path) = write_codex(d.path(), "/w", &text_items(), now(), "0.46.0").unwrap();
    let rel = path.strip_prefix(d.path().join("sessions")).unwrap().to_string_lossy().to_string();
    let local = now().with_timezone(&chrono::Local);
    assert!(rel.starts_with(&local.format("%Y/%m/%d/rollout-%Y-%m-%dT%H-%M-%S-").to_string()), "{rel}");
    assert!(rel.ends_with(&format!("{id}.jsonl")), "{rel}");
    let ls = lines(&path);
    assert_eq!(ls[0]["type"], "session_meta");
    assert_eq!(ls[0]["payload"]["id"], id.as_str());
    assert_eq!(ls[0]["payload"]["cwd"], "/w");
    assert_eq!(ls[0]["payload"]["model_provider"], "demo_provider");
    assert_eq!(ls[0]["payload"]["cli_version"], "0.46.0", "写本机版本号");
    assert_eq!(codex_items(&std::fs::read_to_string(&path).unwrap()), text_items());
    // 无 config.toml → openai。
    let d2 = tempfile::tempdir().unwrap();
    let (_, p2) = write_codex(d2.path(), "/w", &text_items(), now(), "0.46.0").unwrap();
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
    // 假版本执行器：只记被问的二进制名，不起进程；codex 答版本号，claude 答不出（回落常量）。
    let asked = std::sync::Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
    let asked2 = asked.clone();
    let ver: VersionOf = std::sync::Arc::new(move |bin: &str| {
        asked2.lock().unwrap().push(bin.to_string());
        (bin == "codex").then(|| "codex-cli 9.9.9\n".to_string())
    });
    let (dbr, hr, rr) = (&db, &homes, &run);
    let carry = move |from: &'static str, sid: &'static str, wd: &'static str, to: &'static str| {
        session_carry_with(dbr, from, sid, wd, to, hr, ad(), rr, ver.clone())
    };
    let words = |v: Vec<Item>| v.into_iter().filter(|i| !matches!(i, Item::Call(_))).collect::<Vec<_>>();
    let before = counts(&db).await;

    // 原文开关关：Err，不执行、不写目标存储。
    let e = carry("claude_code", "s1", "/w", "codex").await.unwrap_err();
    assert!(e.contains("transcript"), "{e}");
    // 原文开关开但源客户端读取开关关：同样拒。
    save_transcript_consent(&db, true).await.unwrap();
    assert!(carry("claude_code", "s1", "/w", "codex").await.is_err());
    assert!(ran.lock().unwrap().is_empty());
    assert!(asked.lock().unwrap().is_empty(), "闸门关时不问版本");
    assert!(!homes_dir.path().join("codex").exists() && !homes_dir.path().join("projects").exists());

    let mut enabled = std::collections::HashMap::new();
    enabled.insert("claude_code".to_string(), true);
    enabled.insert("codex".to_string(), true);
    save_consent(&db, &ClientUsageConsent { asked: true, enabled }).await.unwrap();

    // Claude → Codex：沿用源会话分组。
    let r = carry("claude_code", "s1", "/w", "codex").await.unwrap();
    let out = only_file(&homes_dir.path().join("codex").join("sessions"));
    let ls = lines(&out);
    let id = ls[0]["payload"]["id"].as_str().unwrap().to_string();
    assert_eq!(ls[0]["payload"]["cli_version"], "9.9.9", "写本机 codex --version");
    assert_eq!(r, SessionResumeResult { launched: true, command: format!("cd -- '/w' && AIDOG_KEY='teamA' codex -p 'teamA' resume '{id}'") });
    let back = codex_items(&std::fs::read_to_string(&out).unwrap());
    assert_eq!(words(back.clone()), words(claude_expected()));
    assert!(back.iter().any(|i| matches!(i, Item::Call(c) if c.shell == "ls" && c.result.as_deref() == Some("ok"))), "工具调用随转写搬过去");

    // Codex → Claude：无分组；claude --version 答不出 → 回落 2.0.0。
    let r = carry("codex", "c1", "/w", "claude_code").await.unwrap();
    let out = only_file(&homes.claude_projects);
    assert_eq!(out.parent().unwrap().file_name().unwrap(), "-w");
    let id = out.file_stem().unwrap().to_string_lossy().to_string();
    assert_eq!(r.command, format!("cd -- '/w' && claude --resume '{id}'"));
    assert_eq!(lines(&out)[0]["version"], "2.0.0");
    // 没有 shell 命令的调用（Codex 夹具的空参 shell）成助手的一段话。
    assert_eq!(
        claude_items(&std::fs::read_to_string(&out).unwrap()),
        vec![u("保留的问题"), a("保留的回答"), u("继续写测试"), a("[shell]\n```\n{}\n```\n"), a("测试已写好")]
    );
    assert_eq!(*asked.lock().unwrap(), vec!["codex".to_string(), "claude".to_string()]);

    // 交接：原文开关关也可用，且不读源文件（s2 的文件根本不存在）。
    save_transcript_consent(&db, false).await.unwrap();
    let r = carry("claude_code", "s2", "/w", "gemini_cli").await.unwrap();
    assert_eq!(r.command, "cd -- '/w' && gemini -i '请阅读 /demo/missing.jsonl 了解上一会话的上下文'");
    let r = carry("claude_code", "s1", "/w", "opencode").await.unwrap();
    assert_eq!(r.command, format!("cd -- '/w' && opencode --prompt '请阅读 {cf} 了解上一会话的上下文'"), "取主线文件，不取子代理文件");

    // 同一客户端 / 无目录 / 无记录 → Err。
    assert!(carry("codex", "c1", "/w", "codex").await.is_err());
    assert!(carry("codex", "c1", "", "gemini_cli").await.is_err());
    assert!(carry("codex", "nope", "/w", "gemini_cli").await.is_err());
    assert_eq!(ran.lock().unwrap().len(), 4);
    assert_eq!(asked.lock().unwrap().len(), 2, "交接不问版本");
    assert_eq!(counts(&db).await, before, "不写 client_usage.db 与 proxy_log");
}
