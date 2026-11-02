// 会话名解析：全部用 tempdir fixture，不读真 ~/.claude。
use std::collections::HashMap;
use std::io::Write;

use aidog_db::Db;

use super::session_name::*;

fn write(path: &std::path::Path, lines: &[&str]) {
    std::fs::write(path, lines.iter().map(|l| format!("{l}\n")).collect::<String>()).unwrap();
}
fn append(path: &std::path::Path, s: &str) {
    let mut f = std::fs::OpenOptions::new().append(true).open(path).unwrap();
    f.write_all(s.as_bytes()).unwrap();
}
const AI: &str = r#"{"type":"ai-title","aiTitle":"自动标题"}"#;
const CUSTOM: &str = r#"{"type":"custom-title","customTitle":"我起的名"}"#;
const NOISE: &str = r#"{"type":"user","message":{"content":"hi"}}"#;

fn scan(path: &std::path::Path, prev: ClaudeCache) -> ClaudeCache {
    scan_transcript(path, prev).unwrap()
}

#[test]
fn truncate_cases() {
    assert_eq!(truncate_name("  hello \n second line"), "hello");
    assert_eq!(truncate_name("\n\n  首行  \n次行"), "首行");
    assert_eq!(truncate_name(""), "");
    let s24 = "字".repeat(24);
    assert_eq!(truncate_name(&s24), s24);
    let s25 = "字".repeat(25);
    assert_eq!(truncate_name(&s25), format!("{}…", "字".repeat(24)));
    assert_eq!(truncate_name(&s25).chars().count(), 25);
    // emoji 按 Unicode 字符计，不切断
    let e = "😀".repeat(30);
    assert_eq!(truncate_name(&e), format!("{}…", "😀".repeat(24)));
}

#[test]
fn claude_priority_and_sources() {
    let d = tempfile::tempdir().unwrap();
    let p = d.path().join("t.jsonl");
    // 只有 ai-title
    write(&p, &[NOISE, AI]);
    let c = scan(&p, ClaudeCache::default());
    assert_eq!((c.name.as_str(), c.source.as_str()), ("自动标题", "ai_title"));
    // 只有 custom-title
    write(&p, &[NOISE, CUSTOM]);
    let c = scan(&p, ClaudeCache::default());
    assert_eq!((c.name.as_str(), c.source.as_str()), ("我起的名", "custom_title"));
    // ai-title 在 custom-title 之后：custom 不被覆盖
    write(&p, &[CUSTOM, AI]);
    let c = scan(&p, ClaudeCache::default());
    assert_eq!((c.name.as_str(), c.source.as_str()), ("我起的名", "custom_title"));
    // 都没有
    write(&p, &[NOISE]);
    assert_eq!(scan(&p, ClaudeCache::default()).name, "");
    // 多次 ai-title 取最后一次
    write(&p, &[AI, r#"{"type":"ai-title","aiTitle":"新标题"}"#]);
    assert_eq!(scan(&p, ClaudeCache::default()).name, "新标题");
}

#[test]
fn claude_incremental_rename_truncate_and_bad_lines() {
    let d = tempfile::tempdir().unwrap();
    let p = d.path().join("t.jsonl");
    write(&p, &[AI]);
    let c1 = scan(&p, ClaudeCache::default());
    assert_eq!(c1.name, "自动标题");
    assert_eq!(c1.offset, std::fs::metadata(&p).unwrap().len());
    // 无新内容：原样
    assert_eq!(scan(&p, c1.clone()), c1);
    // 中途 /rename：只读新增部分（坏行跳过）
    append(&p, "not json at all\n");
    append(&p, &format!("{CUSTOM}\n"));
    let c2 = scan(&p, c1.clone());
    assert_eq!((c2.name.as_str(), c2.source.as_str()), ("我起的名", "custom_title"));
    assert_eq!(c2.offset, std::fs::metadata(&p).unwrap().len());
    // 文件变短（被替换）→ 从头重读
    write(&p, &[r#"{"type":"ai-title","aiTitle":"短"}"#]);
    let c3 = scan(&p, c2);
    assert_eq!((c3.name.as_str(), c3.source.as_str()), ("短", "ai_title"));
    // 末行写了一半（无换行且不是合法 JSON）→ 不消费，补全后再读到
    let before = std::fs::metadata(&p).unwrap().len();
    append(&p, r#"{"type":"custom-title","customTi"#);
    let c4 = scan(&p, c3.clone());
    assert_eq!(c4.offset, before);
    assert_eq!(c4.name, "短");
    append(&p, "tle\":\"补全\"}\n");
    assert_eq!(scan(&p, c4).name, "补全");
}

#[test]
fn claude_long_name_truncated() {
    let d = tempfile::tempdir().unwrap();
    let p = d.path().join("t.jsonl");
    let long = "长".repeat(30);
    write(&p, &[&format!(r#"{{"type":"ai-title","aiTitle":"{long}"}}"#)]);
    assert_eq!(scan(&p, ClaudeCache::default()).name, format!("{}…", "长".repeat(24)));
}

#[test]
fn missing_file_is_error() {
    let d = tempfile::tempdir().unwrap();
    assert!(scan_transcript(&d.path().join("nope.jsonl"), ClaudeCache::default()).is_err());
}

fn vars(pairs: &[(&str, &str)]) -> HashMap<String, String> {
    pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
}

#[tokio::test]
async fn resolve_uses_and_refreshes_db_cache() {
    let (db, _dir): (Db, _) = aidog_db::test_support::three_file_db().await;
    let d = tempfile::tempdir().unwrap();
    let p = d.path().join("t.jsonl");
    write(&p, &[AI]);
    let v = vars(&[("session_id", "s1"), ("transcript_path", p.to_str().unwrap())]);
    assert_eq!(resolve(&db, &v).await, "自动标题");
    let cached = aidog_db::get_session_name_cache(&db, "claude_code", "s1").await.unwrap().unwrap();
    let c: ClaudeCache = serde_json::from_str(&cached).unwrap();
    assert_eq!((c.name.as_str(), c.offset), ("自动标题", std::fs::metadata(&p).unwrap().len()));
    // /rename 之后再次解析：名字变了、缓存更新
    append(&p, &format!("{CUSTOM}\n"));
    assert_eq!(resolve(&db, &v).await, "我起的名");
    let c2: ClaudeCache = serde_json::from_str(
        &aidog_db::get_session_name_cache(&db, "claude_code", "s1").await.unwrap().unwrap(),
    )
    .unwrap();
    assert_eq!(c2.source, "custom_title");
}

#[tokio::test]
async fn resolve_failures_yield_empty() {
    let (db, _dir) = aidog_db::test_support::three_file_db().await;
    let d = tempfile::tempdir().unwrap();
    // 缺 session_id / transcript_path
    assert_eq!(resolve(&db, &vars(&[("project", "x")])).await, "");
    // 文件不存在
    let v = vars(&[("session_id", "s2"), ("transcript_path", d.path().join("no.jsonl").to_str().unwrap())]);
    assert_eq!(resolve(&db, &v).await, "");
}
