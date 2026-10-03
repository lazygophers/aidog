#![cfg(test)]
use crate::*;
use aidog_db::models::*;
use aidog_db::test_support::*;
use aidog_logs::*;

// ── 纯函数：nice_step / bin_edges ──────────────────────────

#[test]
fn nice_step_picks_1_2_5() {
    assert_eq!(nice_step(0.0), 1.0);
    assert_eq!(nice_step(-3.0), 1.0);
    assert_eq!(nice_step(36.67), 50.0);
    assert_eq!(nice_step(0.031), 0.05);
    assert_eq!(nice_step(1.0), 1.0);
    assert_eq!(nice_step(1.5), 2.0);
    assert_eq!(nice_step(7.0), 10.0);
    assert_eq!(nice_step(500.0), 500.0);
}

#[test]
fn bin_edges_nice_aligned_and_covers_max() {
    // min=120 max=1000 target=24：raw=36.67→step 50，start=100，n=ceil(880/50)=18。
    let e = bin_edges(120.0, 1000.0, 24);
    assert_eq!(e.len(), 19);
    assert_eq!(e[0], 100.0);
    for w in e.windows(2) {
        assert!((w[1] - w[0] - 50.0).abs() < 1e-9);
    }
    assert!(e.last().unwrap() >= &1000.0);
}

#[test]
fn bin_edges_degenerate_single_bin() {
    // max == min（含全 0）→ 单 bin 两边界。
    let e = bin_edges(0.0, 0.0, 24);
    assert_eq!(e.len(), 2);
    assert_eq!(e[0], 0.0);
    let e2 = bin_edges(700.0, 700.0, 16);
    assert_eq!(e2.len(), 2);
    assert!(e2[0] <= 700.0 && e2[1] > 700.0);
}

// ── 纯函数：build_histogram 计数正确性（构造已知分布） ──────────

#[test]
fn build_histogram_counts_known_distribution() {
    // duration ∈ [120, 980]，cost ∈ [0.001, 0.021]。
    let pts: Vec<(f64, f64)> = vec![
        (120.0, 0.001),
        (130.0, 0.002), // 与上同 duration bin（step 50 → [100,150)）、相邻 cost bin
        (500.0, 0.020),
        (980.0, 0.021), // duration max：clamp 进最后一个 bin
        (500.0, 0.020), // 重复点累计
    ];
    let h = build_histogram(&pts, 24, 16);
    assert_eq!(h.counts.len(), h.duration_bins.len() - 1);
    assert_eq!(h.counts[0].len(), h.cost_bins.len() - 1);
    // 总数守恒。
    let total: u64 = h.counts.iter().flat_map(|r| r.iter()).sum();
    assert_eq!(total, 5);
    // 逐点可复算：每点都能在自己的 (i,j) 里找到累计。
    for &(d, c) in &pts {
        let i = (((d - h.duration_bins[0]) / (h.duration_bins[1] - h.duration_bins[0])) as usize)
            .min(h.counts.len() - 1);
        let j = (((c - h.cost_bins[0]) / (h.cost_bins[1] - h.cost_bins[0])) as usize)
            .min(h.counts[0].len() - 1);
        assert!(h.counts[i][j] >= 1, "(d={d}, c={c}) 落空 bin ({i},{j})");
    }
    // 已知落点：(500,0.020) ×2 在同一格。
    let (i5, j5) = {
        let i = ((500.0 - h.duration_bins[0]) / (h.duration_bins[1] - h.duration_bins[0])) as usize;
        let j = ((0.020 - h.cost_bins[0]) / (h.cost_bins[1] - h.cost_bins[0])) as usize;
        ((i).min(h.counts.len() - 1), (j).min(h.counts[0].len() - 1))
    };
    assert_eq!(h.counts[i5][j5], 2);
}

#[test]
fn build_histogram_empty_points_empty_matrix() {
    let h = build_histogram(&[], 24, 16);
    assert!(h.duration_bins.is_empty());
    assert!(h.cost_bins.is_empty());
    assert!(h.counts.is_empty());
}

// ── 集成：proxy_log 真库（test_db） ─────────────────────────

fn q(start: i64, end: i64) -> ScatterHistogramQuery {
    ScatterHistogramQuery {
        start: Some(start),
        end: Some(end),
        filter_group: None,
        filter_model: None,
        filter_platform: None,
        filter_coding_plan: None,
    }
}

#[tokio::test]
async fn scatter_histogram_counts_rows_in_window() {
    let db = test_db().await;
    let now = chrono::Utc::now().timestamp_millis();
    // 两条窗内（duration/est_cost 各异）+ 一条窗外。
    let mut a = sample_log("sa", "g1", now);
    a.duration_ms = 100;
    a.est_cost = 0.01;
    insert_proxy_log_columns(&db, ProxyLogColumns::from_log(&a, false, false))
        .await
        .unwrap();
    let mut b = sample_log("sb", "g1", now);
    b.duration_ms = 900;
    b.est_cost = 0.05;
    insert_proxy_log_columns(&db, ProxyLogColumns::from_log(&b, false, false))
        .await
        .unwrap();
    let mut old = sample_log("sc", "g1", now - 10 * 86_400_000);
    old.duration_ms = 5000;
    old.est_cost = 9.9;
    insert_proxy_log_columns(&db, ProxyLogColumns::from_log(&old, false, false))
        .await
        .unwrap();

    let h = scatter_histogram(&db, &q(now - 3_600_000, now + 3_600_000))
        .await
        .unwrap();
    let total: u64 = h.counts.iter().flat_map(|r| r.iter()).sum();
    assert_eq!(total, 2, "窗内应恰好 2 条（窗外行不计数）");
    assert_eq!(h.counts.len(), h.duration_bins.len() - 1);
    assert!(!h.duration_bins.is_empty());
}

#[tokio::test]
async fn scatter_histogram_empty_window_returns_empty() {
    let db = test_db().await;
    let now = chrono::Utc::now().timestamp_millis();
    let mut a = sample_log("ea", "g1", now);
    a.duration_ms = 100;
    a.est_cost = 0.01;
    insert_proxy_log_columns(&db, ProxyLogColumns::from_log(&a, false, false))
        .await
        .unwrap();
    // 远古窗口：无命中行 → 全空数组，不报错。
    let h = scatter_histogram(&db, &q(1000, 2000)).await.unwrap();
    assert!(h.duration_bins.is_empty());
    assert!(h.cost_bins.is_empty());
    assert!(h.counts.is_empty());
}

#[tokio::test]
async fn scatter_histogram_filters_apply() {
    let db = test_db().await;
    let p = insert_test_platform(&db, "SP").await;
    let now = chrono::Utc::now().timestamp_millis();
    // P 平台 1 条 + 未知平台（platform_id=1 未注册）1 条。
    let mut a = sample_log("fa", "gA", now);
    a.platform_id = p;
    a.duration_ms = 100;
    a.est_cost = 0.01;
    insert_proxy_log_columns(&db, ProxyLogColumns::from_log(&a, false, false))
        .await
        .unwrap();
    let mut b = sample_log("fb", "gB", now);
    b.platform_id = 999; // 未注册平台：与 P 的 eff_pid 区分开
    b.duration_ms = 200;
    b.est_cost = 0.02;
    insert_proxy_log_columns(&db, ProxyLogColumns::from_log(&b, false, false))
        .await
        .unwrap();

    // filter_platform = P → 只剩 1 条。
    let mut qp = q(now - 3_600_000, now + 3_600_000);
    qp.filter_platform = Some(p.to_string());
    let h = scatter_histogram(&db, &qp).await.unwrap();
    let total: u64 = h.counts.iter().flat_map(|r| r.iter()).sum();
    assert_eq!(total, 1);

    // filter_group = gB → 只剩 b。
    let mut qg = q(now - 3_600_000, now + 3_600_000);
    qg.filter_group = Some("gB".into());
    let h = scatter_histogram(&db, &qg).await.unwrap();
    let total: u64 = h.counts.iter().flat_map(|r| r.iter()).sum();
    assert_eq!(total, 1);

    // filter_model = actual_model（sample_log actual_model = glm-4-plus）→ 命中 OR 分支。
    let mut qm = q(now - 3_600_000, now + 3_600_000);
    qm.filter_model = Some("glm-4-plus".into());
    let h = scatter_histogram(&db, &qm).await.unwrap();
    let total: u64 = h.counts.iter().flat_map(|r| r.iter()).sum();
    assert_eq!(total, 2);
}
