use super::*;
use crate::gateway::db::now;

// ── Kimi 精确增量：每 token % = 100/limit ──
#[test]
fn kimi_precise_increment() {
    let mut tier = EstTier {
        name: "five_hour".into(),
        est_utilization: 40.0,
        coef_per_token: 0.0,
        util_at_last_real: 40.0,
        tokens_since_real: 0.0,
        has_base: true,
        limit: 10_000.0,
        window_start: 0,
    };
    apply_tier_delta(&mut tier, 1000.0); // 1000 × (100/10000) = 10%
    assert!((tier.est_utilization - 50.0).abs() < 1e-9, "got {}", tier.est_utilization);
    // clamp 到 100
    apply_tier_delta(&mut tier, 100_000.0);
    assert!((tier.est_utilization - 100.0).abs() < 1e-9);
}

// ── 方案 B 拟合后增量 ──
#[test]
fn fitted_increment_with_coef() {
    let mut tier = EstTier {
        name: "five_hour".into(),
        est_utilization: 40.0,
        coef_per_token: 0.0001, // 每 token 0.0001%
        util_at_last_real: 40.0,
        tokens_since_real: 0.0,
        has_base: false,
        limit: 0.0,
        window_start: 0,
    };
    apply_tier_delta(&mut tier, 50_000.0); // 40 + 50000×0.0001 = 45
    assert!((tier.est_utilization - 45.0).abs() < 1e-9, "got {}", tier.est_utilization);
    assert!((tier.tokens_since_real - 50_000.0).abs() < 1e-9);
}

// ── 冷启动（无 coef）不预估，只累计 tokens ──
#[test]
fn cold_start_no_estimate() {
    let mut tier = EstTier {
        name: "five_hour".into(),
        est_utilization: 40.0,
        coef_per_token: 0.0, // 冷启动
        util_at_last_real: 40.0,
        tokens_since_real: 0.0,
        has_base: false,
        limit: 0.0,
        window_start: 0,
    };
    apply_tier_delta(&mut tier, 50_000.0);
    assert!((tier.est_utilization - 40.0).abs() < 1e-9, "冷启动不应预估，got {}", tier.est_utilization);
    // 但 tokens 仍累计，供下次真查拟合
    assert!((tier.tokens_since_real - 50_000.0).abs() < 1e-9);
}

// ── 方案 B 真查拟合 coef ──
#[test]
fn calibrate_fits_coef() {
    let prev = EstTier {
        name: "five_hour".into(),
        est_utilization: 45.0,
        coef_per_token: 0.0,
        util_at_last_real: 40.0,
        tokens_since_real: 50_000.0,
        has_base: false,
        limit: 0.0,
        window_start: 0,
    };
    // 真查得 util_real = 50% → coef = (50-40)/50000 = 0.0002
    let cal = calibrate_tier(&prev, "five_hour", 50.0, false, None, None, now());
    assert!((cal.coef_per_token - 0.0002).abs() < 1e-12, "coef = {}", cal.coef_per_token);
    assert!((cal.est_utilization - 50.0).abs() < 1e-9);
    assert!((cal.util_at_last_real - 50.0).abs() < 1e-9);
    assert_eq!(cal.tokens_since_real, 0.0);
}

// ── reset 检测：util_real < util_at_last_real → 丢样本，coef 保留 ──
#[test]
fn calibrate_reset_discards_sample() {
    let prev = EstTier {
        name: "five_hour".into(),
        est_utilization: 90.0,
        coef_per_token: 0.0003, // 上一窗口已拟合
        util_at_last_real: 80.0,
        tokens_since_real: 30_000.0,
        has_base: false,
        limit: 0.0,
        window_start: 0,
    };
    // 窗口 reset，真值跌到 5%（< 80）→ 丢弃本窗口样本，coef 保留旧值
    let cal = calibrate_tier(&prev, "five_hour", 5.0, false, None, None, now());
    assert!((cal.coef_per_token - 0.0003).abs() < 1e-12, "reset 应保留旧 coef，got {}", cal.coef_per_token);
    assert!((cal.est_utilization - 5.0).abs() < 1e-9);
    assert!((cal.util_at_last_real - 5.0).abs() < 1e-9);
    assert_eq!(cal.tokens_since_real, 0.0);
}

// ── Kimi 校准记 limit ──
#[test]
fn calibrate_kimi_records_base() {
    let prev = EstTier::default();
    let cal = calibrate_tier(&prev, "five_hour", 30.0, true, Some(20_000.0), None, now());
    assert!(cal.has_base);
    assert!((cal.limit - 20_000.0).abs() < 1e-9);
    assert!((cal.est_utilization - 30.0).abs() < 1e-9);
}

// ── 校准阈值触发 ──
#[test]
fn calibrate_thresholds() {
    let now_ms = 1_000_000_000_000;
    // 时间未到 + 次数未到 → 不校准
    assert!(!should_calibrate(now_ms, now_ms - 100, 50));
    // 时间超 5min → 校准
    assert!(should_calibrate(now_ms, now_ms - 300_001, 0));
    // 次数 >= 100 → 校准
    assert!(should_calibrate(now_ms, now_ms, 100));
    assert!(should_calibrate(now_ms, now_ms, 150));
}
