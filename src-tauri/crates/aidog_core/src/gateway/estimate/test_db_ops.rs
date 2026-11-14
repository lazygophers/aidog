use super::*;
use aidog_db as db;
use aidog_db::{Db, now};
use aidog_stats::DbInitTables;
// TODO-unknown: self
use crate::gateway::estimate::{EstCodingPlan, EstTier, UnifiedTier};
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
            quota_source: None,
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

    let cost = balance_cost(1000, 500, 200, 0, 0.001, 0.002, 0.0005, 0.0);
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
        &QuotaSnapshotsQuery {
            start: None,
            end: None,
            platform_id: Some(id),
        },
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
        &QuotaSnapshotsQuery {
            start: None,
            end: None,
            platform_id: Some(id),
        },
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
        0,     // cache_write
        false, // 非 coding plan
        0,     // created_at
        0.0,
        0,
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
        0,
        false,
        0,
        0.0,
        0,
    )
    .await;

    let p = db::get_platform(&db, id).await.unwrap().unwrap();
    // 基准 cost = 1000*0.001 + 500*0.002 = 2.0；命中 x2 倍率 → 实扣 4.0
    let base_cost = balance_cost(1000, 500, 0, 0, 0.001, 0.002, 0.0, 0.0);
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
        0,
        true,
        0,
        0.0,
        0,
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
    assert!(
        !claim_refresh_slot(pid, now + 3_600_000, now),
        "同时刻不重复排"
    );
    assert!(
        !claim_refresh_slot(pid, now + 3_630_000, now),
        "相差 <60s 视作同一次"
    );
    assert!(
        claim_refresh_slot(pid, now + 7_200_000, now),
        "另一个时刻单独排"
    );
    // 已排的时刻到点后（now 越过它）不再挡新的一次
    assert!(claim_refresh_slot(pid, now + 7_200_000, now + 7_200_001));
}

// ── 校准尝试退避：失败后 CALIBRATE_INTERVAL_MS 内不再每请求真查，成功清除后立即可再试 ──
#[test]
fn claim_calibration_attempt_backs_off_until_cleared() {
    let now = 1_700_000_000_000i64;
    let pid = 987_655u64; // 本测试专用 id，避免与其他测试共用全局表冲突
    let interval = super::super::model::CALIBRATE_INTERVAL_MS;
    assert!(claim_calibration_attempt(pid, now), "首次尝试放行");
    assert!(
        !claim_calibration_attempt(pid, now + 1_000),
        "进行中/刚失败退避"
    );
    assert!(
        claim_calibration_attempt(pid, now + interval),
        "退避期满再放行"
    );
    clear_calibration_attempt(pid);
    assert!(
        claim_calibration_attempt(pid, now + interval + 1),
        "成功清除后立即放行"
    );
}

// ── 通知体系票 01：预测频控（每窗口每档 1 次）───────────────────────────────

#[test]
fn forecast_rate_gate_once_per_window() {
    // 直接测同模块私有频控表：同 (platform, tier, window) 第二次 false，换窗口 true。
    // FORECAST_SENT 是进程级 static，用独立 key 避免与其它测试互踩。
    let pid = 987_654_321u64;
    assert!(super::claim_forecast_window(pid, "five_hour", 1000));
    assert!(!super::claim_forecast_window(pid, "five_hour", 1000));
    assert!(super::claim_forecast_window(pid, "five_hour", 2000)); // 新窗口
    assert!(super::claim_forecast_window(pid, "weekly_limit", 1000)); // 新档
}

// ── 余额币种换算 + 按次费入余额 ──
fn quota_with_ccy(remaining: f64, ccy: &str) -> PlatformQuota {
    PlatformQuota {
        success: true,
        error: None,
        queried_at: now(),
        balance: Some(crate::gateway::quota::BalanceInfo {
            remaining,
            total: None,
            used: None,
            currency: ccy.into(),
            is_valid: true,
        }),
        coding_plan: None,
        newapi_user_id: None,
    }
}

/// 1000 in + 500 out；deepseek-ccy 条目 input 0.001 / output 0.002 CNY/token（currency=CNY → 出口 USD）。
async fn run_est(db: &Db, id: u64, tool_usd: f64, in_tok: i64, out_tok: i64) {
    estimate_after_request(
        db,
        id,
        "deepseek",
        "https://example.com",
        "sk",
        "ccy-model",
        "",
        in_tok,
        out_tok,
        0,
        0,
        false,
        0,
        tool_usd,
        0,
    )
    .await;
}

async fn seed_cny_model(db: &Db) {
    seed_entry(
        db,
        "deepseek",
        "ccy-model",
        r#"{"price":{"input":0.001,"output":0.002,"currency":"CNY"}}"#,
    )
    .await;
}

#[tokio::test]
async fn balance_cny_platform_cny_model_matches_pre_fx_behaviour() {
    let db = mem_db().await;
    let id = mk_platform(&db, false).await;
    seed_cny_model(&db).await;
    write_real_quota(&db, id, 100.0, "", now()).await.unwrap();
    calibrate_from_quota(&db, id, &quota_with_ccy(100.0, "CNY"), false).await;
    run_est(&db, id, 0.0, 1000, 500).await;
    let p = db::get_platform(&db, id).await.unwrap().unwrap();
    // 换算前：CNY 价直接扣 CNY 余额 = 1000*0.001 + 500*0.002 = 2.0
    assert!(
        (p.est_balance_remaining - 98.0).abs() < 1e-6,
        "got {}",
        p.est_balance_remaining
    );
}

#[tokio::test]
async fn balance_usd_platform_cny_model_deducts_usd() {
    let db = mem_db().await;
    let id = mk_platform(&db, false).await;
    seed_cny_model(&db).await;
    calibrate_from_quota(&db, id, &quota_with_ccy(100.0, "USD"), false).await;
    run_est(&db, id, 0.0, 1000, 500).await;
    let p = db::get_platform(&db, id).await.unwrap().unwrap();
    let usd = 2.0 / aidog_db::CNY_PER_USD;
    assert!(
        (p.est_balance_remaining - (100.0 - usd)).abs() < 1e-6,
        "got {}",
        p.est_balance_remaining
    );
}

#[tokio::test]
async fn balance_missing_currency_cny_model_falls_back_to_cny() {
    let db = mem_db().await;
    let id = mk_platform(&db, false).await;
    seed_cny_model(&db).await;
    write_real_quota(&db, id, 100.0, "", now()).await.unwrap(); // 从未真查 → 无币种记录 → 回落模型条目币种 CNY，扣 2.0（等于换算前行为）
    run_est(&db, id, 0.0, 1000, 500).await;
    let p = db::get_platform(&db, id).await.unwrap().unwrap();
    assert!(
        (p.est_balance_remaining - 98.0).abs() < 1e-6,
        "got {}",
        p.est_balance_remaining
    );
}

#[tokio::test]
async fn balance_missing_currency_usd_model_deducts_usd() {
    let db = mem_db().await;
    let id = mk_platform(&db, false).await;
    seed_entry(
        &db,
        "deepseek",
        "ccy-model",
        r#"{"price":{"input":0.001,"output":0.002}}"#,
    )
    .await;
    write_real_quota(&db, id, 100.0, "", now()).await.unwrap();
    run_est(&db, id, 0.0, 1000, 500).await;
    let p = db::get_platform(&db, id).await.unwrap().unwrap();
    assert!(
        (p.est_balance_remaining - 98.0).abs() < 1e-6,
        "got {}",
        p.est_balance_remaining
    );
}

#[tokio::test]
async fn tool_fee_deducted_with_zero_tokens_and_converted_for_cny_balance() {
    let db = mem_db().await;
    let id = mk_platform(&db, false).await;
    calibrate_from_quota(&db, id, &quota_with_ccy(100.0, "CNY"), false).await;
    // token 全 0 + 按次费 0.5 USD：不早退，CNY 余额扣 0.5 × 6.6675
    run_est(&db, id, 0.5, 0, 0).await;
    let p = db::get_platform(&db, id).await.unwrap().unwrap();
    let want = 100.0 - 0.5 * aidog_db::CNY_PER_USD;
    assert!(
        (p.est_balance_remaining - want).abs() < 1e-6,
        "got {}",
        p.est_balance_remaining
    );
    assert!(p.estimate_count >= 1);
}

#[tokio::test]
async fn tool_fee_not_multiplied_by_peak() {
    let db = mem_db().await;
    let id = mk_platform(&db, false).await;
    calibrate_from_quota(&db, id, &quota_with_ccy(100.0, "USD"), false).await;
    let extra = r#"{"peak":[{"start_hour":0,"end_hour":24,"multiplier":2.0}]}"#;
    estimate_after_request(
        &db,
        id,
        "deepseek",
        "https://example.com",
        "sk",
        "ccy-model",
        extra,
        0,
        0,
        0,
        0,
        false,
        0,
        0.5,
        0,
    )
    .await;
    let p = db::get_platform(&db, id).await.unwrap().unwrap();
    assert!(
        (p.est_balance_remaining - 99.5).abs() < 1e-9,
        "got {}",
        p.est_balance_remaining
    );
}

// ── issue #61：按次计价（unit:request）入余额与手动预算，与 est_cost 同口径 ──
#[tokio::test]
async fn per_request_fee_deducted_from_balance_and_manual_budget() {
    let db = mem_db().await;
    let id = mk_platform(&db, false).await;
    seed_entry(
        &db,
        "deepseek",
        "unit-model",
        r#"{"price":{"unit":"request","unit_price":0.002}}"#,
    )
    .await;
    // 手动预算：usd 总额 10（无预算时 apply_manual_budgets no-op，必须先有预算）
    db.platform_write_conn()
        .call(move |conn| {
            conn.execute(
                "UPDATE platform SET manual_budgets = ?1 WHERE id = ?2",
                rusqlite::params![
                    r#"[{"id":"b1","kind":"total","unit":"usd","amount":10.0,"enabled":true}]"#,
                    id as i64
                ],
            )?;
            Ok::<(), rusqlite::Error>(())
        })
        .await
        .unwrap();
    write_real_quota(&db, id, 100.0, "", now()).await.unwrap();
    estimate_after_request(
        &db,
        id,
        "deepseek",
        "https://example.com",
        "sk",
        "unit-model",
        "",
        0, // token 全 0：纯按次请求不得被「无 token 跳过」丢掉
        0,
        0,
        0,
        false,
        0,
        0.0,
        1, // request_units
    )
    .await;
    let p = db::get_platform(&db, id).await.unwrap().unwrap();
    assert!(p.estimate_count >= 1, "纯按次请求必须进入预估链");
    assert!(
        (p.est_balance_remaining - 99.998).abs() < 1e-12,
        "got {}",
        p.est_balance_remaining
    );
    let budgets = p.manual_budgets;
    assert!(
        (budgets[0].consumed - 0.002).abs() < 1e-12,
        "got {}",
        budgets[0].consumed
    );
    // 与日志 est_cost 同口径：同 (platform, model, tokens) 解析 = unit_price × 1 次
    let est = crate::gateway::billing::calc_est_cost_detail(
        &db,
        "unit-model",
        "deepseek",
        "",
        0,
        0,
        0,
        0,
        0,
        id as i64,
        0,
        &[(crate::gateway::billing::SEARCH_UNITS_KEY.to_string(), 1)],
    )
    .await
    .est_cost;
    assert!((est - 0.002).abs() < 1e-12, "got {est}");
}

// ── apply_quota_verdict：真查判定写入调度器额度冷却 ──
mod verdict_apply {
    use super::*;
    use crate::gateway::estimate::algo::{QUOTA_OPEN_ENDED_UNTIL_MS, QuotaVerdict};
    use crate::gateway::scheduling::SchedulerState;

    const NOW: i64 = 1_800_000_000_000;
    const FAR: i64 = NOW + 365 * 86_400_000;

    #[test]
    fn open_ended_holds_until_available_clears() {
        let s = SchedulerState::new();
        apply_quota_verdict(
            &s,
            1,
            QuotaVerdict::Exhausted {
                until_ms: QUOTA_OPEN_ENDED_UNTIL_MS,
            },
        );
        assert!(s.quota_cooled(1, NOW));
        assert!(
            s.quota_cooled(1, FAR),
            "open-ended must outlast any fixed window"
        );
        apply_quota_verdict(&s, 1, QuotaVerdict::Available);
        assert!(!s.quota_cooled(1, NOW));
    }

    /// spec 第 17 条：真查判定与 429 冷却分开记，真查 Available 不清 429 冷却。
    #[test]
    fn verdict_available_keeps_429_cooldown() {
        let s = SchedulerState::new();
        s.set_quota_cooldown(8, NOW + 60_000);
        apply_quota_verdict(&s, 8, QuotaVerdict::Available);
        assert!(s.quota_cooled(8, NOW), "429 冷却不被真查 Available 清掉");
        assert_eq!(s.quota_cooldown_until_ms(8), NOW + 60_000);
    }

    /// 真查 Exhausted 给更近的截止，不缩短更长的 429 冷却；两者取最大。
    #[test]
    fn verdict_exhausted_does_not_shorten_429_cooldown() {
        let s = SchedulerState::new();
        s.set_quota_cooldown(2, FAR);
        apply_quota_verdict(
            &s,
            2,
            QuotaVerdict::Exhausted {
                until_ms: NOW + 60_000,
            },
        );
        assert!(s.quota_cooled(2, NOW + 60_000), "更长的 429 冷却仍在");
        assert_eq!(s.quota_cooldown_until_ms(2), FAR);
    }

    /// 真查 Available 解除自己那份后，仅剩判定通道的冷却被清。
    #[test]
    fn verdict_available_clears_verdict_channel() {
        let s = SchedulerState::new();
        apply_quota_verdict(
            &s,
            9,
            QuotaVerdict::Exhausted {
                until_ms: NOW + 60_000,
            },
        );
        assert!(s.quota_cooled(9, NOW));
        apply_quota_verdict(&s, 9, QuotaVerdict::Available);
        assert!(!s.quota_cooled(9, NOW));
    }

    /// 两条通道各自写穿落库、重启（seed）后都恢复。
    #[test]
    fn both_channels_survive_restart_seed() {
        let s = SchedulerState::new();
        let row = aidog_db::health_state::PlatformHealthStateRow {
            platform_id: 6,
            quota_cooldown_until_ms: NOW + 1000,
            quota_verdict_until_ms: QUOTA_OPEN_ENDED_UNTIL_MS,
            ..Default::default()
        };
        s.seed_from_rows(vec![row], NOW);
        assert!(s.quota_cooled(6, FAR), "判定通道（开放式）恢复");
        let s2 = SchedulerState::new();
        s2.seed_from_rows(
            vec![aidog_db::health_state::PlatformHealthStateRow {
                platform_id: 6,
                quota_cooldown_until_ms: FAR,
                ..Default::default()
            }],
            NOW,
        );
        assert_eq!(s2.quota_cooldown_until_ms(6), FAR, "429 通道恢复");
    }

    /// 写后重开读回：真查判定经 persist 落到 platform.db 的独立列。
    /// 写池多连接无顺序保证，两步写入之间用读回确认落库后再做下一步。
    #[tokio::test]
    async fn verdict_persists_to_own_column() {
        let (db, _dir) = aidog_db::test_support::three_file_db().await;
        let db = std::sync::Arc::new(db);
        let s = SchedulerState::with_db(db.clone());
        s.set_quota_cooldown(11, NOW + 5000);
        let row11 = || async {
            aidog_db::health_state::load_platform_health_states(&db)
                .await
                .unwrap()
                .into_iter()
                .find(|r| r.platform_id == 11)
        };
        let mut landed = false;
        for _ in 0..40 {
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            if row11().await.is_some() {
                landed = true;
                break;
            }
        }
        assert!(landed, "429 冷却应先落库");
        apply_quota_verdict(
            &s,
            11,
            QuotaVerdict::Exhausted {
                until_ms: NOW + 9000,
            },
        );
        let mut got = None;
        for _ in 0..40 {
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            if let Some(r) = row11().await
                && r.quota_verdict_until_ms == NOW + 9000
            {
                got = Some(r);
                break;
            }
        }
        let r = got.expect("判定列应落库");
        assert_eq!(r.quota_cooldown_until_ms, NOW + 5000, "429 列不被判定覆盖");
    }

    #[test]
    fn verdict_channel_overrides_itself() {
        let s = SchedulerState::new();
        // 开放式 → 之后得到明确重置时刻：改成固定截止
        apply_quota_verdict(
            &s,
            3,
            QuotaVerdict::Exhausted {
                until_ms: QUOTA_OPEN_ENDED_UNTIL_MS,
            },
        );
        apply_quota_verdict(
            &s,
            3,
            QuotaVerdict::Exhausted {
                until_ms: NOW + 1000,
            },
        );
        assert!(!s.quota_cooled(3, NOW + 1000));
    }

    #[test]
    fn unknown_leaves_cooldown_untouched() {
        let s = SchedulerState::new();
        s.set_quota_cooldown(4, NOW + 5000);
        apply_quota_verdict(&s, 4, QuotaVerdict::Unknown);
        assert!(s.quota_cooled(4, NOW));
    }

    /// 手动刷新 / 自动校准同走 calibrate_from_quota：用完 → 写冷却；估算外推不参与；恢复 → 解除。
    #[tokio::test]
    async fn calibrate_from_quota_drives_global_scheduler() {
        use crate::gateway::quota::{CodingPlanInfo, QuotaTier};
        let db = mem_db().await;
        let id = mk_platform(&db, true).await;
        let sched = std::sync::Arc::new(SchedulerState::new());
        let mk = |util: f64| PlatformQuota {
            success: true,
            error: None,
            queried_at: now(),
            balance: None,
            coding_plan: Some(CodingPlanInfo {
                tiers: vec![QuotaTier {
                    name: "five_hour".into(),
                    utilization: util,
                    resets_at: None,
                    limit: None,
                    remaining: None,
                    unit: None,
                }],
                level: None,
            }),
            newapi_user_id: None,
        };
        calibrate_from_quota_with(&db, id, &mk(100.0), true, Some(sched.clone())).await;
        assert!(sched.quota_cooled(id, now() + 86_400_000 * 365));
        calibrate_from_quota_with(&db, id, &mk(20.0), true, Some(sched.clone())).await;
        assert!(!sched.quota_cooled(id, now()));
    }

    /// 开放式冷却经 platform_health_state 行回灌后仍在（重启保持）。
    #[test]
    fn open_ended_survives_restart_seed() {
        let s = SchedulerState::new();
        let row = aidog_db::health_state::PlatformHealthStateRow {
            platform_id: 5,
            breaker_state: "closed".into(),
            breaker_until_ms: 0,
            quota_cooldown_until_ms: QUOTA_OPEN_ENDED_UNTIL_MS,
            quota_verdict_until_ms: 0,
            auth_cooldown_until_ms: 0,
            balance_cooldown_until_ms: 0,
            last_connect_fail_ms: 0,
            updated_at: NOW,
            breaker_fails: 0,
            breaker_last_fail_ms: 0,
        };
        s.seed_from_rows(vec![row], NOW);
        assert!(s.quota_cooled(5, FAR));
    }
}

// ── 响应头订阅窗口用量 → est_coding_plan（卡片读的同一存储）──
fn ut(name: &'static str, pct: f64, reset_ms: Option<i64>) -> UnifiedTier {
    UnifiedTier { name, utilization_pct: pct, resets_at_ms: reset_ms }
}

#[tokio::test]
async fn unified_tiers_written_and_idempotent() {
    let (db, _dir) = aidog_db::test_support::three_file_db().await;
    let id = mk_platform(&db, false).await;
    let reset5 = now() + 3_600_000;
    let reset7 = now() + 86_400_000;
    let tiers = [ut("five_hour", 42.0, Some(reset5)), ut("weekly_limit", 7.0, Some(reset7))];
    assert!(write_unified_tiers(&db, id, &tiers, now()).await, "首次必写");
    // 写后读回（platform.db）
    let p = db::get_platform(&db, id).await.unwrap().unwrap();
    let plan = EstCodingPlan::from_json(&p.est_coding_plan);
    assert_eq!(plan.tiers.len(), 2);
    let five = plan.tiers.iter().find(|t| t.name == "five_hour").unwrap();
    assert!((five.est_utilization - 42.0).abs() < 1e-9);
    assert_eq!(five.window_start, reset5 - 5 * 3_600_000, "window_start = reset - 周期");
    let wk = plan.tiers.iter().find(|t| t.name == "weekly_limit").unwrap();
    assert!((wk.est_utilization - 7.0).abs() < 1e-9);
    // 同值再写 → 不写库（节流）
    assert!(!write_unified_tiers(&db, id, &tiers, now()).await, "值未变不得再写");
    // 值变 → 写，且另一窗口不动
    assert!(write_unified_tiers(&db, id, &[ut("five_hour", 50.0, Some(reset5))], now()).await);
    let plan = EstCodingPlan::from_json(&db::get_platform(&db, id).await.unwrap().unwrap().est_coding_plan);
    assert!((plan.tiers.iter().find(|t| t.name == "five_hour").unwrap().est_utilization - 50.0).abs() < 1e-9);
    assert!((plan.tiers.iter().find(|t| t.name == "weekly_limit").unwrap().est_utilization - 7.0).abs() < 1e-9);
}

#[tokio::test]
async fn unified_tiers_empty_is_noop() {
    let (db, _dir) = aidog_db::test_support::three_file_db().await;
    let id = mk_platform(&db, false).await;
    assert!(!write_unified_tiers(&db, id, &[], now()).await);
    let p = db::get_platform(&db, id).await.unwrap().unwrap();
    assert!(p.est_coding_plan.is_empty() || EstCodingPlan::from_json(&p.est_coding_plan).tiers.is_empty());
}
