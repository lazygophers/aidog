//! client_usage.db 票 01 验收测试：四文件拆库路由、幂等定位键、保留设置、清理语义。
use crate::client_usage::*;
use crate::test_support::four_file_db;

use crate::Db;
use rusqlite::params;

async fn count_calls(db: &Db) -> i64 {
    db.call_read_client_usage_traced(None, std::panic::Location::caller(), |conn| {
        conn.query_row("SELECT COUNT(*) FROM client_call", [], |r| r.get(0))
    })
    .await
    .expect("count client_call")
}

async fn count_sessions(db: &Db, code: String) -> i64 {
    db.call_read_client_usage_traced(None, std::panic::Location::caller(), move |conn| {
        conn.query_row(
            "SELECT COUNT(*) FROM client_session WHERE client_code = ?1",
            params![code],
            |r| r.get(0),
        )
    })
    .await
    .expect("count client_session")
}

async fn count_progress(db: &Db, code: String) -> i64 {
    db.call_read_client_usage_traced(None, std::panic::Location::caller(), move |conn| {
        conn.query_row(
            "SELECT COUNT(*) FROM client_file_progress WHERE client_code = ?1",
            params![code],
            |r| r.get(0),
        )
    })
    .await
    .expect("count progress")
}

/// 票面验收 ①：四个独立临时文件拆库 + 新表读写走新 handle。
/// 文件库下主库 handle 查 client_call 必 `no such table`（错路由当场炸）；
/// 写后重开 DB 读回。内存库 fallback 三 handle 同连接测不出拆库，故用 four_file_db。
#[tokio::test]
async fn client_usage_four_file_split_and_routing() {
    let (db, dir) = four_file_db().await;
    // 4 个独立 db 文件都在盘上（client_usage.db 由 Db::new 随主库派生）。
    for f in ["aidog.db", "log.db", "platform.db", "client_usage.db"] {
        assert!(
            dir.path().join(f).exists(),
            "{f} 应由 Db::new 建在主库同目录"
        );
    }
    let row = ClientCallRow {
        client_code: "claude_code".into(),
        session_id: "sess-1".into(),
        model: "claude-x".into(),
        input_tokens: 10,
        output_tokens: 20,
        cost: None, // 未定价哨兵
        source_file_id: "f1".into(),
        file_pos: 1,
        ..Default::default()
    };
    assert_eq!(insert_client_calls(&db, std::slice::from_ref(&row)).await.expect("insert"), 1);
    upsert_client_file_progress(
        &db,
        ClientFileProgress {
            client_code: "claude_code".into(),
            path: "/tmp/x.jsonl".into(),
            file_size: 100,
            mtime_ms: 1,
            read_offset: 100,
            parse_errors: 0,
            last_msg_id: String::new(),
        },
    )
    .await
    .expect("upsert progress");
    upsert_client_session(&db, "claude_code".into(), "sess-1".into(), "t".into(), "/w".into(), 1, 2)
        .await
        .expect("upsert session");

    // 错路由判定：主库 / log.db handle 上没有 client_call（拆库实锤）。
    for err_db in [
        db.call_traced(None, std::panic::Location::caller(), |conn| {
            conn.query_row("SELECT COUNT(*) FROM client_call", [], |r| r.get::<_, i64>(0))
        })
        .await
        .expect_err("主库 handle 查 client_call 应报错"),
        db.call_proxy_log_traced(None, std::panic::Location::caller(), |conn| {
            conn.query_row("SELECT COUNT(*) FROM client_call", [], |r| r.get::<_, i64>(0))
        })
        .await
        .expect_err("log.db handle 查 client_call 应报错"),
    ] {
        assert!(
            err_db.to_string().contains("no such table"),
            "错路由应报 no such table，实际: {err_db}"
        );
    }

    // 写后重开 DB 读回。
    drop(db);
    let (db2, _dir) = {
        // TempDir 被 move 进 first db? No—— dir 未 move；重建 Db 于同路径。
        let path = dir.path().join("aidog.db");
        let db2 = Db::new(path.to_str().expect("utf8"))
            .await
            .expect("reopen");
        crate::schema::init_tables_raw(
            &db2,
            std::sync::Arc::new(|_conn, _map| Ok(())),
        )
        .await
        .expect("init tables");
        (db2, dir)
    };
    let cnt = count_calls(&db2).await;
    assert_eq!(cnt, 1, "重开后明细仍在");
    let cost: Option<f64> = db2
        .call_read_client_usage_traced(None, std::panic::Location::caller(), |conn| {
            conn.query_row("SELECT cost FROM client_call LIMIT 1", [], |r| r.get(0))
        })
        .await
        .expect("read cost");
    assert_eq!(cost, None, "未定价 cost 应为 NULL");
}

/// 票面验收 ②：同一定位键重复插入幂等，行数不变。
#[tokio::test]
async fn client_call_insert_idempotent_on_position_key() {
    let (db, _dir) = four_file_db().await;
    let row = ClientCallRow {
        client_code: "codex".into(),
        session_id: "s1".into(),
        source_file_id: "f9".into(),
        file_pos: 7,
        ..Default::default()
    };
    assert_eq!(insert_client_calls(&db, std::slice::from_ref(&row)).await.expect("1st"), 1);
    assert_eq!(insert_client_calls(&db, &[row]).await.expect("2nd"), 0);
    assert_eq!(count_calls(&db).await, 1);
}

/// 票面验收 ③：保留天数 setting 读写持久化（缺省 90，0 = 永久）。
#[tokio::test]
async fn client_usage_retention_setting_roundtrip() {
    let (db, _dir) = four_file_db().await;
    assert_eq!(load_retention_days(&db).await, 90, "缺省 90");
    save_retention_days(&db, 7).await.expect("save 7");
    assert_eq!(load_retention_days(&db).await, 7);
    save_retention_days(&db, 0).await.expect("save 0");
    assert_eq!(load_retention_days(&db).await, 0, "0 = 永久保留");
}

/// 票面验收 ④：清理删超期明细 + 无明细残留的会话行，保留进度表与未过期数据。
#[tokio::test]
async fn cleanup_client_usage_deletes_expired_and_empty_sessions_keeps_progress() {
    let (db, _dir) = four_file_db().await;
    let now_ms = crate::now();
    let day = 86_400_000i64;
    let old = now_ms - 200 * day;
    // 旧明细（sess-old，200 天前）+ 新明细（sess-live，现在）。
    let mk = |code: &str, sess: &str, pos: i64, t: i64| ClientCallRow {
        client_code: code.into(),
        session_id: sess.into(),
        call_time_ms: t,
        source_file_id: format!("{sess}-f"),
        file_pos: pos,
        ..Default::default()
    };
    insert_client_calls(
        &db,
        &[mk("claude_code", "sess-old", 1, old), mk("claude_code", "sess-live", 1, now_ms)],
    )
    .await
    .expect("insert");
    // 三个会话行：sess-old（明细将被清空 → 会话行随之删）、sess-live（明细保留 → 会话行保留）、
    // sess-empty（本来无明细 → 空壳删除）。
    for s in ["sess-old", "sess-live", "sess-empty"] {
        upsert_client_session(&db, "claude_code".into(), s.into(), String::new(), String::new(), old, old)
            .await
            .expect("upsert session");
    }
    // 无任何明细的会话壳行。
    upsert_client_session(&db, "claude_code".into(), "sess-empty".into(), String::new(), String::new(), old, old)
        .await
        .expect("upsert empty session");
    // 进度行必须保留。
    upsert_client_file_progress(
        &db,
        ClientFileProgress {
            client_code: "claude_code".into(),
            path: "/p/a.jsonl".into(),
            file_size: 1,
            mtime_ms: 1,
            read_offset: 1,
            parse_errors: 0,
            last_msg_id: String::new(),
        },
    )
    .await
    .expect("upsert progress");

    cleanup_client_usage(&db, 90).await.expect("cleanup");

    assert_eq!(count_calls(&db).await, 1, "仅剩未过期明细");
    assert_eq!(count_sessions(&db, "claude_code".into()).await, 1, "仅剩有明细的会话");
    assert_eq!(count_progress(&db, "claude_code".into()).await, 1, "进度表不动");
}

/// 会话行未定价口径：unpriced_calls = cost IS NULL 的调用数；免费模型（cost=0.0）不算未定价。
#[tokio::test]
async fn client_usage_sessions_reports_unpriced_calls() {
    let (db, _dir) = four_file_db().await;
    let now = crate::now();
    let mk = |sess: &str, pos: i64, cost: Option<f64>| ClientCallRow {
        client_code: "claude_code".into(),
        session_id: sess.into(),
        call_time_ms: now,
        source_file_id: format!("{sess}-f"),
        file_pos: pos,
        cost,
        ..Default::default()
    };
    insert_client_calls(
        &db,
        &[
            mk("sess-full", 1, None),
            mk("sess-full", 2, None),
            mk("sess-mixed", 1, Some(0.5)),
            mk("sess-mixed", 2, None),
            mk("sess-free", 1, Some(0.0)),
        ],
    )
    .await
    .expect("insert calls");
    for (s, t) in [("sess-full", 3), ("sess-mixed", 2), ("sess-free", 1)] {
        upsert_client_session(&db, "claude_code".into(), s.into(), String::new(), "/w".into(), now, t)
            .await
            .expect("upsert session");
    }

    let rows = client_usage_sessions(&db, 0, now + 1000, None, 100).await.expect("sessions");
    let by_id = |id: &str| rows.iter().find(|r| r.session_id == id).unwrap_or_else(|| panic!("missing {id}"));
    let full = by_id("sess-full");
    assert_eq!((full.calls, full.cost), (2, 0.0), "全未定价会话");
    assert_eq!(full.unpriced_calls, 2, "全未定价会话 unpriced == calls");
    let mixed = by_id("sess-mixed");
    assert_eq!(mixed.cost, 0.5, "混合会话 cost 只含已定价行");
    assert_eq!(mixed.unpriced_calls, 1, "混合会话 unpriced 居中");
    let free = by_id("sess-free");
    assert_eq!((free.calls, free.cost), (1, 0.0), "免费模型会话");
    assert_eq!(free.unpriced_calls, 0, "免费模型（cost=0.0 已定价）不算未定价");
}
/// 票 03：会话汇总带缓存 / 推理 token 与子代理调用数；详情返回模型链（相邻同模型合并）与按时间升序的 token 序列。
#[tokio::test]
async fn client_usage_session_cache_fields_and_detail_chain() {
    let (db, _dir) = four_file_db().await;
    let base = crate::now() - 10_000;
    #[allow(clippy::too_many_arguments)]
    let mk = |sess: &str, pos: i64, t: i64, model: &str, side: bool, io: (i64, i64), cache: (i64, i64), reason: i64| ClientCallRow {
        client_code: "claude_code".into(),
        session_id: sess.into(),
        is_sidechain: side,
        call_time_ms: base + t,
        model: model.into(),
        input_tokens: io.0,
        output_tokens: io.1,
        cache_read_tokens: cache.0,
        cache_write_tokens: cache.1,
        reasoning_tokens: reason,
        cost: Some(0.1),
        source_file_id: format!("{sess}-f"),
        file_pos: pos,
    };
    // 故意乱序插入：详情必须按 call_time_ms 升序。
    insert_client_calls(
        &db,
        &[
            mk("s1", 3, 300, "opus", false, (5, 6), (7, 8), 1),
            mk("s1", 1, 100, "sonnet", false, (10, 20), (30, 40), 2),
            mk("s1", 2, 200, "sonnet", true, (1, 2), (3, 4), 0),
            mk("s1", 4, 400, "sonnet", true, (1, 1), (0, 0), 0),
            mk("s2", 1, 100, "other", false, (99, 99), (99, 99), 9),
        ],
    )
    .await
    .expect("insert calls");
    upsert_client_session(&db, "claude_code".into(), "s1".into(), String::new(), "/w".into(), base + 100, base + 400)
        .await
        .expect("upsert session");

    let rows = client_usage_sessions(&db, 0, base + 10_000, None, 100).await.expect("sessions");
    let s1 = rows.iter().find(|r| r.session_id == "s1").expect("s1");
    assert_eq!(
        (s1.cache_read_tokens, s1.cache_write_tokens, s1.reasoning_tokens, s1.sidechain_calls),
        (40, 52, 3, 2),
        "汇总缓存读 / 写、推理、子代理调用数"
    );

    let d = client_usage_session_detail(&db, "claude_code", "s1", None).await.expect("detail");
    let chain: Vec<(&str, i64)> = d.model_chain.iter().map(|s| (s.model.as_str(), s.calls)).collect();
    assert_eq!(chain, vec![("sonnet", 2), ("opus", 1), ("sonnet", 1)], "相邻同模型合并、按时间排");
    assert_eq!(
        d.model_chain[0],
        ClientUsageModelSegment {
            model: "sonnet".into(),
            calls: 2,
            first_call_ms: base + 100,
            last_call_ms: base + 200,
            input_tokens: 11,
            output_tokens: 22,
            cache_read_tokens: 33,
            cache_write_tokens: 44,
        },
        "段内 token 求和"
    );
    let times: Vec<i64> = d.points.iter().map(|p| p.call_time_ms - base).collect();
    assert_eq!(times, vec![100, 200, 300, 400], "时间序列升序且只含本会话");
    assert_eq!(
        d.points[2],
        ClientUsageTokenPoint { call_time_ms: base + 300, input_tokens: 5, output_tokens: 6, cache_read_tokens: 7, cache_write_tokens: 8 }
    );

    let empty = client_usage_session_detail(&db, "claude_code", "nope", None).await.expect("detail empty");
    assert!(empty.model_chain.is_empty() && empty.points.is_empty(), "无调用的会话返回空");
}

/// 票 04：详情按 (source_file_id, is_sidechain) 分组，组内 token / 成本求和，组按首次调用时间升序；只含本会话。
#[tokio::test]
async fn client_usage_session_detail_groups_by_file() {
    let (db, _dir) = four_file_db().await;
    let base = crate::now() - 10_000;
    let mk = |sess: &str, file: &str, pos: i64, t: i64, side: bool, cost: Option<f64>| ClientCallRow {
        client_code: "claude_code".into(),
        session_id: sess.into(),
        is_sidechain: side,
        call_time_ms: base + t,
        model: "m".into(),
        input_tokens: 1,
        output_tokens: 2,
        cache_read_tokens: 3,
        cache_write_tokens: 4,
        cost,
        source_file_id: file.into(),
        file_pos: pos,
        ..Default::default()
    };
    insert_client_calls(
        &db,
        &[
            mk("s1", "/p/s1/subagents/agent-b.jsonl", 1, 300, true, None),
            mk("s1", "/p/s1.jsonl", 1, 100, false, Some(0.5)),
            mk("s1", "/p/s1.jsonl", 2, 400, false, Some(0.25)),
            mk("s1", "/p/s1/subagents/agent-a.jsonl", 1, 200, true, Some(1.0)),
            mk("s1", "/p/s1/subagents/agent-a.jsonl", 2, 500, true, None),
            mk("s2", "/p/s2.jsonl", 1, 100, false, Some(9.0)),
        ],
    )
    .await
    .expect("insert calls");
    let d = client_usage_session_detail(&db, "claude_code", "s1", None).await.expect("detail");
    let g = |f: &str, side: bool, calls: i64, cost: f64, unpriced: i64| ClientUsageFileGroup {
        source_file_id: f.into(),
        is_sidechain: side,
        calls,
        input_tokens: calls,
        output_tokens: 2 * calls,
        cache_read_tokens: 3 * calls,
        cache_write_tokens: 4 * calls,
        cost,
        unpriced_calls: unpriced,
    };
    assert_eq!(
        d.files,
        vec![
            g("/p/s1.jsonl", false, 2, 0.75, 0),
            g("/p/s1/subagents/agent-a.jsonl", true, 2, 1.0, 1),
            g("/p/s1/subagents/agent-b.jsonl", true, 1, 0.0, 1),
        ],
        "按文件分组、首次调用时间升序、只含本会话"
    );
    let json = serde_json::to_string(&d).expect("serialize");
    assert!(!json.contains("/p/s1"), "文件路径不回传前端: {json}");

    // 票 12：按 part 只读对应部分（行展开区懒加载）。
    let p = client_usage_session_detail(&db, "claude_code", "s1", Some("points")).await.expect("points");
    assert_eq!((p.points.len(), p.model_chain.len(), p.files.len()), (5, 0, 0), "points 只带时间序列");
    let c = client_usage_session_detail(&db, "claude_code", "s1", Some("chain")).await.expect("chain");
    assert_eq!((c.points.len(), c.model_chain.len(), c.files.len()), (0, 1, 0), "chain 只带模型链");
    let a = client_usage_session_detail(&db, "claude_code", "s1", Some("agents")).await.expect("agents");
    assert_eq!((a.points.len(), a.model_chain.len(), a.files.len()), (0, 0, 3), "agents 只带分组");
    assert!(client_usage_session_detail(&db, "claude_code", "s1", Some("bogus")).await.is_err(), "未知 part 报错");
}

/// 票 05：会话活动时刻 = max(last_call_ms, 独占文件 mtime)；共用文件不计 mtime；client 过滤生效。
#[tokio::test]
async fn client_session_activity_max_of_call_and_exclusive_file_mtime() {
    let (db, _dir) = four_file_db().await;
    let call = |code: &str, sid: &str, file: &str, pos: i64| ClientCallRow {
        client_code: code.into(),
        session_id: sid.into(),
        model: "m".into(),
        source_file_id: file.into(),
        file_pos: pos,
        ..Default::default()
    };
    insert_client_calls(
        &db,
        &[
            call("codex", "a", "/f/a.jsonl", 1),
            call("codex", "b", "/f/b.jsonl", 1),
            call("opencode", "x", "/f/oc.db", 1),
            call("opencode", "y", "/f/oc.db", 2),
        ],
    )
    .await
    .expect("insert");
    for (code, path, mtime) in [("codex", "/f/a.jsonl", 9_000), ("codex", "/f/b.jsonl", 100), ("opencode", "/f/oc.db", 99_999)] {
        upsert_client_file_progress(
            &db,
            ClientFileProgress {
                client_code: code.into(),
                path: path.into(),
                file_size: 1,
                mtime_ms: mtime,
                read_offset: 1,
                parse_errors: 0,
                last_msg_id: String::new(),
            },
        )
        .await
        .expect("progress");
    }
    for (code, sid, last) in [("codex", "a", 5_000), ("codex", "b", 7_000), ("opencode", "x", 300), ("opencode", "y", 400)] {
        upsert_client_session(&db, code.into(), sid.into(), String::new(), String::new(), 1, last)
            .await
            .expect("session");
    }
    let ids: Vec<String> = ["a", "b", "x", "y", "nope"].iter().map(|s| s.to_string()).collect();
    let mut got = client_session_activity(&db, None, &ids).await.expect("activity");
    got.sort();
    assert_eq!(
        got,
        vec![
            ("codex".into(), "a".into(), 9_000),   // 文件 mtime 较新
            ("codex".into(), "b".into(), 7_000),   // 最近调用较新
            ("opencode".into(), "x".into(), 300),  // 共用文件 mtime 不计
            ("opencode".into(), "y".into(), 400),
        ]
    );
    let only = client_session_activity(&db, Some("codex"), &ids).await.expect("filtered");
    assert_eq!(only.len(), 2, "client 过滤：只返回 codex 的两个会话");
}

// ── 票 09：从列表隐藏与回收站 ──

async fn seed_session(db: &Db, sess: &str, t: i64) {
    let row = ClientCallRow {
        client_code: "claude_code".into(),
        session_id: sess.into(),
        call_time_ms: t,
        source_file_id: format!("{sess}-f"),
        file_pos: 1,
        cost: Some(0.1),
        ..Default::default()
    };
    insert_client_calls(db, &[row]).await.expect("insert call");
    upsert_client_session(db, "claude_code".into(), sess.into(), format!("title-{sess}"), "/w".into(), t, t)
        .await
        .expect("upsert session");
}

async fn reopen(dir: &tempfile::TempDir) -> Db {
    let db = Db::new(dir.path().join("aidog.db").to_str().expect("utf8")).await.expect("reopen");
    crate::schema::init_tables_raw(&db, std::sync::Arc::new(|_conn, _map| Ok(()))).await.expect("init tables");
    db
}

fn ids(rows: &[ClientUsageSessionRow]) -> Vec<String> {
    let mut v: Vec<String> = rows.iter().map(|r| r.session_id.clone()).collect();
    v.sort();
    v
}

/// hide → 会话列表不含、回收站含（写后重开读回）；unhide 还原。
#[tokio::test]
async fn hide_session_excludes_from_list_and_unhide_restores() {
    let (db, dir) = four_file_db().await;
    let now = crate::now();
    seed_session(&db, "s1", now).await;
    seed_session(&db, "s2", now).await;
    hide_client_session(&db, "claude_code", "s1").await.expect("hide");
    hide_client_session(&db, "claude_code", "s1").await.expect("hide twice is idempotent");
    drop(db);

    let db = reopen(&dir).await;
    let rows = client_usage_sessions(&db, 0, now + 1000, None, 100).await.expect("sessions");
    assert_eq!(ids(&rows), vec!["s2".to_string()], "隐藏会话不在列表");
    let bin = client_usage_hidden_sessions(&db, 0, i64::MAX).await.expect("hidden");
    assert_eq!(bin.len(), 1, "回收站含隐藏会话");
    assert_eq!((bin[0].session.session_id.as_str(), bin[0].session.title.as_str()), ("s1", "title-s1"));
    assert_eq!(bin[0].session.calls, 1, "回收站行带聚合");
    assert!(bin[0].hidden_at > 0, "hidden_at 盖章");

    unhide_client_session(&db, "claude_code", "s1").await.expect("unhide");
    drop(db);
    let db = reopen(&dir).await;
    let rows = client_usage_sessions(&db, 0, now + 1000, None, 100).await.expect("sessions");
    assert_eq!(ids(&rows), vec!["s1".to_string(), "s2".to_string()], "恢复后回到列表");
    assert!(client_usage_hidden_sessions(&db, 0, i64::MAX).await.expect("hidden").is_empty(), "回收站清空");
}

/// 文件截断重读（delete_client_file_state 删会话壳）后重扫，会话仍保持隐藏。
#[tokio::test]
async fn hidden_session_survives_file_state_reset_and_rescan() {
    let (db, _dir) = four_file_db().await;
    let now = crate::now();
    seed_session(&db, "s1", now).await;
    hide_client_session(&db, "claude_code", "s1").await.expect("hide");
    delete_client_file_state(&db, "claude_code", "s1-f").await.expect("reset file");
    assert_eq!(count_sessions(&db, "claude_code".into()).await, 0, "会话壳已删");
    seed_session(&db, "s1", now).await; // 重扫
    let rows = client_usage_sessions(&db, 0, now + 1000, None, 100).await.expect("sessions");
    assert!(rows.is_empty(), "重扫后仍隐藏");
    let bin = client_usage_hidden_sessions(&db, 0, i64::MAX).await.expect("hidden");
    assert_eq!(bin.len(), 1, "回收站仍含");
}

/// 保留期清理：会话行已不存在且隐藏早于保留期的隐藏行删；会话仍在的隐藏行保留。
#[tokio::test]
async fn cleanup_drops_hidden_rows_of_vanished_sessions() {
    let (db, _dir) = four_file_db().await;
    let now = crate::now();
    let old = now - 200 * 86_400_000;
    seed_session(&db, "live", now).await;
    db.call_client_usage_traced(None, std::panic::Location::caller(), move |conn| {
        conn.execute(
            "INSERT INTO client_session_hidden (client_code, session_id, hidden_at) VALUES \
             ('claude_code','live',?1), ('claude_code','gone',?1)",
            params![old],
        )
    })
    .await
    .expect("seed hidden");
    cleanup_client_usage(&db, 90).await.expect("cleanup");
    let left: Vec<String> = db
        .call_read_client_usage_traced(None, std::panic::Location::caller(), |conn| {
            let mut s = conn.prepare("SELECT session_id FROM client_session_hidden ORDER BY 1")?;
            let v = s.query_map([], |r| r.get(0))?.collect::<Result<Vec<String>, _>>()?;
            Ok(v)
        })
        .await
        .expect("read hidden");
    assert_eq!(left, vec!["live".to_string()], "只留会话仍在的隐藏行");
}

/// 票 13：增量扫描传空 workdir 不覆盖已存非空值；传新非空值仍更新（写后重开读回）。
#[tokio::test]
async fn upsert_session_empty_workdir_keeps_stored_value() {
    let (db, dir) = four_file_db().await;
    upsert_client_session(&db, "claude_code".into(), "s1".into(), "t".into(), "/proj".into(), 1, 2)
        .await
        .expect("first");
    upsert_client_session(&db, "claude_code".into(), "s1".into(), String::new(), String::new(), 3, 4)
        .await
        .expect("incremental empty");
    drop(db);
    let db = reopen(&dir).await;
    async fn read(db: &Db) -> String {
        db.call_read_client_usage_traced(None, std::panic::Location::caller(), |conn| {
            conn.query_row("SELECT workdir FROM client_session WHERE session_id = 's1'", [], |r| r.get::<_, String>(0))
        })
        .await
        .expect("read workdir")
    }
    assert_eq!(read(&db).await, "/proj", "空 workdir 不覆盖");
    upsert_client_session(&db, "claude_code".into(), "s1".into(), String::new(), "/moved".into(), 5, 6)
        .await
        .expect("new workdir");
    drop(db);
    let db = reopen(&dir).await;
    assert_eq!(read(&db).await, "/moved", "非空新值照常更新");
}

/// 迁移重复执行幂等，隐藏行不丢（回归护栏）。
#[test]
fn client_session_hidden_migration_idempotent() {
    let conn = rusqlite::Connection::open_in_memory().expect("mem");
    crate::schema_client_usage::run_migrations_client_usage(&conn).expect("1st");
    conn.execute("INSERT INTO client_session_hidden (client_code, session_id, hidden_at) VALUES ('c','s',1)", [])
        .expect("insert");
    crate::schema_client_usage::run_migrations_client_usage(&conn).expect("2nd");
    let n: i64 = conn.query_row("SELECT COUNT(*) FROM client_session_hidden", [], |r| r.get(0)).expect("count");
    assert_eq!(n, 1);
}
