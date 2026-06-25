use super::*;
use crate::gateway::db::{self, now, Db};
use crate::gateway::estimate::{EstCodingPlan, EstTier};
use crate::gateway::models::*;
use crate::gateway::quota::{CodingPlanInfo, PlatformQuota, QuotaTier};

async fn mem_db() -> Db {
    let db = Db::new(":memory:").await.unwrap();
    db.init_tables().await.unwrap();
    db
}

async fn mk_platform(db: &Db, coding: bool) -> u64 {
    let p = db::create_platform(
        db,
        CreatePlatform {
            name: "p".into(),
            platform_type: if coding { Protocol::Kimi } else { Protocol::DeepSeek },
            base_url: "https://example.com".into(),
            api_key: "sk".into(),
            extra: String::new(),
            models: None,
            available_models: None,
            endpoints: None,
            manual_budgets: None,
            auto_group: None,
            join_group_ids: None, default_level_priority: None,
        },
    )
    .await
    .unwrap();
    p.id
}

// ── 余额原子自减 + cost 计算 ──
#[tokio::test]
async fn balance_atomic_decrement() {
    let db = mem_db().await;
    let id = mk_platform(&db, false).await;
    // 先设个初始余额
    write_real_quota(&db, id, 100.0, "", now()).await.unwrap();

    let cost = balance_cost(1000, 500, 200, 0.001, 0.002, 0.0005);
    assert!((cost - (1.0 + 1.0 + 0.1)).abs() < 1e-9, "cost = {cost}");

    apply_balance_delta(&db, id, cost).await.unwrap();
    let p = db::get_platform(&db, id).await.unwrap().unwrap();
    assert!((p.est_balance_remaining - (100.0 - cost)).abs() < 1e-9);
    assert_eq!(p.estimate_count, 1);

    // 再扣一次，验证累加自减 + count
    apply_balance_delta(&db, id, cost).await.unwrap();
    let p2 = db::get_platform(&db, id).await.unwrap().unwrap();
    assert!((p2.est_balance_remaining - (100.0 - 2.0 * cost)).abs() < 1e-9);
    assert_eq!(p2.estimate_count, 2);
}

// ── coding plan delta read-modify-write 持久化 ──
#[tokio::test]
async fn coding_plan_delta_persists() {
    let db = mem_db().await;
    let id = mk_platform(&db, true).await;
    // 初始化一个 Kimi tier（has_base）
    let plan = EstCodingPlan {
        tiers: vec![EstTier {
            name: "five_hour".into(),
            est_utilization: 0.0,
            coef_per_token: 0.0,
            util_at_last_real: 0.0,
            tokens_since_real: 0.0,
            has_base: true,
            limit: 10_000.0,
            window_start: 0,
        }],
        level: None,
    };
    write_real_quota(&db, id, 0.0, &plan.to_json(), now()).await.unwrap();

    apply_coding_plan_delta(&db, id, 1000.0).await.unwrap(); // +10%
    let p = db::get_platform(&db, id).await.unwrap().unwrap();
    let stored = EstCodingPlan::from_json(&p.est_coding_plan);
    assert!((stored.tiers[0].est_utilization - 10.0).abs() < 1e-9, "got {}", stored.tiers[0].est_utilization);
    assert_eq!(p.estimate_count, 1);
}

// ── 校准覆盖重置 count/time + Kimi 基数写入（端到端经 build_calibrated_coding_plan）──
#[tokio::test]
async fn calibration_overwrite_resets() {
    let db = mem_db().await;
    let id = mk_platform(&db, true).await;
    // 先制造预估次数
    write_real_quota(&db, id, 0.0, "", 0).await.unwrap();
    apply_balance_delta(&db, id, 1.0).await.unwrap();
    let before = db::get_platform(&db, id).await.unwrap().unwrap();
    assert_eq!(before.estimate_count, 1);

    // 模拟真查 coding plan（Kimi 带 limit）
    let quota = PlatformQuota {
        success: true,
        error: None,
        queried_at: now(),
        balance: None,
        coding_plan: Some(CodingPlanInfo {
            tiers: vec![QuotaTier {
                name: "five_hour".into(),
                utilization: 30.0,
                resets_at: None,
                limit: Some(10_000.0),
                remaining: Some(7_000.0),
            }],
            level: Some("pro".into()),
        }),
        newapi_user_id: None,
    };
    let prev = EstCodingPlan::from_json(&before.est_coding_plan);
    let calibrated = build_calibrated_coding_plan(&prev, &quota);
    write_real_quota(&db, id, 0.0, &calibrated.to_json(), now()).await.unwrap();

    let after = db::get_platform(&db, id).await.unwrap().unwrap();
    assert_eq!(after.estimate_count, 0, "校准应重置 count");
    assert!(after.last_real_query_at > 0);
    let stored = EstCodingPlan::from_json(&after.est_coding_plan);
    assert!(stored.tiers[0].has_base);
    assert!((stored.tiers[0].limit - 10_000.0).abs() < 1e-9);
    assert!((stored.tiers[0].est_utilization - 30.0).abs() < 1e-9);
}

// ── 真查校准入口 calibrate_from_quota：est 严格对齐真实 + 重置基线/计数（coding plan）──
#[tokio::test]
async fn calibrate_from_quota_aligns_coding_plan() {
    let db = mem_db().await;
    let id = mk_platform(&db, true).await;
    // 制造预估漂移：先初始化方案 B tier（非 has_base），再累积 token 让 est 偏离真值。
    let drift = EstCodingPlan {
        tiers: vec![EstTier {
            name: "five_hour".into(),
            est_utilization: 88.0, // 预估漂到 88%
            coef_per_token: 0.0001,
            util_at_last_real: 40.0,
            tokens_since_real: 480_000.0,
            has_base: false,
            limit: 0.0,
            window_start: 0,
        }],
        level: None,
    };
    write_real_quota(&db, id, 0.0, &drift.to_json(), 0).await.unwrap();
    apply_balance_delta(&db, id, 1.0).await.unwrap(); // count=1

    // 真查得 util_real=55%（GLM 方案 B，无 limit）
    let quota = PlatformQuota {
        success: true,
        error: None,
        queried_at: now(),
        balance: None,
        coding_plan: Some(CodingPlanInfo {
            tiers: vec![QuotaTier {
                name: "five_hour".into(),
                utilization: 55.0,
                resets_at: None,
                limit: None,
                remaining: None,
            }],
            level: Some("max".into()),
        }),
        newapi_user_id: None,
    };
    calibrate_from_quota(&db, id, &quota, true).await;

    let after = db::get_platform(&db, id).await.unwrap().unwrap();
    assert_eq!(after.estimate_count, 0, "校准重置 count");
    assert!(after.last_real_query_at > 0, "校准记 last_real_query_at");
    let stored = EstCodingPlan::from_json(&after.est_coding_plan);
    let t = &stored.tiers[0];
    // est 严格对齐真实（不被旧漂移 88% 残留）
    assert!((t.est_utilization - 55.0).abs() < 1e-9, "est 应=真实 55，got {}", t.est_utilization);
    assert!((t.util_at_last_real - 55.0).abs() < 1e-9, "基线应=真实");
    assert_eq!(t.tokens_since_real, 0.0, "累积应清零");
    // 拟合 coef = (55-40)/480000
    assert!((t.coef_per_token - (15.0 / 480_000.0)).abs() < 1e-12, "coef = {}", t.coef_per_token);
}

// ── calibrate_from_quota：余额平台 est_balance 严格对齐真实 ──
#[tokio::test]
async fn calibrate_from_quota_aligns_balance() {
    let db = mem_db().await;
    let id = mk_platform(&db, false).await;
    // 制造漂移：est 余额扣到很低
    write_real_quota(&db, id, 3.5, "", 0).await.unwrap();
    apply_balance_delta(&db, id, 1.0).await.unwrap();

    let quota = PlatformQuota {
        success: true,
        error: None,
        queried_at: now(),
        balance: Some(crate::gateway::quota::BalanceInfo {
            remaining: 99.9,
            total: None,
            used: None,
            currency: "USD".into(),
            is_valid: true,
        }),
        coding_plan: None,
        newapi_user_id: None,
    };
    calibrate_from_quota(&db, id, &quota, false).await;

    let after = db::get_platform(&db, id).await.unwrap().unwrap();
    assert!((after.est_balance_remaining - 99.9).abs() < 1e-9, "est_balance 应=真实 99.9，got {}", after.est_balance_remaining);
    assert_eq!(after.estimate_count, 0);
    assert!(after.last_real_query_at > 0);
}

// ── 真查失败不重置（保留预估）──
#[tokio::test]
async fn calibrate_from_quota_failure_preserves() {
    let db = mem_db().await;
    let id = mk_platform(&db, false).await;
    write_real_quota(&db, id, 50.0, "", 12345).await.unwrap();
    apply_balance_delta(&db, id, 1.0).await.unwrap();

    let quota = PlatformQuota {
        success: false,
        error: Some("boom".into()),
        queried_at: now(),
        balance: None,
        coding_plan: None,
        newapi_user_id: None,
    };
    calibrate_from_quota(&db, id, &quota, false).await;

    let after = db::get_platform(&db, id).await.unwrap().unwrap();
    // 不重置：count 保留、last_real_query_at 保留、est 不变
    assert_eq!(after.estimate_count, 1, "失败不应重置 count");
    assert_eq!(after.last_real_query_at, 12345, "失败不应改 last_real_query_at");
    assert!((after.est_balance_remaining - (50.0 - 1.0)).abs() < 1e-9);
}

#[tokio::test]
async fn read_estimate_state_returns_fields() {
    let db = mem_db().await;
    let id = mk_platform(&db, false).await;
    write_real_quota(&db, id, 10.0, "", 555).await.unwrap();
    let (last_real, count) = read_estimate_state(&db, id).await.unwrap();
    assert_eq!(last_real, 555);
    assert_eq!(count, 0);
}

#[test]
fn build_calibrated_coding_plan_none_returns_default() {
    let prev = EstCodingPlan::default();
    let quota = PlatformQuota {
        success: true,
        error: None,
        queried_at: now(),
        balance: None,
        coding_plan: None,
        newapi_user_id: None,
    };
    let r = build_calibrated_coding_plan(&prev, &quota);
    assert!(r.tiers.is_empty());
}

#[tokio::test]
async fn estimate_after_request_balance_path_no_calibration() {
    let db = mem_db().await;
    let id = mk_platform(&db, false).await;
    // last_real recent + count low → should_calibrate false → 不触发网络真查
    write_real_quota(&db, id, 100.0, "", now()).await.unwrap();
    estimate_after_request(
        &db,
        id,
        "deepseek",
        "https://example.com",
        "sk",
        "deepseek-chat",
        "",
        1000,
        500,
        0,
        false, // 非 coding plan
    )
    .await;
    let p = db::get_platform(&db, id).await.unwrap().unwrap();
    // balance 自减 + estimate_count 增加（resolve_price fallback 默认价 0 → cost 0，但 count 必增）
    assert!(p.estimate_count >= 1);
}

#[tokio::test]
async fn estimate_after_request_coding_path_no_calibration() {
    let db = mem_db().await;
    let id = mk_platform(&db, true).await;
    let plan = EstCodingPlan {
        tiers: vec![EstTier { name: "five_hour".into(), has_base: true, limit: 10_000.0, ..Default::default() }],
        level: None,
    };
    write_real_quota(&db, id, 0.0, &plan.to_json(), now()).await.unwrap();
    estimate_after_request(
        &db, id, "kimi", "https://example.com", "sk", "kimi-k2", "", 1000, 0, 0, true,
    )
    .await;
    let p = db::get_platform(&db, id).await.unwrap().unwrap();
    let stored = EstCodingPlan::from_json(&p.est_coding_plan);
    assert!(stored.tiers[0].est_utilization > 0.0);
}
