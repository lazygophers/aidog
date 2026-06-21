#![cfg(test)]
use super::*;
use super::test_support::*;

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
