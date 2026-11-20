//! 票 04 Codex 读取器测试：真实格式夹具（内容脱敏），tempdir 注入，禁碰真 ~/.codex。
use std::path::PathBuf;

use super::reader_codex::scan_file;
use super::*;

/// 行写入器：JSONL 一行一条（rel 支持日期子目录）。
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

/// 与 test_reader 共用一把锁（全局 IMPORT_STATE 只有一份；锁本体在 mod.rs）。
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

/// token_count 事件：last = 给定用量（total 与计数无关，夹具缩写）。
fn tok(ts: &str, i: i64, o: i64, cr: i64, ro: i64) -> serde_json::Value {
    serde_json::json!({
        "timestamp": ts,
        "type": "event_msg",
        "payload": {
            "type": "token_count",
            "info": {
                "last_token_usage": {
                    "input_tokens": i, "cached_input_tokens": cr,
                    "output_tokens": o, "reasoning_output_tokens": ro, "total_tokens": i + o
                }
            }
        }
    })
}

/// turn_context 行：模型来源。
fn ctx(model: &str) -> serde_json::Value {
    serde_json::json!({"type":"turn_context","payload":{"model": model, "turn_id": "t1"}})
}

/// session_meta 行（forked 可选）。
fn meta(id: &str, cwd: &str, forked: Option<&str>) -> serde_json::Value {
    let mut payload = serde_json::json!({
        "id": id, "timestamp": "2026-11-19T08:00:00.000Z", "cwd": cwd,
        "originator": "codex-tui", "cli_version": "0.135.0"
    });
    if let Some(f) = forked {
        payload["forked_from_id"] = serde_json::json!(f);
    }
    serde_json::json!({"type":"session_meta","payload": payload})
}

/// user_message 行：标题回落来源。
fn user_msg(ts: &str, text: &str) -> serde_json::Value {
    serde_json::json!({
        "timestamp": ts, "type": "event_msg",
        "payload": {"type": "user_message", "message": text}
    })
}

/// 正常解析：token 用量、毫秒时间戳、模型来自 turn_context、meta 会话 id / cwd、标题回落。
#[test]
fn scan_normal_codex() {
    let d = tmpdir();
    let p = write_lines(
        d.path(),
        &["2026", "11", "19", "rollout-x.jsonl"],
        &[
            meta("sess-codex-1", "/work/proj", None),
            user_msg("2026-11-19T08:00:01.000Z", "帮我修登录页面的一个很长很长很长很长很长很长很长很长很长的 bug 说明继续继续继续继续继续继续继续继续"),
            ctx("gpt-5.1"),
            tok("2026-11-19T08:00:02.000Z", 100, 20, 30, 5),
        ],
    );
    let o = scan_file(&p, 0, "", Some(d.path())).unwrap();
    assert_eq!(o.entries.len(), 1);
    let e = &o.entries[0];
    assert_eq!(e.model, "gpt-5.1");
    assert_eq!(e.input_tokens, 100);
    assert_eq!(e.cache_read_tokens, 30, "cached_input_tokens → cache_read");
    assert_eq!(e.cache_write_tokens, 0, "codex 无 cache write 概念");
    assert_eq!(e.reasoning_tokens, 5);
    assert_eq!(e.call_time_ms, 1795075202000);
    assert!(!e.is_sidechain);
    assert_eq!(o.session_id, "sess-codex-1");
    assert_eq!(o.workdir, "/work/proj");
    let t = o.first_user_prompt.unwrap();
    assert!(t.chars().count() <= 60, "标题回落截 60 字：{t}");
    assert!(t.chars().count() > 30);
    assert_eq!(o.parse_errors, 0);
}

/// 会话 id 回落文件名：meta 缺失时从文件名 uuid 段取。
#[test]
fn session_id_from_filename_fallback() {
    let d = tmpdir();
    let uuid = "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee";
    let p = write_lines(
        d.path(),
        &[format!("rollout-{}-{}.jsonl", "x".repeat(19), uuid).as_str()] as &[&str],
        &[tok("2026-11-19T08:00:02.000Z", 100, 20, 30, 5)],
    );
    let o = scan_file(&p, 0, "", Some(d.path())).unwrap();
    assert_eq!(o.entries.len(), 1);
    assert_eq!(o.entries[0].session_id, uuid, "meta 缺失回落文件名 uuid");
    assert_eq!(o.session_id, uuid);
}

/// 连续重复 token_count 事件只计一次（UI 刷新重复发射）；last_msg_id 持久化最终签名。
#[test]
fn consecutive_duplicate_counted_once() {
    let d = tmpdir();
    let p = write_lines(
        d.path(),
        &["a.jsonl"],
        &[
            ctx("gpt-5.1"),
            tok("2026-11-19T08:00:02.000Z", 100, 20, 30, 5),
            tok("2026-11-19T08:00:03.000Z", 100, 20, 30, 5),
            tok("2026-11-19T08:00:04.000Z", 200, 40, 60, 10),
        ],
    );
    let o = scan_file(&p, 0, "", Some(d.path())).unwrap();
    assert_eq!(o.entries.len(), 2, "连续重复跳过");
    assert_eq!(o.entries[0].input_tokens, 100);
    assert_eq!(o.entries[1].input_tokens, 200);
    assert_eq!(o.last_msg_id, "200,40,60,10", "持久化 client_file_progress.last_msg_id");
}

/// 增量续读：从持久化签名起连续重复不再计；追加新事件只进新增；幂等重读零条目。
#[test]
fn incremental_and_idempotent() {
    let d = tmpdir();
    let p = write_lines(
        d.path(),
        &["a.jsonl"],
        &[
            ctx("gpt-5.1"),
            tok("2026-11-19T08:00:02.000Z", 100, 20, 30, 5),
        ],
    );
    let o1 = scan_file(&p, 0, "", Some(d.path())).unwrap();
    assert_eq!(o1.entries.len(), 1);
    let o1b = scan_file(&p, o1.new_offset, &o1.last_msg_id, Some(d.path())).unwrap();
    assert_eq!(o1b.entries.len(), 0, "幂等重读");
    // 追加：同值重复一次 + 新值一次 → 只计新值。
    let mut s = std::fs::read_to_string(&p).unwrap();
    s.push_str(&serde_json::to_string(&tok("2026-11-19T08:00:03.000Z", 100, 20, 30, 5)).unwrap());
    s.push('\n');
    s.push_str(&serde_json::to_string(&tok("2026-11-19T08:00:04.000Z", 200, 40, 60, 10)).unwrap());
    s.push('\n');
    std::fs::write(&p, s).unwrap();
    let o2 = scan_file(&p, o1.new_offset, &o1.last_msg_id, Some(d.path())).unwrap();
    assert_eq!(o2.entries.len(), 1, "只进新增（重复跳过）");
    assert_eq!(o2.entries[0].input_tokens, 200);
}

/// 截断判定：size < offset → Truncated。
#[test]
fn truncated_detection() {
    let d = tmpdir();
    let p = write_lines(
        d.path(),
        &["a.jsonl"],
        &[ctx("gpt-5.1"), tok("2026-11-19T08:00:02.000Z", 100, 20, 30, 5)],
    );
    let o = scan_file(&p, 0, "", Some(d.path())).unwrap();
    assert_eq!(o.entries.len(), 1);
    let full = std::fs::read(&p).unwrap();
    std::fs::write(&p, &full[..full.len() / 2]).unwrap();
    assert!(matches!(scan_file(&p, o.new_offset, "", Some(d.path())), Err(reader::ScanErr::Truncated)));
}

/// 损坏行跳过并计 parse_errors；零用量事件不计。
#[test]
fn corrupt_lines_and_zero_usage_skipped() {
    let d = tmpdir();
    let p = d.path().join("c.jsonl");
    let mut s = String::new();
    s.push_str("{\"type\":\"event_msg\" \"broken\":\n");
    s.push_str(&serde_json::to_string(&tok("2026-11-19T08:00:00Z", 0, 0, 0, 0)).unwrap());
    s.push('\n');
    s.push_str(&serde_json::to_string(&tok("2026-11-19T08:00:01Z", 10, 1, 0, 0)).unwrap());
    s.push('\n');
    std::fs::write(&p, s).unwrap();
    let o = scan_file(&p, 0, "", Some(d.path())).unwrap();
    assert_eq!(o.parse_errors, 1, "坏行 +1");
    assert_eq!(o.entries.len(), 1, "零用量事件不计");
}

/// 分叉去重：父文件在 → 复制前缀（与父 token 序列逐条一致的头部）不计，只计自有调用。
#[test]
fn fork_dedup_with_parent() {
    let d = tmpdir();
    // 父会话：两个 token 事件。
    write_lines(
        d.path(),
        &["2026", "11", "18", "rollout-p.jsonl"],
        &[
            ctx("gpt-5.1"),
            tok("2026-11-18T10:00:00.000Z", 100, 10, 0, 0),
            tok("2026-11-18T10:00:01.000Z", 200, 20, 0, 0),
        ],
    );
    // 子会话：复制父两事件 + 自有一事件。
    let child_path = write_lines(
        d.path(),
        &["2026", "11", "19", "rollout-c.jsonl"],
        &[
            meta("child-session-1", "/work/fork-proj", Some("p")),
            ctx("gpt-5.1"),
            tok("2026-11-18T10:00:00.000Z", 100, 10, 0, 0),
            tok("2026-11-18T10:00:01.000Z", 200, 20, 0, 0),
            tok("2026-11-19T10:00:02.000Z", 300, 30, 0, 0),
        ],
    );
    let o = scan_file(&child_path, 0, "", Some(d.path())).unwrap();
    assert_eq!(o.entries.len(), 1, "复制前缀两事件跳过，只计自有");
    assert_eq!(o.entries[0].input_tokens, 300);
    let o2 = scan_file(&child_path, 0, "", None).unwrap();
    assert_eq!(o2.entries.len(), 3, "无父查找根按普通读");
}

/// 分叉去重按 token 签名比对（不含时间戳）：复制段时间戳与父漂移仍去重，只计自有调用。
/// 行为锁定用例（现有实现按 42 个真实分叉样本验证过）。
#[test]
fn fork_dedup_ignores_copy_ts_drift() {
    let d = tmpdir();
    // 父会话：两个 token 事件。
    write_lines(
        d.path(),
        &["2026", "11", "18", "rollout-p.jsonl"],
        &[
            ctx("gpt-5.1"),
            tok("2026-11-18T10:00:00.000Z", 100, 10, 0, 0),
            tok("2026-11-18T10:00:01.000Z", 200, 20, 0, 0),
        ],
    );
    // 子会话：复制父两事件（token 值一致、时间戳改写为子侧时刻）+ 自有一事件。
    let child_path = write_lines(
        d.path(),
        &["2026", "11", "19", "rollout-c.jsonl"],
        &[
            meta("child-session-2", "/work/fork-proj", Some("p")),
            ctx("gpt-5.1"),
            tok("2026-11-19T09:00:00.000Z", 100, 10, 0, 0),
            tok("2026-11-19T09:00:01.000Z", 200, 20, 0, 0),
            tok("2026-11-19T10:00:02.000Z", 300, 30, 0, 0),
        ],
    );
    let o = scan_file(&child_path, 0, "", Some(d.path())).unwrap();
    assert_eq!(o.entries.len(), 1, "时间戳漂移的复制前缀仍跳过，只计自有");
    assert_eq!(o.entries[0].input_tokens, 300);
}

/// 找不到父会话文件 → 按普通会话读。
#[test]
fn fork_dedup_parent_missing() {
    let d = tmpdir();
    let child_path = write_lines(
        d.path(),
        &["2026", "11", "19", "rollout-c.jsonl"],
        &[
            meta("child-session-1", "/work/fork-proj", Some("q")),
            ctx("gpt-5.1"),
            tok("2026-11-19T10:00:00.000Z", 100, 10, 0, 0),
            tok("2026-11-19T10:00:01.000Z", 200, 20, 0, 0),
        ],
    );
    let o = scan_file(&child_path, 0, "", Some(d.path())).unwrap();
    assert_eq!(o.entries.len(), 2, "父文件不存在 → 全计");
}

/// 导入链 e2e：枚举（日期子目录递归）、折算（openai 条目）、幂等、增量、截断重建。
#[tokio::test]
async fn codex_import_e2e() {
    let _g = STATE_LOCK.lock().await;
    let (db, _dir) = aidog_db::test_support::four_file_db().await;
    let d = tmpdir();
    let ap = write_lines(
        d.path(),
        &["2026", "11", "19", "rollout-a.jsonl"],
        &[
            meta("sess-1", "/work/p", None),
            ctx("gpt-5.1"),
            tok("2026-11-19T08:00:02.000Z", 1_000_000, 100_000, 0, 0),
        ],
    );
    write_lines(
        d.path(),
        &["2026", "11", "18", "rollout-b.jsonl"],
        &[
            ctx("gpt-5.1"),
            tok("2026-11-18T08:00:02.000Z", 10, 1, 0, 0),
        ],
    );
    save_consent(&db, &consent_on(&["codex"])).await.unwrap();
    let root = Some(d.path().to_path_buf());
    let clients = vec!["codex".to_string()];
    run_import(&db, root.clone(), &clients, false).await.unwrap();
    assert_eq!(count_calls(&db).await, 2, "递归枚举日期子目录");
    // 幂等：重复导入零新增。
    run_import(&db, root.clone(), &clients, false).await.unwrap();
    assert_eq!(count_calls(&db).await, 2, "幂等零新增");
    // 折算：gpt-5.1 走 openai 条目，cost > 0。
    let cost: Option<f64> = db
        .call_read_client_usage_traced(None, std::panic::Location::caller(), |conn| {
            conn.query_row("SELECT cost FROM client_call WHERE model='gpt-5.1' LIMIT 1", [], |r| r.get(0))
        })
        .await
        .unwrap();
    let c = cost.expect("openai 条目命中应已折算");
    assert!(c > 0.0, "gpt-5.1 cost > 0，实际 {c}");
    // 增量：追加新 token 事件，只进新增。
    let mut s = std::fs::read_to_string(&ap).unwrap();
    s.push_str(&serde_json::to_string(&tok("2026-11-19T08:00:03.000Z", 50, 5, 0, 0)).unwrap());
    s.push('\n');
    std::fs::write(&ap, s).unwrap();
    run_import(&db, root.clone(), &clients, false).await.unwrap();
    assert_eq!(count_calls(&db).await, 3, "增量只进新增");
    // 截断重建：文件重写为只剩 meta+ctx+首条 token。
    let short = std::fs::read_to_string(&ap).unwrap();
    let head: Vec<&str> = short.lines().take(3).collect();
    std::fs::write(&ap, format!("{}\n", head.join("\n"))).unwrap();
    run_import(&db, root.clone(), &clients, false).await.unwrap();
    assert_eq!(count_calls(&db).await, 2, "截断重建：a 重读 1 条 + b 保留 1 条");
}