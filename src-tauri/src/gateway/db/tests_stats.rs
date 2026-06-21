#![cfg(test)]
use super::*;
use super::test_support::*;

    /// 批量 `platform_usage_stats_all` 结果必须逐平台等于单平台 `get_platform_usage_stats`，
    /// 含 platform_id=0 自动分组日志按 group_key → auto_from_platform 回溯归属源平台。
    #[tokio::test]
    async fn platform_usage_stats_all_matches_per_platform() {
        let db = test_db().await;
        let p1 = create_platform(&db, sample_platform("P1")).await.unwrap();
        let p2 = create_platform(&db, sample_platform("P2")).await.unwrap();
        // 自动分组：显示名 name=auto_p1，路由/归属键 group_key=gk_auto_p1（刻意 ≠ name，
        // 复刻真实 gk_<hex> 场景，防回归 join 误用 g.name），auto_from_platform=p1.id（十进制字符串）。
        let mut g = sample_group("auto_p1", vec![]);
        g.group_key = Some("gk_auto_p1".to_string());
        g.auto_from_platform = p1.id.to_string();
        create_group(&db, g).await.unwrap();

        let now = chrono::Utc::now().timestamp_millis();

        // P1 直挂日志（platform_id=p1）：2 条，1 成功 1 失败。
        let mut a1 = sample_log("a1", "g1", now);
        a1.platform_id = p1.id;
        a1.status_code = 200;
        a1.input_tokens = 10;
        a1.output_tokens = 20;
        a1.cache_tokens = 5;
        a1.est_cost = 0.01;
        insert_proxy_log_columns(&db, ProxyLogColumns::from_log(&a1, false, false)).await.unwrap();
        let mut a2 = sample_log("a2", "g1", now);
        a2.platform_id = p1.id;
        a2.status_code = 500;
        a2.input_tokens = 7;
        a2.output_tokens = 0;
        a2.est_cost = 0.0;
        insert_proxy_log_columns(&db, ProxyLogColumns::from_log(&a2, false, false)).await.unwrap();

        // platform_id=0 自动分组日志，group_key=gk_auto_p1（= group.group_key，≠ name）→ 回溯归 p1。
        let mut a0 = sample_log("a0", "gk_auto_p1", now);
        a0.platform_id = 0;
        a0.status_code = 200;
        a0.input_tokens = 100;
        a0.output_tokens = 200;
        a0.cache_tokens = 50;
        a0.est_cost = 0.05;
        insert_proxy_log_columns(&db, ProxyLogColumns::from_log(&a0, false, false)).await.unwrap();

        // P2 直挂日志：1 条成功。
        let mut b1 = sample_log("b1", "g2", now);
        b1.platform_id = p2.id;
        b1.status_code = 200;
        b1.input_tokens = 3;
        b1.output_tokens = 4;
        b1.est_cost = 0.002;
        insert_proxy_log_columns(&db, ProxyLogColumns::from_log(&b1, false, false)).await.unwrap();

        // 回溯不到的孤儿 platform_id=0 日志（无匹配 auto 分组）→ 不应归任何平台卡片。
        let mut orphan = sample_log("orphan", "no_such_group", now);
        orphan.platform_id = 0;
        orphan.status_code = 200;
        orphan.est_cost = 0.99;
        insert_proxy_log_columns(&db, ProxyLogColumns::from_log(&orphan, false, false)).await.unwrap();

        let batch = platform_usage_stats_all(&db).await.expect("batch");

        for pid in [p1.id, p2.id] {
            let single = get_platform_usage_stats(&db, pid).await.expect("single");
            let b = batch.get(&pid).unwrap_or_else(|| panic!("missing pid {pid} in batch"));
            assert_eq!(b.total_requests, single.total_requests, "pid {pid} total_requests");
            assert_eq!(b.success_count, single.success_count, "pid {pid} success_count");
            assert_eq!(b.total_input_tokens, single.total_input_tokens, "pid {pid} input");
            assert_eq!(b.total_output_tokens, single.total_output_tokens, "pid {pid} output");
            assert_eq!(b.total_cache_tokens, single.total_cache_tokens, "pid {pid} cache");
            assert!((b.total_cost - single.total_cost).abs() < 1e-9, "pid {pid} cost");
            assert!((b.cache_rate - single.cache_rate).abs() < 1e-9, "pid {pid} cache_rate");
            // recent_* 必须与单平台版一致（批量补齐近期健康度，供平台卡片健康点配色）。
            assert_eq!(b.recent_total, single.recent_total, "pid {pid} recent_total");
            assert_eq!(b.recent_failures, single.recent_failures, "pid {pid} recent_failures");
        }

        // p1 含回溯日志：total=3（a1+a2+a0），success=2，input=117。
        let p1b = batch.get(&p1.id).unwrap();
        assert_eq!(p1b.total_requests, 3, "p1 含 platform_id=0 回溯归属");
        assert_eq!(p1b.success_count, 2);
        assert_eq!(p1b.total_input_tokens, 117);
        // p1 近期 3 条（a1 200 / a2 500 / a0 200）：recent_total=3，recent_failures=1（a2）。
        // 非恒 0 → 平台卡片健康点恢复（healthStatus(3,1)=warning）。
        assert_eq!(p1b.recent_total, 3, "p1 recent_total 应非 0（健康点回归）");
        assert_eq!(p1b.recent_failures, 1, "p1 recent_failures（a2 500）");

        // 孤儿日志（est_cost=0.99）不得归任何平台 → 平台合计 cost 不含 0.99。
        let summed: f64 = batch.values().map(|s| s.total_cost).sum();
        assert!(summed < 0.9, "orphan platform_id=0 leaked into platform stats: {summed}");
    }



    /// cache_rate 必须 ≤100%：cache_tokens=9900（命中缓存）+ input_tokens=100（新输入），
    /// 旧公式 cache/input=9900% 错误；新公式 cache/(input+cache)≈99%。
    #[tokio::test]
    async fn cache_rate_never_exceeds_100() {
        let db = test_db().await;
        let now = chrono::Utc::now().timestamp_millis();
        let mut lg = sample_log("c1", "g1", now);
        lg.input_tokens = 100;
        lg.cache_tokens = 9900;
        lg.output_tokens = 50;
        insert_proxy_log_columns(&db, ProxyLogColumns::from_log(&lg, false, false)).await.unwrap();
        rebuild_stats_agg_from_logs(&db).await.unwrap();

        let ts = today_stats(&db).await.expect("today_stats");
        println!("today cache_rate = {}", ts.cache_rate);
        assert!(ts.cache_rate <= 100.0, "today cache_rate > 100: {}", ts.cache_rate);
        assert!(ts.cache_rate > 98.0 && ts.cache_rate < 100.0, "today cache_rate expected ~99: {}", ts.cache_rate);

        let q = StatsQuery { start: None, end: None, granularity: None, group_by: None, filter_group: None, filter_model: None, filter_platform: None };
        let s = query_stats(&db, &q).await.expect("query_stats");
        println!("overview cache_rate = {}", s.overview.cache_rate);
        assert!(s.overview.cache_rate <= 100.0, "overview cache_rate > 100: {}", s.overview.cache_rate);
        // buckets 非空（防 query_stats_inner 回归致趋势图无数据）
        assert!(!s.buckets.is_empty(), "buckets empty — trend chart would not render");
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



    /// upsert_stats_agg：同 (hour,model,group,pid) 累加；2xx→success，非2xx→error。
    #[tokio::test]
    async fn upsert_stats_agg_accumulates_and_classifies() {
        let db = test_db().await;
        let now = chrono::Utc::now().timestamp_millis();
        let mk = |status: i32| StatsAggInput {
            created_at: now,
            model: "glm-4-plus".into(),
            group_key: "g1".into(),
            platform_id: 1,
            status_code: status,
            input_tokens: 10,
            output_tokens: 20,
            cache_tokens: 5,
            est_cost: 0.01,
            duration_ms: 100,
        };
        upsert_stats_agg(&db, mk(200)).await.unwrap();
        upsert_stats_agg(&db, mk(200)).await.unwrap();
        upsert_stats_agg(&db, mk(500)).await.unwrap(); // 终态非 2xx → error

        let (req, succ, err, inp, cost): (i64, i64, i64, i64, f64) = db.call_traced(None, std::panic::Location::caller(), |conn| {
            Ok(conn.query_row(
                "SELECT request_count, success_count, error_count, sum_input_tokens, sum_est_cost \
                 FROM stats_agg_hourly WHERE model='glm-4-plus' AND group_key='g1' AND platform_id=1",
                [], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
            )?)
        }).await.unwrap();
        assert_eq!(req, 3, "3 次累加同一桶");
        assert_eq!(succ, 2, "2 条 2xx");
        assert_eq!(err, 1, "1 条非 2xx");
        assert_eq!(inp, 30, "input 累加 10*3");
        assert!((cost - 0.03).abs() < 1e-9, "cost 累加 0.01*3");
    }



    /// 回填幂等：migration 011 已在 init 回填；再跑回填 SQL（带 NOT EXISTS 守卫）不应翻倍。
    #[tokio::test]
    async fn agg_backfill_idempotent() {
        let db = test_db().await;
        let now = chrono::Utc::now().timestamp_millis();
        insert_proxy_log_columns(&db, ProxyLogColumns::from_log(&sample_log("x1", "g1", now), false, false)).await.unwrap();
        // 全量重建一次得到基线行数。
        rebuild_stats_agg_from_logs(&db).await.unwrap();
        let n1: i64 = db.call_traced(None, std::panic::Location::caller(), |c| Ok(c.query_row("SELECT COUNT(*) FROM stats_agg_hourly", [], |r| r.get(0))?)).await.unwrap();
        // 再跑带 NOT EXISTS 守卫的回填 SQL（模拟 migration 重放）：表非空 → 不插。
        db.call_traced(None, std::panic::Location::caller(), move |c| {
            c.execute_batch(STATS_AGG_HOURLY_SQL)?;
            Ok(())
        }).await.unwrap();
        let n2: i64 = db.call_traced(None, std::panic::Location::caller(), |c| Ok(c.query_row("SELECT COUNT(*) FROM stats_agg_hourly", [], |r| r.get(0))?)).await.unwrap();
        assert_eq!(n1, n2, "回填幂等：重放 migration 不翻倍");
    }



    /// 回归：两条 raw model 不同但 actual_model 相同（同小时/分组/平台），回填/重建
    /// 必须按 SELECT 输出别名（actual_model 优先）聚合成一行。否则 `GROUP BY model` 会绑到
    /// proxy_log 真实列 model → 聚成两行但 SELECT/UNIQUE 复合键相同 → INSERT 撞
    /// `UNIQUE(time_hour,model,group_key,platform_id)` panic（真实启动 init_tables 必崩）。
    /// 修法 = migration 011 回填 与 agg_rebuild_insert_sql 都用 `GROUP BY 1,2,3,4` 位置引用消歧。
    #[tokio::test]
    async fn agg_rebuild_dedups_raw_models_to_same_actual_model() {
        let db = test_db().await;
        let now = chrono::Utc::now().timestamp_millis();
        let mut a = sample_log("ra1", "g1", now);
        a.model = "claude-sonnet-4".into();
        a.actual_model = "glm-4-plus".into();
        a.platform_id = 1;
        insert_proxy_log_columns(&db, ProxyLogColumns::from_log(&a, false, false)).await.unwrap();
        let mut b = sample_log("ra2", "g1", now);
        b.model = "gpt-4o".into();
        b.actual_model = "glm-4-plus".into();
        b.platform_id = 1;
        insert_proxy_log_columns(&db, ProxyLogColumns::from_log(&b, false, false)).await.unwrap();

        // 重建（与 migration 回填共用 GROUP BY 语义）必须不 panic（不撞 UNIQUE）。
        rebuild_stats_agg_from_logs(&db).await.expect("rebuild must not violate UNIQUE");

        let (rows, reqs): (i64, i64) = db.call_traced(None, std::panic::Location::caller(), |conn| {
            Ok(conn.query_row(
                "SELECT COUNT(*), COALESCE(SUM(request_count),0) FROM stats_agg_hourly \
                 WHERE model = 'glm-4-plus' AND group_key = 'g1' AND platform_id = 1",
                [], |r| Ok((r.get(0)?, r.get(1)?)),
            )?)
        }).await.unwrap();
        assert_eq!(rows, 1, "两条 raw model 须聚合成一行（actual_model 优先）");
        assert_eq!(reqs, 2, "聚合行 request_count 应为 2");
    }



    // ── 单平台动态窗口日速率：从 stats_agg_hourly 算，按 platform_id 过滤 ──
    #[tokio::test]
    async fn platform_hourly_rate_filters_by_platform() {
        let db = test_db().await;
        let now_ms = now();
        // platform 1：~2h 前一条 est_cost=4.0；platform 2：另一条 est_cost=99（不应计入 p1）。
        // 用稍大于 2h 偏移确保 earliest 小时桶起点 >= now-2h（span 上界 ~2h），rate 不被低估。
        let mut l1 = sample_log("r1", "g", now_ms - 2 * 3_600_000 + 60_000);
        l1.platform_id = 1;
        l1.est_cost = 4.0;
        let mut l2 = sample_log("r2", "g", now_ms - 1_000);
        l2.platform_id = 2;
        l2.est_cost = 99.0;
        upsert_proxy_log(&db, l1).await.unwrap();
        upsert_proxy_log(&db, l2).await.unwrap();
        // 从 proxy_log 回填 stats_agg_hourly（speed rate 现在查 agg 表）。
        rebuild_stats_agg_from_logs(&db).await.unwrap();

        // p1：span = clamp(now - earliest桶起点, 5min, 7d)，earliest 桶是 ~2h 前那个整点。
        // rate = 4.0 / span_hours。span 介于 ~2h（earliest 桶起点）与 ~3h（跨整点）间，
        // 故 rate 落 [1.0, 2.5]；断言只校验「计入 p1 的 4.0、未串入 p2 的 99」即可。
        let rate = get_platform_hourly_rate(&db, 1).await.unwrap();
        assert!(rate.is_some(), "p1 应有速率");
        let r = rate.unwrap();
        assert!((1.0..=2.5).contains(&r), "p1 rate 应 ~4.0/2h 量级，got {r}");

        // 无任何用量的平台 → None。
        let none = get_platform_hourly_rate(&db, 999).await.unwrap();
        assert!(none.is_none(), "无用量平台应 None，got {none:?}");
    }



    // ── 关日志场景：proxy_log 无行，仅 stats_agg_hourly 有聚合 → 速率仍可算 ──
    #[tokio::test]
    async fn platform_hourly_rate_from_agg_without_logs() {
        let db = test_db().await;
        let now_ms = now();
        // 不写 proxy_log，直接写 agg（模拟关日志期间 proxy 终态仍 upsert_stats_agg）。
        upsert_stats_agg(
            &db,
            StatsAggInput {
                created_at: now_ms - 2 * 3_600_000 + 60_000,
                model: "m".into(),
                group_key: "g".into(),
                platform_id: 7,
                status_code: 200,
                input_tokens: 10,
                output_tokens: 20,
                cache_tokens: 0,
                est_cost: 4.0,
                duration_ms: 100,
            },
        )
        .await
        .unwrap();

        // proxy_log 为空（关日志），速率仍应从 agg 表算出。
        let rate = get_platform_hourly_rate(&db, 7).await.unwrap();
        assert!(rate.is_some(), "关日志但 agg 有数据，速率不应为 None");
        let r = rate.unwrap();
        assert!((1.0..=2.5).contains(&r), "agg-only rate 应 ~4.0/2h 量级，got {r}");
    }

    /// today_token_total：仅统计今日（本地 0 点起）未删除日志的 input+output。
    #[tokio::test]
    async fn today_token_total_sums_today_only() {
        use chrono::{Local, Duration};
        let db = test_db().await;
        let now_ms = now();
        // 今日两条：(10+20) + (10+20) = 60
        upsert_proxy_log(&db, sample_log("a", "g", now_ms)).await.unwrap();
        upsert_proxy_log(&db, sample_log("b", "g", now_ms)).await.unwrap();
        // 昨日一条：不计入。
        let yesterday_ms = (Local::now() - Duration::days(1)).timestamp_millis();
        upsert_proxy_log(&db, sample_log("c", "g", yesterday_ms)).await.unwrap();

        assert_eq!(today_token_total(&db).await.unwrap(), 60);
    }



    /// today_platform_stats：按平台分组今日用量；platform_id=0 自动分组日志经
    /// group.auto_from_platform 回溯到源平台后归并；只返回有用量的平台；昨日日志不计。
    #[tokio::test]
    async fn today_platform_stats_groups_and_retraces() {
        use chrono::{Local, Duration};
        let db = test_db().await;
        let now_ms = now();

        // 平台 1（源平台），平台 2（无用量，不应出现）。
        let p1 = create_platform(&db, sample_platform("p-one")).await.unwrap();
        let _p2 = create_platform(&db, sample_platform("p-two")).await.unwrap();

        // 自动分组：auto_from_platform = p1.id 的十进制字符串。
        let mut g = sample_group("autog", vec![]);
        g.auto_from_platform = p1.id.to_string();
        let group = create_group(&db, g).await.unwrap();

        // 直连 p1 的日志（platform_id = p1.id），10+20 = 30 tok。
        let mut direct = sample_log("d1", "autog", now_ms);
        direct.platform_id = p1.id;
        upsert_proxy_log(&db, direct).await.unwrap();

        // 自动分组日志（platform_id=0），回溯到 p1。10+20 = 30 tok。
        let mut auto = sample_log("a1", &group.name, now_ms);
        auto.platform_id = 0;
        upsert_proxy_log(&db, auto).await.unwrap();

        // 昨日日志：不计入。
        let yesterday_ms = (Local::now() - Duration::days(1)).timestamp_millis();
        let mut old = sample_log("o1", "autog", yesterday_ms);
        old.platform_id = p1.id;
        upsert_proxy_log(&db, old).await.unwrap();

        // today_platform_stats 读聚合表；测试经 upsert_proxy_log 直插，须先重建聚合。
        rebuild_stats_agg_from_logs(&db).await.unwrap();
        let stats = today_platform_stats(&db).await.unwrap();
        // 只 p1 有今日用量（direct + auto 归并），p2 无用量不出现。
        assert_eq!(stats.len(), 1, "仅有用量的平台出现");
        let s = &stats[0];
        assert_eq!(s.platform_id, p1.id);
        assert_eq!(s.platform_name, "p-one");
        assert_eq!(s.tokens, 60, "direct(30) + auto retrace(30) 归并");
        assert_eq!(s.requests, 2);
    }



    /// 批量 group stats（问题6）：单查 GROUP BY group_key 返回所有 group 聚合，
    /// 与逐 group get_group_usage_stats 数值一致；不同 group 互不串味；空 group_key 不出现。
    #[tokio::test]
    async fn all_group_usage_stats_matches_per_group() {
        let db = test_db().await;
        let now_ms = now();
        // group "ga"：2 条成功（各 10+20 tok）；group "gb"：1 条成功。
        upsert_proxy_log(&db, sample_log("a1", "ga", now_ms)).await.unwrap();
        upsert_proxy_log(&db, sample_log("a2", "ga", now_ms)).await.unwrap();
        upsert_proxy_log(&db, sample_log("b1", "gb", now_ms)).await.unwrap();
        // 空 group_key 的日志（未匹配分组场景）：批量结果中不应出现。
        upsert_proxy_log(&db, sample_log("e1", "", now_ms)).await.unwrap();

        // group usage 读聚合表；测试直插 proxy_log，须先重建聚合。
        rebuild_stats_agg_from_logs(&db).await.unwrap();
        let all = get_all_group_usage_stats(&db).await.unwrap();
        assert_eq!(all.len(), 2, "仅 ga/gb 两个非空 group");
        assert!(!all.contains_key(""), "空 group_key 不计入");

        // 与逐 group 查询数值逐字段一致。
        for name in ["ga", "gb"] {
            let single = get_group_usage_stats(&db, name).await.unwrap();
            let batch = all.get(name).expect("group in batch");
            assert_eq!(batch.total_requests, single.total_requests, "{name} requests");
            assert_eq!(batch.success_count, single.success_count, "{name} success");
            assert_eq!(batch.total_input_tokens, single.total_input_tokens, "{name} input");
            assert_eq!(batch.total_output_tokens, single.total_output_tokens, "{name} output");
            assert_eq!(batch.total_cache_tokens, single.total_cache_tokens, "{name} cache");
        }
        assert_eq!(all["ga"].total_requests, 2);
        assert_eq!(all["gb"].total_requests, 1);
    }
