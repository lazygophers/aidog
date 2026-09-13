use super::*;
use aidog_db as db;
use aidog_db::{Db, now};
use aidog_stats::DbInitTables;
// TODO-unknown: self
use crate::gateway::estimate::{EstCodingPlan, EstTier};
use crate::gateway::models::*;
use crate::gateway::quota::{CodingPlanInfo, PlatformQuota, QuotaTier};

async fn mem_db() -> Db {
    let db = Db::new(":memory:").await.unwrap();
    db.init_tables().await.unwrap();
    db
}

/// 落一条 `model_entry`（计费价格真值源，票 T4）。`pd` 是整份模型 JSON。
async fn seed_entry(db: &Db, platform_code: &str, model_id: &str, pd: &str) {
    let e = ModelEntry {
        platform_code: platform_code.to_string(),
        model_id: model_id.to_string(),
        display_name: String::new(),
        canonical_model: model_id.to_string(),
        family: String::new(),
        version: String::new(),
        predecessor: String::new(),
        capabilities: vec![],
        builtin_tools_excluded: vec![],
        max_input_tokens: None,
        max_output_tokens: None,
        context_window: None,
        official: true,
        price_data: pd.to_string(),
        updated_at: 0,
    };
    db::upsert_model_entries(db, vec![e]).await.unwrap();
}

async fn mk_platform(db: &Db, coding: bool) -> u64 {
    let p = db::create_platform(
        db,
        CreatePlatform {
            name: "p".into(),
            platform_type: if coding {
                Protocol::Kimi
            } else {
                Protocol::DeepSeek
            },
            base_url: "https://example.com".into(),
            api_key: "sk".into(),
            extra: String::new(),
            models: None,
            available_models: None,
            endpoints: None,
            manual_budgets: None,
            auto_group: None,
            join_group_ids: None,
            expires_at: None,
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
            coef_per_request: 0.0,
            requests_since_real: 0.0,
            unit: String::new(),
        }],
        level: None,
    };
    write_real_quota(&db, id, 0.0, &plan.to_json(), now())
        .await
        .unwrap();

    apply_coding_plan_delta(&db, id, 0, 1000.0).await.unwrap(); // +10%
    let p = db::get_platform(&db, id).await.unwrap().unwrap();
    let stored = EstCodingPlan::from_json(&p.est_coding_plan);
    assert!(
        (stored.tiers[0].est_utilization - 10.0).abs() < 1e-9,
        "got {}",
        stored.tiers[0].est_utilization
    );
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
                unit: Some("prompt_count".into()),
            }],
            level: Some("pro".into()),
        }),
        newapi_user_id: None,
    };
    let prev = EstCodingPlan::from_json(&before.est_coding_plan);
    let calibrated = build_calibrated_coding_plan(&prev, &quota);
    write_real_quota(&db, id, 0.0, &calibrated.to_json(), now())
        .await
        .unwrap();

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
            coef_per_request: 0.0,
            requests_since_real: 0.0,
            unit: String::new(),
        }],
        level: None,
    };
    write_real_quota(&db, id, 0.0, &drift.to_json(), 0)
        .await
        .unwrap();
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
                unit: None,
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
    assert!(
        (t.est_utilization - 55.0).abs() < 1e-9,
        "est 应=真实 55，got {}",
        t.est_utilization
    );
    assert!((t.util_at_last_real - 55.0).abs() < 1e-9, "基线应=真实");
    assert_eq!(t.tokens_since_real, 0.0, "累积应清零");
    // 拟合 coef = (55-40)/480000
    assert!(
        (t.coef_per_token - (15.0 / 480_000.0)).abs() < 1e-12,
        "coef = {}",
        t.coef_per_token
    );

    // chart-engine T4 / #34：coding plan 无按量余额，不落 quota_snapshot（防 0 污染趋势）。
    let snaps = aidog_stats::quota_snapshots(
        &db,
        &QuotaSnapshotsQuery { start: None, end: None, platform_id: Some(id) },
    )
    .await
    .unwrap();
    assert!(snaps.is_empty());
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
    assert!(
        (after.est_balance_remaining - 99.9).abs() < 1e-9,
        "est_balance 应=真实 99.9，got {}",
        after.est_balance_remaining
    );
    assert_eq!(after.estimate_count, 0);
    assert!(after.last_real_query_at > 0);

    // chart-engine T4 / #34：真查成功顺手落一条 quota_snapshot（值 = 真实余额）。
    let snaps = aidog_stats::quota_snapshots(
        &db,
        &QuotaSnapshotsQuery { start: None, end: None, platform_id: Some(id) },
    )
    .await
    .unwrap();
    assert_eq!(snaps.len(), 1);
    assert!((snaps[0].est_balance_remaining - 99.9).abs() < 1e-9);
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
    assert_eq!(
        after.last_real_query_at, 12345,
        "失败不应改 last_real_query_at"
    );
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

// ── peak 倍率接入 estimate 链：命中窗口时扣减额 = 基准 x multiplier ──
#[tokio::test]
async fn estimate_after_request_applies_peak_multiplier() {
    let db = mem_db().await;
    let id = mk_platform(&db, false).await;
    write_real_quota(&db, id, 100.0, "", now()).await.unwrap();
    // 价格真值源是 model_entry（票 T4）：按 (platform_code=deepseek, model_id=test-model) 落条目。
    seed_entry(
        &db,
        "deepseek",
        "test-model",
        r#"{"price":{"input":0.001,"output":0.002,"cache_read":0.0}}"#,
    )
    .await;
    // 全天永久窗口（无 days_of_week/start_at 限制），x2 倍率，用户覆盖优先于 bundled preset。
    let extra = r#"{"peak":[{"start_hour":0,"end_hour":24,"multiplier":2.0}]}"#;

    estimate_after_request(
        &db,
        id,
        "deepseek",
        "https://example.com",
        "sk",
        "test-model",
        extra,
        1000,
        500,
        0,
        false,
    )
    .await;

    let p = db::get_platform(&db, id).await.unwrap().unwrap();
    // 基准 cost = 1000*0.001 + 500*0.002 = 2.0；命中 x2 倍率 → 实扣 4.0
    let base_cost = balance_cost(1000, 500, 0, 0.001, 0.002, 0.0);
    assert!((base_cost - 2.0).abs() < 1e-9, "base_cost = {base_cost}");
    assert!(
        (p.est_balance_remaining - (100.0 - 2.0 * base_cost)).abs() < 1e-9,
        "got {}",
        p.est_balance_remaining
    );
}

#[tokio::test]
async fn estimate_after_request_coding_path_no_calibration() {
    let db = mem_db().await;
    let id = mk_platform(&db, true).await;
    let plan = EstCodingPlan {
        tiers: vec![EstTier {
            name: "five_hour".into(),
            has_base: true,
            limit: 10_000.0,
            ..Default::default()
        }],
        level: None,
    };
    write_real_quota(&db, id, 0.0, &plan.to_json(), now())
        .await
        .unwrap();
    estimate_after_request(
        &db,
        id,
        "kimi",
        "https://example.com",
        "sk",
        "kimi-k2",
        "",
        1000,
        0,
        0,
        true,
    )
    .await;
    let p = db::get_platform(&db, id).await.unwrap().unwrap();
    let stored = EstCodingPlan::from_json(&p.est_coding_plan);
    assert!(stored.tiers[0].est_utilization > 0.0);
}

// ── 窗口重置自续定时：取最早的未来 resets_at，过去/临界/超 24h 的不排 ──
#[test]
fn earliest_reset_picks_nearest_future_tier() {
    let now = 1_700_000_000_000i64;
    let mk = |tiers: Vec<QuotaTier>| PlatformQuota {
        success: true,
        error: None,
        queried_at: now,
        balance: None,
        coding_plan: Some(CodingPlanInfo { tiers, level: None }),
        newapi_user_id: None,
    };
    let tier = |name: &str, resets_at: Option<i64>| QuotaTier {
        name: name.into(),
        utilization: 10.0,
        resets_at: resets_at.map(|ms| ms.to_string()),
        limit: None,
        remaining: None,
        unit: None,
    };

    // 两档都在未来 → 取最早
    let q = mk(vec![
        tier("weekly_limit", Some(now + 6 * 3_600_000)),
        tier("five_hour", Some(now + 3_600_000)),
    ]);
    assert_eq!(earliest_reset_ms(&q, now), Some(now + 3_600_000));

    // 过去 / 30s 内 / 超 24h 的都不排
    let q = mk(vec![
        tier("five_hour", Some(now - 1000)),
        tier("weekly_limit", Some(now + 10_000)),
    ]);
    assert_eq!(earliest_reset_ms(&q, now), None);
    let q = mk(vec![tier("weekly_limit", Some(now + 25 * 3_600_000))]);
    assert_eq!(earliest_reset_ms(&q, now), None);

    // 无 resets_at / 非 coding plan → None
    let q = mk(vec![tier("five_hour", None)]);
    assert_eq!(earliest_reset_ms(&q, now), None);
    let q = PlatformQuota {
        success: true,
        error: None,
        queried_at: now,
        balance: None,
        coding_plan: None,
        newapi_user_id: None,
    };
    assert_eq!(earliest_reset_ms(&q, now), None);
}

// ── 定时去重：同平台同时刻只排一次，到点后可再排；不同时刻各排各的 ──
#[test]
fn claim_refresh_slot_dedups_same_target() {
    let now = 1_700_000_000_000i64;
    let pid = 987_654u64; // 本测试专用 id，避免与其他测试共用全局表冲突
    assert!(claim_refresh_slot(pid, now + 3_600_000, now));
    assert!(!claim_refresh_slot(pid, now + 3_600_000, now), "同时刻不重复排");
    assert!(!claim_refresh_slot(pid, now + 3_630_000, now), "相差 <60s 视作同一次");
    assert!(claim_refresh_slot(pid, now + 7_200_000, now), "另一个时刻单独排");
    // 已排的时刻到点后（now 越过它）不再挡新的一次
    assert!(claim_refresh_slot(pid, now + 7_200_000, now + 7_200_001));
}
