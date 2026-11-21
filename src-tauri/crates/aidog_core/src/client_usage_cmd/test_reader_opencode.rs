//! 票 06 OpenCode 读取器夹具测试：JSON store（全局 + per-project）与 sqlite db 两形态，
//! tempdir 注入，禁碰真 ~/.local/share/opencode。
use std::path::Path;

use super::*;

/// 与 test_reader / test_reader_codex / test_reader_gemini 共用一把锁（全局 IMPORT_STATE 唯一）。
use super::TEST_IMPORT_STATE_LOCK as STATE_LOCK;

fn tmpdir() -> tempfile::TempDir {
    tempfile::tempdir().unwrap()
}

fn json_file(p: &Path, v: &serde_json::Value) {
    if let Some(parent) = p.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    std::fs::write(p, serde_json::to_string_pretty(v).unwrap()).unwrap();
}

/// 会话信息 JSON（`parent` 传 Some 时为子会话）。
fn session_json(id: &str, parent: Option<&str>, title: &str, dir: &str) -> serde_json::Value {
    let mut v = serde_json::json!({
        "id": id, "title": title, "directory": dir,
        "time": {"created": "2026-11-19T08:00:00.000Z"},
    });
    if let Some(p) = parent {
        v["parentID"] = serde_json::json!(p);
    }
    v
}

/// assistant 消息 JSON（1.x/2.x 形态：model:{id,providerID}、ISO 时间）。
#[allow(clippy::too_many_arguments)]
fn msg_json(id: &str, model: &str, iso: &str, inp: i64, out: i64, cr: i64, cw: i64, rz: i64) -> serde_json::Value {
    serde_json::json!({
        "id": id, "type": "assistant",
        "model": {"id": model, "providerID": "anthropic"},
        "tokens": {"input": inp, "output": out, "reasoning": rz, "cache": {"read": cr, "write": cw}},
        "time": {"created": iso},
    })
}

/// 计数与取值帮手。
async fn call_count(db: &aidog_db::Db) -> i64 {
    db.call_read_client_usage_traced(None, std::panic::Location::caller(), |conn| {
        conn.query_row("SELECT COUNT(*) FROM client_call", [], |r| r.get(0))
    })
    .await
    .unwrap()
}

async fn scalar(db: &aidog_db::Db, sql: &'static str) -> (i64, Option<f64>) {
    db.call_read_client_usage_traced(None, std::panic::Location::caller(), move |conn| {
        conn.query_row(sql, [], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, Option<f64>>(1)?)))
    })
    .await
    .unwrap()
}

fn consent_on(clients: &[&str]) -> ClientUsageConsent {
    let mut c = ClientUsageConsent {
        asked: true,
        ..Default::default()
    };
    for s in clients {
        c.enabled.insert(s.to_string(), true);
    }
    c
}

// ---------- JSON store：全局布局 ----------

#[tokio::test]
async fn opencode_json_store_import_e2e() {
    let _g = STATE_LOCK.lock().await;
    let (db, _dir) = aidog_db::test_support::four_file_db().await;
    let d = tmpdir();
    // 根会话 sess-r + 子会话 sess-c（parentID → sess-r）。
    json_file(
        &d.path().join("storage/session/proj1/sess-r.json"),
        &session_json("sess-r", None, "修登录页", "/work/app"),
    );
    json_file(
        &d.path().join("storage/session/proj1/sess-c.json"),
        &session_json("sess-c", Some("sess-r"), "Child session", "/work/app"),
    );
    json_file(
        &d.path().join("storage/message/sess-r/m1.json"),
        &msg_json("m1", "claude-sonnet-4-5", "2026-11-19T08:00:01.000Z", 10_000, 2_000, 5_000, 500, 300),
    );
    // 子会话消息：归属根会话，is_sidechain=true。
    json_file(
        &d.path().join("storage/message/sess-c/m2.json"),
        &msg_json("m2", "claude-sonnet-4-5", "2026-11-19T08:01:00.000Z", 1_000, 100, 0, 0, 0),
    );
    // 未定价模型。
    json_file(
        &d.path().join("storage/message/sess-r/m3.json"),
        &msg_json("m3", "oc-x-unknown", "2026-11-19T08:02:00.000Z", 100, 10, 0, 0, 0),
    );
    save_consent(&db, &consent_on(&["opencode"])).await.unwrap();
    let root = d.path().to_path_buf();
    let clients = vec!["opencode".to_string()];
    let n = run_import(&db, Some(root.clone()), &clients, false).await.unwrap();
    assert_eq!(n, 3, "三条消息入库，实际 {n}");
    // 子会话归属根：m2 的 session_id = sess-r 且 is_sidechain=1。
    let (sid, side, cost_known, cost_unknown): (String, i64, Option<f64>, Option<f64>) = db
        .call_read_client_usage_traced(None, std::panic::Location::caller(), |conn| {
            let a: (String, i64) = conn
                .query_row("SELECT session_id, is_sidechain FROM client_call WHERE model='claude-sonnet-4-5' AND input_tokens=1000", [], |r| Ok((r.get(0)?, r.get(1)?)))
                .unwrap();
            let b: Option<f64> = conn
                .query_row("SELECT cost FROM client_call WHERE model='claude-sonnet-4-5' AND input_tokens=10000", [], |r| r.get(0))
                .unwrap();
            let c: Option<f64> = conn
                .query_row("SELECT cost FROM client_call WHERE model='oc-x-unknown'", [], |r| r.get(0))
                .unwrap();
            Ok((a.0, a.1, b, c))
        })
        .await
        .unwrap();
    assert_eq!(sid, "sess-r", "子会话调用归属根会话");
    assert_eq!(side, 1, "子会话调用 is_sidechain=1");
    assert!(cost_known.unwrap_or(0.0) > 0.0, "官方平台条目命中应已折算");
    assert!(cost_unknown.is_none(), "未定价模型 cost=None");
    // 会话行：标题 / 目录。
    let (title, workdir): (String, String) = db
        .call_read_client_usage_traced(None, std::panic::Location::caller(), |conn| {
            conn.query_row("SELECT title, workdir FROM client_session WHERE session_id='sess-r'", [], |r| Ok((r.get(0)?, r.get(1)?)))
        })
        .await
        .unwrap();
    assert_eq!(title, "修登录页");
    assert_eq!(workdir, "/work/app");
    // 幂等：重复导入零新增（文件未变跳过）。
    let n2 = run_import(&db, Some(root.clone()), &clients, false).await.unwrap();
    assert_eq!(n2, 0, "未变文件跳过，零新增");
    assert_eq!(call_count(&db).await, 3);
}

/// 消息文件原地改写（token 更新）→ 删状态重读，明细被替换不残留旧值。
#[tokio::test]
async fn opencode_json_store_token_update() {
    let _g = STATE_LOCK.lock().await;
    let (db, _dir) = aidog_db::test_support::four_file_db().await;
    let d = tmpdir();
    json_file(
        &d.path().join("storage/session/proj1/sess-u.json"),
        &session_json("sess-u", None, "t", "/w"),
    );
    let mpath = d.path().join("storage/message/sess-u/m1.json");
    json_file(&mpath, &msg_json("m1", "gemini-2.5-pro", "2026-11-19T08:00:01.000Z", 100, 5, 0, 0, 0));
    save_consent(&db, &consent_on(&["opencode"])).await.unwrap();
    let root = Some(d.path().to_path_buf());
    let clients = vec!["opencode".to_string()];
    run_import(&db, root.clone(), &clients, false).await.unwrap();
    let (_, c1) = scalar(&db, "SELECT COUNT(*), SUM(output_tokens) FROM client_call").await;
    let c1 = c1.map(|x| x as i64);
    assert_eq!(c1, Some(5));
    // 原地改写（流式完成后 tokens 更新，文件 size/mtime 变）。
    std::thread::sleep(std::time::Duration::from_millis(1100));
    json_file(&mpath, &msg_json("m1", "gemini-2.5-opus-4-5", "2026-11-19T08:00:01.000Z", 100, 99, 0, 0, 0));
    run_import(&db, root.clone(), &clients, false).await.unwrap();
    let (cnt, c2) = scalar(&db, "SELECT COUNT(*), SUM(output_tokens) FROM client_call").await;
    let c2 = c2.map(|x| x as i64);
    assert_eq!(cnt, 1, "仍一行明细");
    assert_eq!(c2, Some(99), "token 更新落库");
}

/// per-project 布局（0.x/1.x 早期）同样可读。
#[tokio::test]
async fn opencode_json_store_per_project_layout() {
    let _g = STATE_LOCK.lock().await;
    let (db, _dir) = aidog_db::test_support::four_file_db().await;
    let d = tmpdir();
    json_file(
        &d.path().join("project/-Users-demo-app/storage/session/info/sess-p.json"),
        &session_json("sess-p", None, "老布局", "/Users/demo/app"),
    );
    // 旧版时间形态：毫秒数（非 ISO 字符串）。
    let mut m9 = msg_json("m9", "claude-sonnet-4-5", "2026-11-19T08:00:01.000Z", 500, 50, 0, 0, 0);
    m9["time"]["created"] = serde_json::json!(20261119080001000_i64);
    json_file(
        &d.path().join("project/-Users-demo-app/storage/session/message/sess-p/m9.json"),
        &m9,
    );
    save_consent(&db, &consent_on(&["opencode"])).await.unwrap();
    let clients = vec!["opencode".to_string()];
    let n = run_import(&db, Some(d.path().to_path_buf()), &clients, false).await.unwrap();
    assert_eq!(n, 1, "per-project 布局消息入库");
    // 旧版毫秒数时间形态。
    let ms: i64 = db
        .call_read_client_usage_traced(None, std::panic::Location::caller(), |conn| {
            conn.query_row("SELECT call_time_ms FROM client_call", [], |r| r.get::<_, i64>(0))
        })
        .await
        .unwrap();
    assert_eq!(ms, 20261119080001000, "毫秒数 time.created 直收");
}

// ---------- sqlite db 形态 ----------

/// 新代库（session_v2 + session_message）。flush=true 时补第二条消息（制造 size/mtime 变化）。
fn build_db_v2(path: &Path, flush: bool) {
    let _ = std::fs::remove_file(path);
    let conn = rusqlite::Connection::open(path).unwrap();
    conn.execute_batch(
        "CREATE TABLE session_v2 (id text PRIMARY KEY, parent_id text, directory text NOT NULL DEFAULT '', title text);
         CREATE TABLE session_message (id text PRIMARY KEY, session_id text NOT NULL, type text NOT NULL, seq integer NOT NULL, time_created integer NOT NULL, time_updated integer NOT NULL, data text NOT NULL);",
    )
    .unwrap();
    conn.execute(
        "INSERT INTO session_v2 (id, parent_id, directory, title) VALUES ('db-r', NULL, '/work/db', 'db 根会话')",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO session_v2 (id, parent_id, directory, title) VALUES ('db-c', 'db-r', '/work/db', 'Child')",
        [],
    )
    .unwrap();
    let m1 = msg_json("msg_1", "claude-sonnet-4-5", "2026-11-19T09:00:00.000Z", 2_000, 200, 100, 0, 10);
    conn.execute(
        "INSERT INTO session_message (id, session_id, type, seq, time_created, time_updated, data) VALUES ('msg_1', 'db-r', 'assistant', 1, 1, 1, ?1)",
        [serde_json::to_string(&m1).unwrap()],
    )
    .unwrap();
    if flush {
        let m2 = msg_json("msg_2", "claude-sonnet-4-5", "2026-11-19T09:05:00.000Z", 300, 30, 0, 0, 0);
        conn.execute(
            "INSERT INTO session_message (id, session_id, type, seq, time_created, time_updated, data) VALUES ('msg_2', 'db-c', 'assistant', 2, 2, 2, ?1)",
            [serde_json::to_string(&m2).unwrap()],
        )
        .unwrap();
    }
    conn.close().unwrap();
}

/// 旧代库（session + message，无 type 列）。
fn build_db_v1(path: &Path) {
    let _ = std::fs::remove_file(path);
    let conn = rusqlite::Connection::open(path).unwrap();
    conn.execute_batch(
        "CREATE TABLE session (id text PRIMARY KEY, parent_id text, directory text NOT NULL DEFAULT '', title text);
         CREATE TABLE message (id text PRIMARY KEY, session_id text NOT NULL, time_created integer NOT NULL, time_updated integer NOT NULL, data text NOT NULL);",
    )
    .unwrap();
    conn.execute(
        "INSERT INTO session (id, parent_id, directory, title) VALUES ('old-r', NULL, '/w', '旧代会话')",
        [],
    )
    .unwrap();
    let m = serde_json::json!({
        "id": "om1", "type": "assistant", "modelID": "gemini-2.5-pro",
        "tokens": {"input": 1000, "output": 100, "reasoning": 0, "cache": {"read": 0, "write": 0}},
        "time": {"created": "2026-11-19T10:00:00.000Z"},
    });
    conn.execute(
        "INSERT INTO message (id, session_id, time_created, time_updated, data) VALUES ('om1', 'old-r', 1, 1, ?1)",
        [serde_json::to_string(&m).unwrap()],
    )
    .unwrap();
    conn.close().unwrap();
}

fn db_root(d: &tempfile::TempDir) -> std::path::PathBuf {
    d.path().join("opencode.db")
}

#[tokio::test]
async fn opencode_db_v2_import_e2e() {
    let _g = STATE_LOCK.lock().await;
    let (db, _dir) = aidog_db::test_support::four_file_db().await;
    let d = tmpdir();
    let dbp = db_root(&d);
    build_db_v2(&dbp, false);
    save_consent(&db, &consent_on(&["opencode"])).await.unwrap();
    let root = Some(d.path().to_path_buf());
    let clients = vec!["opencode".to_string()];
    let n = run_import(&db, root.clone(), &clients, false).await.unwrap();
    assert_eq!(n, 1, "新代库一条 assistant 消息入库");
    // 幂等：库未变零新增。
    let n2 = run_import(&db, root.clone(), &clients, false).await.unwrap();
    assert_eq!(n2, 0, "库未变跳过");
    // 库追加消息（size/mtime 变）→ 重读，子会话消息归属根。
    build_db_v2(&dbp, true);
    let n3 = run_import(&db, root.clone(), &clients, false).await.unwrap();
    assert_eq!(n3, 2, "重建后全量重读两条（旧行已删重插）");
    let (sid, side): (String, i64) = db
        .call_read_client_usage_traced(None, std::panic::Location::caller(), |conn| {
            conn.query_row("SELECT session_id, is_sidechain FROM client_call WHERE file_pos=2", [], |r| Ok((r.get(0)?, r.get(1)?)))
        })
        .await
        .unwrap();
    assert_eq!(sid, "db-r", "子会话消息归属根会话");
    assert_eq!(side, 1);
    let cnt: i64 = db
        .call_read_client_usage_traced(None, std::panic::Location::caller(), |conn| {
            conn.query_row("SELECT COUNT(*) FROM client_call", [], |r| r.get::<_, i64>(0))
        })
        .await
        .unwrap();
    assert_eq!(cnt, 2, "全量重读后仍是两行明细");
    // 会话行标题。
    let title: String = db
        .call_read_client_usage_traced(None, std::panic::Location::caller(), |conn| {
            conn.query_row("SELECT title FROM client_session WHERE session_id='db-r'", [], |r| r.get::<_, String>(0))
        })
        .await
        .unwrap();
    assert_eq!(title, "db 根会话");
    std::fs::remove_file(&dbp).ok();
}

#[tokio::test]
async fn opencode_db_v1_import() {
    let _g = STATE_LOCK.lock().await;
    let (db, _dir) = aidog_db::test_support::four_file_db().await;
    let d = tmpdir();
    let dbp = db_root(&d);
    build_db_v1(&dbp);
    save_consent(&db, &consent_on(&["opencode"])).await.unwrap();
    let clients = vec!["opencode".to_string()];
    let n = run_import(&db, Some(d.path().to_path_buf()), &clients, false).await.unwrap();
    assert_eq!(n, 1, "旧代库消息入库");
    let (model, cost): (String, Option<f64>) = db
        .call_read_client_usage_traced(None, std::panic::Location::caller(), |conn| {
            conn.query_row("SELECT model, cost FROM client_call", [], |r| Ok((r.get(0)?, r.get(1)?)))
        })
        .await
        .unwrap();
    assert_eq!(model, "gemini-2.5-pro", "旧版顶层 modelID 形态");
    assert!(cost.unwrap_or(0.0) > 0.0, "按名匹配官方平台条目折算");
    std::fs::remove_file(&dbp).ok();
}

/// 票 07 会话存在性：新代 session_v2 / 旧代 session 两种 db + JSON store；查不到 / 无库 → false。
#[test]
fn session_exists_db_generations_and_json() {
    let d = tmpdir();
    let v2 = d.path().join("v2");
    std::fs::create_dir_all(&v2).unwrap();
    let c = rusqlite::Connection::open(v2.join("opencode.db")).unwrap();
    c.execute_batch("CREATE TABLE session_v2 (id text PRIMARY KEY, parent_id text, directory text, title text); INSERT INTO session_v2 VALUES ('ses_a', '', '/w', 't');").unwrap();
    drop(c);
    assert!(reader_opencode::session_exists(&v2, "ses_a"), "父目录形态");
    assert!(reader_opencode::session_exists(&v2.join("opencode.db"), "ses_a"), "db 文件形态");
    assert!(!reader_opencode::session_exists(&v2, "ses_b"));
    assert!(!reader_opencode::session_exists(&v2, "x' OR '1'='1"), "参数化，不拼 SQL");

    let old = d.path().join("old");
    std::fs::create_dir_all(&old).unwrap();
    let c = rusqlite::Connection::open(old.join("opencode.db")).unwrap();
    c.execute_batch("CREATE TABLE session (id text PRIMARY KEY, parent_id text, directory text, title text); INSERT INTO session VALUES ('ses_o', '', '/w', 't');").unwrap();
    drop(c);
    assert!(reader_opencode::session_exists(&old, "ses_o"));
    assert!(!reader_opencode::session_exists(&old, "ses_a"));

    let js = d.path().join("js");
    json_file(&js.join("storage/session/proj1/ses_j.json"), &session_json("ses_j", None, "t", "/w"));
    assert!(reader_opencode::session_exists(&js, "ses_j"));
    assert!(!reader_opencode::session_exists(&js, "ses_k"));
    assert!(!reader_opencode::session_exists(&d.path().join("none"), "ses_j"));
}
