#![cfg(test)]
use super::*;
use super::test_support::*;

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
