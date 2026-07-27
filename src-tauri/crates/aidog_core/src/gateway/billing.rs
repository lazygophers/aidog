//! 计费规则：根据 model_price 定价 + peak_hours 倍率算单次请求预估花费（$）。
//!
//! 从 `db/stats_today.rs` 迁出（locality：计费是业务规则，不属于「今日统计」DB 模块）。
//! 纯函数 `est_cost_from` 不碰 DB，供单测直接验证计费规则；`calc_est_cost` 是薄壳，
//! 只负责取数据（价格 / peak_hours 窗口）再调纯函数。

use super::db::Db;

/// 根据 model_price 定价计算单次请求预估花费（$），含 peak_hours 倍率调整。
///
/// 复用 `resolve_price` 的回退链（pricing[platform_type] > top_level >
/// default_platform > fallback 默认价），与 preview 命令 `model_price_resolve` 行为一致：
/// 无模型价 / 价为 0 时回退到 `PriceSyncSettings` 的 fallback 默认价（默认 3.0 $/M），不再返回 0。
///
/// peak_hours（高峰/低峰倍率）混合源（PRD 决策 B），见 `peak_hours::peak_hours_for`：
/// 1. `platform.extra.peak_hours`（用户覆盖，非空 → 用之）
/// 2. `default_peak_hours(platform_type)`（bundled preset 默认）
/// 3. 1.0（无调整）
///
/// 倍率 × base cost 落 `est_cost`（无新列；审计凭 time + platform_id 可重建窗口命中）。
///
/// 锁安全：本函数不持有 `db.0.lock()`；`get_sync_settings` / `resolve_price`
/// （内部 `get_model_price`）/ `get_platform` 各自获取并释放 db 锁，不会重入死锁。
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
    let rp = super::db::resolve_price(
        db,
        model_name,
        platform_type,
        settings.fallback_input_price,
        settings.fallback_output_price,
        input_tokens as i64,
    )
    .await
    .unwrap_or_else(|_| crate::gateway::models::ResolvedPrice {
        // 安全默认：直接用 fallback 默认价（$/M → $/token），保证非 0、不 panic
        input_cost_per_token: settings.fallback_input_price / 1_000_000.0,
        output_cost_per_token: settings.fallback_output_price / 1_000_000.0,
        cache_read_input_token_cost: 0.0,
        source: "fallback".to_string(),
    });

    // peak_hours 窗口：仅当有真实平台 + 时间戳才查（mock / 隧道 / 缺失上下文 → 空 → multiplier=1.0）。
    let windows = if platform_id > 0 && created_at_ms > 0 {
        match super::db::get_platform(db, platform_id as u64).await {
            Ok(Some(p)) => super::peak_hours::peak_hours_for(&p.extra, platform_type),
            _ => super::peak_hours::default_peak_hours(platform_type),
        }
    } else {
        Vec::new()
    };

    est_cost_from(
        input_tokens,
        output_tokens,
        cache_tokens,
        rp.input_cost_per_token,
        rp.output_cost_per_token,
        rp.cache_read_input_token_cost,
        &windows,
        created_at_ms,
        model_name,
    )
}

/// 计费规则的纯函数核心：base cost（token × 单价）× peak_hours multiplier。不碰 DB / 全局状态，
/// 输入全部显式传参，供单测直接验证。`created_at_ms<=0` 或 `windows` 为空 → multiplier=1.0。
#[allow(clippy::too_many_arguments)]
pub fn est_cost_from(
    input_tokens: i32,
    output_tokens: i32,
    cache_tokens: i32,
    input_cost_per_token: f64,
    output_cost_per_token: f64,
    cache_read_input_token_cost: f64,
    windows: &[super::peak_hours::PeakWindow],
    created_at_ms: i64,
    model_name: &str,
) -> f64 {
    let base = input_tokens as f64 * input_cost_per_token
        + output_tokens as f64 * output_cost_per_token
        + cache_tokens as f64 * cache_read_input_token_cost;

    if created_at_ms <= 0 {
        return base;
    }
    base * super::peak_hours::resolve_multiplier(windows, created_at_ms, model_name)
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
            days_of_week: None,
            start_minute: None,
            end_minute: None,
            days_of_month: None,
            models: None,
            start_at: None,
            end_at: None,
        }
    }

    /// ① base 无倍率：created_at_ms<=0 → 直接 token × 单价，不查窗口。
    #[test]
    fn est_cost_from_base_no_multiplier() {
        let cost = est_cost_from(1000, 500, 0, 3.0 / 1_000_000.0, 15.0 / 1_000_000.0, 0.0, &[], 0, "claude-3");
        assert!((cost - (1000.0 * 3.0 / 1_000_000.0 + 500.0 * 15.0 / 1_000_000.0)).abs() < 1e-12);
    }

    /// ② peak multiplier 生效：命中窗口 → base × multiplier。
    #[test]
    fn est_cost_from_peak_multiplier_applies() {
        // 2026-01-01 08:00:00 UTC
        let created_at_ms = 1_767_254_400_000;
        let windows = vec![window(6, 10, 2.0)];
        let base = 1000.0 * 3.0 / 1_000_000.0;
        let cost = est_cost_from(1000, 0, 0, 3.0 / 1_000_000.0, 0.0, 0.0, &windows, created_at_ms, "claude-3");
        assert!((cost - base * 2.0).abs() < 1e-12);
    }

    /// ③ cache_read 折扣：cache_tokens 按更低的 cache_read_input_token_cost 计费。
    #[test]
    fn est_cost_from_cache_read_discount() {
        let input_cost = 3.0 / 1_000_000.0;
        let cache_cost = 0.3 / 1_000_000.0; // 折扣价，远低于 input_cost
        let cost = est_cost_from(0, 0, 1000, input_cost, 0.0, cache_cost, &[], 0, "claude-3");
        assert!((cost - 1000.0 * cache_cost).abs() < 1e-12);
        assert!(cost < 1000.0 * input_cost);
    }
}
