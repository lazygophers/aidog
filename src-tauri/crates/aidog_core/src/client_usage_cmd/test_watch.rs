//! 票 03 监听与定时对齐测试：tempdir 注入，禁碰真实用户目录。
//!
//! 实时链路走真实 notify watcher（FSEvents）：写文件 → 事件入池 → 测试侧轮询
//! `process_due_paths` 消费（事件循环生产中不启动）。全局去抖池被并行测试共享，
//! 所有 watch 测试经 WATCH_LOCK 串行。

use std::path::PathBuf;
use std::time::Duration;

use super::watch::{
    align_once, cached_roots, process_due_paths, refresh_roots_cache, scan_count,
    sync_watchers_with, watched_snapshot, CLAUDE_PID_KEY,
};
use super::*;

/// 全局 PENDING / WATCHED / WATCHER 静态共享，watch 测试串行化。
static WATCH_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

fn assistant(
    id: &str,
    sid: &str,
    model: &str,
    ts: &str,
    in_tok: i64,
    out_tok: i64,
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
                "cache_read_input_tokens": 0,
                "cache_creation_input_tokens": 0
            }
        }
    })
}

fn write_jsonl(dir: &std::path::Path, name: &str, lines: &[serde_json::Value]) -> PathBuf {
    let p = dir.join(name);
    let mut s = String::new();
    for l in lines {
        s.push_str(&serde_json::to_string(l).unwrap());
        s.push('\n');
    }
    std::fs::write(&p, s).unwrap();
    p
}

async fn count(db: &aidog_db::Db, sql: &'static str) -> i64 {
    db.call_read_client_usage_traced(None, std::panic::Location::caller(), |conn| {
        conn.query_row(sql, [], |r| r.get(0))
    })
    .await
    .unwrap()
}

async fn count_calls(db: &aidog_db::Db) -> i64 {
    count(db, "SELECT COUNT(*) FROM client_call").await
}

async fn count_progress(db: &aidog_db::Db) -> i64 {
    count(db, "SELECT COUNT(*) FROM client_file_progress").await
}

async fn count_sessions(db: &aidog_db::Db) -> i64 {
    count(db, "SELECT COUNT(*) FROM client_session").await
}

fn consent_on(clients: &[&str]) -> ClientUsageConsent {
    let mut c = ClientUsageConsent { asked: true, ..Default::default() };
    for x in clients {
        c.enabled.insert(x.to_string(), true);
    }
    c
}

/// 轮询：反复消费去抖池直至条件满足或超时（测试内等待异步监听收敛）。
async fn poll_until(
    db: &aidog_db::Db,
    roots: &[(String, PathBuf)],
    mut expect: impl FnMut() -> bool,
    secs: u64,
) -> bool {
    let deadline = std::time::Instant::now() + Duration::from_secs(secs);
    loop {
        process_due_paths(db, roots, aidog_db::now(), &|_| {}).await;
        if expect() {
            return true;
        }
        if std::time::Instant::now() >= deadline {
            return false;
        }
        tokio::time::sleep(Duration::from_millis(300)).await;
    }
}

/// 实时链路端到端：新建/追加/截断/删除，明细随之变化；删除后明细、进度、空会话壳清零。
#[tokio::test]
async fn watch_imports_append_truncate_and_delete() {
    let _g = WATCH_LOCK.lock().await;
    let (db, _dir) = aidog_db::test_support::four_file_db().await;
    let d = tempfile::tempdir().unwrap();
    let roots = vec![("claude_code".to_string(), d.path().to_path_buf())];
    sync_watchers_with(&roots);

    // 新建 → 1 条明细。
    write_jsonl(
        d.path(),
        "a.jsonl",
        &[assistant("m1", "s1", "claude-haiku-5-5", "2026-11-19T08:00:00Z", 1, 1)],
    );
    assert!(
        poll_until(&db, &roots, || {
            futures::executor::block_on(count_calls(&db)) == 1
        }, 20)
        .await,
        "新建文件应被监听导入"
    );

    // 追加 → 2 条明细（增量，只读新增）。
    let p = d.path().join("a.jsonl");
    let mut s = std::fs::read_to_string(&p).unwrap();
    s.push_str(&serde_json::to_string(&assistant("m2", "s1", "claude-haiku-5-5", "2026-11-19T08:01:00Z", 5, 5)).unwrap());
    s.push('\n');
    std::fs::write(&p, s).unwrap();
    assert!(
        poll_until(&db, &roots, || {
            futures::executor::block_on(count_calls(&db)) == 2
        }, 20)
        .await,
        "追加应增量导入"
    );

    // 截断 → 重建（1 条）。
    write_jsonl(
        d.path(),
        "a.jsonl",
        &[assistant("m3", "s1", "claude-haiku-5-5", "2026-11-19T08:02:00Z", 7, 7)],
    );
    assert!(
        poll_until(&db, &roots, || {
            futures::executor::block_on(count_calls(&db)) == 1
        }, 20)
        .await,
        "截断应触发重建"
    );

    // 删除 → 明细/进度/空会话壳清零。
    std::fs::remove_file(&p).unwrap();
    assert!(
        poll_until(&db, &roots, || {
            futures::executor::block_on(async {
                count_calls(&db).await == 0
                    && count_progress(&db).await == 0
                    && count_sessions(&db).await == 0
            })
        }, 20)
        .await,
        "删除文件应清明细/进度/空会话壳"
    );
}

/// 关掉客户端（consent 关 + 监听面摘除）后写文件不再触发读取。
#[tokio::test]
async fn disabled_client_no_read() {
    let _g = WATCH_LOCK.lock().await;
    let (db, _dir) = aidog_db::test_support::four_file_db().await;
    let d = tempfile::tempdir().unwrap();
    let roots = vec![("claude_code".to_string(), d.path().to_path_buf())];
    sync_watchers_with(&roots);
    let canonical = d.path().canonicalize().unwrap().to_string_lossy().into_owned();
    assert!(
        watched_snapshot().iter().any(|r| r == &canonical),
        "开启时应已挂监听"
    );
    // 关闭：该根目录监听摘除。
    sync_watchers_with(&[]);
    assert!(
        !watched_snapshot().iter().any(|r| r == &canonical),
        "关闭后应摘除监听"
    );
    // 池内残留事件（模拟关闭前收到的写事件）也不得读取。
    let p = write_jsonl(
        d.path(),
        "b.jsonl",
        &[assistant("m1", "s2", "claude-haiku-5-5", "2026-11-19T08:00:00Z", 3, 3)],
    );
    super::watch::take_due_paths(aidog_db::now() + 10_000);
    super::watch::note_event_for_test(p.to_string_lossy().into_owned(), aidog_db::now() - 5_000);
    let n = process_due_paths(&db, &[], aidog_db::now(), &|_| {}).await;
    assert_eq!(n, 0, "关闭后零读取");
    assert_eq!(count_calls(&db).await, 0, "零明细");
    assert_eq!(count_progress(&db).await, 0, "零进度行");
}

/// 去抖：同一文件 100ms 内连写 5 次，读取次数为 1。
#[tokio::test]
async fn debounce_coalesces_burst() {
    let _g = WATCH_LOCK.lock().await;
    let (db, _dir) = aidog_db::test_support::four_file_db().await;
    let d = tempfile::tempdir().unwrap();
    let roots = vec![("claude_code".to_string(), d.path().to_path_buf())];
    sync_watchers_with(&roots);
    let p = write_jsonl(
        d.path(),
        "c.jsonl",
        &[assistant("m1", "s3", "claude-haiku-5-5", "2026-11-19T08:00:00Z", 1, 1)],
    );
    // 100ms 内连写 5 次。
    for i in 0..5 {
        let mut s = std::fs::read_to_string(&p).unwrap();
        s.push_str(&serde_json::to_string(&assistant(
            &format!("mb{i}"), "s3", "claude-haiku-5-5", "2026-11-19T08:01:00Z", 1, 1,
        )).unwrap());
        s.push('\n');
        std::fs::write(&p, s).unwrap();
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    let before = scan_count();
    // 等去抖窗口过完再消费一次。
    tokio::time::sleep(Duration::from_millis(2600)).await;
    let n = process_due_paths(&db, &roots, aidog_db::now(), &|_| {}).await;
    assert_eq!(scan_count() - before, 1, "同一文件连写 5 次只读取 1 次");
    // 单次扫描读全：首次 + 5 次追加 = 6 条明细。
    assert_eq!(n, 6, "去抖合并后一次读入全部内容");
    assert_eq!(count_calls(&db).await, 6);
}

/// 定时对齐：变化 → 增量读取；无变化不重扫；删除 → 清明细/进度/空会话壳。
#[tokio::test]
async fn align_detects_change_and_delete() {
    let _g = WATCH_LOCK.lock().await;
    let (db, _dir) = aidog_db::test_support::four_file_db().await;
    save_consent(&db, &consent_on(&["claude_code"])).await.unwrap();
    let d = tempfile::tempdir().unwrap();
    let p = write_jsonl(
        d.path(),
        "a.jsonl",
        &[assistant("m1", "s1", "claude-haiku-5-5", "2026-11-19T08:00:00Z", 1, 1)],
    );
    // 基线：先导一次（模拟票 02 启动导入已建进度）。
    let (n, _) = scan_one_file(&db, "claude_code", &p, None, d.path()).await.unwrap();
    assert_eq!(n, 1);

    // 追加 → 对齐读到 2。
    let mut s = std::fs::read_to_string(&p).unwrap();
    s.push_str(&serde_json::to_string(&assistant("m2", "s1", "claude-haiku-5-5", "2026-11-19T08:01:00Z", 5, 5)).unwrap());
    s.push('\n');
    std::fs::write(&p, &s).unwrap();
    let n = align_once(&db).await;
    assert_eq!(n, 1, "对齐应增量读 1 条");
    assert_eq!(count_calls(&db).await, 2);

    // 无变化 → 不产生新明细。
    let n = align_once(&db).await;
    assert_eq!(n, 0, "无变化零导入");
    assert_eq!(count_calls(&db).await, 2);

    // 删除 → 明细/进度/空会话壳清零。
    std::fs::remove_file(&p).unwrap();
    let n = align_once(&db).await;
    let _ = n;
    assert_eq!(count_calls(&db).await, 0, "删除后明细清零");
    assert_eq!(count_progress(&db).await, 0, "删除后进度清零");
    assert_eq!(count_sessions(&db).await, 0, "空会话壳清零");
}

/// 关掉同意开关后对齐零动作。
#[tokio::test]
async fn align_respects_consent_gate() {
    let _g = WATCH_LOCK.lock().await;
    let (db, _dir) = aidog_db::test_support::four_file_db().await;
    let d = tempfile::tempdir().unwrap();
    let p = write_jsonl(
        d.path(),
        "a.jsonl",
        &[assistant("m1", "s9", "claude-haiku-5-5", "2026-11-19T08:00:00Z", 1, 1)],
    );
    save_consent(&db, &consent_on(&["claude_code"])).await.unwrap();
    scan_one_file(&db, "claude_code", &p, None, d.path()).await.unwrap();
    // 关开关：align 不再碰文件（即使 mtime 变了也不读）。
    save_consent(&db, &ClientUsageConsent { asked: true, ..Default::default() })
        .await
        .unwrap();
    let mut s = std::fs::read_to_string(&p).unwrap();
    s.push('\n');
    std::fs::write(&p, &s).unwrap();
    let n = align_once(&db).await;
    assert_eq!(n, 0, "关着零对齐读取");
    assert_eq!(count_calls(&db).await, 1);
}

/// 票 05：有新明细时 emit 一次 `client-usage-updated`；无新数据不 emit；pid 文件目录事件 emit 一次。
#[tokio::test]
async fn process_due_paths_emits_updated_once_on_new_data() {
    let _g = WATCH_LOCK.lock().await;
    let (db, _dir) = aidog_db::test_support::four_file_db().await;
    let d = tempfile::tempdir().unwrap();
    let pid_dir = tempfile::tempdir().unwrap();
    let roots = vec![
        ("claude_code".to_string(), d.path().to_path_buf()),
        (super::watch::CLAUDE_PID_KEY.to_string(), pid_dir.path().to_path_buf()),
    ];
    let events = std::sync::Mutex::new(Vec::<String>::new());
    let emit = |e: &str| events.lock().unwrap().push(e.to_string());
    let now = aidog_db::now();
    super::watch::take_due_paths(now + 10_000);

    let p = write_jsonl(d.path(), "e.jsonl", &[
        assistant("m1", "se", "claude-haiku-5-5", "2026-11-19T08:00:00Z", 1, 1),
        assistant("m2", "se", "claude-haiku-5-5", "2026-11-19T08:01:00Z", 1, 1),
    ]);
    super::watch::note_event_for_test(p.to_string_lossy().into_owned(), now - 5_000);
    assert_eq!(process_due_paths(&db, &roots, now, &emit).await, 2);
    assert_eq!(*events.lock().unwrap(), vec!["client-usage-updated"], "新数据 emit 一次");

    // 同文件无新内容：不 emit。
    super::watch::note_event_for_test(p.to_string_lossy().into_owned(), now - 5_000);
    assert_eq!(process_due_paths(&db, &roots, now, &emit).await, 0);
    assert_eq!(events.lock().unwrap().len(), 1, "无新数据不 emit");

    // pid 文件目录事件：不读取，只 emit 一次。
    let pf = pid_dir.path().join("1.json");
    std::fs::write(&pf, "{}").unwrap();
    super::watch::note_event_for_test(pf.to_string_lossy().into_owned(), now - 5_000);
    process_due_paths(&db, &roots, now, &emit).await;
    assert_eq!(events.lock().unwrap().len(), 2, "pid 文件变化 emit 一次");
}

/// 监听面缓存：consent 变更不经刷新不可见（缓存语义），sync_watchers 刷新后可见。
#[tokio::test]
async fn roots_cache_consent_invalidation() {
    let _g = WATCH_LOCK.lock().await;
    let _home = aidog_db::test_support::HomeGuard::new();
    let (db, _dir) = aidog_db::test_support::four_file_db().await;
    // 初始：同意全关 → 缓存为空。
    assert!(refresh_roots_cache(&db).await.is_empty(), "全关时零监听面");
    // DB 打开 claude_code 但不刷新 → cached_roots 仍走旧缓存（每拍零 DB 读的语义）。
    save_consent(&db, &consent_on(&["claude_code"])).await.unwrap();
    let roots = cached_roots(&db).await;
    assert!(roots.is_empty(), "缓存未失效时仍返回旧监听面");
    // sync_watchers（consent_set 生产路径）刷新 → 新监听面可见；sessions 目录不存在则无 pid 根。
    super::watch::sync_watchers(&db).await;
    let roots = cached_roots(&db).await;
    assert_eq!(roots.len(), 1, "刷新后应只有 claude_code 根");
    assert_eq!(roots[0].0, "claude_code");
    assert_eq!(roots[0].1, _home.home().join(".claude").join("projects"));
    assert!(!roots.iter().any(|(c, _)| c == CLAUDE_PID_KEY), "sessions 目录不存在时无 pid 根");
}

/// 监听面缓存：pid 目录出现/消失由每拍 stat 检测，cached_roots 自动回源刷新。
#[tokio::test]
async fn roots_cache_pid_dir_lifecycle() {
    let _g = WATCH_LOCK.lock().await;
    let _home = aidog_db::test_support::HomeGuard::new();
    let (db, _dir) = aidog_db::test_support::four_file_db().await;
    save_consent(&db, &consent_on(&["claude_code"])).await.unwrap();
    super::watch::sync_watchers(&db).await;
    let sessions = _home.home().join(".claude").join("sessions");
    assert!(
        !cached_roots(&db).await.iter().any(|(c, _)| c == CLAUDE_PID_KEY),
        "目录不存在时无 pid 根"
    );

    // 目录出现 → 缓存存在性标记不符 → 回源刷新，pid 根进入监听面。
    std::fs::create_dir_all(&sessions).unwrap();
    let roots = cached_roots(&db).await;
    assert!(
        roots.iter().any(|(c, r)| c == CLAUDE_PID_KEY && r == &sessions),
        "目录出现应刷新缓存纳入 pid 根"
    );

    // 目录消失 → 同一检测路径回源刷新，pid 根摘除。
    std::fs::remove_dir(&sessions).unwrap();
    let roots = cached_roots(&db).await;
    assert!(
        !roots.iter().any(|(c, _)| c == CLAUDE_PID_KEY),
        "目录消失应刷新缓存摘除 pid 根"
    );
}
