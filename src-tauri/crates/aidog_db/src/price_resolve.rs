//! 计费价格解析：`(platform_code, model_id)` → [`ResolvedPrice`]（票 T4）。
//!
//! 真值源是 `model_entry` 表的 `price_data`（整份 registry 模型 JSON），
//! DB 未同步时由 [`crate::get_model_entry`] 自动回落编译期内置 registry。
//! 旧 `model_price` 表的「模型名单键 + pricing[platform] 归并视图」到此为止：
//! 同一模型在不同平台的价格是**不同的行**，不再靠一份合并文档的 `pricing` 子键区分。
//!
//! ADR 0006 的解析顺序（同 `SPEC.md`「计费解析顺序」）：
//! 1. 命中 preset `peak_hours` 窗口（由调用方判定并以 `is_peak` 传入）且条目带 `peak`
//!    → 用模型 `peak` **绝对价**，此时 [`PriceResolution::peak_applied`] = true，
//!    调用方**不得**再乘平台倍率（否则双重计价，见笔记 R6）。
//! 2. 否则条目默认价（含 `time_tiers` / `context_tiers` 分档）× 平台倍率（调用方乘）。
//! 3. 本平台无该模型条目 → 按 `model_id` 借用其它平台的条目（`official` 优先），
//!    价格 `source` 记 `official_entry`。registry 里 50 个中转镜像平台 `models/` 目录为空，
//!    这一层就是旧 `resolve_price` 的 `pricing[platform] → 顶层单价` 回退链（票 13-A/B）。
//! 4. 哪个平台都没有 → `PriceSyncSettings` 的 fallback 默认价（不跑分档，同旧行为）。

use crate::models::ResolvedPrice;
use crate::Db;
use serde_json::Value;

/// 价格解析结果。`peak_applied` 只在 Rust 内部流转（不进 TS 契约）：
/// true 表示价格已是高峰绝对价，调用方须把平台倍率视为 1.0。
#[derive(Debug, Clone)]
pub struct PriceResolution {
    pub price: ResolvedPrice,
    pub peak_applied: bool,
}

impl PriceResolution {
    /// 调用方该乘的平台倍率：高峰绝对价已含涨价，返回 1.0；否则原样返回 `multiplier`。
    pub fn multiplier(&self, multiplier: f64) -> f64 {
        if self.peak_applied {
            1.0
        } else {
            multiplier
        }
    }
}

/// 三价字段非 null 覆盖 base（null 字段继承 base）。`apply_context_tier` / `apply_tiers` 共用。
fn overlay_prices(base: &mut ResolvedPrice, tier: &Value) {
    if let Some(v) = tier.get("input_cost_per_token").and_then(Value::as_f64) {
        base.input_cost_per_token = v;
    }
    if let Some(v) = tier.get("output_cost_per_token").and_then(Value::as_f64) {
        base.output_cost_per_token = v;
    }
    if let Some(v) = tier.get("cache_read_input_token_cost").and_then(Value::as_f64) {
        base.cache_read_input_token_cost = v;
    }
}

/// 上下文阶梯选档：取 `context_tiers` 中 `min_tokens <= input_tokens` 的最大档，
/// 非 null 字段覆盖 base 价（null 字段继承 base，如某些模型长档无 cache 价）。
/// `context_tiers` 缺失/非数组/无命中档 → 返回 base 不变。
pub fn apply_context_tier(mut base: ResolvedPrice, pd: &Value, input_tokens: i64) -> ResolvedPrice {
    let Some(tiers) = pd.get("context_tiers").and_then(Value::as_array) else {
        return base;
    };
    let best = tiers
        .iter()
        .filter_map(|t| {
            let min_tokens = t.get("min_tokens").and_then(Value::as_i64)?;
            (min_tokens <= input_tokens).then_some((min_tokens, t))
        })
        .max_by_key(|(min_tokens, _)| *min_tokens);
    let Some((_, tier)) = best else {
        return base;
    };
    overlay_prices(&mut base, tier);
    base.source.push_str("+tier");
    base
}

/// 时间阶梯选档：取 `time_tiers` 中 `start_at * 1000 <= now_ms` 的最大档。命中后该条目
/// 整体作为价表（三价覆盖 + 其内嵌 `context_tiers` 替代顶层），再跑 context 分档 ——
/// 顺序 time→context，因为涨价后的长文档价只能表达在 time 条目内部。
/// `now_ms <= 0` = 无时间上下文，跳过分档。
pub fn apply_tiers(mut base: ResolvedPrice, pd: &Value, input_tokens: i64, now_ms: i64) -> ResolvedPrice {
    let hit = (now_ms > 0)
        .then(|| pd.get("time_tiers").and_then(Value::as_array))
        .flatten()
        .and_then(|tiers| {
            tiers
                .iter()
                .filter_map(|t| {
                    let at = t.get("start_at").and_then(Value::as_i64)?;
                    (at.saturating_mul(1000) <= now_ms).then_some((at, t))
                })
                .max_by_key(|(at, _)| *at)
        });
    let ctx_src = match hit {
        Some((_, tier)) => {
            overlay_prices(&mut base, tier);
            base.source.push_str("+time");
            tier
        }
        None => pd,
    };
    apply_context_tier(base, ctx_src, input_tokens)
}

/// 计费解析的纯函数核心：一份模型条目 `price_data` + 高峰态 → 价格。不碰 DB，供单测直接验证。
///
/// `pd` 传 `None` = 条目缺失 → fallback 默认价（`$/M` → `$/token`），不跑分档、不看 peak。
/// `fallback_input` / `fallback_output` 单位是 `$/M tokens`（同 `PriceSyncSettings`）。
pub fn resolve_price_from(
    pd: Option<&Value>,
    is_peak: bool,
    fallback_input: f64,
    fallback_output: f64,
    input_tokens: i64,
    now_ms: i64,
) -> PriceResolution {
    let fallback = || ResolvedPrice {
        input_cost_per_token: fallback_input / 1_000_000.0,
        output_cost_per_token: fallback_output / 1_000_000.0,
        cache_read_input_token_cost: 0.0,
        source: "fallback".to_string(),
    };
    let Some(pd) = pd else {
        return PriceResolution { price: fallback(), peak_applied: false };
    };

    let num = |k: &str| pd.get(k).and_then(Value::as_f64).unwrap_or(0.0);
    let input = num("input_cost_per_token");
    let output = num("output_cost_per_token");
    let mut price = if input > 0.0 || output > 0.0 {
        apply_tiers(
            ResolvedPrice {
                input_cost_per_token: input,
                output_cost_per_token: output,
                cache_read_input_token_cost: num("cache_read_input_token_cost"),
                source: "model_entry".to_string(),
            },
            pd,
            input_tokens,
            now_ms,
        )
    } else {
        fallback()
    };

    // 高峰绝对价：覆盖已分档的默认价（peak 未写的字段继承默认价，如 cache_read）。
    let peak = is_peak.then(|| pd.get("peak")).flatten().filter(|p| {
        p.get("input_cost_per_token").and_then(Value::as_f64).unwrap_or(0.0) > 0.0
            || p.get("output_cost_per_token").and_then(Value::as_f64).unwrap_or(0.0) > 0.0
    });
    match peak {
        Some(p) => {
            overlay_prices(&mut price, p);
            price.source.push_str("+peak");
            PriceResolution { price, peak_applied: true }
        }
        None => PriceResolution { price, peak_applied: false },
    }
}

/// 查 `model_entry` 解析价格。`platform_code` 是 registry 平台目录名（= protocol 裸名，
/// `Protocol::wire_str()`），`model_id` 是该平台上的真实请求名（路由后的 actual model）。
///
/// `now_ms <= 0` = 无时间上下文，跳过 `time_tiers`（同旧 `resolve_price` 约定）。
#[allow(clippy::too_many_arguments)]
pub async fn resolve_price(
    db: &Db,
    platform_code: &str,
    model_id: &str,
    fallback_input: f64,
    fallback_output: f64,
    input_tokens: i64,
    now_ms: i64,
    is_peak: bool,
) -> Result<PriceResolution, String> {
    let found = crate::model_entry_for_billing(db, platform_code, model_id).await?;
    let cross_platform = found.as_ref().is_some_and(|(_, x)| *x);
    let pd: Option<Value> =
        found.as_ref().and_then(|(e, _)| serde_json::from_str(&e.price_data).ok());
    let mut out =
        resolve_price_from(pd.as_ref(), is_peak, fallback_input, fallback_output, input_tokens, now_ms);
    if cross_platform {
        mark_cross_platform(&mut out.price.source);
    }
    Ok(out)
}

/// 条目来自别的平台（`model_entry` → `official_entry`）时改写 `source` 前缀，
/// 让日志 / 用量明细能把「本平台自带价」和「借用官方价」分开看。分档后缀原样保留。
fn mark_cross_platform(source: &mut String) {
    if let Some(rest) = source.strip_prefix("model_entry") {
        *source = format!("official_entry{rest}");
    }
}

/// 取模型最大输出 token（出站裁剪用）。列优先，NULL 时回退 `price_data` JSON。
/// 本平台无条目时同样借用官方条目（票 13-B：否则中转镜像类平台恒返 None，
/// 客户端发的超限 `max_tokens` 原样转发给上游，直接 400）。
/// 返回 None = 未知/无限制（不裁剪）。
pub async fn model_max_output_tokens(db: &Db, platform_code: &str, model_id: &str) -> Result<Option<i64>, String> {
    let Some((entry, _)) = crate::model_entry_for_billing(db, platform_code, model_id).await? else {
        return Ok(None);
    };
    if let Some(v) = entry.max_output_tokens {
        return Ok(Some(v));
    }
    let pd: Value = serde_json::from_str(&entry.price_data).unwrap_or_default();
    Ok(pd.get("max_output_tokens").and_then(Value::as_i64))
}
