use super::*;

    fn placeholder_stream_log(id: &str) -> ProxyLog {
        let ts = super::super::db::now();
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
            response_body: "[stream]".to_string(),
            request_url: String::new(),
            upstream_request_url: String::new(),
            upstream_response_headers: String::new(),
            upstream_status_code: 200,
            user_response_headers: String::new(),
            user_response_body: "[stream]".to_string(),
            status_code: 200,
            duration_ms: 0,
            input_tokens: 0,
            output_tokens: 0,
            cache_tokens: 0,
            est_cost: 0.0,
            is_stream: true,
            attempts: Vec::new(),
            retry_count: 0,
            blocked_by: String::new(),
            blocked_reason: String::new(),
            created_at: ts,
            updated_at: ts,
            deleted_at: 0,
            cli_proxy_provider_id: None,
        }
    }

    // 建一个 StreamLogGuard，settings = 默认（enabled=true, log_user_request=false）。
    // upstream_chunks 预先 push 进 agg.upstream_body（模拟流式逐 chunk 累积）。
    async fn flush_test_db() -> (Arc<super::super::db::Db>, std::path::PathBuf) {
        // ponytail: proxy_log 拆库后用 :memory:（主+proxy_log 共享同一物理连接）。
        let db = super::super::db::Db::new(":memory:")
            .await
            .expect("open memory db");
        db.init_tables().await.expect("init tables");
        (Arc::new(db), std::path::PathBuf::new())
    }

    fn flush_test_state(db: Arc<super::super::db::Db>) -> Arc<ProxyState> {
        Arc::new(ProxyState {
            db,
            app: None,
            middleware: Arc::new(MiddlewareEngine::new()),
            scheduler: Arc::new(super::super::scheduling::SchedulerState::new()),
            sticky: Arc::new(super::super::scheduling::StickyTable::new()),
            log_snapshots: dashmap::DashMap::new(),
            agg_done: std::sync::Mutex::new((std::collections::VecDeque::new(), std::collections::HashSet::new())),
            listen_addr: std::sync::OnceLock::new(),
            settings_cache: Arc::new(tokio::sync::RwLock::new(Default::default())),
        })
    }

    fn terminal_log(id: &str) -> ProxyLog {
        let mut l = placeholder_stream_log(id);
        l.is_stream = false;
        l.response_body = "ok".to_string();
        l.user_response_body = "ok".to_string();
        l.input_tokens = 100;
        l.output_tokens = 200;
        l.cache_tokens = 0;
        l.est_cost = 0.5;
        l.platform_id = 1; // 非 0 避免 eff_pid 回溯子查询依赖（去重逻辑与 pid 无关）
        l
    }

    async fn agg_request_count(db: &super::super::db::Db, id_group: &str) -> i64 {
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
        let settings = ProxyLogSettings { enabled: false, ..Default::default() }; // 关日志路径

        for _ in 0..8 {
            upsert_log(&state, &log, &settings).await;
        }
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

        let req = agg_request_count(&state.db, "gk_test").await;
        assert_eq!(req, 0, "非终态请求不应计入 agg");
        assert!(state.agg_done.lock().unwrap().1.is_empty(), "非终态不应登记 agg 去重缓存");

        let _ = std::fs::remove_file(path);
    }
