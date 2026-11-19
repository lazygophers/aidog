//! 票 02 读取器测试：真实格式夹具（内容脱敏），tempdir 注入，禁碰真 ~/.claude。
use std::path::PathBuf;

use super::reader::{ScanErr, scan_file};
use super::*;

fn write_lines(dir: &std::path::Path, name: &str, lines: &[serde_json::Value]) -> PathBuf {
    let p = dir.join(name);
    let mut s = String::new();
    for l in lines {
        s.push_str(&serde_json::to_string(l).unwrap());
        s.push('\n');
    }
    std::fs::write(&p, s).unwrap();
    p
}

#[allow(clippy::too_many_arguments)]
fn assistant(
    id: &str,
    sid: &str,
    model: &str,
    ts: &str,
    in_tok: i64,
    out_tok: i64,
    cr: i64,
    cw: i64,
) -> serde_json::Value {
    serde_json::json!({
        "type": "assistant",
        "sessionId": sid,
        "isSidechain": false,
        "cwd": "/work/proj",
        "timestamp": ts,
        "message": {
            "id": id,
            "model": model,
            "usage": {
                "input_tokens": in_tok,
                "output_tokens": out_tok,
                "cache_read_input_tokens": cr,
                "cache_creation_input_tokens": cw,
                "output_tokens_details": {"thinking_tokens": 3}
            }
        }
    })
}

fn tmpdir() -> tempfile::TempDir {
    tempfile::tempdir().unwrap()
}

/// 静态 IMPORT_STATE 被并行 tokio 测试共享，碰状态的测试先取这把锁串行化。
static STATE_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

/// 正常解析：usage 字段、毫秒时间戳、sidechain、workdir、60 字标题回落。
#[test]
fn scan_normal_and_sidechain() {
    let d = tmpdir();
    let p = write_lines(
        d.path(),
        "a.jsonl",
        &[
            serde_json::json!({"type":"user","sessionId":"s1","message":{"content":"帮我修登录页面的一个很长很长很长很长很长很长很长很长很长的 bug 说明继续继续继续继续继续继续继续继续"}}),
            assistant("msg-1", "s1", "claude-x", "2026-11-19T08:00:00.500Z", 100, 20, 30, 5),
            serde_json::json!({"type":"assistant","sessionId":"s1","isSidechain":true,"cwd":"/work/proj","timestamp":"2026-11-19T08:00:05Z","message":{"id":"msg-side","model":"m2","usage":{"input_tokens":7,"output_tokens":1}}}),
        ],
    );
    let o = scan_file(&p, 0, "").unwrap();
    assert_eq!(o.entries.len(), 2);
    let e = &o.entries[0];
    assert_eq!(e.model, "claude-x");
    assert_eq!(e.input_tokens, 100);
    assert_eq!(e.cache_read_tokens, 30);
    assert_eq!(e.cache_write_tokens, 5);
    assert_eq!(e.reasoning_tokens, 3);
    assert_eq!(e.call_time_ms, 1795075200500);
    assert!(!o.entries[0].is_sidechain);
    assert!(o.entries[1].is_sidechain, "sidechain 归属会话照记");
    assert_eq!(o.workdir, "/work/proj");
    assert_eq!(o.session_id, "s1");
    let t = o.first_user_prompt.unwrap();
    assert!(t.chars().count() <= 60, "标题回落截 60 字：{t}");
    assert!(t.chars().count() > 30);
    assert_eq!(o.parse_errors, 0);
}

/// 同 message.id 多行只计一次；跨扫描（last_msg_id 传入）续行不再计。
#[test]
fn same_message_id_counted_once() {
    let d = tmpdir();
    let p = write_lines(
        d.path(),
        "b.jsonl",
        &[
            assistant("msg-1", "s1", "m", "2026-11-19T08:00:00Z", 10, 1, 0, 0),
            assistant("msg-1", "s1", "m", "2026-11-19T08:00:01Z", 10, 1, 0, 0),
            assistant("msg-2", "s1", "m", "2026-11-19T08:00:02Z", 20, 2, 0, 0),
        ],
    );
    let o = scan_file(&p, 0, "").unwrap();
    assert_eq!(o.entries.len(), 2, "同 id 续行跳过");
    assert_eq!(o.entries[1].input_tokens, 20);
    assert_eq!(o.last_msg_id, "msg-2");
    // 增量续读：延续行从上一次 last_msg_id 起。
    let o2 = scan_file(&p, o.new_offset, &o.last_msg_id).unwrap();
    assert_eq!(o2.entries.len(), 0, "空增量");
    // 截断判定：size < offset → Truncated。
    let truncated = std::fs::read(&p).unwrap();
    std::fs::write(&p, &truncated[..truncated.len() / 2]).unwrap();
    assert!(matches!(scan_file(&p, o.new_offset, ""), Err(ScanErr::Truncated)));
}

/// 损坏行跳过并计 parse_errors；标题行 custom > ai 优先，ai 不覆盖已存 custom。
#[test]
fn corrupt_lines_and_title_priority() {
    let d = tmpdir();
    let p = d.path().join("c.jsonl");
    let mut s = String::new();
    s.push_str("{\"type\":\"assistant\" \"broken\":\n");
    s.push_str(&serde_json::to_string(&assistant("m1", "s", "m", "2026-11-19T08:00:00Z", 1, 1, 0, 0)).unwrap());
    s.push('\n');
    s.push_str("{\"type\":\"ai-title\",\"aiTitle\":\"AI 名\"}\n");
    s.push_str("{\"type\":\"custom-title\",\"customTitle\":\"自定义名\"}\n");
    std::fs::write(&p, s).unwrap();
    let o = scan_file(&p, 0, "").unwrap();
    assert_eq!(o.parse_errors, 1, "坏行跳过 +1");
    assert_eq!(o.entries.len(), 1);
    let t = o.title.unwrap();
    assert_eq!(t.0, "custom_title");
    assert_eq!(t.1, "自定义名");
    // 只有 ai-title 时取 ai。
    let p2 = write_lines(
        d.path(),
        "c2.jsonl",
        &[serde_json::json!({"type":"ai-title","aiTitle":"只有 AI"}), assistant("m1", "s", "m", "2026-11-19T08:00:00Z", 1, 1, 0, 0)],
    );
    let o2 = scan_file(&p2, 0, "").unwrap();
    assert_eq!(o2.title.unwrap().1, "只有 AI");
}

/// 增量追加只读新增；重复读幂等（同偏移重读零新增）。
#[test]
fn incremental_append_and_idempotent() {
    let d = tmpdir();
    let p = write_lines(
        d.path(),
        "d.jsonl",
        &[assistant("m1", "s", "m", "2026-11-19T08:00:00Z", 1, 1, 0, 0)],
    );
    let o1 = scan_file(&p, 0, "").unwrap();
    assert_eq!(o1.entries.len(), 1);
    // 无新增重读：零条目、offset 不变（幂等）。
    let o1b = scan_file(&p, o1.new_offset, &o1.last_msg_id).unwrap();
    assert_eq!(o1b.entries.len(), 0);
    // 追加两行（其一为同 id 续行）。
    let mut s = std::fs::read_to_string(&p).unwrap();
    s.push_str(&serde_json::to_string(&assistant("m2", "s", "m", "2026-11-19T09:00:00Z", 5, 5, 0, 0)).unwrap());
    s.push('\n');
    s.push_str(&serde_json::to_string(&assistant("m2", "s", "m", "2026-11-19T09:00:01Z", 5, 5, 0, 0)).unwrap());
    s.push('\n');
    std::fs::write(&p, s).unwrap();
    let o2 = scan_file(&p, o1.new_offset, &o1.last_msg_id).unwrap();
    assert_eq!(o2.entries.len(), 1, "只读新增，续行跳过");
    assert_eq!(o2.entries[0].input_tokens, 5);
}

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

/// 闸门：关着 = 零读取（文件在盘上也不产生任何明细 / 进度行）。
#[tokio::test]
async fn disabled_consent_zero_read() {
    let _g = STATE_LOCK.lock().await;
    let (db, _dir) = aidog_db::test_support::four_file_db().await;
    let d = tmpdir();
    write_lines(d.path(), "a.jsonl", &[assistant("m1", "s", "claude-haiku-5-5", "2026-11-19T08:00:00Z", 1, 1, 0, 0)]);
    save_consent(&db, &ClientUsageConsent::default()).await.unwrap();
    let root = Some(d.path().to_path_buf());
    let clients = vec!["claude_code".to_string()];
    let n = run_import(&db, root, &clients, false).await.unwrap();
    assert_eq!(n, 0, "关着零导入");
    assert_eq!(count_calls(&db).await, 0, "零文件读取");
}

/// 折算命中（内置价目）与未定价（cost NULL，不回落兜底单价）各一例。
#[tokio::test]
async fn cost_resolved_and_unpriced() {
    let _g = STATE_LOCK.lock().await;
    let (db, _dir) = aidog_db::test_support::four_file_db().await;
    let d = tmpdir();
    write_lines(
        d.path(),
        "a.jsonl",
        &[
            assistant("m1", "s1", "claude-haiku-5-5", "2026-11-19T08:00:00Z", 1_000_000, 100_000, 0, 0),
            assistant("m2", "s1", "totally-unknown-model", "2026-11-19T08:01:00Z", 1_000_000, 0, 0, 0),
        ],
    );
    save_consent(&db, &consent_on(&["claude_code"])).await.unwrap();
    let root = Some(d.path().to_path_buf());
    let clients = vec!["claude_code".to_string()];
    run_import(&db, root, &clients, false).await.unwrap();
    let rows: Vec<(String, Option<f64>)> = db
        .call_read_client_usage_traced(None, std::panic::Location::caller(), |conn| {
            let mut st = conn.prepare("SELECT model, cost FROM client_call ORDER BY model")?;
            let v = st
                .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, Option<f64>>(1)?)))?
                .collect::<Result<Vec<_>, _>>()?;
            Ok(v)
        })
        .await
        .unwrap();
    assert_eq!(rows.len(), 2);
    let hit = rows.iter().find(|(m, _)| m == "claude-haiku-5-5").unwrap();
    let miss = rows.iter().find(|(m, _)| m == "totally-unknown-model").unwrap();
    let c = hit.1.expect("内置价命中应已折算");
    assert!(c > 0.0, "命中价 cost > 0，实际 {c}");
    assert!(miss.1.is_none(), "未定价 cost NULL（不回落兜底单价）");
}

/// 增量导入 + 截断重建：第二次扫描只进新增；文件截短后明细与进度重建。
#[tokio::test]
async fn incremental_then_truncate_rebuild() {
    let _g = STATE_LOCK.lock().await;
    let (db, _dir) = aidog_db::test_support::four_file_db().await;
    let d = tmpdir();
    let p = write_lines(
        d.path(),
        "a.jsonl",
        &[
            assistant("m1", "s", "claude-haiku-5-5", "2026-11-19T08:00:00Z", 10, 1, 0, 0),
            assistant("m2", "s", "claude-haiku-5-5", "2026-11-19T08:01:00Z", 10, 1, 0, 0),
        ],
    );
    save_consent(&db, &consent_on(&["claude_code"])).await.unwrap();
    let root = Some(d.path().to_path_buf());
    let clients = vec!["claude_code".to_string()];
    run_import(&db, root.clone(), &clients, false).await.unwrap();
    assert_eq!(count_calls(&db).await, 2);
    // 幂等：同批重扫零新增。
    run_import(&db, root.clone(), &clients, false).await.unwrap();
    assert_eq!(count_calls(&db).await, 2, "重复读幂等");
    // 追加一行再扫（refresh 模式走增量偏移）。
    let mut s = std::fs::read_to_string(&p).unwrap();
    s.push_str(&serde_json::to_string(&assistant("m3", "s", "claude-haiku-5-5", "2026-11-19T09:00:00Z", 10, 1, 0, 0)).unwrap());
    s.push('\n');
    std::fs::write(&p, s).unwrap();
    run_import(&db, root.clone(), &clients, false).await.unwrap();
    assert_eq!(count_calls(&db).await, 3, "增量只进新增");
    // 截断：文件重写为只剩 1 行（size < read_offset）→ 重建为 1 条明细。
    let short = std::fs::read_to_string(&p).unwrap();
    let first_line = short.lines().next().unwrap();
    std::fs::write(&p, format!("{first_line}\n")).unwrap();
    run_import(&db, root, &clients, false).await.unwrap();
    assert_eq!(count_calls(&db).await, 1, "截断重建");
    let prog = aidog_db::list_client_file_progress(&db, "claude_code").await.unwrap();
    assert_eq!(prog.len(), 1);
    assert!(prog[0].parse_errors == 0);
}

/// 启动模式（only_with_progress）跳过无进度文件；首次全量导入分批可见进度。
#[tokio::test]
async fn startup_mode_skips_new_files_and_state_visible() {
    let _g = STATE_LOCK.lock().await;
    let (db, _dir) = aidog_db::test_support::four_file_db().await;
    let d = tmpdir();
    write_lines(d.path(), "a.jsonl", &[assistant("m1", "s", "claude-haiku-5-5", "2026-11-19T08:00:00Z", 1, 1, 0, 0)]);
    let root = Some(d.path().to_path_buf());
    let clients = vec!["claude_code".to_string()];
    save_consent(&db, &consent_on(&["claude_code"])).await.unwrap();
    // 无进度行 → 启动模式零导入。
    let n = run_import(&db, root.clone(), &clients, true).await.unwrap();
    assert_eq!(n, 0, "启动增量只认进度表已有文件");
    // 全量一次产生进度与状态。
    run_import(&db, root.clone(), &clients, false).await.unwrap();
    assert_eq!(count_calls(&db).await, 1);
    let st = snapshot_import_state().expect("进度态可见");
    assert!(!st.running);
    assert_eq!(st.files_total, 1);
    assert_eq!(st.files_done, 1);
    assert_eq!(st.calls_imported, 1);
    // 再跑启动模式：有进度 → 导入（幂等零新增）。
    let n = run_import(&db, root, &clients, true).await.unwrap();
    assert_eq!(n, 0, "无新增零插入");
    assert_eq!(count_calls(&db).await, 1);
}

/// 会话行落库：标题用 transcript 标题行，回落首条用户提示 60 字；workdir / 首末时间。
#[tokio::test]
async fn session_row_title_and_meta() {
    let _g = STATE_LOCK.lock().await;
    let (db, _dir) = aidog_db::test_support::four_file_db().await;
    let d = tmpdir();
    let p = d.path().join("a.jsonl");
    let mut s = String::new();
    s.push_str(&serde_json::to_string(&serde_json::json!({"type":"custom-title","customTitle":"我的自定义标题"})).unwrap());
    s.push('\n');
    s.push_str(&serde_json::to_string(&assistant("m1", "sess-9", "claude-haiku-5-5", "2026-11-19T08:00:00Z", 1, 1, 0, 0)).unwrap());
    s.push('\n');
    std::fs::write(&p, s).unwrap();
    save_consent(&db, &consent_on(&["claude_code"])).await.unwrap();
    let clients = vec!["claude_code".to_string()];
    run_import(&db, Some(d.path().to_path_buf()), &clients, false).await.unwrap();
    let row: (String, String, i64, i64) = db
        .call_read_client_usage_traced(None, std::panic::Location::caller(), |conn| {
            conn.query_row(
                "SELECT title, workdir, first_call_ms, last_call_ms FROM client_session WHERE session_id='sess-9'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )
        })
        .await
        .unwrap();
    assert_eq!(row.0, "我的自定义标题");
    assert_eq!(row.1, "/work/proj");
    assert_eq!(row.2, 1795075200000);
    assert_eq!(row.3, 1795075200000);
}
