//! 票 05 Gemini CLI 所核格式夹具（结构照官方源码，内容为演示数据），tempdir 注入，
//! 禁碰真 ~/.gemini。
use std::path::PathBuf;

use super::reader_gemini::scan_file;
use super::*;

/// 行写入器：JSONL 一行一条（rel 支持子目录，如 <hash>/chats/ 或 chats/<parent>/）。
fn write_lines(dir: &std::path::Path, rel: &[&str], lines: &[serde_json::Value]) -> PathBuf {
    let mut p = dir.to_path_buf();
    for seg in rel {
        p = p.join(seg);
    }
    if let Some(parent) = p.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    let mut s = String::new();
    for l in lines {
        s.push_str(&serde_json::to_string(l).unwrap());
        s.push('\n');
    }
    std::fs::write(&p, s).unwrap();
    p
}

fn tmpdir() -> tempfile::TempDir {
    tempfile::tempdir().unwrap()
}

/// 与 test_reader / test_reader_codex 共用一把锁（全局 IMPORT_STATE 只有一份；锁本体在 mod.rs）。
use super::TEST_IMPORT_STATE_LOCK as STATE_LOCK;

async fn count_calls(db: &aidog_db::Db) -> i64 {
    db.call_read_client_usage_traced(None, std::panic::Location::caller(), |conn| {
        conn.query_row("SELECT COUNT(*) FROM client_call", [], |r| r.get(0))
    })
    .await
    .unwrap()
}

fn consent_on(clients: &[&str]) -> ClientUsageConsent {
    let mut c = ClientUsageConsent { asked: true, ..Default::default() };
    for x in clients {
        c.enabled.insert(x.to_string(), true);
    }
    c
}

/// 首行元数据对象（无 $set 包裹，appendRecord 初始元数据形态）。
fn meta(session_id: &str, kind: Option<&str>, summary: Option<&str>) -> serde_json::Value {
    let mut m = serde_json::json!({
        "sessionId": session_id,
        "projectHash": "abc123",
        "startTime": "2026-11-19T08:00:00.000Z",
        "lastUpdated": "2026-11-19T08:05:00.000Z",
    });
    if let Some(k) = kind {
        m["kind"] = serde_json::json!(k);
    }
    if let Some(s) = summary {
        m["summary"] = serde_json::json!(s);
    }
    m
}

/// gemini 消息行：tokens 形态照 TokensSummary（input/output/cached/thoughts/tool/total）。
fn gemini_msg(
    id: &str,
    ts: &str,
    model: &str,
    input: i64,
    cached: i64,
    output: i64,
    thoughts: i64,
) -> serde_json::Value {
    serde_json::json!({
        "id": id,
        "timestamp": ts,
        "type": "gemini",
        "content": [],
        "tokens": {
            "input": input,
            "output": output,
            "cached": cached,
            "thoughts": thoughts,
            "total": input + output + thoughts,
        },
        "model": model,
    })
}

fn user_msg(id: &str, ts: &str, text: &str) -> serde_json::Value {
    serde_json::json!({
        "id": id,
        "timestamp": ts,
        "type": "user",
        "content": text,
    })
}

/// 非 gemini / 非消息行：$set、$patch、$rewindTo 各一。
fn set_line(extra: serde_json::Value) -> serde_json::Value {
    serde_json::json!({ "$set": extra })
}

fn patch_line() -> serde_json::Value {
    serde_json::json!({ "$patch": { "id": "m-1" } })
}

fn rewind_line() -> serde_json::Value {
    serde_json::json!({ "$rewindTo": "m-1" })
}

#[test]
fn scan_normal_gemini() {
    let d = tmpdir();
    let _p = write_lines(
        d.path(),
        &["projhash", "chats", "session-20261119T0800-abcdef12.jsonl"],
        &[
            meta("sess-g1", None, None),
            user_msg("u-1", "2026-11-19T08:00:01.000Z", "帮我看看这个函数"),
            gemini_msg("m-1", "2026-11-19T08:00:02.000Z", "gemini-2.5-pro", 10_000, 2_000, 500, 100),
            patch_line(),
            rewind_line(),
            set_line(serde_json::json!({ "summary": "会话摘要标题" })),
        ],
    );
    let out = scan_file(&_p, 0, "").unwrap();
    assert_eq!(out.entries.len(), 1, "仅 gemini 消息计入");
    let e = &out.entries[0];
    // input − cached 计费口径；cached → cache_read；thoughts → reasoning。
    assert_eq!(e.input_tokens, 8_000);
    assert_eq!(e.cache_read_tokens, 2_000);
    assert_eq!(e.output_tokens, 500);
    assert_eq!(e.reasoning_tokens, 100);
    assert_eq!(e.cache_write_tokens, 0);
    assert_eq!(e.model, "gemini-2.5-pro");
    assert!(!e.is_sidechain);
    assert_eq!(e.session_id, "sess-g1");
    assert_eq!(out.session_id, "sess-g1");
    assert_eq!(out.first_user_prompt.as_deref(), Some("帮我看看这个函数"));
    let want_ms =
        chrono::DateTime::parse_from_rfc3339("2026-11-19T08:00:02.000Z").unwrap().timestamp_millis();
    assert_eq!(out.first_ms, want_ms);
    assert_eq!(out.last_ms, want_ms);
    assert_eq!(out.parse_errors, 0);
}

#[test]
fn summary_title_and_user_prompt_fallback() {
    let d = tmpdir();
    let p = write_lines(
        d.path(),
        &["h", "chats", "s1.jsonl"],
        &[
            meta("s-a", None, Some("会话摘要标题")),
            user_msg("u-1", "2026-11-19T08:00:01.000Z", "首条提示"),
            gemini_msg("m-1", "2026-11-19T08:00:02.000Z", "gemini-2.5-flash", 100, 0, 10, 0),
        ],
    );
    let out = scan_file(&p, 0, "").unwrap();
    assert_eq!(out.title.as_ref().map(|(s, t)| (s.as_str(), t.as_str())), Some(("summary", "会话摘要标题")));

    // 无 summary：回落首条用户提示（>60 字截断）。
    let long = "长".repeat(80);
    let p2 = write_lines(
        d.path(),
        &["h", "chats", "s2.jsonl"],
        &[
            meta("s-b", None, None),
            user_msg("u-1", "2026-11-19T08:00:01.000Z", &long),
            gemini_msg("m-1", "2026-11-19T08:00:02.000Z", "gemini-2.5-flash", 100, 0, 10, 0),
        ],
    );
    let out2 = scan_file(&p2, 0, "").unwrap();
    assert!(out2.title.is_none());
    // truncate_name 60 字 + 省略号 = 61 字符。
    assert_eq!(out2.first_user_prompt.as_deref().map(|s| s.chars().count()), Some(61));
}

#[test]
fn subagent_sidechain_and_recursive_dir() {
    let d = tmpdir();
    let _p = write_lines(
        d.path(),
        &["h", "chats", "parent-sess", "subagent-id.jsonl"],
        &[
            meta("sub-1", Some("subagent"), None),
            gemini_msg("m-1", "2026-11-19T08:00:02.000Z", "gemini-2.5-pro", 100, 0, 10, 0),
        ],
    );
    let out = scan_file(&_p, 0, "").unwrap();
    assert!(out.entries[0].is_sidechain, "kind=subagent → sidechain");
}

#[test]
fn incremental_and_idempotent_gemini() {
    let d = tmpdir();
    let p = write_lines(
        d.path(),
        &["h", "chats", "s3.jsonl"],
        &[
            meta("sess-g2", None, None),
            user_msg("u-1", "2026-11-19T08:00:01.000Z", "你好"),
            gemini_msg("m-1", "2026-11-19T08:00:02.000Z", "gemini-2.5-pro", 100, 0, 10, 0),
        ],
    );
    let first = scan_file(&p, 0, "").unwrap();
    assert_eq!(first.entries.len(), 1);
    // 幂等：从 new_offset 续读零新增。
    let second = scan_file(&p, first.new_offset, &first.last_msg_id).unwrap();
    assert_eq!(second.entries.len(), 0, "增量续读零新增");
    // 追加一行后增量读到 1 条。
    let mut s = std::fs::read_to_string(&p).unwrap();
    s.push_str(&serde_json::to_string(&gemini_msg(
        "m-2",
        "2026-11-19T08:01:00.000Z",
        "gemini-2.5-pro",
        50,
        0,
        5,
        0,
    )).unwrap());
    s.push('\n');
    std::fs::write(&p, s).unwrap();
    let third = scan_file(&p, first.new_offset, &first.last_msg_id).unwrap();
    assert_eq!(third.entries.len(), 1, "追加行增量进入");
    assert_eq!(third.entries[0].model, "gemini-2.5-pro");
    assert_eq!(third.entries[0].session_id, "sess-g2", "增量行继承元数据 sessionId");
    assert_eq!(third.session_id, "sess-g2");
    // 增量扫描也带出 workdir/标题？标题以行内出现为准：增量段无 summary 行 → None（保留已存值）。
    assert!(third.title.is_none());
}

#[test]
fn truncated_detection_gemini() {
    let d = tmpdir();
    let p = write_lines(d.path(), &["h", "chats", "s4.jsonl"], &[meta("s", None, None)]);
    let size = std::fs::metadata(&p).unwrap().len();
    assert!(matches!(
        scan_file(&p, size + 10, ""),
        Err(super::reader::ScanErr::Truncated)
    ));
}

#[test]
fn corrupt_lines_and_zero_usage_skipped_gemini() {
    let d = tmpdir();
    let mut p = d.path().to_path_buf();
    for seg in ["h", "chats"] {
        p = p.join(seg);
    }
    std::fs::create_dir_all(&p).unwrap();
    p = p.join("s5.jsonl");
    let mut s = String::new();
    s.push_str(&serde_json::to_string(&meta("s-c", None, None)).unwrap());
    s.push('\n');
    s.push_str("{broken json\n"); // 损坏行 → parse_errors
    // 零用量 gemini 消息（tokens 全 0）不计。
    s.push_str(&serde_json::to_string(&gemini_msg(
        "m-zero", "2026-11-19T08:00:02.000Z", "gemini-2.5-flash", 0, 0, 0, 0,
    )).unwrap());
    s.push('\n');
    // tokens 缺失的 gemini 消息不计。
    s.push_str(&serde_json::to_string(&serde_json::json!({
        "id": "m-notok", "timestamp": "2026-11-19T08:00:03.000Z", "type": "gemini", "content": []
    })).unwrap());
    s.push('\n');
    std::fs::write(&p, s).unwrap();
    let out = scan_file(&p, 0, "").unwrap();
    assert_eq!(out.parse_errors, 1, "损坏行计错数");
    assert_eq!(out.entries.len(), 0, "零用量与缺 tokens 不计");
}

/// 折算：gemini_cli → gemini 平台条目（gemini-2.5-pro 有价 → cost>0；
/// 未知模型条目缺失 → cost=None 未定价，不回落兜底单价）。
#[tokio::test]
async fn gemini_import_e2e() {
    let _g = STATE_LOCK.lock().await;
    let (db, _dir) = aidog_db::test_support::four_file_db().await;
    let d = tmpdir();
    let _p = write_lines(
        d.path(),
        &["h1", "chats", "s10.jsonl"],
        &[
            meta("sess-g10", None, None),
            user_msg("u-1", "2026-11-19T08:00:01.000Z", "查一下"),
            gemini_msg("m-1", "2026-11-19T08:00:02.000Z", "gemini-2.5-pro", 100_000, 20_000, 10_000, 1_000),
            gemini_msg("m-2", "2026-11-19T08:00:03.000Z", "gemini-x-unknown", 100, 0, 10, 0),
        ],
    );
    save_consent(&db, &consent_on(&["gemini_cli"])).await.unwrap();
    let root = Some(d.path().to_path_buf());
    let clients = vec!["gemini_cli".to_string()];
    run_import(&db, root.clone(), &clients, false).await.unwrap();
    assert_eq!(count_calls(&db).await, 2, "两条 gemini 消息入库");
    // 幂等：重复导入零新增。
    run_import(&db, root.clone(), &clients, false).await.unwrap();
    assert_eq!(count_calls(&db).await, 2, "幂等零新增");
    // 折算：gemini-2.5-pro 走 gemini 条目，cost > 0；未知模型 cost 为 NULL。
    let (cost, unknown_cost): (Option<f64>, Option<f64>) = db
        .call_read_client_usage_traced(None, std::panic::Location::caller(), |conn| {
            let a: Option<f64> = conn
                .query_row("SELECT cost FROM client_call WHERE model='gemini-2.5-pro'", [], |r| r.get(0))
                .unwrap();
            let b: Option<f64> = conn
                .query_row("SELECT cost FROM client_call WHERE model='gemini-x-unknown'", [], |r| r.get(0))
                .unwrap();
            Ok((a, b))
        })
        .await
        .unwrap();
    let c = cost.expect("gemini 条目命中应已折算");
    assert!(c > 0.0, "gemini-2.5-pro cost > 0，实际 {c}");
    assert!(unknown_cost.is_none(), "未知模型未定价不折算");
}

/// 票 06 工作目录：`tmp/<slug>/.project_root`（内容 trim）→ workdir；
/// 子代理文件在 `chats/<parent>/` 下同样取 `<slug>` 层；缺失 → 空。
#[test]
fn workdir_from_project_root_gemini() {
    let d = tmpdir();
    let p = write_lines(d.path(), &["slug-a", "chats", "s20.jsonl"], &[meta("s-w1", None, None)]);
    let sub = write_lines(
        d.path(),
        &["slug-a", "chats", "s-w1", "sub.jsonl"],
        &[meta("sub-w1", Some("subagent"), None)],
    );
    std::fs::write(d.path().join("slug-a").join(".project_root"), "/work/gem\n").unwrap();
    assert_eq!(scan_file(&p, 0, "").unwrap().workdir, "/work/gem", "读 .project_root 并 trim");
    assert_eq!(scan_file(&sub, 0, "").unwrap().workdir, "/work/gem", "子代理文件取 slug 层");
    let size = std::fs::metadata(&p).unwrap().len();
    assert_eq!(scan_file(&p, size, "").unwrap().workdir, "/work/gem", "增量扫描同样带出");
    let old = write_lines(d.path(), &["oldhash", "chats", "s21.jsonl"], &[meta("s-w2", None, None)]);
    assert_eq!(scan_file(&old, 0, "").unwrap().workdir, "", "无 .project_root → 空");
}

/// 已入库的空 workdir 会话：补出 `.project_root` 后重扫被补写。
#[tokio::test]
async fn workdir_backfilled_on_rescan_gemini() {
    let _g = STATE_LOCK.lock().await;
    let (db, _dir) = aidog_db::test_support::four_file_db().await;
    let d = tmpdir();
    write_lines(
        d.path(),
        &["slug-b", "chats", "s22.jsonl"],
        &[
            meta("sess-w3", None, None),
            gemini_msg("m-1", "2026-11-19T08:00:02.000Z", "gemini-2.5-pro", 100, 0, 10, 0),
        ],
    );
    save_consent(&db, &consent_on(&["gemini_cli"])).await.unwrap();
    let root = Some(d.path().to_path_buf());
    let clients = vec!["gemini_cli".to_string()];
    let workdir = |db: aidog_db::Db| async move {
        db.call_read_client_usage_traced(None, std::panic::Location::caller(), |conn| {
            conn.query_row("SELECT workdir FROM client_session WHERE session_id='sess-w3'", [], |r| {
                r.get::<_, String>(0)
            })
        })
        .await
        .unwrap()
    };
    run_import(&db, root.clone(), &clients, false).await.unwrap();
    assert_eq!(workdir(db.clone()).await, "", "无 .project_root 入库为空");
    std::fs::write(d.path().join("slug-b").join(".project_root"), "/work/later").unwrap();
    run_import(&db, root.clone(), &clients, false).await.unwrap();
    assert_eq!(workdir(db.clone()).await, "/work/later", "重扫补写");
}
