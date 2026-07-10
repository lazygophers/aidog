#![cfg(test)]
use super::*;
use super::test_support::*;

    // ─── parse/serialize helper tests ─────────────────────────────────────────

    #[test]
    fn parse_models_valid_json() {
        let json = r#"{"default":"claude-sonnet-4","sonnet":"claude-sonnet-4"}"#;
        let m = parse_models(json);
        assert_eq!(m.default.as_deref(), Some("claude-sonnet-4"));
        assert_eq!(m.sonnet.as_deref(), Some("claude-sonnet-4"));
    }

    #[test]
    fn parse_models_corrupt_json_returns_default() {
        let m = parse_models("{not valid json");
        // default() = all None
        assert!(m.default.is_none(), "corrupt JSON should fall back to empty default");
    }

    #[test]
    fn serialize_models_roundtrip() {
        let m = PlatformModels {
            default: Some("gpt-4o".to_string()),
            gpt: Some("gpt-4o".to_string()),
            ..Default::default()
        };
        let json = serialize_models(&m);
        let back = parse_models(&json);
        assert_eq!(back.default.as_deref(), Some("gpt-4o"));
        assert_eq!(back.gpt.as_deref(), Some("gpt-4o"));
    }

    #[test]
    fn parse_available_models_valid() {
        let json = r#"["gpt-4o","claude-sonnet-4"]"#;
        let v = parse_available_models(json);
        assert_eq!(v, vec!["gpt-4o", "claude-sonnet-4"]);
    }

    #[test]
    fn parse_available_models_corrupt_returns_empty() {
        let v = parse_available_models("[not valid");
        assert!(v.is_empty());
    }

    #[test]
    fn serialize_available_models_roundtrip() {
        let models = vec!["m1".to_string(), "m2".to_string()];
        let json = serialize_available_models(&models);
        let back = parse_available_models(&json);
        assert_eq!(back, models);
    }

    #[test]
    fn parse_endpoints_valid() {
        let json = r#"[{"protocol":"anthropic","base_url":"https://api.anthropic.com/v1"}]"#;
        let eps = parse_endpoints(json);
        assert_eq!(eps.len(), 1);
        assert_eq!(eps[0].base_url, "https://api.anthropic.com/v1");
    }

    #[test]
    fn parse_endpoints_corrupt_returns_empty() {
        let eps = parse_endpoints("[{bad");
        assert!(eps.is_empty());
    }

    #[test]
    fn serialize_endpoints_roundtrip() {
        let eps = vec![crate::gateway::models::PlatformEndpoint {
            protocol: crate::gateway::models::Protocol::OpenAI,
            base_url: "https://api.openai.com/v1".to_string(),
            client_type: "default".to_string(),
            coding_plan: false,
        }];
        let json = serialize_endpoints(&eps);
        let back = parse_endpoints(&json);
        assert_eq!(back.len(), 1);
        assert_eq!(back[0].base_url, "https://api.openai.com/v1");
    }

    #[test]
    fn retention_cutoff_zero_returns_none() {
        assert!(retention_cutoff(0).is_none());
    }

    #[test]
    fn retention_cutoff_nonzero_returns_some_past_timestamp() {
        let cutoff = retention_cutoff(7).unwrap();
        let now = chrono::Utc::now().timestamp_millis();
        assert!(cutoff < now, "cutoff should be in the past");
        let days_ago = now - 7 * 24 * 3600 * 1000;
        assert!((cutoff - days_ago).abs() < 60_000, "cutoff should be ~7 days ago");
    }

    // ─────────────────────────────────────────────────────────────────────────

    /// call_traced 进闭包时设上下文（req + caller 可读），闭包结束后清空；
    /// 显式 req 原样使用，caller 被捕获。
    #[tokio::test]
    async fn call_traced_sets_and_clears_thread_local() {
        let db = test_db().await;
        // 闭包内（DB 线程）观测上下文：req = 显式传入的 request_id，caller 非空。
        let observed: (Option<String>, bool) = db
            .call_traced(Some("req-xyz"), std::panic::Location::caller(), |_conn| {
                Ok(CURRENT_DB_CTX.with(|c| {
                    let c = c.borrow();
                    (c.req.clone(), c.caller.is_some())
                }))
            })
            .await
            .expect("call_traced ok");
        assert_eq!(observed.0.as_deref(), Some("req-xyz"));
        assert!(observed.1, "caller location should be captured");

        // 同一 DB 线程上下次操作：caller 已被 guard 清空再设新值（不串味）。
        let after_caller: bool = db
            .call_traced(Some("req-2"), std::panic::Location::caller(), |_conn| {
                Ok(CURRENT_DB_CTX.with(|c| c.borrow().caller.is_some()))
            })
            .await
            .expect("call_traced ok");
        assert!(after_caller, "caller re-captured on next call");
    }



    /// 关键契约：req=None 时**绝不**留空 / 固定常量，而是当场用 new_trace_id() 兜底
    /// 生成真实唯一 id（6 位 [0-9a-z]）。无环境 span 时走兜底；不同次调用 id 不同。
    #[tokio::test]
    async fn call_traced_none_req_falls_back_to_generated_unique_id() {
        let db = test_db().await;

        async fn observe_req(db: &Db) -> String {
            db.call_traced(None, std::panic::Location::caller(), |_conn| {
                Ok(CURRENT_DB_CTX.with(|c| c.borrow().req.clone()))
            })
            .await
            .expect("call_traced ok")
            .expect("req must be set, never None")
        }

        let id1 = observe_req(&db).await;
        let id2 = observe_req(&db).await;

        // 禁固定常量：不得是历史写死值。
        assert_ne!(id1, "bg");
        assert_ne!(id1, "-");
        assert!(!id1.is_empty(), "req must never be empty");
        // 兜底 id 形态：new_trace_id() = 6 位 [0-9a-z] (logging.rs gen_trace_id)。
        assert_eq!(id1.len(), 6, "fallback id is 6 [0-9a-z]: got {id1}");
        assert!(
            id1.chars().all(|ch| ch.is_ascii_digit() || ch.is_ascii_lowercase()),
            "fallback id is [0-9a-z]: {id1}"
        );
        // 真实唯一：两次兜底 id 不同（无环境 span 复用）。
        assert_ne!(id1, id2, "each fallback id must be unique");
    }



    /// req=None 但处于带 trace_id 的活跃 span 内 → 环境捕获该 span 的 id，
    /// 同一 span 内多次调用共享同一 id（后台轮询周期内所有 SQL 同 id 的依据）。
    #[tokio::test]
    async fn call_traced_captures_env_span_trace_id() {
        // 安装一次性的 TraceIdLayer（测试进程可能已有 global subscriber，用 with_default 作用域）。
        use tracing_subscriber::layer::SubscriberExt;
        let subscriber = tracing_subscriber::registry().with(crate::logging::trace_id_layer_for_test());
        let _guard = tracing::subscriber::set_default(subscriber);

        let db = test_db().await;
        let tid = crate::logging::new_trace_id();
        let span = tracing::info_span!("poll_cycle", trace_id = %tid);

        async fn observe_req(db: &Db) -> String {
            db.call_traced(None, std::panic::Location::caller(), |_conn| {
                Ok(CURRENT_DB_CTX.with(|c| c.borrow().req.clone()))
            })
            .await
            .expect("ok")
            .expect("req set")
        }

        // 关键：call_traced 在调用方线程读 current_trace_id，必须在 span 进入态时同步调用。
        let _e = span.enter();
        let a = observe_req(&db).await;
        let b = observe_req(&db).await;
        drop(_e);

        assert_eq!(a, tid, "env span trace_id captured as req");
        assert_eq!(a, b, "same span -> same id across calls");
    }



    // ─── ConnectionClosed 自动重连重试（fix 07-08-route-db-connection-closed）────────

    /// 抑制后台线程 panic 的 stderr 噪音：测试期间临时换成 no-op panic hook。
    /// 不影响 panic 行为本身（仍会 unwind 后台线程），只换输出。
    struct QuietPanicHook;
    type PanicHook = Box<dyn Fn(&std::panic::PanicHookInfo<'_>) + Send + Sync>;
    impl QuietPanicHook {
        fn install() -> impl Drop {
            let prev = std::panic::take_hook();
            std::panic::set_hook(Box::new(|_| {}));
            struct RestoreHook(Option<PanicHook>);
            impl Drop for RestoreHook {
                fn drop(&mut self) {
                    if let Some(h) = self.0.take() {
                        std::panic::set_hook(h);
                    }
                }
            }
            RestoreHook(Some(prev))
        }
    }

    /// 写连接 panic 致后台线程退出 → `call_traced` 应自动重开 + 重试一次。
    /// 历史症：tokio_rusqlite 后台线程 panic 后 channel 永久关闭，所有 `.call()` 返
    /// `ConnectionClosed`；代理 route 路径首调失败直传 → handler 落 400 给用户。
    /// 本测试杀掉写线程，验证 call_traced 不再透传错误。
    #[tokio::test]
    async fn call_traced_reopens_after_panic_kills_thread() {
        // 文件库（非 :memory:），重开后能读到已建表 / 已写数据。
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("test_reopen.db");
        let path_str = path.to_string_lossy().to_string();
        // 清理可能的前次残留。
        for suffix in ["", "-wal", "-shm"] {
            let _ = std::fs::remove_file(format!("{path_str}{suffix}"));
        }

        let db = Db::new(&path_str).await.expect("open db");
        db.init_tables().await.expect("init tables");

        // 基线：正常查 1。
        let n: i64 = db
            .call_traced(None, std::panic::Location::caller(), |conn| {
                Ok(conn.query_row("SELECT 1", [], |r| r.get(0))?)
            })
            .await
            .expect("baseline call_traced ok");
        assert_eq!(n, 1);

        // 杀掉写线程：在闭包里 panic 让 tokio_rusqlite 后台线程退出。
        let _hook = QuietPanicHook::install();
        let r_kill: tokio_rusqlite::Result<()> =
            db.write_conn().call(|_c| panic!("test: kill write thread")).await;
        drop(_hook);
        // panic 后该连接句柄返 ConnectionClosed。
        assert!(
            matches!(r_kill, Err(tokio_rusqlite::Error::ConnectionClosed)),
            "expected ConnectionClosed after panic, got {r_kill:?}"
        );

        // 直接调用方（write_conn）仍拿到死连接 — 验证未自动恢复：
        let r_dead: tokio_rusqlite::Result<i64> = db
            .write_conn()
            .call(|conn| Ok(conn.query_row("SELECT 1", [], |r| r.get(0))?))
            .await;
        assert!(
            matches!(r_dead, Err(tokio_rusqlite::Error::ConnectionClosed)),
            "write_conn() should still be dead until call_traced heals it"
        );

        // 关键断言：call_traced 自动重开 + 重试，不返 ConnectionClosed。
        let n: i64 = db
            .call_traced(None, std::panic::Location::caller(), |conn| {
                Ok(conn.query_row("SELECT 1", [], |r| r.get(0))?)
            })
            .await
            .expect("call_traced should auto-reopen and retry");
        assert_eq!(n, 1);

        // 槽位已替换：write_conn() 现在也指向新连接。
        let n: i64 = db
            .write_conn()
            .call(|conn| Ok(conn.query_row("SELECT 1", [], |r| r.get(0))?))
            .await
            .expect("write_conn() should now work after call_traced swapped the slot");
        assert_eq!(n, 1);

        for suffix in ["", "-wal", "-shm"] {
            let _ = std::fs::remove_file(format!("{path_str}{suffix}"));
        }
    }

    /// 读池某槽位 panic 致死 → `call_read_traced` 应重试下一条（不同槽位）。
    /// 关键契约：route 路径首调 `get_group_platforms` 经 `call_read_traced`，
    /// 单死槽位不应让整次 route 失败。
    ///
    /// 测试机制：`kill_next_read_slot` 杀掉 cursor 当前指向的读连接（仅文件库有独立读池）。
    /// 之后 N 次 `call_read_traced` 中 cursor 必然轮回到死槽位，断言全部透明成功（重试到下一条）。
    #[tokio::test]
    async fn call_read_traced_retries_on_dead_pool_slot() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("test_read_retry.db");
        let path_str = path.to_string_lossy().to_string();
        for suffix in ["", "-wal", "-shm"] {
            let _ = std::fs::remove_file(format!("{path_str}{suffix}"));
        }

        let db = Db::new(&path_str).await.expect("open file db");
        db.init_tables().await.expect("init tables");

        // 基线：读路径正常。
        let n: i64 = db
            .call_read_traced(None, std::panic::Location::caller(), |conn| {
                Ok(conn.query_row("SELECT 1", [], |r| r.get(0))?)
            })
            .await
            .expect("baseline read ok");
        assert_eq!(n, 1);

        // 杀掉下一条读池连接（panic 后台线程）。
        let _hook = QuietPanicHook::install();
        db.kill_next_read_slot().await;
        drop(_hook);

        // 轮询 N 次（N >> READ_POOL_SIZE，必多次命中死槽位）。每次都应透明成功（重试到下一条）。
        for _ in 0..(READ_POOL_SIZE * 4) {
            let n: i64 = db
                .call_read_traced(None, std::panic::Location::caller(), |conn| {
                    Ok(conn.query_row("SELECT 1", [], |r| r.get(0))?)
                })
                .await
                .expect("call_read_traced should retry on dead slot, never surface ConnectionClosed");
            assert_eq!(n, 1);
        }

        for suffix in ["", "-wal", "-shm"] {
            let _ = std::fs::remove_file(format!("{path_str}{suffix}"));
        }
    }

    /// `:memory:` 库禁用重连（重开会读到空库）。杀写线程后 call_traced 应直接透传
    /// ConnectionClosed 而非尝试重开。生产代码用文件库，不受影响。
    #[tokio::test]
    async fn call_traced_skips_reopen_for_memory_db() {
        let db = test_db().await; // :memory:

        let _hook = QuietPanicHook::install();
        let _: tokio_rusqlite::Result<()> =
            db.write_conn().call(|_c| panic!("test: kill memory write thread")).await;
        drop(_hook);

        let r: tokio_rusqlite::Result<i64> = db
            .call_traced(None, std::panic::Location::caller(), |conn| {
                Ok(conn.query_row("SELECT 1", [], |r| r.get(0))?)
            })
            .await;
        assert!(
            matches!(r, Err(tokio_rusqlite::Error::ConnectionClosed)),
            "memory db should NOT auto-reopen (would read empty db), got {r:?}"
        );
    }
