#![cfg(test)]
use super::*;
use super::test_support::*;

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



    /// available_models 只含实际有记录的模型（actual_model 优先），不含未请求的。
    /// 防回归：前端模型筛选项曾派生自配置列表（platform.available_models ∪ group mappings），
    /// 导致下拉列出从未请求过的模型。
    #[tokio::test]
    async fn stats_available_models_only_recorded() {
        let db = test_db().await;
        let now = chrono::Utc::now().timestamp_millis();
        let mut lg1 = sample_log("m1", "g1", now);
        lg1.model = "claude-sonnet-4".into();
        lg1.actual_model = "glm-4-plus".into();
        insert_proxy_log_columns(&db, ProxyLogColumns::from_log(&lg1, false, false)).await.unwrap();
        let mut lg2 = sample_log("m2", "g1", now);
        lg2.model = "gpt-4o".into();
        lg2.actual_model = String::new(); // 回退到 model
        insert_proxy_log_columns(&db, ProxyLogColumns::from_log(&lg2, false, false)).await.unwrap();
        rebuild_stats_agg_from_logs(&db).await.unwrap();

        let q = StatsQuery { start: None, end: None, granularity: None, group_by: None, filter_group: None, filter_model: None, filter_platform: None };
        let s = query_stats(&db, &q).await.expect("query_stats");
        // actual_model 优先 → glm-4-plus；actual_model 空 → 回退 gpt-4o
        assert!(s.available_models.contains(&"glm-4-plus".to_string()), "missing glm-4-plus: {:?}", s.available_models);
        assert!(s.available_models.contains(&"gpt-4o".to_string()), "missing gpt-4o: {:?}", s.available_models);
        // 未请求过的模型不应出现
        assert!(!s.available_models.iter().any(|m| m == "claude-sonnet-4"), "requested model leaked: {:?}", s.available_models);
        assert!(!s.available_models.iter().any(|m| m == "never-used-model"), "unrecorded model leaked: {:?}", s.available_models);

        // filter_model 不应收缩 available_models（否则选中后下拉自缩）
        let q2 = StatsQuery { start: None, end: None, granularity: None, group_by: None, filter_group: None, filter_model: Some("glm-4-plus".into()), filter_platform: None };
        let s2 = query_stats(&db, &q2).await.expect("query_stats filtered");
        assert!(s2.available_models.contains(&"gpt-4o".to_string()), "filter_model shrank available_models: {:?}", s2.available_models);
    }



    /// 分钟 / 5 分钟分桶：合成同一小时内不同分钟的日志，断言分桶宽度正确。
    /// minute → 每分钟一桶；5min → floor 到 5 分钟边界一桶；hourly → 全部归一桶。
    #[tokio::test]
    async fn stats_minute_and_5min_buckets() {
        let db = test_db().await;
        // 固定基准：2026-06-16 10:00:00 UTC（毫秒）
        let base = chrono::DateTime::parse_from_rfc3339("2026-06-16T10:00:00Z")
            .unwrap()
            .timestamp_millis();
        // 6 条日志，分布在 10:00 / 10:01 / 10:03 / 10:06 / 10:12 / 10:14
        let offsets_min = [0i64, 1, 3, 6, 12, 14];
        for (i, m) in offsets_min.iter().enumerate() {
            let ts = base + m * 60_000;
            let lg = sample_log(&format!("b{i}"), "g1", ts);
            insert_proxy_log_columns(&db, ProxyLogColumns::from_log(&lg, false, false))
                .await
                .unwrap();
        }
        let start = base - 60_000;
        let end = base + 20 * 60_000;
        // hourly 读聚合表，须重建（minute/5min 仍读 proxy_log，不受影响）。
        rebuild_stats_agg_from_logs(&db).await.unwrap();

        // minute：6 个不同分钟 → 6 桶
        let q_min = StatsQuery { start: Some(start), end: Some(end), granularity: Some("minute".into()), group_by: None, filter_group: None, filter_model: None, filter_platform: None };
        let r_min = query_stats(&db, &q_min).await.expect("minute stats");
        assert_eq!(r_min.buckets.len(), 6, "minute 应 6 桶: {:?}", r_min.buckets.iter().map(|b| &b.time_bucket).collect::<Vec<_>>());

        // 5min：分钟落入 [00-04]→2(00,01,03), [05-09]→1(06), [10-14]→2(12,14) → 3 桶
        let q_5 = StatsQuery { start: Some(start), end: Some(end), granularity: Some("5min".into()), group_by: None, filter_group: None, filter_model: None, filter_platform: None };
        let r_5 = query_stats(&db, &q_5).await.expect("5min stats");
        assert_eq!(r_5.buckets.len(), 3, "5min 应 3 桶: {:?}", r_5.buckets.iter().map(|b| &b.time_bucket).collect::<Vec<_>>());
        // 第一桶（10:00）应聚合 3 条请求
        let first = &r_5.buckets[0];
        assert_eq!(first.total_requests, 3, "5min 首桶应聚 3 条: {first:?}");

        // hourly：全在 10 点 → 1 桶
        let q_h = StatsQuery { start: Some(start), end: Some(end), granularity: Some("hourly".into()), group_by: None, filter_group: None, filter_model: None, filter_platform: None };
        let r_h = query_stats(&db, &q_h).await.expect("hourly stats");
        assert_eq!(r_h.buckets.len(), 1, "hourly 应 1 桶: {:?}", r_h.buckets.iter().map(|b| &b.time_bucket).collect::<Vec<_>>());
        assert_eq!(r_h.buckets[0].total_requests, 6, "hourly 桶应聚 6 条");
    }
