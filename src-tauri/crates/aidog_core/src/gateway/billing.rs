//! 计费规则：根据 `model_entry` 定价 + peak 倍率算单次请求预估花费（$）。
//!
//! 从 `db/stats_today.rs` 迁出（locality：计费是业务规则，不属于「今日统计」DB 模块）。
//! 纯函数 `est_cost_from` 不碰 DB，供单测直接验证计费规则；`calc_est_cost` 是薄壳，
//! 只负责取数据（价格 / peak 窗口）再调纯函数。

use aidog_db::Db;

use super::registry::effective_presets;

/// CNY→USD 汇率（1 USD 兑 CNY 数）：`tools_pricing` 声明 `currency: "CNY"` 的按次价换算
/// USD 计费用（est_cost 列全库统一 USD 口径）。值 6.6675 = ECB 参考汇率（frankfurter.dev
/// `/v1/latest?base=USD&symbols=CNY`，2026-10-27 参考价，2026-10-28 取值）。
/// 随版本更新（spec B6：不进 schema / DB / 运行时配置，汇率大变时改此常量重发版）。
pub const CNY_PER_USD: f64 = 6.6675;

/// 根据 `model_entry` 定价计算单次请求预估花费（$），含 peak 调价。
///
/// 价格走 `resolve_price`（票 T4）：按 `(platform_type, model_name)` 查 `model_entry` 条目
/// （DB 未同步时自动回落 bundled registry），条目缺失 / 无价 → `PriceSyncSettings` 的
/// fallback 默认价（默认 3.0 $/M），不返回 0。
///
/// **高峰只调价一次**：命中窗口且条目带 `peak` → 用模型 peak 绝对价，此时倍率压成 1.0
/// （`PriceResolution::multiplier`）；条目无 `peak` 才乘平台 `peak` 倍率。
///
/// peak（高峰/低峰倍率）混合源（PRD 决策 B），见 `peak::peak_for`：
/// 1. `platform.extra.peak`（用户覆盖，非空 → 用之）
/// 2. `default_peak(platform_type)`（bundled preset 默认）
/// 3. 1.0（无调整）
///
/// 倍率 × base cost 落 `est_cost`（无新列；审计凭 time + platform_id 可重建窗口命中）。
///
/// 锁安全：本函数不持有 `db.0.lock()`；`get_sync_settings` / `resolve_price`
/// （内部 `get_model_entry`）/ `get_platform` 各自获取并释放 db 锁，不会重入死锁。
///
/// `platform_type` 传入平台主类型的 serde 裸名（如 `"deepseek"`）以启用 pricing override；
/// `platform_extra` 是已读取的平台行 extra，避免调用方为 peak 再次查询同一行。
/// 传 `""` 时 override 不命中，但回退链仍保证非 0。`platform_id`=0（自动分组日志无源平台）
/// / `created_at_ms`=0（缺失）时 peak 不生效（multiplier=1.0）。
#[allow(clippy::too_many_arguments)]
pub async fn calc_est_cost(
    db: &Db,
    model_name: &str,
    platform_type: &str,
    platform_extra: &str,
    input_tokens: i32,
    output_tokens: i32,
    cache_tokens: i32,
    cache_write_tokens: i32,
    cache_write_1h_tokens: i32,
    platform_id: i64,
    created_at_ms: i64,
) -> f64 {
    // 透传平台 est_cost 恒 0（ADR 0008 决策 1）：不查 registry 价格，也不走 fallback
    // 单价——对完全未知上游套 3.0 $/M 是编造数据，假账比没账更误导统计。
    // log.rs 的 est_cost==0 重算路径对本分支无害：重算仍走这里返 0，单次调用无循环。
    // 未知协议（票 architecture-deepening 06，用户裁决 2026-10-16）同样不计价（照 passthrough
    // 先例）：此类平台不参与路由（正常路径不会到达计价），历史行 / 边角重算落到这里也保 0。
    if platform_type == "passthrough" || platform_type == "unknown" {
        return 0.0;
    }

    let settings = crate::gateway::price_sync::get_sync_settings(db).await;

    // peak 窗口：仅当有真实平台 + 时间戳才查（mock / 隧道 / 缺失上下文 → 空 → multiplier=1.0）。
    // 必须先于价格解析——`is_peak` 决定条目里的 peak 绝对价是否生效。
    let windows = if platform_id > 0 && created_at_ms > 0 {
        super::peak::peak_for(platform_extra, platform_type)
    } else {
        Vec::new()
    };
    let is_peak = super::peak::is_in_peak_window(&windows, created_at_ms, model_name);

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
            cache_write_input_token_cost: 0.0,
            cache_write_1h_input_token_cost: 0.0,
            source: "fallback".to_string(),
        },
        peak_applied: false,
    });

    let multiplier = if created_at_ms <= 0 {
        1.0
    } else {
        resolved.multiplier(super::peak::resolve_multiplier(
            &windows,
            created_at_ms,
            model_name,
        ))
    };
    let rp = &resolved.price;
    est_cost_from(
        input_tokens,
        output_tokens,
        cache_tokens,
        cache_write_tokens,
        cache_write_1h_tokens,
        rp.input_cost_per_token,
        rp.output_cost_per_token,
        rp.cache_read_input_token_cost,
        rp.cache_write_input_token_cost,
        rp.cache_write_1h_input_token_cost,
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
    cache_write_tokens: i32,
    cache_write_1h_tokens: i32,
    input_cost_per_token: f64,
    output_cost_per_token: f64,
    cache_read_input_token_cost: f64,
    cache_write_input_token_cost: f64,
    cache_write_1h_input_token_cost: f64,
    multiplier: f64,
) -> f64 {
    let base = input_tokens as f64 * input_cost_per_token
        + output_tokens as f64 * output_cost_per_token
        + cache_tokens as f64 * cache_read_input_token_cost
        + cache_write_tokens as f64 * cache_write_input_token_cost
        + cache_write_1h_tokens as f64 * cache_write_1h_input_token_cost;
    base * multiplier
}

/// 按次工具计费纯函数（spec-按次计费 B3）：Σ `per_1000 ÷ 1000 × 计数`，
/// `currency: "CNY"` 的条目经 [`CNY_PER_USD`] 换算 USD，其余（含缺省）按 USD 直用。
/// `tools_pricing` = platform.json 顶层 `tools_pricing` 容器（None = 平台无声明）；
/// `counts` = (工具键, 成功调用次数)。未声明工具键 / per_1000 缺失或非正 → 该键计 0；
/// 平台无声明或存在未声明键时汇总 warn 一条（spec B5，每请求至多一条防刷屏）。
/// 按次费不乘 peak 倍率（官方按次价无峰谷差证据，spec B3）。
pub fn tools_cost(tools_pricing: Option<&serde_json::Value>, counts: &[(&str, i64)]) -> f64 {
    if counts.is_empty() {
        return 0.0;
    }
    let mut cost = 0.0;
    let mut undeclared: Vec<&str> = Vec::new();
    for (key, count) in counts {
        if *count <= 0 {
            continue;
        }
        let entry = tools_pricing.and_then(|tp| tp.get(*key));
        match entry
            .and_then(|e| e.get("per_1000"))
            .and_then(serde_json::Value::as_f64)
        {
            Some(per_1000) if per_1000 > 0.0 => {
                let usd = per_1000 / 1000.0 * *count as f64;
                let is_cny =
                    entry.and_then(|e| e.get("currency")).and_then(serde_json::Value::as_str)
                        == Some("CNY");
                cost += if is_cny { usd / CNY_PER_USD } else { usd };
            }
            _ => undeclared.push(key),
        }
    }
    if tools_pricing.is_none() || !undeclared.is_empty() {
        tracing::warn!(
            undeclared = ?undeclared,
            "tool call counts without tools_pricing declaration; billed as 0"
        );
    }
    cost
}

/// 按 protocol 名（serde 裸名，如 "deepseek"）查生效 preset（DB 优先）的 `tools_pricing`
/// 按次计价容器。协议 / 字段缺失 → None；字段非 object（解析失败）→ warn 返 None
/// （照 `peak::peak_in` 惯例，禁抄第二份真值源）。数据唯一落点 = platform.json
/// （spec-按次计费 A3）。
pub fn default_tools_pricing(protocol: &str) -> Option<serde_json::Value> {
    tools_pricing_in(&effective_presets(), protocol)
}

/// [`default_tools_pricing`] 的纯函数核心：从任意一篇 presets 文档取某协议的 tools_pricing。
pub fn tools_pricing_in(doc: &serde_json::Value, protocol: &str) -> Option<serde_json::Value> {
    let proto_obj = doc.get("protocols").and_then(|p| p.get(protocol))?;
    let val = proto_obj.get("tools_pricing")?;
    if !val.is_object() {
        tracing::warn!(protocol, "tools_pricing preset parse failed; skipping");
        return None;
    }
    Some(val.clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gateway::peak::TimeWindow;

    /// 透传平台（ADR 0008 决策 1）：est_cost 恒 0——不查 registry 价格，也不走 fallback
    /// 单价（fallback 之前短路），无论 tokens 多少。
    #[tokio::test]
    async fn passthrough_platform_est_cost_always_zero() {
        use aidog_stats::DbInitTables;
        let db = Db::new(":memory:").await.expect("open memory db");
        db.init_tables().await.expect("init tables");

        let cost = calc_est_cost(&db, "any-model", "passthrough", "", 1000, 2000, 0, 0, 0, 1, 1).await;
        assert_eq!(
            cost, 0.0,
            "passthrough platform must short-circuit to 0 before fallback"
        );
        // 非 passthrough 对照：走 fallback 默认价，非 0
        let other = calc_est_cost(&db, "unknown-model", "anthropic", "", 1000, 0, 0, 0, 0, 0, 0).await;
        assert!(
            other > 0.0,
            "non-passthrough unknown model falls back to default price, got {other}"
        );
    }

    /// 未知协议（票 architecture-deepening 06，用户裁决 2026-10-16）：est_cost 恒 0，
    /// 照 passthrough 先例——未知协议无 registry 条目，套 fallback 单价是编造数据。
    #[tokio::test]
    async fn unknown_protocol_est_cost_always_zero() {
        use aidog_stats::DbInitTables;
        let db = Db::new(":memory:").await.expect("open memory db");
        db.init_tables().await.expect("init tables");

        let cost = calc_est_cost(&db, "any-model", "unknown", "", 1000, 2000, 0, 0, 0, 1, 1).await;
        assert_eq!(
            cost, 0.0,
            "unknown protocol must short-circuit to 0 before fallback"
        );
    }

    fn window(start_hour: i32, end_hour: i32, multiplier: f64) -> TimeWindow {
        TimeWindow {
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
        let cost = est_cost_from(
            1000,
            500,
            0,
            0,
            0,
            3.0 / 1_000_000.0,
            15.0 / 1_000_000.0,
            0.0,
            0.0,
            0.0,
            1.0,
        );
        assert!((cost - (1000.0 * 3.0 / 1_000_000.0 + 500.0 * 15.0 / 1_000_000.0)).abs() < 1e-12);
    }

    /// ② peak multiplier 生效：命中窗口 → base × multiplier（倍率由调用方从窗口算出）。
    #[test]
    fn est_cost_from_peak_multiplier_applies() {
        // 2026-01-01 08:00:00 UTC
        let created_at_ms = 1_767_254_400_000;
        let windows = vec![window(6, 10, 2.0)];
        let mult = super::super::peak::resolve_multiplier(&windows, created_at_ms, "claude-3");
        assert_eq!(mult, 2.0);
        let base = 1000.0 * 3.0 / 1_000_000.0;
        let cost = est_cost_from(1000, 0, 0, 0, 0, 3.0 / 1_000_000.0, 0.0, 0.0, 0.0, 0.0, mult);
        assert!((cost - base * 2.0).abs() < 1e-12);
    }

    /// ③ cache_read 折扣：cache_tokens 按更低的 cache_read_input_token_cost 计费。
    #[test]
    fn est_cost_from_cache_read_discount() {
        let input_cost = 3.0 / 1_000_000.0;
        let cache_cost = 0.3 / 1_000_000.0; // 折扣价，远低于 input_cost
        let cost = est_cost_from(0, 0, 1000, 0, 0, input_cost, 0.0, cache_cost, 0.0, 0.0, 1.0);
        assert!((cost - 1000.0 * cache_cost).abs() < 1e-12);
        assert!(cost < 1000.0 * input_cost);
    }

    /// ④ 高峰绝对价不再被倍率二次放大：`peak_applied` → multiplier 压成 1.0。
    #[test]
    fn peak_absolute_price_suppresses_multiplier() {
        let created_at_ms = 1_767_254_400_000; // 2026-01-01 08:00:00 UTC，命中 6-10 窗口
        let windows = vec![window(6, 10, 3.0)];
        let raw = super::super::peak::resolve_multiplier(&windows, created_at_ms, "glm-5.2");
        // 条目带 peak 绝对价（3 倍价直接写在 price 子树里）
        let pd = serde_json::json!({
            "price": { "input": 1.0e-6, "peak": { "input": 3.0e-6 } }
        });
        let hit = aidog_db::resolve_price_from(Some(&pd), true, 3.0, 3.0, 0, created_at_ms);
        assert_eq!(hit.multiplier(raw), 1.0);
        let cost = est_cost_from(
            1000,
            0,
            0,
            0,
            0,
            hit.price.input_cost_per_token,
            0.0,
            0.0,
            0.0,
            0.0,
            hit.multiplier(raw),
        );
        // 3 倍只来自绝对价本身，不是 1e-6 × 3 再 × 3
        assert!((cost - 1000.0 * 3.0e-6).abs() < 1e-12);
    }

    #[tokio::test]
    async fn calc_est_cost_uses_supplied_platform_extra() {
        let _ = tracing_subscriber::fmt().with_env_filter("sql=debug").try_init();
        use aidog_stats::DbInitTables;
        let db = Db::new(":memory:").await.expect("open memory db");
        db.init_tables().await.expect("init tables");
        let created_at_ms = 1_767_254_400_000; // 2026-01-01 08:00 UTC
        let extra = r#"{"peak":[{"start_hour":6,"end_hour":10,"multiplier":2.0}]}"#;
        let cost = calc_est_cost(
            &db, "unknown-model", "anthropic", extra, 1000, 0, 0, 0, 0, 1, created_at_ms,
        )
        .await;
        assert!((cost - 0.003 * 2.0).abs() < 1e-12, "cost = {cost}");
    }

    /// tools_cost ① CNY 按次价经汇率换算 USD（est_cost 全库统一 USD 口径）。
    #[test]
    fn tools_cost_cny_converts_to_usd() {
        let tp = serde_json::json!({ "web_search": { "per_1000": 5.0, "currency": "CNY" } });
        let cost = tools_cost(Some(&tp), &[("web_search", 1000)]);
        assert!((cost - 5.0 / CNY_PER_USD).abs() < 1e-12, "cost = {cost}");
    }

    /// tools_cost ② USD（缺省币种，currency 字段省略）直用不换算。
    #[test]
    fn tools_cost_usd_direct() {
        let tp = serde_json::json!({ "web_search": { "per_1000": 10.0 } });
        let cost = tools_cost(Some(&tp), &[("web_search", 1000)]);
        assert!((cost - 10.0).abs() < 1e-12, "cost = {cost}");
    }

    /// tools_cost ③ 平台无 tools_pricing 声明 → 恒 0（不设按次 fallback，spec B5：兜底必错）。
    #[test]
    fn tools_cost_no_declaration_zero() {
        assert_eq!(tools_cost(None, &[("web_search", 1000)]), 0.0);
    }

    /// tools_cost ④ 声明了 web_search 价、请求带未声明的 web_fetch 计数 → 该键计 0，
    /// 已声明键照常计（Anthropic 只声明 web_search 的真实形状）。
    #[test]
    fn tools_cost_undeclared_key_zero() {
        let tp = serde_json::json!({ "web_search": { "per_1000": 10.0 } });
        let cost = tools_cost(Some(&tp), &[("web_search", 1000), ("web_fetch", 500)]);
        assert!((cost - 10.0).abs() < 1e-12, "cost = {cost}");
    }

    /// tools_pricing 读取链端到端：bundled registry 的 deepseek / anthropic 条目
    /// 可经 default_tools_pricing 取到（无 DB 时回落编译期内置那份）。
    #[test]
    fn default_tools_pricing_reads_bundled_registry() {
        let ds = default_tools_pricing("deepseek").expect("deepseek tools_pricing bundled");
        assert_eq!(ds["search"]["per_1000"].as_f64(), Some(5.0));
        assert_eq!(ds["search"]["currency"].as_str(), Some("CNY"));
        assert_eq!(ds["fetch"]["per_1000"].as_f64(), Some(5.0));
        let an = default_tools_pricing("anthropic").expect("anthropic tools_pricing bundled");
        assert_eq!(an["web_search"]["per_1000"].as_f64(), Some(10.0));
        assert_eq!(an["web_search"]["currency"].as_str(), None, "缺省 USD");
    }

    /// B3：1h 档计价——cache_write_1h 用量落 1h 单价，与 5m 桶分开（1h=2× input 例）。
    #[test]
    fn est_cost_from_cache_write_1h_priced_separately() {
        let pw5m = 3.75 / 1_000_000.0; // 1.25× input
        let pw1h = 6.0 / 1_000_000.0; // 2× input
        let cost = est_cost_from(0, 0, 0, 100, 50, 0.0, 0.0, 0.0, pw5m, pw1h, 1.0);
        let expected = 100.0 * pw5m + 50.0 * pw1h;
        assert!( (cost - expected).abs() < 1e-12, "cost = {cost}");
        // 反证：若 1h 桶误按 5m 价计，结果会低 50×(6.0-3.75)/1M
        assert!(cost > 150.0 * pw5m);
    }

    /// B3 缺省继承：条目无 `price.cache_write_1h` → 1h 单价回落 cache_write 值
    /// （无该字段平台 5m/1h 同价，零回归）；有该字段 → 各走各价。
    #[test]
    fn resolve_price_cache_write_1h_defaults_to_cache_write() {
        let without = serde_json::json!({
            "price": { "input": 3.0e-6, "cache_write": 3.75e-6 }
        });
        let hit = aidog_db::resolve_price_from(Some(&without), false, 3.0, 3.0, 0, 0);
        assert_eq!(hit.price.cache_write_input_token_cost, 3.75e-6);
        assert_eq!(
            hit.price.cache_write_1h_input_token_cost, 3.75e-6,
            "缺省回落 cache_write，无该字段平台零回归"
        );
        let with = serde_json::json!({
            "price": { "input": 3.0e-6, "cache_write": 3.75e-6, "cache_write_1h": 6.0e-6 }
        });
        let hit = aidog_db::resolve_price_from(Some(&with), false, 3.0, 3.0, 0, 0);
        assert_eq!(hit.price.cache_write_input_token_cost, 3.75e-6);
        assert_eq!(hit.price.cache_write_1h_input_token_cost, 6.0e-6);
    }

    /// tools_pricing_in 照 peak_in 惯例：协议 / 字段缺失 → None；容器非 object → warn 返 None。
    #[test]
    fn tools_pricing_in_missing_or_malformed_returns_none() {
        let doc = serde_json::json!({
            "protocols": {
                "no_field": {},
                "malformed": { "tools_pricing": 5 }
            }
        });
        assert_eq!(tools_pricing_in(&doc, "no_field"), None);
        assert_eq!(tools_pricing_in(&doc, "malformed"), None);
        assert_eq!(tools_pricing_in(&doc, "absent"), None);
    }
}

/// ⑤ cache_write 独立计价：cache_write_tokens × cache_write 价（1.25× input 例），
/// 不吃 cache_read 折扣价（2026-09-28 cache-write 记账）。
#[test]
fn est_cost_from_cache_write_priced_separately() {
    let input_cost = 3.0 / 1_000_000.0;
    let cache_read_cost = 0.3 / 1_000_000.0;
    let cache_write_cost = 3.75 / 1_000_000.0; // 1.25× input
    let cost = est_cost_from(0, 0, 1000, 2000, 0, input_cost, 0.0, cache_read_cost, cache_write_cost, 0.0, 1.0);
    assert!((cost - (1000.0 * cache_read_cost + 2000.0 * cache_write_cost)).abs() < 1e-12);
    // 反证：若 write 误按 read 价计，结果会低 2000×(3.75-0.3)/1M
    assert!(cost > 2000.0 * cache_read_cost);
}
