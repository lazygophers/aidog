#![cfg(test)]
use super::*;
use super::test_support::*;

    /// fmt_caller 取路径末段 + 行号，紧凑显示。
    #[test]
    fn fmt_caller_uses_basename_and_line() {
        let loc = std::panic::Location::caller(); // 本测试函数所在位置
        let out = fmt_caller(loc);
        // 形如 "<basename>.rs:<line>"：仅文件名末段（无路径分隔符）+ 冒号 + 数字行号。
        assert!(!out.contains('/') && !out.contains('\\'), "应只含文件名末段, got {out}");
        assert!(out.contains(".rs:"), "got {out}");
        assert!(out.rsplit(':').next().unwrap().parse::<u32>().is_ok(), "got {out}");
    }



    /// 空上下文（无 call_traced 设置）→ profile 回调取值应回退为 "-"。
    #[test]
    fn empty_ctx_renders_dash() {
        CURRENT_DB_CTX.with(|c| *c.borrow_mut() = DbCallCtx::default());
        let (req, caller) = CURRENT_DB_CTX.with(|c| {
            let c = c.borrow();
            (
                c.req.clone().unwrap_or_else(|| "-".to_string()),
                c.caller.map(fmt_caller).unwrap_or_else(|| "-".to_string()),
            )
        });
        assert_eq!(req, "-");
        assert_eq!(caller, "-");
    }



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
    /// 生成真实唯一 id（8-hex）。无环境 span 时走兜底；不同次调用 id 不同。
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
        // 兜底 id 形态：new_trace_id() = 8 位小写 hex。
        assert_eq!(id1.len(), 8, "fallback id is 8-hex: got {id1}");
        assert!(id1.chars().all(|ch| ch.is_ascii_hexdigit()), "fallback id is hex: {id1}");
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



    #[test]
    fn apply_context_tier_selects_long_tier() {
        // OpenAI gpt-5.5: short in=5e-6/out=3e-5/cache=5e-7, long@272000 in=1e-5/out=4.5e-5/cache=1e-6
        let pd = serde_json::json!({
            "input_cost_per_token": 5e-6,
            "output_cost_per_token": 3e-5,
            "cache_read_input_token_cost": 5e-7,
            "context_tiers": [{
                "min_tokens": 272000,
                "input_cost_per_token": 1e-5,
                "output_cost_per_token": 4.5e-5,
                "cache_read_input_token_cost": 1e-6
            }]
        });
        let base = crate::gateway::models::ResolvedPrice {
            input_cost_per_token: 5e-6,
            output_cost_per_token: 3e-5,
            cache_read_input_token_cost: 5e-7,
            source: "top_level".to_string(),
        };
        // 短档: input < 272000 → base 不变 (无 +tier 后缀)
        let short = apply_context_tier(base.clone(), &pd, 100_000);
        assert_eq!(short.input_cost_per_token, 5e-6);
        assert_eq!(short.output_cost_per_token, 3e-5);
        assert_eq!(short.source, "top_level");
        // 长档: input >= 272000 → tier 覆盖
        let long = apply_context_tier(base.clone(), &pd, 300_000);
        assert_eq!(long.input_cost_per_token, 1e-5);
        assert_eq!(long.output_cost_per_token, 4.5e-5);
        assert_eq!(long.cache_read_input_token_cost, 1e-6);
        assert_eq!(long.source, "top_level+tier");
        // 边界: 恰好等于阈值 → long
        let edge = apply_context_tier(base.clone(), &pd, 272_000);
        assert_eq!(edge.input_cost_per_token, 1e-5);
    }



    #[test]
    fn apply_context_tier_no_tier_passthrough() {
        // 无 context_tiers 字段 → base 不变 (向后兼容旧 price_data)
        let pd = serde_json::json!({"input_cost_per_token": 2.5e-6});
        let base = crate::gateway::models::ResolvedPrice {
            input_cost_per_token: 2.5e-6,
            output_cost_per_token: 1.5e-5,
            cache_read_input_token_cost: 2.5e-7,
            source: "top_level".to_string(),
        };
        let r = apply_context_tier(base.clone(), &pd, 999_999_999);
        assert_eq!(r.input_cost_per_token, 2.5e-6);
        assert_eq!(r.source, "top_level");
        // tiers 为空数组 → 同样不变
        let pd2 = serde_json::json!({"context_tiers": []});
        let r2 = apply_context_tier(base, &pd2, 999_999_999);
        assert_eq!(r2.source, "top_level");
    }



    #[test]
    fn apply_context_tier_partial_override() {
        // 长档仅覆盖部分字段 (如某些模型长档无 cache 价 → 继承 base cache)
        let pd = serde_json::json!({
            "input_cost_per_token": 3e-5,
            "output_cost_per_token": 1.8e-4,
            "cache_read_input_token_cost": 0.0,
            "context_tiers": [{
                "min_tokens": 272000,
                "input_cost_per_token": 6e-5,
                "output_cost_per_token": 2.7e-4
                // cache_read_input_token_cost 缺失 → 继承 base
            }]
        });
        let base = crate::gateway::models::ResolvedPrice {
            input_cost_per_token: 3e-5,
            output_cost_per_token: 1.8e-4,
            cache_read_input_token_cost: 0.0,
            source: "top_level".to_string(),
        };
        let r = apply_context_tier(base, &pd, 300_000);
        assert_eq!(r.input_cost_per_token, 6e-5);
        assert_eq!(r.output_cost_per_token, 2.7e-4);
        assert_eq!(r.cache_read_input_token_cost, 0.0); // 继承 base
    }



    #[tokio::test]
    async fn query_stats_platform_dim_and_filter() {
        let db = test_db().await;
        let p = create_platform(&db, sample_platform("P1")).await.unwrap();
        let now = chrono::Utc::now().timestamp_millis();
        let mut lg = sample_log("l1", "g1", now);
        lg.platform_id = p.id;
        insert_proxy_log_columns(&db, ProxyLogColumns::from_log(&lg, false, false)).await.unwrap();
        let q = StatsQuery { start: None, end: None, granularity: Some("daily".into()), group_by: Some("platform".into()), filter_group: None, filter_model: None, filter_platform: None };
        let r = query_stats(&db, &q).await;
        println!("NO-FILTER platform dim: {:?}", r.as_ref().err());
        assert!(r.is_ok(), "no-filter platform dim failed: {:?}", r.err());
        let q2 = StatsQuery { start: None, end: None, granularity: Some("daily".into()), group_by: Some("platform".into()), filter_group: None, filter_model: None, filter_platform: Some(p.id.to_string()) };
        let r2 = query_stats(&db, &q2).await;
        println!("PLATFORM-FILTER: {:?}", r2.as_ref().err());
        assert!(r2.is_ok(), "platform filter failed: {:?}", r2.err());
        let res = r2.unwrap();
        println!("overview total_requests = {}", res.overview.total_requests);
        println!("dim entries = {}", res.dimension_data.len());
    }



    /// 批量 `query_stats_batch` 必须逐项等于逐卡 `query_stats`（同 query 同结果，顺序对齐）。
    /// 覆盖浮窗各卡参数组合：overall/platform/group × today(hourly)/7d(daily)。
    #[tokio::test]
    async fn query_stats_batch_matches_per_query() {
        let db = test_db().await;
        let p = create_platform(&db, sample_platform("P1")).await.unwrap();
        let now = chrono::Utc::now().timestamp_millis();

        // 两条日志：一条挂 P1/g1，一条挂 g2，覆盖 group/platform 过滤分支。
        let mut a = sample_log("a", "g1", now);
        a.platform_id = p.id;
        a.status_code = 200;
        a.input_tokens = 10;
        a.output_tokens = 20;
        a.cache_tokens = 5;
        a.est_cost = 0.01;
        insert_proxy_log_columns(&db, ProxyLogColumns::from_log(&a, false, false)).await.unwrap();
        let mut b = sample_log("b", "g2", now);
        b.platform_id = p.id;
        b.status_code = 500;
        b.input_tokens = 3;
        b.output_tokens = 0;
        b.est_cost = 0.0;
        insert_proxy_log_columns(&db, ProxyLogColumns::from_log(&b, false, false)).await.unwrap();

        let day = 86_400_000i64;
        let queries = vec![
            // overall 7d daily
            StatsQuery { start: Some(now - 7 * day), end: Some(now), granularity: Some("daily".into()), group_by: None, filter_group: None, filter_model: None, filter_platform: None },
            // overall today hourly
            StatsQuery { start: Some(now - day), end: Some(now), granularity: Some("hourly".into()), group_by: None, filter_group: None, filter_model: None, filter_platform: None },
            // platform 7d daily
            StatsQuery { start: Some(now - 7 * day), end: Some(now), granularity: Some("daily".into()), group_by: None, filter_group: None, filter_model: None, filter_platform: Some(p.id.to_string()) },
            // group today hourly
            StatsQuery { start: Some(now - day), end: Some(now), granularity: Some("hourly".into()), group_by: None, filter_group: Some("g1".into()), filter_model: None, filter_platform: None },
        ];

        let batch = query_stats_batch(&db, queries.clone()).await.expect("batch");
        assert_eq!(batch.len(), queries.len(), "batch 长度须等于 query 数");

        for (i, q) in queries.iter().enumerate() {
            let single = query_stats(&db, q).await.expect("single");
            let bz = &batch[i];
            // overview 全字段对账
            assert_eq!(bz.overview.total_requests, single.overview.total_requests, "q{i} total_requests");
            assert_eq!(bz.overview.total_input_tokens, single.overview.total_input_tokens, "q{i} input");
            assert_eq!(bz.overview.total_output_tokens, single.overview.total_output_tokens, "q{i} output");
            assert_eq!(bz.overview.total_cache_tokens, single.overview.total_cache_tokens, "q{i} cache");
            assert!((bz.overview.total_cost - single.overview.total_cost).abs() < 1e-12, "q{i} cost");
            assert!((bz.overview.success_rate - single.overview.success_rate).abs() < 1e-9, "q{i} success_rate");
            // buckets：桶数与逐桶 cost/req 一致（曲线卡口径）
            assert_eq!(bz.buckets.len(), single.buckets.len(), "q{i} bucket count");
            for (j, (bb, sb)) in bz.buckets.iter().zip(single.buckets.iter()).enumerate() {
                assert_eq!(bb.time_bucket, sb.time_bucket, "q{i} bucket{j} time");
                assert_eq!(bb.total_requests, sb.total_requests, "q{i} bucket{j} req");
                assert!((bb.total_cost - sb.total_cost).abs() < 1e-12, "q{i} bucket{j} cost");
            }
        }
    }



    /// `bucket_time_expr` 必须带 `'localtime'`：分桶按本地时区切分，与 today_stats 同语义。
    /// 缺 localtime 时跨日/小时桶在非 UTC 时区错位（曲线 bug）。
    #[test]
    fn bucket_time_expr_uses_localtime() {
        for g in [None, Some("daily"), Some("hourly"), Some("minute"), Some("5min")] {
            let expr = bucket_time_expr(g);
            assert!(expr.contains("'localtime'"), "粒度 {g:?} 的分桶表达式必须含 'localtime'：{expr}");
            assert!(expr.contains("'unixepoch'"), "粒度 {g:?} 须先 'unixepoch' 再 'localtime'：{expr}");
        }
    }



    /// localtime 分桶按本地日界切分：构造跨本地午夜的两条日志，daily 桶须落不同日期键。
    /// 用 SQLite 自身求出「本地午夜 ±1h」的 epoch ms，避免硬编码时区。
    #[tokio::test]
    async fn bucket_daily_splits_on_local_midnight() {
        let db = test_db().await;
        // 本地午夜的 epoch 秒：strftime 本地日期 00:00 转回 unixepoch。
        let local_midnight_ms: i64 = db.call_traced(None, std::panic::Location::caller(), |conn| {
            let secs: i64 = conn.query_row(
                "SELECT CAST(strftime('%s', strftime('%Y-%m-%d 00:00:00', 'now', 'localtime'), 'utc') AS INTEGER)",
                [],
                |r| r.get(0),
            )?;
            Ok(secs * 1000)
        }).await.unwrap();

        // 午夜前 1 小时 + 午夜后 1 小时 → 本地相邻两天。
        let before = local_midnight_ms - 3_600_000;
        let after = local_midnight_ms + 3_600_000;
        let mut a = sample_log("before", "g1", before);
        a.est_cost = 0.01;
        insert_proxy_log_columns(&db, ProxyLogColumns::from_log(&a, false, false)).await.unwrap();
        let mut b = sample_log("after", "g1", after);
        b.est_cost = 0.02;
        insert_proxy_log_columns(&db, ProxyLogColumns::from_log(&b, false, false)).await.unwrap();

        // 测试直插 proxy_log（绕过 proxy upsert_log 的聚合写）；daily 粒度读聚合表，须先重建。
        rebuild_stats_agg_from_logs(&db).await.unwrap();
        let q = StatsQuery {
            start: Some(before - 3_600_000),
            end: Some(after + 3_600_000),
            granularity: Some("daily".into()),
            group_by: None, filter_group: None, filter_model: None, filter_platform: None,
        };
        let res = query_stats(&db, &q).await.unwrap();
        // 本地午夜两侧 → 两个不同的本地日桶。
        assert_eq!(res.buckets.len(), 2, "跨本地午夜须分到 2 个 daily 桶，得到 {:?}", res.buckets.iter().map(|x| &x.time_bucket).collect::<Vec<_>>());
        assert_ne!(res.buckets[0].time_bucket, res.buckets[1].time_bucket);
    }
