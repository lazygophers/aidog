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
            client_type: crate::gateway::models::ClientType::default(),
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
