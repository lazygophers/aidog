#![cfg(test)]
use crate::*;
use aidog_db::now;
use aidog_db::test_support::*;
use aidog_logs::*;

/// 批量 `platform_usage_stats_all` 结果必须逐平台等于单平台 `get_platform_usage_stats`，
/// 含 platform_id=0 自动分组日志按 group_key → auto_from_platform 回溯归属源平台。
#[tokio::test]
async fn platform_usage_stats_all_matches_per_platform() {
    let db = test_db().await;
    let p1 = insert_test_platform(&db, "P1").await;
    let p2 = insert_test_platform(&db, "P2").await;
    // 自动分组：显示名 name=auto_p1，路由/归属键 group_key=gk_auto_p1（刻意 ≠ name，
    // 复刻真实 gk_<hex> 场景，防回归 join 误用 g.name），auto_from_platform=p1（十进制字符串）。
    insert_test_group(&db, "auto_p1", Some("gk_auto_p1"), &p1.to_string()).await;

    let now = chrono::Utc::now().timestamp_millis();

    // P1 直挂日志（platform_id=p1）：2 条，1 成功 1 失败。
    let mut a1 = sample_log("a1", "g1", now);
    a1.platform_id = p1;
    a1.status_code = 200;
    a1.input_tokens = 10;
    a1.output_tokens = 20;
    a1.cache_tokens = 5;
    a1.est_cost = 0.01;
    insert_proxy_log_columns(&db, ProxyLogColumns::from_log(&a1, false, false))
        .await
        .unwrap();
    let mut a2 = sample_log("a2", "g1", now);
    a2.platform_id = p1;
    a2.status_code = 500;
    a2.input_tokens = 7;
    a2.output_tokens = 0;
    a2.est_cost = 0.0;
    insert_proxy_log_columns(&db, ProxyLogColumns::from_log(&a2, false, false))
        .await
        .unwrap();

    // platform_id=0 自动分组日志，group_key=gk_auto_p1（= group.group_key，≠ name）→ 回溯归 p1。
    let mut a0 = sample_log("a0", "gk_auto_p1", now);
    a0.platform_id = 0;
    a0.status_code = 200;
    a0.input_tokens = 100;
    a0.output_tokens = 200;
    a0.cache_tokens = 50;
    a0.est_cost = 0.05;
    insert_proxy_log_columns(&db, ProxyLogColumns::from_log(&a0, false, false))
        .await
        .unwrap();

    // P2 直挂日志：1 条成功。
    let mut b1 = sample_log("b1", "g2", now);
    b1.platform_id = p2;
    b1.status_code = 200;
    b1.input_tokens = 3;
    b1.output_tokens = 4;
    b1.est_cost = 0.002;
    insert_proxy_log_columns(&db, ProxyLogColumns::from_log(&b1, false, false))
        .await
        .unwrap();

    // 回溯不到的孤儿 platform_id=0 日志（无匹配 auto 分组）→ 不应归任何平台卡片。
    let mut orphan = sample_log("orphan", "no_such_group", now);
    orphan.platform_id = 0;
    orphan.status_code = 200;
    orphan.est_cost = 0.99;
    insert_proxy_log_columns(&db, ProxyLogColumns::from_log(&orphan, false, false))
        .await
        .unwrap();

    // platform usage 累计/今日现读 stats_agg_hourly；测试直插 proxy_log，须先回填聚合表。
    // recent-5 健康度仍裸查 proxy_log，不依赖此回填。
    rebuild_stats_agg_from_logs(&db).await.unwrap();

    let batch = platform_usage_stats_all(&db).await.expect("batch");

    for pid in [p1, p2] {
        let single = get_platform_usage_stats(&db, pid).await.expect("single");
        let b = batch
            .get(&pid)
            .unwrap_or_else(|| panic!("missing pid {pid} in batch"));
        assert_eq!(
            b.total_requests, single.total_requests,
            "pid {pid} total_requests"
        );
        assert_eq!(
            b.success_count, single.success_count,
            "pid {pid} success_count"
        );
        assert_eq!(
            b.total_input_tokens, single.total_input_tokens,
            "pid {pid} input"
        );
        assert_eq!(
            b.total_output_tokens, single.total_output_tokens,
            "pid {pid} output"
        );
        assert_eq!(
            b.total_cache_tokens, single.total_cache_tokens,
            "pid {pid} cache"
        );
        assert!(
            (b.total_cost - single.total_cost).abs() < 1e-9,
            "pid {pid} cost"
        );
        assert!(
            (b.cache_rate - single.cache_rate).abs() < 1e-9,
            "pid {pid} cache_rate"
        );
        // recent_* 必须与单平台版一致（批量补齐近期健康度，供平台卡片健康点配色）。
        assert_eq!(
            b.recent_total, single.recent_total,
            "pid {pid} recent_total"
        );
        assert_eq!(
            b.recent_failures, single.recent_failures,
            "pid {pid} recent_failures"
        );
    }

    // p1 含回溯日志：total=3（a1+a2+a0），success=2，input=117。
    let p1b = batch.get(&p1).unwrap();
    assert_eq!(p1b.total_requests, 3, "p1 含 platform_id=0 回溯归属");
    assert_eq!(p1b.success_count, 2);
    assert_eq!(p1b.total_input_tokens, 117);
    // p1 近期 3 条（a1 200 / a2 500 / a0 200）：recent_total=3，recent_failures=1（a2）。
    // 非恒 0 → 平台卡片健康点恢复（healthStatus(3,1)=warning）。
    assert_eq!(p1b.recent_total, 3, "p1 recent_total 应非 0（健康点回归）");
    assert_eq!(p1b.recent_failures, 1, "p1 recent_failures（a2 500）");

    // 孤儿日志（est_cost=0.99）不得归任何平台 → 平台合计 cost 不含 0.99。
    let summed: f64 = batch.values().map(|s| s.total_cost).sum();
    assert!(
        summed < 0.9,
        "orphan platform_id=0 leaked into platform stats: {summed}"
    );
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
            client: String::new(),
            req_model: String::new(),
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
    assert!(
        (1.0..=2.5).contains(&r),
        "agg-only rate 应 ~4.0/2h 量级，got {r}"
    );
}

/// 批量 group stats（问题6）：单查 GROUP BY group_key 返回所有 group 聚合，
/// 与逐 group get_group_usage_stats 数值一致；不同 group 互不串味；空 group_key 不出现。
#[tokio::test]
async fn all_group_usage_stats_matches_per_group() {
    let db = test_db().await;
    let now_ms = now();
    // group "ga"：2 条成功（各 10+20 tok）；group "gb"：1 条成功。
    upsert_proxy_log(&db, sample_log("a1", "ga", now_ms))
        .await
        .unwrap();
    upsert_proxy_log(&db, sample_log("a2", "ga", now_ms))
        .await
        .unwrap();
    upsert_proxy_log(&db, sample_log("b1", "gb", now_ms))
        .await
        .unwrap();
    // 空 group_key 的日志（未匹配分组场景）：批量结果中不应出现。
    upsert_proxy_log(&db, sample_log("e1", "", now_ms))
        .await
        .unwrap();

    // group usage 读聚合表；测试直插 proxy_log，须先重建聚合。
    rebuild_stats_agg_from_logs(&db).await.unwrap();
    let all = get_all_group_usage_stats(&db).await.unwrap();
    assert_eq!(all.len(), 2, "仅 ga/gb 两个非空 group");
    assert!(!all.contains_key(""), "空 group_key 不计入");

    // 与逐 group 查询数值逐字段一致。
    for name in ["ga", "gb"] {
        let single = get_group_usage_stats(&db, name).await.unwrap();
        let batch = all.get(name).expect("group in batch");
        assert_eq!(
            batch.total_requests, single.total_requests,
            "{name} requests"
        );
        assert_eq!(batch.success_count, single.success_count, "{name} success");
        assert_eq!(
            batch.total_input_tokens, single.total_input_tokens,
            "{name} input"
        );
        assert_eq!(
            batch.total_output_tokens, single.total_output_tokens,
            "{name} output"
        );
        assert_eq!(
            batch.total_cache_tokens, single.total_cache_tokens,
            "{name} cache"
        );
    }
    assert_eq!(all["ga"].total_requests, 2);
    assert_eq!(all["gb"].total_requests, 1);
}

/// 虚拟桶「未匹配」（MITM 解密非 API 流量 fallback 直通）的 group_key 非空 →
/// 出现在 get_all_group_usage_stats 批量结果中，不被 group_key <> '' 过滤吞掉。
#[tokio::test]
async fn unmatched_virtual_bucket_appears_in_group_stats() {
    let db = test_db().await;
    let now_ms = now();
    // 模拟 fallback 直通落库的虚拟桶日志：group_key = "未匹配"，platform_id=0，cost=0。
    let mut unmatched = sample_log("u1", "未匹配", now_ms);
    unmatched.platform_id = 0;
    unmatched.status_code = 200;
    unmatched.est_cost = 0.0;
    upsert_proxy_log(&db, unmatched).await.unwrap();
    // 普通分组的日志，确保不互相串味。
    upsert_proxy_log(&db, sample_log("g1", "ga", now_ms))
        .await
        .unwrap();
    // 重建聚合表。
    rebuild_stats_agg_from_logs(&db).await.unwrap();

    let all = get_all_group_usage_stats(&db).await.unwrap();
    // 虚拟桶应出现（group_key = "未匹配" 非空，不被过滤）。
    assert!(
        all.contains_key("未匹配"),
        "虚拟桶「未匹配」应在批量统计结果中，got keys: {:?}",
        all.keys().collect::<Vec<_>>()
    );
    assert_eq!(all["未匹配"].total_requests, 1);
    assert_eq!(all["未匹配"].total_cost, 0.0, "虚拟桶不计费");
    // 普通分组也在。
    assert_eq!(all["ga"].total_requests, 1);
    assert_eq!(all.len(), 2, "仅「未匹配」+ ga 两个非空 group");
}

// ── G3 分组页（整体风格票 03）：breakdown + 最近路由快照 ─────────────────────

#[tokio::test]
async fn group_platform_breakdown_windows_and_model_dim() {
    let db = test_db().await;
    let p1 = insert_test_platform(&db, "P1").await;
    insert_test_group(&db, "g1", Some("gk_g1"), &p1.to_string()).await;
    let now = chrono::Utc::now().timestamp_millis();
    let mk = |id: &str, cost: f64, model: &str| {
        let mut l = sample_log(id, "gk_g1", now);
        l.platform_id = p1;
        l.status_code = 200;
        l.est_cost = cost;
        l.model = model.to_string();
        l.actual_model = model.to_string();
        l
    };
    for l in [mk("a1", 0.01, "m-a"), mk("a2", 0.02, "m-a"), mk("a3", 0.10, "m-b")] {
        insert_proxy_log_columns(&db, ProxyLogColumns::from_log(&l, false, false))
            .await
            .unwrap();
    }
    rebuild_stats_agg_from_logs(&db).await.unwrap();

    // 平台级（不分模型）：单平台聚合全部
    let rows = group_platform_breakdown(&db, "gk_g1", "7d", false).await.unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].platform_id, p1 as i64);
    assert_eq!(rows[0].requests, 3);
    assert_eq!(rows[0].success, 3);
    assert!((rows[0].est_cost - 0.13).abs() < 1e-9);
    assert!(rows[0].model.is_none());

    // 模型维度：两行，按请求量降序（m-a 2 在前）
    let rows = group_platform_breakdown(&db, "gk_g1", "7d", true).await.unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].model.as_deref(), Some("m-a"));
    assert_eq!(rows[0].requests, 2);
    assert_eq!(rows[1].model.as_deref(), Some("m-b"));

    // 其他 group 拿不到
    assert!(group_platform_breakdown(&db, "gk_other", "7d", false).await.unwrap().is_empty());
}

#[tokio::test]
async fn latest_routing_snapshot_reads_most_recent() {
    let db = test_db().await;
    let p1 = insert_test_platform(&db, "P1").await;
    insert_test_group(&db, "g2", Some("gk_g2"), &p1.to_string()).await;
    let now = chrono::Utc::now().timestamp_millis();
    // 两条带 routing 的日志：后者（id 更大）胜出
    for (id, selected_pid) in [("r1", 1u64), ("r2", p1)] {
        let mut l = sample_log(id, "gk_g2", now);
        l.platform_id = selected_pid;
        l.routing = Some(aidog_db::models::RoutingSnapshot {
            mode: None,
            entries: vec![aidog_db::models::RouteCandidate {
                order: 1,
                platform: format!("P{selected_pid}"),
                platform_id: selected_pid,
                level_priority: 1,
                priority: 1,
                reason: aidog_db::models::RouteReason::Selected,
                target_model: None,
                tags: vec![],
            }],
        });
        insert_proxy_log_columns(&db, ProxyLogColumns::from_log(&l, false, false))
            .await
            .unwrap();
    }
    let latest = latest_routing_snapshot(&db, "gk_g2").await.unwrap().expect("应有快照");
    assert_eq!(latest.platform_id, p1 as i64);
    assert!(latest.snapshot.entries.iter().any(|e| e.platform_id == p1));
    // 无快照 group → None
    assert!(latest_routing_snapshot(&db, "gk_none").await.unwrap().is_none());
}

// ── 请求流向票 03：stats_flow_breakdown ──────────────────────────────────────

/// 直接写一行聚合桶（绕过日志重建，精确控制 time_hour / 六元组 / deleted_at）。
#[allow(clippy::too_many_arguments)]
async fn put_agg(
    db: &aidog_db::Db,
    time_hour: &str,
    client: &str,
    group_key: &str,
    req_model: &str,
    platform_id: i64,
    model: &str,
    req: i64,
    (i, o, c): (i64, i64, i64),
    cost: f64,
    deleted_at: i64,
) {
    let (th, cl, gk, rm, m) = (time_hour.to_string(), client.to_string(), group_key.to_string(), req_model.to_string(), model.to_string());
    db.call_traced(None, std::panic::Location::caller(), move |conn| {
        conn.execute(
            "INSERT INTO stats_agg_hourly (time_hour, model, group_key, platform_id, client, req_model, \
             request_count, sum_input_tokens, sum_output_tokens, sum_cache_tokens, sum_est_cost, deleted_at) \
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12)",
            rusqlite::params![th, m, gk, platform_id, cl, rm, req, i, o, c, cost, deleted_at],
        )?;
        Ok(())
    })
    .await
    .unwrap();
}

/// 固定时钟：2026-10-30 本地 15:30（key 均由同一函数推出，与时区无关）。
fn flow_now() -> i64 {
    use chrono::{Local, TimeZone};
    Local.with_ymd_and_hms(2026, 10, 30, 15, 30, 0).unwrap().timestamp_millis()
}

#[tokio::test]
async fn flow_breakdown_filters_tokens_unknown_and_deleted() {
    let (db, _dir) = three_file_db().await;
    let now = flow_now();
    let t = utc_ms_to_local_hour_key(now - 3_600_000);
    // 两个客户端 × 两个分组；client='' 历史行单独成行
    put_agg(&db, &t, "claude-code", "g1", "opus", 1, "m1", 2, (10, 20, 5), 0.5, 0).await;
    put_agg(&db, &t, "codex", "g1", "opus", 1, "m1", 1, (1, 1, 1), 0.1, 0).await;
    put_agg(&db, &t, "", "g1", "", 1, "m1", 4, (100, 0, 0), 1.0, 0).await;
    put_agg(&db, &t, "claude-code", "g2", "opus", 2, "m2", 8, (7, 7, 7), 2.0, 0).await;
    // 软删行不计
    put_agg(&db, &t, "claude-code", "g1", "opus", 9, "m9", 99, (1, 1, 1), 9.0, 1).await;

    let all = stats_flow_breakdown_at(&db, "24h", None, now).await.unwrap();
    assert_eq!(all.len(), 4, "{all:?}");
    assert_eq!(all.iter().map(|r| r.requests).sum::<i64>(), 15);
    let cc = all.iter().find(|r| r.client == "claude-code" && r.group_key == "g1").unwrap();
    assert_eq!(cc.tokens, 10 + 20 + 5);
    assert_eq!((cc.requests, cc.platform_id, cc.req_model.as_str(), cc.model.as_str()), (2, 1, "opus", "m1"));
    let unknown = all.iter().find(|r| r.client.is_empty()).unwrap();
    assert_eq!((unknown.requests, unknown.tokens, unknown.req_model.as_str()), (4, 100, ""));
    assert!(all.iter().all(|r| r.platform_id != 9), "deleted_at!=0 不计入");
    assert!(all.windows(2).all(|w| w[0].requests >= w[1].requests), "按请求数降序");

    let g1 = stats_flow_breakdown_at(&db, "24h", Some("g1"), now).await.unwrap();
    assert_eq!(g1.len(), 3);
    assert!(g1.iter().all(|r| r.group_key == "g1"));
    assert!(stats_flow_breakdown_at(&db, "24h", Some("nope"), now).await.unwrap().is_empty());
    assert!(stats_flow_breakdown_at(&db, "bogus", None, now).await.is_err());
}

#[tokio::test]
async fn flow_breakdown_window_edges() {
    use chrono::{Local, TimeZone};
    let (db, _dir) = three_file_db().await;
    let now = flow_now();
    const H: i64 = 3_600_000;
    let key = utc_ms_to_local_hour_key;
    let yesterday_23 = {
        let d = Local.timestamp_millis_opt(now).unwrap().date_naive().pred_opt().unwrap();
        format!("{d} 23:00:00")
    };
    let today_00 = format!("{} 00:00:00", &key(now)[..10]);
    // model 列记标签，便于断言集合；每个窗口边界一 in 一 out
    let rows = [
        ("h24_in", key(now - 24 * H)),
        ("h24_out", key(now - 25 * H)),
        ("now_in", key(now)),
        ("future_out", key(now + H)),
        ("today_in", today_00),
        ("today_out", yesterday_23),
        ("d7_in", key(now - 7 * 24 * H)),
        ("d7_out", key(now - 7 * 24 * H - H)),
    ];
    for (i, (label, th)) in rows.iter().enumerate() {
        put_agg(&db, th, "c", "g", "r", 1, label, 1 + i as i64, (0, 0, 0), 0.0, 0).await;
    }
    let labels = |w: &'static str| {
        let db = &db;
        async move {
            stats_flow_breakdown_at(db, w, None, now).await.unwrap().into_iter().map(|r| r.model).collect::<Vec<_>>()
        }
    };
    let has = |v: &[String], s: &str| v.iter().any(|x| x == s);
    let h24 = labels("24h").await;
    assert!(has(&h24, "h24_in") && has(&h24, "now_in"), "{h24:?}");
    assert!(!has(&h24, "h24_out") && !has(&h24, "future_out"), "{h24:?}");
    let today = labels("today").await;
    assert!(has(&today, "today_in") && has(&today, "now_in"), "{today:?}");
    assert!(!has(&today, "today_out") && !has(&today, "future_out"), "{today:?}");
    let d7 = labels("7d").await;
    assert!(has(&d7, "d7_in") && has(&d7, "h24_out") && has(&d7, "today_out"), "{d7:?}");
    assert!(!has(&d7, "d7_out") && !has(&d7, "future_out"), "{d7:?}");

    // 24h 边界与首页维度趋势一致：同夹具下两边请求总数相同（桶集合相同）
    let q = aidog_db::models::StatsQuery {
        start: Some(now - 24 * H),
        end: Some(now),
        granularity: Some("hourly".into()),
        group_by: None,
        filter_group: None,
        filter_model: None,
        filter_platform: None,
        filter_coding_plan: None,
        series_by: None,
        limit: None,
    };
    let trend = query_stats(&db, &q).await.unwrap();
    let trend_total: i64 = trend.buckets.iter().map(|b| b.total_requests as i64).sum();
    let flow_total: i64 = stats_flow_breakdown_at(&db, "24h", None, now).await.unwrap().iter().map(|r| r.requests).sum();
    assert_eq!(flow_total, trend_total);
    assert!(flow_total > 0);
}

/// 行数膨胀回归栏杆：168h × 20 组合 × 3 客户端 × 3 请求模型 ≈ 3 万聚合行，7d 查询须远小于 500ms。
#[tokio::test]
async fn flow_breakdown_7d_scale_guard() {
    let (db, _dir) = three_file_db().await;
    let now = flow_now();
    let keys: std::collections::BTreeSet<String> =
        (0..168).map(|h| utc_ms_to_local_hour_key(now - h * 3_600_000)).collect(); // 去 DST 重复键
    let n_rows = keys.len() * 20 * 3 * 3;
    db.call_traced(None, std::panic::Location::caller(), move |conn| {
        let tx = conn.transaction()?;
        {
            let mut st = tx.prepare(
                "INSERT INTO stats_agg_hourly (time_hour, model, group_key, platform_id, client, req_model, \
                 request_count, sum_input_tokens, sum_output_tokens, sum_cache_tokens, sum_est_cost) \
                 VALUES (?1,?2,?3,?4,?5,?6,1,1,1,1,0.01)",
            )?;
            for k in &keys {
                for combo in 0..20 {
                    for c in ["claude-code", "codex", "gemini"] {
                        for r in ["opus", "sonnet", "haiku"] {
                            st.execute(rusqlite::params![k, format!("m{}", combo % 5), format!("g{}", combo % 4), combo, c, r])?;
                        }
                    }
                }
            }
        }
        tx.commit()?;
        Ok(())
    })
    .await
    .unwrap();
    let t0 = std::time::Instant::now();
    let rows = stats_flow_breakdown_at(&db, "7d", None, now).await.unwrap();
    let el = t0.elapsed();
    println!("flow 7d scale: {n_rows} agg rows -> {} flow rows in {el:?}", rows.len());
    assert_eq!(rows.len(), 20 * 3 * 3, "六元组基数不随小时数膨胀");
    assert!(el < std::time::Duration::from_millis(500), "7d 查询 {el:?} 超上界");
}
