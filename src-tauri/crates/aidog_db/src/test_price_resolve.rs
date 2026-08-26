#![cfg(test)]
//! 计费解析（票 T4）：分档纯函数 + `(platform_code, model_id)` 四路径 + 高峰绝对价。
use super::*;
use crate::models::{ModelEntry, ResolvedPrice};
use crate::test_support::test_db;

fn base_price(source: &str) -> ResolvedPrice {
    ResolvedPrice {
        input_cost_per_token: 5e-6,
        output_cost_per_token: 3e-5,
        cache_read_input_token_cost: 5e-7,
        source: source.to_string(),
    }
}

/// 落一条 model_entry：`pd` 既是 price_data 也提供 max_output_tokens 列。
async fn put_entry(db: &Db, platform: &str, model: &str, pd: &serde_json::Value, max_out: Option<i64>) {
    upsert_model_entries(
        db,
        vec![ModelEntry {
            platform_code: platform.to_string(),
            model_id: model.to_string(),
            display_name: String::new(),
            canonical_model: model.to_string(),
            family: String::new(),
            version: String::new(),
            predecessor: String::new(),
            capabilities: vec![],
            builtin_tools_excluded: vec![],
            max_input_tokens: None,
            max_output_tokens: max_out,
            context_window: None,
            official: true,
            price_data: pd.to_string(),
            updated_at: 0,
        }],
    )
    .await
    .unwrap();
}

// ── 分档纯函数（从 model_price.rs 迁来，行为逐字不变）──

#[test]
fn apply_context_tier_selects_long_tier() {
    // OpenAI gpt-5.5: short in=5e-6/out=3e-5/cache=5e-7, long@272000 in=1e-5/out=4.5e-5/cache=1e-6
    let pd = serde_json::json!({
        "input_cost_per_token": 5e-6,
        "output_cost_per_token": 3e-5,
        "cache_read_input_token_cost": 5e-7,
        "context_tiers": [{
            "min_tokens": 272000,
            "input_cost_per_token": 1e-5,
            "output_cost_per_token": 4.5e-5,
            "cache_read_input_token_cost": 1e-6
        }]
    });
    // 短档: input < 272000 → base 不变 (无 +tier 后缀)
    let short = apply_context_tier(base_price("model_entry"), &pd, 100_000);
    assert_eq!(short.input_cost_per_token, 5e-6);
    assert_eq!(short.source, "model_entry");
    // 长档: input >= 272000 → tier 覆盖
    let long = apply_context_tier(base_price("model_entry"), &pd, 300_000);
    assert_eq!(long.input_cost_per_token, 1e-5);
    assert_eq!(long.output_cost_per_token, 4.5e-5);
    assert_eq!(long.cache_read_input_token_cost, 1e-6);
    assert_eq!(long.source, "model_entry+tier");
    // 边界: 恰好等于阈值 → long
    assert_eq!(apply_context_tier(base_price("model_entry"), &pd, 272_000).input_cost_per_token, 1e-5);
}

#[test]
fn apply_context_tier_no_tier_passthrough() {
    // 无 context_tiers 字段 → base 不变
    let pd = serde_json::json!({"input_cost_per_token": 2.5e-6});
    let r = apply_context_tier(base_price("model_entry"), &pd, 999_999_999);
    assert_eq!(r.input_cost_per_token, 5e-6);
    assert_eq!(r.source, "model_entry");
    // tiers 为空数组 → 同样不变
    let pd2 = serde_json::json!({"context_tiers": []});
    assert_eq!(apply_context_tier(base_price("model_entry"), &pd2, 999_999_999).source, "model_entry");
}

#[test]
fn apply_context_tier_partial_override() {
    // 长档仅覆盖部分字段（如某些模型长档无 cache 价 → 继承 base cache）
    let pd = serde_json::json!({
        "context_tiers": [{
            "min_tokens": 272000,
            "input_cost_per_token": 6e-5,
            "output_cost_per_token": 2.7e-4
        }]
    });
    let r = apply_context_tier(base_price("model_entry"), &pd, 300_000);
    assert_eq!(r.input_cost_per_token, 6e-5);
    assert_eq!(r.output_cost_per_token, 2.7e-4);
    assert_eq!(r.cache_read_input_token_cost, 5e-7); // 继承 base
}

/// registry 里 glm-5-turbo 的真实形状：条目顶层三价 + time_tiers（内嵌 context_tiers）。
fn glm_turbo_pd() -> serde_json::Value {
    serde_json::json!({
        "input_cost_per_token": 6.944444444444444e-07,
        "output_cost_per_token": 3.055555555555555e-06,
        "cache_read_input_token_cost": 1.6666666666666665e-07,
        "time_tiers": [{
            "start_at": 1790784000,
            "input_cost_per_token": 1.3888888888888888e-06,
            "output_cost_per_token": 6.111111111111111e-06,
            "cache_read_input_token_cost": 3.333333333333333e-07,
            "context_tiers": [{
                "min_tokens": 32768,
                "input_cost_per_token": 1.9444444444444444e-06,
                "output_cost_per_token": 7.222222222222222e-06,
                "cache_read_input_token_cost": 5.0e-07
            }]
        }]
    })
}

#[test]
fn apply_tiers_time_hit_miss_and_nested_context() {
    let pd = glm_turbo_pd();
    let base = base_price("model_entry");
    // 命中 time 档 → 三价换成 time 条目的价 + `+time`
    let hit = apply_tiers(base.clone(), &pd, 100, 1790784000_i64 * 1000 + 1);
    assert_eq!(hit.input_cost_per_token, 1.3888888888888888e-06);
    assert_eq!(hit.cache_read_input_token_cost, 3.333333333333333e-07);
    assert_eq!(hit.source, "model_entry+time");
    // 未越过 start_at → 不变；now_ms <= 0 → 跳过 time_tiers
    assert_eq!(apply_tiers(base.clone(), &pd, 100, 1790784000_i64 * 1000 - 1).source, "model_entry");
    assert_eq!(apply_tiers(base.clone(), &pd, 100, 0).source, "model_entry");
    // time 命中后 context 分档改读 time 条目内嵌的 context_tiers
    let long = apply_tiers(base, &pd, 40_000, 1790784000_i64 * 1000 + 1);
    assert_eq!(long.input_cost_per_token, 1.9444444444444444e-06);
    assert_eq!(long.source, "model_entry+time+tier");
}

// ── 四路径纯函数：peak 命中 / peak 未命中 / 条目无 peak / 条目缺失 ──

fn peak_pd() -> serde_json::Value {
    serde_json::json!({
        "input_cost_per_token": 1.1e-6,
        "output_cost_per_token": 4.2e-6,
        "cache_read_input_token_cost": 1.1e-7,
        "peak": { "input_cost_per_token": 3.3e-6, "output_cost_per_token": 1.26e-5 }
    })
}

#[test]
fn resolve_price_from_peak_hit_uses_absolute_price() {
    let pd = peak_pd();
    let r = resolve_price_from(Some(&pd), true, 3.0, 3.0, 0, 0);
    assert!(r.peak_applied, "命中窗口 + 条目带 peak → 绝对价生效");
    assert_eq!(r.price.input_cost_per_token, 3.3e-6);
    assert_eq!(r.price.output_cost_per_token, 1.26e-5);
    // peak 未写 cache 价 → 沿用同条目默认字段（票 T4 第 2 条）
    assert_eq!(r.price.cache_read_input_token_cost, 1.1e-7);
    assert_eq!(r.price.source, "model_entry+peak");
    // 绝对价已含涨价，调用方倍率被压成 1.0
    assert_eq!(r.multiplier(3.0), 1.0);
}

#[test]
fn resolve_price_from_peak_miss_uses_default_price() {
    let pd = peak_pd();
    let r = resolve_price_from(Some(&pd), false, 3.0, 3.0, 0, 0);
    assert!(!r.peak_applied);
    assert_eq!(r.price.input_cost_per_token, 1.1e-6);
    assert_eq!(r.price.source, "model_entry");
    assert_eq!(r.multiplier(3.0), 3.0, "非高峰绝对价 → 平台倍率原样生效");
}

#[test]
fn resolve_price_from_entry_without_peak_falls_back_to_multiplier() {
    // 条目无 peak 子树，即使命中窗口也走「默认价 × 平台倍率」
    let pd = serde_json::json!({"input_cost_per_token": 1.1e-6, "output_cost_per_token": 4.2e-6});
    let r = resolve_price_from(Some(&pd), true, 3.0, 3.0, 0, 0);
    assert!(!r.peak_applied);
    assert_eq!(r.price.input_cost_per_token, 1.1e-6);
    assert_eq!(r.multiplier(2.0), 2.0);
}

#[test]
fn resolve_price_from_missing_entry_uses_settings_fallback() {
    let r = resolve_price_from(None, true, 3.0, 6.0, 0, 0);
    assert!(!r.peak_applied);
    assert_eq!(r.price.source, "fallback");
    assert!((r.price.input_cost_per_token - 3.0 / 1_000_000.0).abs() < 1e-12);
    assert!((r.price.output_cost_per_token - 6.0 / 1_000_000.0).abs() < 1e-12);
}

#[test]
fn resolve_price_from_zero_priced_entry_uses_fallback() {
    // 条目在但没有价格字段（如仅登记能力的模型）→ 与条目缺失同样落 fallback，不返回 0
    let pd = serde_json::json!({"capabilities": ["text"]});
    let r = resolve_price_from(Some(&pd), false, 3.0, 6.0, 0, 0);
    assert_eq!(r.price.source, "fallback");
}

// ── DB 路径 ──

#[tokio::test]
async fn resolve_price_reads_platform_scoped_entry() {
    let db = test_db().await;
    let cheap = serde_json::json!({"input_cost_per_token": 1e-6, "output_cost_per_token": 2e-6});
    let dear = serde_json::json!({"input_cost_per_token": 9e-6, "output_cost_per_token": 9e-6});
    put_entry(&db, "openai", "shared-model", &cheap, None).await;
    put_entry(&db, "anthropic", "shared-model", &dear, None).await;

    // 同一 model_id 在两个平台是两条独立价格，按 platform_code 取
    let a = resolve_price(&db, "openai", "shared-model", 99.0, 99.0, 0, 0, false).await.unwrap();
    assert_eq!(a.price.input_cost_per_token, 1e-6);
    let b = resolve_price(&db, "anthropic", "shared-model", 99.0, 99.0, 0, 0, false).await.unwrap();
    assert_eq!(b.price.input_cost_per_token, 9e-6);
    // 该平台没有这条模型 → fallback（不跨平台借价）
    let miss = resolve_price(&db, "glm", "shared-model", 3.0, 3.0, 0, 0, false).await.unwrap();
    assert_eq!(miss.price.source, "fallback");
}

#[tokio::test]
async fn resolve_price_falls_back_to_bundled_when_db_empty() {
    let db = test_db().await;
    // DB 未同步 → get_model_entry 回落编译期 registry，而非直接掉进 settings fallback
    let r = resolve_price(&db, "glm_coding", "glm-5.2", 3.0, 3.0, 0, 0, false).await.unwrap();
    assert_ne!(r.price.source, "fallback", "DB 未同步时应命中 bundled registry 兜底");
    assert!(r.price.input_cost_per_token > 0.0);
}

#[tokio::test]
async fn model_max_output_tokens_column_then_json_then_none() {
    let db = test_db().await;
    // 列有值 → 用列
    put_entry(&db, "openai", "a", &serde_json::json!({}), Some(4096)).await;
    assert_eq!(model_max_output_tokens(&db, "openai", "a").await.unwrap(), Some(4096));
    // 列 NULL → 回退 price_data JSON
    put_entry(&db, "openai", "b", &serde_json::json!({"max_output_tokens": 1234}), None).await;
    assert_eq!(model_max_output_tokens(&db, "openai", "b").await.unwrap(), Some(1234));
    // 条目缺失 → None（不裁剪）
    assert_eq!(model_max_output_tokens(&db, "openai", "none").await.unwrap(), None);
    // 平台不匹配同样是 None
    assert_eq!(model_max_output_tokens(&db, "anthropic", "a").await.unwrap(), None);
}
