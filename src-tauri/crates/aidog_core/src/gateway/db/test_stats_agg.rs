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



    /// 回填幂等：migration 011 已在 init 回填；再跑回填（带空表守卫）不应翻倍。
    /// 去 JOIN/子查询重构后回填改 Rust `backfill_stats_agg_if_empty`（空表守卫在 Rust 内判），
    /// 替代旧 NOT EXISTS SQL；表非空时它应直接返回不插，保持幂等。
    #[tokio::test]
    async fn agg_backfill_idempotent() {
        let db = test_db().await;
        let now = chrono::Utc::now().timestamp_millis();
        insert_proxy_log_columns(&db, ProxyLogColumns::from_log(&sample_log("x1", "g1", now), false, false)).await.unwrap();
        // 全量重建一次得到基线行数。
        rebuild_stats_agg_from_logs(&db).await.unwrap();
        let n1: i64 = db.call_traced(None, std::panic::Location::caller(), |c| Ok(c.query_row("SELECT COUNT(*) FROM stats_agg_hourly", [], |r| r.get(0))?)).await.unwrap();
        // 再跑带空表守卫的回填（模拟 init 重放）：表非空 → 不插。
        db.call_traced(None, std::panic::Location::caller(), move |c| {
            let auto_map = load_auto_from_map(c)?;
            backfill_stats_agg_if_empty(c, &auto_map)?;
            Ok(())
        }).await.unwrap();
        let n2: i64 = db.call_traced(None, std::panic::Location::caller(), |c| Ok(c.query_row("SELECT COUNT(*) FROM stats_agg_hourly", [], |r| r.get(0))?)).await.unwrap();
        assert_eq!(n1, n2, "回填幂等：重放回填不翻倍");
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

    /// cleanup_stats_agg 按 retention_days 删除旧行。
    /// upsert_stats_agg 写 created_at=now，所以只能直接 INSERT 一条旧 created_at 行再测删除。
    #[tokio::test]
    async fn cleanup_stats_agg_removes_old_rows() {
        let db = test_db().await;
        let now = chrono::Utc::now().timestamp_millis();
        // 新行（保留）：直接 INSERT created_at=now
        let old_ts = now - 2 * 24 * 3600 * 1000_i64; // 2 days ago
        db.call_traced(None, std::panic::Location::caller(), move |c| {
            c.execute_batch(&format!(
                "INSERT INTO stats_agg_hourly \
                 (time_hour,model,group_key,platform_id,request_count,success_count,error_count,\
                  sum_input_tokens,sum_output_tokens,sum_cache_tokens,sum_est_cost,sum_duration_ms,\
                  created_at,updated_at,deleted_at) VALUES \
                 ('2025-01-01 00:00:00','new-model','g1',1,1,1,0,5,5,0,0.001,50,{now},{now},0), \
                 ('2025-01-02 00:00:00','old-model','g1',1,1,1,0,5,5,0,0.001,50,{old_ts},{old_ts},0)"
            ))?;
            Ok(())
        }).await.unwrap();

        let before: i64 = db.call_traced(None, std::panic::Location::caller(), |c| {
            Ok(c.query_row("SELECT COUNT(*) FROM stats_agg_hourly", [], |r| r.get(0))?)
        }).await.unwrap();
        assert_eq!(before, 2, "should have 2 rows before cleanup");

        // retention_days=1 → 2天前的老行应被删
        cleanup_stats_agg(&db, 1).await.unwrap();

        let after: i64 = db.call_traced(None, std::panic::Location::caller(), |c| {
            Ok(c.query_row("SELECT COUNT(*) FROM stats_agg_hourly", [], |r| r.get(0))?)
        }).await.unwrap();
        assert_eq!(after, 1, "old row should be deleted, new row kept");
    }

    /// rebuild_stats_agg_once_if_needed: 首次调用触发重建，再次调用跳过。
    #[tokio::test]
    async fn rebuild_stats_agg_once_if_needed_runs_once() {
        let db = test_db().await;
        let now = chrono::Utc::now().timestamp_millis();
        insert_proxy_log_columns(&db, ProxyLogColumns::from_log(&sample_log("x1", "g1", now), false, false)).await.unwrap();
        // 先清空 agg 表（模拟未建）
        db.call_traced(None, std::panic::Location::caller(), |c| {
            c.execute_batch("DELETE FROM stats_agg_hourly")?;
            Ok(())
        }).await.unwrap();

        let built = rebuild_stats_agg_once_if_needed(&db).await.unwrap();
        assert!(built, "first call should trigger rebuild and return true");

        // 再次调用应跳过（表已有数据）
        let again = rebuild_stats_agg_once_if_needed(&db).await.unwrap();
        assert!(!again, "second call should skip and return false");
    }

    /// rebuild upsert 语义：已存在的聚合行被 proxy_log 真值【覆盖】，不翻倍、不累加。
    /// 先 upsert 一条单请求形成桶，再插 2 条同桶 proxy_log，rebuild 后 request_count=2（真值）非 3。
    #[tokio::test]
    async fn rebuild_overwrites_existing_row_no_doubling() {
        let db = test_db().await;
        let now = chrono::Utc::now().timestamp_millis();
        // 先 upsert 一条单请求 → 形成 (hour, glm-4-plus, g1, pid=1) 行，request_count=1。
        upsert_stats_agg(&db, StatsAggInput {
            created_at: now,
            model: "glm-4-plus".into(),
            group_key: "g1".into(),
            platform_id: 1,
            status_code: 200,
            input_tokens: 99,
            output_tokens: 0,
            cache_tokens: 0,
            est_cost: 9.9,
            duration_ms: 0,
        }).await.unwrap();

        // 同桶插 2 条 proxy_log（actual_model=glm-4-plus, platform_id=1, group g1）。
        for id in ["ow1", "ow2"] {
            let mut l = sample_log(id, "g1", now);
            l.actual_model = "glm-4-plus".into();
            l.platform_id = 1;
            l.input_tokens = 10;
            insert_proxy_log_columns(&db, ProxyLogColumns::from_log(&l, false, false)).await.unwrap();
        }

        rebuild_stats_agg_from_logs(&db).await.unwrap();

        let (req, inp): (i64, i64) = db.call_traced(None, std::panic::Location::caller(), |conn| {
            Ok(conn.query_row(
                "SELECT request_count, sum_input_tokens FROM stats_agg_hourly \
                 WHERE model='glm-4-plus' AND group_key='g1' AND platform_id=1",
                [], |r| Ok((r.get(0)?, r.get(1)?)),
            )?)
        }).await.unwrap();
        assert_eq!(req, 2, "被 proxy_log 真值覆盖（2 条），非累加（1+2=3）");
        assert_eq!(inp, 20, "input 覆盖为真值 10*2，非 99+20");
    }

    /// rebuild 不再清空整表：stats_agg 有行但 proxy_log 无对应行时，rebuild 后该行保留。
    #[tokio::test]
    async fn rebuild_preserves_rows_without_matching_proxy_log() {
        let db = test_db().await;
        let now = chrono::Utc::now().timestamp_millis();
        // 形成一条聚合行（模拟关日志期间已聚合但 proxy_log 无对应行）。
        upsert_stats_agg(&db, StatsAggInput {
            created_at: now,
            model: "orphan-model".into(),
            group_key: "g-orphan".into(),
            platform_id: 7,
            status_code: 200,
            input_tokens: 42,
            output_tokens: 0,
            cache_tokens: 0,
            est_cost: 1.5,
            duration_ms: 0,
        }).await.unwrap();
        // proxy_log 里无任何匹配行 → rebuild 的 SELECT 不会产出该桶。

        rebuild_stats_agg_from_logs(&db).await.unwrap();

        let (req, inp): (i64, i64) = db.call_traced(None, std::panic::Location::caller(), |conn| {
            Ok(conn.query_row(
                "SELECT request_count, sum_input_tokens FROM stats_agg_hourly \
                 WHERE model='orphan-model' AND group_key='g-orphan' AND platform_id=7",
                [], |r| Ok((r.get(0)?, r.get(1)?)),
            )?)
        }).await.unwrap();
        assert_eq!(req, 1, "无对应 proxy_log 行的旧聚合行须保留（不再清空整表）");
        assert_eq!(inp, 42, "旧行数值不变");
    }
