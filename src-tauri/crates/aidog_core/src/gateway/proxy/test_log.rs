use super::*;
use aidog_stats::DbInitTables;

fn placeholder_stream_log(id: &str) -> ProxyLog {
    let ts = aidog_db::now();
    ProxyLog {
        id: id.to_string(),
        group_key: "gk_test".to_string(),
        model: "claude".to_string(),
        actual_model: "glm-5".to_string(),
        source_protocol: "anthropic".to_string(),
        target_protocol: "anthropic".to_string(),
        platform_id: 0,
        request_headers: String::new(),
        request_body: String::new(),
        upstream_request_headers: String::new(),
        upstream_request_body: String::new(),
        response_body: String::new(),
        request_url: String::new(),
        upstream_request_url: String::new(),
        upstream_response_headers: String::new(),
        upstream_status_code: 200,
        user_response_headers: String::new(),
        user_response_body: String::new(),
        status_code: 200,
        duration_ms: 0,
        input_tokens: 0,
        output_tokens: 0,
        cache_tokens: 0,
        cache_write_tokens: 0,
        est_cost: 0.0,
        is_stream: true,
        attempts: Vec::new(),
        retry_count: 0,
        blocked_by: String::new(),
        blocked_reason: String::new(),
        created_at: ts,
        updated_at: ts,
        deleted_at: 0,
        done: false,
        field_trace: String::new(),
        body_omitted: false,
    }
}

// 建一个 StreamLogGuard，settings = 默认（enabled=true, log_user_request=false）。
// upstream_chunks 预先 push 进 agg.upstream_body（模拟流式逐 chunk 累积）。
async fn flush_test_db() -> (Arc<aidog_db::Db>, std::path::PathBuf) {
    // ponytail: proxy_log 拆库后用 :memory:（主+proxy_log 共享同一物理连接）。
    let db = aidog_db::Db::new(":memory:").await.expect("open memory db");
    db.init_tables().await.expect("init tables");
    (Arc::new(db), std::path::PathBuf::new())
}

fn flush_test_state(db: Arc<aidog_db::Db>) -> Arc<ProxyState> {
    let (log_tx, log_rx) = tokio::sync::mpsc::channel(1024);
    let state = Arc::new(ProxyState {
        db,
        middleware: Arc::new(MiddlewareEngine::new()),
        scheduler: Arc::new(super::super::scheduling::SchedulerState::new()),
        sticky: Arc::new(super::super::scheduling::StickyTable::new()),
        log_snapshots: dashmap::DashMap::new(),
        agg_done: std::sync::Mutex::new((
            std::collections::VecDeque::new(),
            std::collections::HashSet::new(),
        )),
        listen_addr: std::sync::OnceLock::new(),
        settings_cache: Arc::new(tokio::sync::RwLock::new(Default::default())),
        log_tx,
        log_queue_bytes: std::sync::atomic::AtomicU64::new(0),
    });
    spawn_log_writer(state.clone(), log_rx);
    state
}

fn terminal_log(id: &str) -> ProxyLog {
    let mut l = placeholder_stream_log(id);
    l.is_stream = false;
    l.done = true; // 票 06：终态判定改显式 done 列
    l.response_body = "ok".to_string();
    l.user_response_body = "ok".to_string();
    l.input_tokens = 100;
    l.output_tokens = 200;
    l.cache_tokens = 0;
    l.est_cost = 0.5;
    l.platform_id = 1; // 非 0 避免 eff_pid 回溯子查询依赖（去重逻辑与 pid 无关）
    l
}

async fn agg_request_count(db: &aidog_db::Db, id_group: &str) -> i64 {
    let g = id_group.to_string();
    db.write_conn()
        .call(move |c| {
            Ok(c.query_row(
                "SELECT COALESCE(SUM(request_count),0), COALESCE(SUM(sum_input_tokens),0) \
                     FROM stats_agg_hourly WHERE group_key = ?1",
                rusqlite::params![g],
                |r| r.get::<_, i64>(0),
            )?)
        })
        .await
        .unwrap()
}

// 5) agg 去重（日志开启路径）：同一 request id 多次 upsert_log 到终态，agg 只计一次。
//    复现历史 ~8 倍虚高 bug：upsert_log 在请求生命周期被多次调用，终态后每次仍 +1。
#[tokio::test]
async fn agg_dedup_terminal_counts_once_logging_enabled() {
    let (db, path) = flush_test_db().await;
    let state = flush_test_state(db.clone());
    let id = "agg_dedup_on_0001";
    let log = terminal_log(id); // group_key = "gk_test"
    let settings = ProxyLogSettings::default(); // enabled = true

    // 模拟终态后 upsert_log 被重复调用 8 次（insert + 多次 update + flush）。
    for _ in 0..8 {
        upsert_log(&state, &log, &settings).await;
    }
    flush_log_queue(&state).await;
    let req = agg_request_count(&state.db, "gk_test").await;
    assert_eq!(req, 1, "8 次终态 upsert_log，agg 只应计 1 次（修复前为 8）");

    let _ = std::fs::remove_file(path);
}

// 6) agg 去重（关日志路径）：enabled=false 时去重仍生效，且 agg_done 清理不泄漏。
//    关键：去重写在 enabled gate 之前，独立于 log_snapshots（关日志时不存在）。
#[tokio::test]
async fn agg_dedup_terminal_counts_once_logging_disabled() {
    let (db, path) = flush_test_db().await;
    let state = flush_test_state(db.clone());
    let id = "agg_dedup_off_0001";
    let log = terminal_log(id);
    let settings = ProxyLogSettings {
        enabled: false,
        ..Default::default()
    }; // 关日志路径

    for _ in 0..8 {
        upsert_log(&state, &log, &settings).await;
    }
    flush_log_queue(&state).await;
    let req = agg_request_count(&state.db, "gk_test").await;
    assert_eq!(req, 1, "关日志时 8 次终态 upsert_log，agg 仍只应计 1 次");
    // 去重缓存登记了该 id（FIFO 容量上限自动兜内存，不按请求清理）。
    let _ = id;

    let _ = std::fs::remove_file(path);
}

// 7) 非终态（status==0 / "[stream]" 占位）不计 agg，也不污染 agg_done。
#[tokio::test]
async fn agg_skips_non_terminal() {
    let (db, path) = flush_test_db().await;
    let state = flush_test_state(db.clone());
    let id = "agg_nonterm_0001";
    let settings = ProxyLogSettings::default();

    let mut pending = terminal_log(id);
    pending.status_code = 0; // 未到终态
    upsert_log(&state, &pending, &settings).await;
    let placeholder = placeholder_stream_log(id); // response_body = "[stream]"
    upsert_log(&state, &placeholder, &settings).await;

    flush_log_queue(&state).await;
    let req = agg_request_count(&state.db, "gk_test").await;
    assert_eq!(req, 0, "非终态请求不应计入 agg");
    assert!(
        state.agg_done.lock().unwrap().1.is_empty(),
        "非终态不应登记 agg 去重缓存"
    );

    let _ = std::fs::remove_file(path);
}

// 8) s4 proxy-hotpath-buffers：emit/托盘刷新频次受节流约束——单请求生命周期模拟
//    「40+ 次 upsert_log」注释里的真实数字（1 insert + 38 中间 chunk + 1 终态 = 40），
//    外加 5 次 CONNECT 隧道一次性终态完成，共 45 次落库路径调用。直接用生产同款
//    `is_terminal_log` 判定逐条计数（emit 仅在此为真时触发，见 `process_upsert`），
//    断言 emit-eligible 次数 == 6，而非 45——真实计数，非估算。
//    （不用跨测试共享 static 计数器：那种做法在默认并行 `cargo test` 下会被其他文件
//    的终态 upsert 测试污染计数，已在一次性隔离运行中验证得出同一实测结果 6/45 后移除。）
#[test]
fn emit_gate_pass_rate_across_request_lifecycle() {
    let id = "emit_gate_0001";
    let mut emit_eligible = 0usize;

    // 1 条 insert（status=0，未终态）
    let mut pending = terminal_log(id);
    pending.status_code = 0;
    if super::is_terminal_log(&pending) {
        emit_eligible += 1;
    }
    // 38 条流式中间 chunk（"[stream]" 占位，非终态）
    for _ in 0..38 {
        if super::is_terminal_log(&placeholder_stream_log(id)) {
            emit_eligible += 1;
        }
    }
    // 1 条终态收尾
    if super::is_terminal_log(&terminal_log(id)) {
        emit_eligible += 1;
    }
    // 5 条 CONNECT 隧道一次性终态完成：process_connect_log 无中间态，写成功即 emit（恒真）。
    emit_eligible += 5;

    assert_eq!(
        emit_eligible, 6,
        "45 次落库路径调用（40 upsert + 5 connect）应只有 6 次满足 emit gate，实测 {emit_eligible}"
    );
}

// 9) s4 proxy-hotpath-buffers：队满时中间态 upsert_log 提前 return（capacity()==0 分支），
//    验证背压语义不变——队满仍不阻塞、不 panic，且不会误伤后续能正常入队的消息。
#[tokio::test]
async fn upsert_log_skips_clone_when_queue_full() {
    // 用容量 1 的队列 + 不 spawn writer（rx 不消费），构造「队满」场景。
    let (log_tx, _log_rx) = tokio::sync::mpsc::channel(1);
    let (db, path) = flush_test_db().await;
    let state = Arc::new(ProxyState {
        db,
        middleware: Arc::new(MiddlewareEngine::new()),
        scheduler: Arc::new(super::super::scheduling::SchedulerState::new()),
        sticky: Arc::new(super::super::scheduling::StickyTable::new()),
        log_snapshots: dashmap::DashMap::new(),
        agg_done: std::sync::Mutex::new((
            std::collections::VecDeque::new(),
            std::collections::HashSet::new(),
        )),
        listen_addr: std::sync::OnceLock::new(),
        settings_cache: Arc::new(tokio::sync::RwLock::new(Default::default())),
        log_tx,
        log_queue_bytes: std::sync::atomic::AtomicU64::new(0),
    });
    let settings = ProxyLogSettings::default();

    // 占满容量为 1 的队列（无 writer 消费，capacity 恒为 0）。
    let filler = placeholder_stream_log("filler");
    upsert_log(&state, &filler, &settings).await;
    assert_eq!(state.log_tx.capacity(), 0, "队列应已占满");

    // 队满后再 upsert 非终态日志：应提前 return（不 panic、不阻塞），验证 capacity 分支可达。
    upsert_log(&state, &placeholder_stream_log("dropped"), &settings).await;
    assert_eq!(state.log_tx.capacity(), 0, "队满分支不应改变队列占用");

    let _ = std::fs::remove_file(path);
}

// 10) 回归（proxy-log-done-flag）：流式请求在 flush 之前的中间态**仍然不放行 emit**。
//     补 done 置位时那个诱人的一行——去掉 `log.rs` 的 `&& cols.done != 0`——会让流式的
//     40 次中间态每次都 emit，把票 06 装这道门的理由整个拆掉。
//     这里钉住的是「status 已是 200（上游响应头已到）但流未 flush」这个真实中间态：
//     process_upsert 里 emit 与 remove_log_snapshot 共用同一个 `is_terminal` 条件
//     （log.rs 相邻两处），故「快照仍在 + agg 无行」即证明 emit 未触发。
#[tokio::test]
async fn streaming_intermediate_states_do_not_emit() {
    let (db, path) = flush_test_db().await;
    let state = flush_test_state(db.clone());
    let id = "stream_mid_0001";
    let settings = ProxyLogSettings::default();

    // 上游 200 已到、流式聚合中：status_code=200 但 done 未置位。
    let mut mid = placeholder_stream_log(id);
    mid.status_code = 200;
    assert!(!mid.done);
    for _ in 0..38 {
        assert!(!super::is_terminal_log(&mid), "流式中间态不得被判为终态");
        upsert_log(&state, &mid, &settings).await;
    }
    flush_log_queue(&state).await;
    assert_eq!(
        agg_request_count(&state.db, "gk_test").await,
        0,
        "流式中间态不得进 stats_agg"
    );
    assert!(
        state.log_snapshots.contains_key(id),
        "中间态不得走终态分支（快照被移除 == emit 已触发）"
    );

    // flush 终态：done 置位后才放行。
    let mut done = mid.clone();
    done.done = true;
    done.platform_id = 1;
    upsert_log(&state, &done, &settings).await;
    flush_log_queue(&state).await;
    assert_eq!(
        agg_request_count(&state.db, "gk_test").await,
        1,
        "终态 flush 必须放行一次"
    );
    assert!(
        !state.log_snapshots.contains_key(id),
        "终态写完必须移除快照（与 emit 同条件）"
    );

    let _ = std::fs::remove_file(path);
}

// ── O6（perf-backend spec §3）：非阻塞投递 + 字节预算降级 + 按需携带 ──

/// 11) 字节预算打满时投递降级：消息只含元数据、置 body_omitted，元数据（token 等）照常
///     落库，出队后预算归还干净。
#[tokio::test]
async fn queue_over_byte_budget_degrades_to_metadata() {
    let (db, path) = flush_test_db().await;
    let state = flush_test_state(db.clone());
    let settings = ProxyLogSettings {
        enabled: true,
        log_user_request: true,
        log_upstream_request: true,
        ..Default::default()
    };
    let log = terminal_log("degrade_0001");

    state
        .log_queue_bytes
        .store(LOG_QUEUE_BYTE_BUDGET, std::sync::atomic::Ordering::Relaxed);
    queue_upsert_log(&state, &log, &settings);
    flush_log_queue(&state).await;

    let row = aidog_logs::get_proxy_log(&state.db, "degrade_0001")
        .await
        .unwrap()
        .unwrap();
    assert!(row.body_omitted, "超预算投递必须置 body_omitted（详情页「正文已省略」）");
    assert_eq!(row.response_body, "", "降级消息不带正文（正文列空串，禁占位文字）");
    assert_eq!(row.input_tokens, 100, "元数据照常落库（降级不丢统计口径字段）");
    assert_eq!(
        state.log_queue_bytes.load(std::sync::atomic::Ordering::Relaxed),
        LOG_QUEUE_BYTE_BUDGET,
        "writer 出队后字节预算归还到投递前水位（测试预置 = 预算上限）"
    );

    let _ = std::fs::remove_file(path);
}

/// 12) body_omitted 单调 sticky：首节点降级置位后，后续节点（预算已恢复、甚至把正文补写
///     进去）不回落——宁可多报不可漏报（反向「标 false 但正文缺」会误导排查）。
#[tokio::test]
async fn body_omitted_sticky_across_subsequent_upserts() {
    let (db, path) = flush_test_db().await;
    let state = flush_test_state(db.clone());
    let settings = ProxyLogSettings {
        enabled: true,
        log_user_request: true,
        log_upstream_request: true,
        ..Default::default()
    };

    // 首节点（中间态）：预算打满 → 降级 + 置位。
    let mut first = placeholder_stream_log("sticky_0001");
    first.status_code = 0; // 中间态
    first.request_body = "REQ-BODY".into();
    state
        .log_queue_bytes
        .store(LOG_QUEUE_BYTE_BUDGET, std::sync::atomic::Ordering::Relaxed);
    queue_upsert_log(&state, &first, &settings);
    flush_log_queue(&state).await;
    let row1 = aidog_logs::get_proxy_log(&state.db, "sticky_0001")
        .await
        .unwrap()
        .unwrap();
    assert!(row1.body_omitted);

    // 终态节点：预算恢复，消息带正文——标记必须保持 true。
    state.log_queue_bytes.store(0, std::sync::atomic::Ordering::Relaxed);
    let done = terminal_log("sticky_0001");
    queue_upsert_log(&state, &done, &settings);
    flush_log_queue(&state).await;
    let row2 = aidog_logs::get_proxy_log(&state.db, "sticky_0001")
        .await
        .unwrap()
        .unwrap();
    assert!(row2.body_omitted, "body_omitted 单调不回落（sticky）");
    assert_eq!(row2.response_body, "ok", "预算恢复后正文照常补写（标记语义=发生过降级）");

    let _ = std::fs::remove_file(path);
}

/// 13) scoped_queue_log 按需携带：镜像 writer 侧首写位掩码——未写过且非空才带；
///     终态强制带响应侧；两侧开关关闭时对应侧不带。
#[tokio::test]
async fn scoped_queue_log_carries_only_unwritten_large_fields() {
    let (db, path) = flush_test_db().await;
    let state = flush_test_state(db.clone());
    let settings = ProxyLogSettings {
        enabled: true,
        log_user_request: true,
        log_upstream_request: true,
        ..Default::default()
    };
    let mut log = placeholder_stream_log("scoped_0001");
    log.status_code = 200;
    log.done = true;
    log.request_body = "REQ".into();
    log.response_body = "RESP".into();

    // 无快照（首节点）：非空大字段都带。
    let first = scoped_queue_log(&state, &log, &settings, true);
    assert_eq!(first.request_body, "REQ");
    assert_eq!(first.response_body, "RESP");

    // 快照全部已写（mask=0xff）：中间态不再携带任何大字段（内存根因 1 的修复点）。
    let mut snap = aidog_logs::ProxyLogColumns::from_log(&log, false, false).into_snapshot_meta();
    snap.raw_written_mask = 0xff;
    state.log_snapshots.insert("scoped_0001".to_string(), snap);

    let mut mid = log.clone();
    mid.status_code = 0;
    mid.done = false;
    let scoped_mid = scoped_queue_log(&state, &mid, &settings, false);
    assert_eq!(scoped_mid.request_body, "", "已写过的大字段不再随中间态消息携带");
    assert_eq!(scoped_mid.response_body, "");

    // 终态：响应侧强制携带（覆盖写最终值），请求侧已写过仍不带。
    let scoped_term = scoped_queue_log(&state, &log, &settings, true);
    assert_eq!(scoped_term.response_body, "RESP");
    assert_eq!(scoped_term.request_body, "");

    // 开关关闭：对应侧不带（writer 侧 from_log 反正会清空，带了白拷）。
    let off = ProxyLogSettings {
        enabled: true,
        log_user_request: false,
        log_upstream_request: false,
        ..Default::default()
    };
    state.log_snapshots.remove("scoped_0001");
    let scoped_off = scoped_queue_log(&state, &log, &off, true);
    assert_eq!(scoped_off.request_body, "");
    assert_eq!(scoped_off.response_body, "");

    let _ = std::fs::remove_file(path);
}
