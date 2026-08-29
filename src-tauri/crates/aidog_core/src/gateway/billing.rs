//! 计费规则：根据 `model_entry` 定价 + peak_hours 倍率算单次请求预估花费（$）。
//!
//! 从 `db/stats_today.rs` 迁出（locality：计费是业务规则，不属于「今日统计」DB 模块）。
//! 纯函数 `est_cost_from` 不碰 DB，供单测直接验证计费规则；`calc_est_cost` 是薄壳，
//! 只负责取数据（价格 / peak_hours 窗口）再调纯函数。

use aidog_db::Db;

/// 根据 `model_entry` 定价计算单次请求预估花费（$），含 peak_hours 调价。
///
/// 价格走 `resolve_price`（票 T4）：按 `(platform_type, model_name)` 查 `model_entry` 条目
/// （DB 未同步时自动回落 bundled registry），条目缺失 / 无价 → `PriceSyncSettings` 的
/// fallback 默认价（默认 3.0 $/M），不返回 0。
///
/// **高峰只调价一次**：命中窗口且条目带 `peak` → 用模型 peak 绝对价，此时倍率压成 1.0
/// （`PriceResolution::multiplier`）；条目无 `peak` 才乘平台 `peak_hours` 倍率。
///
/// peak_hours（高峰/低峰倍率）混合源（PRD 决策 B），见 `peak_hours::peak_hours_for`：
/// 1. `platform.extra.peak_hours`（用户覆盖，非空 → 用之）
/// 2. `default_peak_hours(platform_type)`（bundled preset 默认）
/// 3. 1.0（无调整）
///
/// 倍率 × base cost 落 `est_cost`（无新列；审计凭 time + platform_id 可重建窗口命中）。
///
/// 锁安全：本函数不持有 `db.0.lock()`；`get_sync_settings` / `resolve_price`
/// （内部 `get_model_entry`）/ `get_platform` 各自获取并释放 db 锁，不会重入死锁。
///
/// `platform_type` 传入平台主类型的 serde 裸名（如 `"deepseek"`）以启用 pricing override；
/// 传 `""` 时 override 不命中，但回退链仍保证非 0。`platform_id`=0（自动分组日志无源平台）
/// / `created_at_ms`=0（缺失）时 peak_hours 不生效（multiplier=1.0）。
#[allow(clippy::too_many_arguments)]
pub async fn calc_est_cost(
    db: &Db,
    model_name: &str,
    platform_type: &str,
    input_tokens: i32,
    output_tokens: i32,
    cache_tokens: i32,
    platform_id: i64,
    created_at_ms: i64,
) -> f64 {
    let settings = crate::gateway::price_sync::get_sync_settings(db).await;

    // peak_hours 窗口：仅当有真实平台 + 时间戳才查（mock / 隧道 / 缺失上下文 → 空 → multiplier=1.0）。
    // 必须先于价格解析——`is_peak` 决定条目里的 peak 绝对价是否生效。
    let windows = if platform_id > 0 && created_at_ms > 0 {
        match aidog_db::get_platform(db, platform_id as u64).await {
            Ok(Some(p)) => super::peak_hours::peak_hours_for(&p.extra, platform_type),
            _ => super::peak_hours::default_peak_hours(platform_type),
        }
    } else {
        Vec::new()
    };
    let is_peak = super::peak_hours::is_in_peak_window(&windows, created_at_ms, model_name);

    let resolved = aidog_db::resolve_price(
        db,
        platform_type,
        model_name,
        settings.fallback_input_price,
        settings.fallback_output_price,
        input_tokens as i64,
        created_at_ms,
        is_peak,
    )
    .await
    .unwrap_or_else(|_| aidog_db::PriceResolution {
        // 安全默认：直接用 fallback 默认价（$/M → $/token），保证非 0、不 panic
        price: crate::gateway::models::ResolvedPrice {
            input_cost_per_token: settings.fallback_input_price / 1_000_000.0,
            output_cost_per_token: settings.fallback_output_price / 1_000_000.0,
            cache_read_input_token_cost: 0.0,
            source: "fallback".to_string(),
        },
        peak_applied: false,
    });

    let multiplier = if created_at_ms <= 0 {
        1.0
    } else {
        resolved.multiplier(super::peak_hours::resolve_multiplier(&windows, created_at_ms, model_name))
    };
    let rp = &resolved.price;
    est_cost_from(
        input_tokens,
        output_tokens,
        cache_tokens,
        rp.input_cost_per_token,
        rp.output_cost_per_token,
        rp.cache_read_input_token_cost,
        multiplier,
    )
}

/// 计费规则的纯函数核心：base cost（token × 单价）× multiplier。不碰 DB / 全局状态，
/// 输入全部显式传参，供单测直接验证。`multiplier` 由调用方算好——高峰绝对价已含涨价时传 1.0，
/// 避免同一次高峰被调价两次（笔记 R6）。
#[allow(clippy::too_many_arguments)]
pub fn est_cost_from(
    input_tokens: i32,
    output_tokens: i32,
    cache_tokens: i32,
    input_cost_per_token: f64,
    output_cost_per_token: f64,
    cache_read_input_token_cost: f64,
    multiplier: f64,
) -> f64 {
    let base = input_tokens as f64 * input_cost_per_token
        + output_tokens as f64 * output_cost_per_token
        + cache_tokens as f64 * cache_read_input_token_cost;
    base * multiplier
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gateway::peak_hours::PeakWindow;

    fn window(start_hour: i32, end_hour: i32, multiplier: f64) -> PeakWindow {
        PeakWindow {
            start_hour,
            end_hour,
            multiplier,
            timezone: None,
            days_of_week: None,
            start_minute: None,
            end_minute: None,
            days_of_month: None,
            models: None,
            start_at: None,
            end_at: None,
        }
    }

    /// ① base 无倍率：multiplier=1.0 → 直接 token × 单价。
    #[test]
    fn est_cost_from_base_no_multiplier() {
        let cost = est_cost_from(1000, 500, 0, 3.0 / 1_000_000.0, 15.0 / 1_000_000.0, 0.0, 1.0);
        assert!((cost - (1000.0 * 3.0 / 1_000_000.0 + 500.0 * 15.0 / 1_000_000.0)).abs() < 1e-12);
    }

    /// ② peak multiplier 生效：命中窗口 → base × multiplier（倍率由调用方从窗口算出）。
    #[test]
    fn est_cost_from_peak_multiplier_applies() {
        // 2026-01-01 08:00:00 UTC
        let created_at_ms = 1_767_254_400_000;
        let windows = vec![window(6, 10, 2.0)];
        let mult = super::super::peak_hours::resolve_multiplier(&windows, created_at_ms, "claude-3");
        assert_eq!(mult, 2.0);
        let base = 1000.0 * 3.0 / 1_000_000.0;
        let cost = est_cost_from(1000, 0, 0, 3.0 / 1_000_000.0, 0.0, 0.0, mult);
        assert!((cost - base * 2.0).abs() < 1e-12);
    }

    /// ③ cache_read 折扣：cache_tokens 按更低的 cache_read_input_token_cost 计费。
    #[test]
    fn est_cost_from_cache_read_discount() {
        let input_cost = 3.0 / 1_000_000.0;
        let cache_cost = 0.3 / 1_000_000.0; // 折扣价，远低于 input_cost
        let cost = est_cost_from(0, 0, 1000, input_cost, 0.0, cache_cost, 1.0);
        assert!((cost - 1000.0 * cache_cost).abs() < 1e-12);
        assert!(cost < 1000.0 * input_cost);
    }

    /// ④ 高峰绝对价不再被倍率二次放大：`peak_applied` → multiplier 压成 1.0。
    #[test]
    fn peak_absolute_price_suppresses_multiplier() {
        let created_at_ms = 1_767_254_400_000; // 2026-01-01 08:00:00 UTC，命中 6-10 窗口
        let windows = vec![window(6, 10, 3.0)];
        let raw = super::super::peak_hours::resolve_multiplier(&windows, created_at_ms, "glm-5.2");
        // 条目带 peak 绝对价（3 倍价直接写在条目里）
        let pd = serde_json::json!({
            "input_cost_per_token": 1.0e-6,
            "peak": { "input_cost_per_token": 3.0e-6 }
        });
        let hit = aidog_db::resolve_price_from(Some(&pd), true, 3.0, 3.0, 0, created_at_ms);
        assert_eq!(hit.multiplier(raw), 1.0);
        let cost = est_cost_from(1000, 0, 0, hit.price.input_cost_per_token, 0.0, 0.0, hit.multiplier(raw));
        // 3 倍只来自绝对价本身，不是 1e-6 × 3 再 × 3
        assert!((cost - 1000.0 * 3.0e-6).abs() < 1e-12);
    }
}
