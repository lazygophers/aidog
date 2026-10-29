//! 票 11 模拟器命令单测：platform / model_entry 属 split 表，一律 `three_file_db()`
//! （.claude/rules/db-pool-routing.md）。

use super::estimate_platform;
use aidog_db::test_support::three_file_db;

/// 插入最小平台行，返回 id（test_schema.rs 同款形状）。
async fn insert_platform(db: &aidog_db::Db, platform_type: String, extra: String) -> i64 {
    db.call_platform_traced(None, std::panic::Location::caller(), move |conn| {
        conn.execute(
            "INSERT INTO platform (name, platform_type, extra, endpoints, created_at, updated_at, deleted_at)
             VALUES ('sim-test', ?1, ?2, '[]', 0, 0, 0)",
            rusqlite::params![platform_type, extra],
        )?;
        Ok(conn.last_insert_rowid())
    })
    .await
    .expect("insert platform")
}

/// ① 正常计价：anthropic 平台 + 未登记模型 → fallback 默认价（3.0 $/M），非 0。
#[tokio::test]
async fn simulator_normal_fallback_pricing() {
    let (db, _dir) = three_file_db().await;
    let id = insert_platform(&db, "anthropic".into(), "{}".into()).await;
    let dto = estimate_platform(&db, id, "unknown-model", 1_000_000, 0, 0, 0, None)
        .await
        .expect("estimate");
    assert!((dto.est_cost - 3.0).abs() < 1e-9, "est_cost = {}", dto.est_cost);
    assert_eq!(dto.price_source, "fallback");
    assert!(!dto.is_peak);
    assert_eq!(dto.multiplier, 1.0);
}

/// ② 条目带 peak 绝对价 + 命中窗口：用绝对价计费且倍率压成 1.0（不叠平台倍率）。
#[tokio::test]
async fn simulator_peak_absolute_price_suppresses_multiplier() {
    let (db, _dir) = three_file_db().await;
    let id = insert_platform(&db, "deepseek".into(), r#"{"peak":[{"start_hour":0,"end_hour":24,"multiplier":3.0}]}"#.into())
        .await;
    let entry = aidog_db::model_entry_from_json(
        "deepseek",
        r#"{"model_id":"sim-model","price":{"input":1.0e-6,"peak":{"input":3.0e-6}}}"#,
    )
    .expect("entry parse");
    aidog_db::upsert_model_entries(&db, vec![entry])
        .await
        .expect("upsert entry");

    let dto = estimate_platform(&db, id, "sim-model", 1000, 0, 0, 0, Some(1_767_254_400_000))
        .await
        .expect("estimate");
    // 1000 × 3.0e-6 绝对价；窗口倍率 3.0 被压成 1.0，不二次放大
    assert!((dto.est_cost - 0.003).abs() < 1e-12, "est_cost = {}", dto.est_cost);
    assert!(dto.is_peak, "hit window → is_peak");
    assert_eq!(dto.multiplier, 1.0, "peak absolute price suppresses multiplier");
    assert_ne!(dto.price_source, "unsupported");
}

/// ③ 透传平台：est_cost=0 + price_source="unsupported"，tokens 多少都不计价。
#[tokio::test]
async fn simulator_passthrough_unsupported() {
    let (db, _dir) = three_file_db().await;
    let id = insert_platform(&db, "passthrough".into(), "{}".into()).await;
    let dto = estimate_platform(&db, id, "any-model", 100_000, 50_000, 10_000, 5_000, None)
        .await
        .expect("estimate");
    assert_eq!(dto.est_cost, 0.0);
    assert_eq!(dto.price_source, "unsupported");
    assert!(!dto.is_peak);
    assert_eq!(dto.multiplier, 1.0);
}

/// ④ CNY 条目：模拟器与 calc_est_cost 同走 resolve_price，est_cost = 原价 / CNY_PER_USD × token，
/// 两条路径结果一致；USD 对照条目不换算。
#[tokio::test]
async fn simulator_cny_entry_converted_and_matches_calc_est_cost() {
    use crate::gateway::billing::{calc_est_cost, CNY_PER_USD};
    let (db, _dir) = three_file_db().await;
    let id = insert_platform(&db, "deepseek".into(), "{}".into()).await;
    for (model, cur) in [("cny-model", "CNY"), ("usd-model", "USD")] {
        let entry = aidog_db::model_entry_from_json(
            "deepseek",
            &format!(r#"{{"model_id":"{model}","price":{{"currency":"{cur}","input":6.6675e-6,"output":1.3335e-5}}}}"#),
        )
        .expect("entry parse");
        aidog_db::upsert_model_entries(&db, vec![entry]).await.expect("upsert");
    }
    let sim = estimate_platform(&db, id, "cny-model", 1000, 1000, 0, 0, None).await.expect("sim");
    let want = (1000.0 * 6.6675e-6 + 1000.0 * 1.3335e-5) / CNY_PER_USD;
    assert!((sim.est_cost - want).abs() < 1e-12, "sim = {}", sim.est_cost);
    let real = calc_est_cost(&db, "cny-model", "deepseek", "{}", 1000, 1000, 0, 0, 0, id, aidog_db::now(), &[]).await;
    assert!((real - sim.est_cost).abs() < 1e-12, "calc = {real}, sim = {}", sim.est_cost);
    let usd = estimate_platform(&db, id, "usd-model", 1000, 1000, 0, 0, None).await.expect("sim");
    assert!((usd.est_cost - 1000.0 * 6.6675e-6 - 1000.0 * 1.3335e-5).abs() < 1e-12);
}
