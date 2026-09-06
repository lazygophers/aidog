#![cfg(test)]
//! 票 06 预算窗口聚合查询测试：三维过滤、窗口边界（跨月归零）、索引使用。

use crate::*;
use aidog_db::models::AppliesTo;
use aidog_db::test_support::*;
use aidog_logs::*;

/// 插一条终态日志并重建聚合表（`window_spend` 读 stats_agg_hourly）。
async fn seed(db: &aidog_db::Db, id: &str, group_key: &str, pid: u64, model: &str, cost: f64) {
    let mut lg = sample_log(id, group_key, aidog_db::now());
    lg.platform_id = pid;
    lg.actual_model = model.to_string();
    lg.est_cost = cost;
    upsert_proxy_log(db, lg).await.unwrap();
}

fn at(platforms: Vec<i64>, groups: Vec<&str>, models: Vec<&str>) -> AppliesTo {
    AppliesTo {
        platforms,
        groups: groups.into_iter().map(String::from).collect(),
        models: models.into_iter().map(String::from).collect(),
    }
}

/// 三维空 = 不限，累加全部；三维各自过滤生效且维间 AND。
#[tokio::test]
async fn window_spend_filters_three_dimensions() {
    let db = test_db().await;
    seed(&db, "l1", "ga", 1, "m-one", 1.0).await;
    seed(&db, "l2", "gb", 2, "m-two", 2.0).await;
    seed(&db, "l3", "ga", 2, "m-one", 4.0).await;
    rebuild_stats_agg_from_logs(&db).await.unwrap();

    let start = local_month_start_key();
    let sum = |a: AppliesTo| {
        let db = &db;
        let start = start.clone();
        async move { window_spend(db, &a, start).await.unwrap() }
    };

    assert_eq!(sum(at(vec![], vec![], vec![])).await, 7.0, "三维空 = 全部");
    assert_eq!(sum(at(vec![2], vec![], vec![])).await, 6.0, "platform 维");
    assert_eq!(sum(at(vec![], vec!["ga"], vec![])).await, 5.0, "group 维");
    assert_eq!(sum(at(vec![], vec![], vec!["m-one"])).await, 5.0, "model 维");
    // 维间 AND：platform=2 且 group=ga 只剩 l3。
    assert_eq!(sum(at(vec![2], vec!["ga"], vec![])).await, 4.0, "维间 AND");
    // 多值 = 任一命中。
    assert_eq!(sum(at(vec![1, 2], vec![], vec![])).await, 7.0, "多值 OR");
    // 无交集 → 0。
    assert_eq!(sum(at(vec![1], vec!["gb"], vec![])).await, 0.0, "无交集");
}

/// 跨月归零：窗口起点推进到下个月后，本月花费不再计入。
#[tokio::test]
async fn window_spend_resets_across_month_boundary() {
    let db = test_db().await;
    seed(&db, "l1", "ga", 1, "m-one", 3.5).await;
    rebuild_stats_agg_from_logs(&db).await.unwrap();

    let all = at(vec![], vec![], vec![]);
    assert_eq!(
        window_spend(&db, &all, local_month_start_key()).await.unwrap(),
        3.5,
        "本月窗口内计入"
    );
    // 下个月 1 号 00:00 起的窗口：本月的花费全部落在窗口之前 → 归零。
    let next_month = {
        use chrono::{Datelike, Local};
        let n = Local::now();
        let (y, m) = if n.month() == 12 {
            (n.year() + 1, 1)
        } else {
            (n.year(), n.month() + 1)
        };
        format!("{y:04}-{m:02}-01 00:00:00")
    };
    assert_eq!(
        window_spend(&db, &all, next_month).await.unwrap(),
        0.0,
        "跨月归零"
    );
}

/// 性能红线：聚合必须走 `idx_stats_agg_time` 的范围扫，不得 SCAN 全表。
#[tokio::test]
async fn window_spend_uses_time_index() {
    let db = test_db().await;
    seed(&db, "l1", "ga", 1, "m-one", 1.0).await;
    rebuild_stats_agg_from_logs(&db).await.unwrap();

    // 两种形态各查一次执行计划：无三维过滤（只有 time_hour 范围）/ 带 platform 过滤。
    for (label, sql) in [
        (
            "time-range only",
            "EXPLAIN QUERY PLAN SELECT COALESCE(SUM(sum_est_cost), 0.0) \
             FROM stats_agg_hourly WHERE deleted_at = 0 AND time_hour >= '2000-01-01 00:00:00'",
        ),
        (
            "three-dim filter",
            "EXPLAIN QUERY PLAN SELECT COALESCE(SUM(sum_est_cost), 0.0) \
             FROM stats_agg_hourly WHERE deleted_at = 0 AND time_hour >= '2000-01-01 00:00:00' \
             AND platform_id IN (1) AND group_key IN ('ga') AND model IN ('m-one')",
        ),
    ] {
        let plan: String = db
            .call_read_traced(None, std::panic::Location::caller(), move |conn| {
                let mut stmt = conn.prepare(sql)?;
                let rows = stmt.query_map([], |r| r.get::<_, String>(3))?;
                Ok(rows.filter_map(|r| r.ok()).collect::<Vec<_>>().join(" | "))
            })
            .await
            .unwrap();
        println!("[{label}] query plan: {plan}");
        assert!(
            plan.contains("USING INDEX") || plan.contains("USING COVERING INDEX"),
            "[{label}] 聚合查询未走索引（会全表扫）: {plan}"
        );
        assert!(
            !plan.contains("SCAN stats_agg_hourly"),
            "[{label}] 聚合查询退化为全表 SCAN: {plan}"
        );
    }
}
