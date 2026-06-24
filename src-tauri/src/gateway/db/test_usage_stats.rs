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

        // platform usage 累计/今日现读 stats_agg_hourly；测试直插 proxy_log，须先回填聚合表。
        // recent-5 健康度仍裸查 proxy_log，不依赖此回填。
        rebuild_stats_agg_from_logs(&db).await.unwrap();

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
