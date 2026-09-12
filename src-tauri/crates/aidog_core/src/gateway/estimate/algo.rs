//! 纯算法（可单测）：校准判定 / 增量预估 / pace 配色 / 真查拟合

use super::model::{CALIBRATE_COUNT, CALIBRATE_INTERVAL_MS, EstTier};

/// 校准判定：距上次真查 >5min 或 预估次数 >=100
pub fn should_calibrate(now_ms: i64, last_real_query_at: i64, estimate_count: i64) -> bool {
    now_ms - last_real_query_at > CALIBRATE_INTERVAL_MS || estimate_count >= CALIBRATE_COUNT
}

/// 余额预估增量 cost = in×in_cost + out×out_cost + cache×cache_cost
pub fn balance_cost(
    input_tokens: i64,
    output_tokens: i64,
    cache_tokens: i64,
    input_cost_per_token: f64,
    output_cost_per_token: f64,
    cache_read_input_token_cost: f64,
) -> f64 {
    input_tokens as f64 * input_cost_per_token
        + output_tokens as f64 * output_cost_per_token
        + cache_tokens as f64 * cache_read_input_token_cost
}

/// 对单个 tier 应用一次请求的增量（read-modify-write 的纯函数部分）。
///   - `prompt_count`（GLM/Kimi/MiniMax/百炼）：按次扣。has_base → 每请求 +`100/limit`
///     （Kimi 精确）；无 → 拟合 `coef_per_request`（`est = util_at_last_real + requests_since_real × coef`，
///     冷启动不预估，est 维持真值）。token 不参与。
///   - `mcp_time`（GLM mcp_monthly）：MCP 使用时长口径，与请求无关 → 不增量，只靠真查校准。
///   - `response_inline`：响应头/体自带配额真值 → 不增量。
///   - `tokens` / `""`（缺省，向后兼容旧 JSON）：has_base → 每 token +`100/limit`；
///     无 → 拟合 `coef_per_token`（`est = util_at_last_real + tokens_since_real × coef`）。
///   - 旧 JSON 无 unit 但 name == "mcp_monthly" 的存量行：同样不增量（历史特判保留，
///     校准后 unit 会被脚本返回值覆盖为 `mcp_time`）。
pub fn apply_tier_delta(tier: &mut EstTier, requests: i64, tokens: f64) {
    let per_request_unit = tier.unit == "prompt_count";
    let no_increment = tier.unit == "mcp_time"
        || tier.unit == "response_inline"
        || (tier.unit.is_empty() && tier.name == "mcp_monthly");
    if no_increment {
        return;
    }
    if per_request_unit {
        if requests <= 0 {
            return;
        }
        tier.requests_since_real += requests as f64;
        if tier.has_base && tier.limit > 0.0 {
            // Kimi 精确增量：每请求 % = 100/limit
            tier.est_utilization += requests as f64 * (100.0 / tier.limit);
            if tier.est_utilization > 100.0 {
                tier.est_utilization = 100.0;
            }
            return;
        }
        // 方案 B（按次拟合）
        if tier.coef_per_request > 0.0 {
            tier.est_utilization =
                tier.util_at_last_real + tier.requests_since_real * tier.coef_per_request;
            if tier.est_utilization > 100.0 {
                tier.est_utilization = 100.0;
            }
        }
        // 冷启动（无 coef）：不预估，est_utilization 保持真值不动
        return;
    }
    // tokens / 缺省兜底
    if tokens <= 0.0 {
        return;
    }
    if tier.has_base && tier.limit > 0.0 {
        // Kimi 精确增量（token 口径）
        tier.est_utilization += tokens * (100.0 / tier.limit);
        if tier.est_utilization > 100.0 {
            tier.est_utilization = 100.0;
        }
        return;
    }
    // 方案 B（按 token 拟合）
    tier.tokens_since_real += tokens;
    if tier.coef_per_token > 0.0 {
        tier.est_utilization =
            tier.util_at_last_real + tier.tokens_since_real * tier.coef_per_token;
        if tier.est_utilization > 100.0 {
            tier.est_utilization = 100.0;
        }
    }
    // 冷启动（无 coef）：不预估，est_utilization 保持真值不动
}

/// 单 tier 的预期消耗速率分级（statusline 第 3 行动态色用）。
///
/// 语义：以「窗口内利用率随时间的预期推进」判定该档配额耗尽的快慢——
///   - `Fast`：利用率已偏高 / 预期快于配额时间线（接近耗尽）→ 上游标红。
///   - `Normal`：接近时间线 → 标黄。
///   - `Busy`：利用率低 / 慢于时间线（仍宽裕）→ 标绿。
///
/// 数据不足（无窗口起止时间戳持久化）时退化为按当前 `est_utilization` 阈值估算：
///   >= 80 → Fast；>= 40 → Normal；否则 Busy。
///
/// 拿不到利用率（NaN/负）一律 `Normal` 降级。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TierPace {
    Fast,
    Normal,
    Busy,
}

impl TierPace {
    pub fn as_str(self) -> &'static str {
        match self {
            TierPace::Fast => "fast",
            TierPace::Normal => "normal",
            TierPace::Busy => "busy",
        }
    }
}

/// 由 tier 利用率估算 pace。无可靠窗口时间线时按利用率阈值降级。
pub fn tier_pace(tier: &EstTier) -> TierPace {
    let util = tier.est_utilization;
    if !util.is_finite() || util < 0.0 {
        return TierPace::Normal;
    }
    if util >= 80.0 {
        TierPace::Fast
    } else if util >= 40.0 {
        TierPace::Normal
    } else {
        TierPace::Busy
    }
}

/// 由预估 tier 算「使用速率配色」级别（usage_color，唯一阈值源）。
///
/// remain = window_start + cycle - now（预估侧）。无 window_start（0）/ 未知 name（无周期）
/// → 配色中性，不静默走旧利用率阈值（不误报）。
pub fn tier_usage_level(tier: &EstTier, now_ms: i64) -> crate::gateway::usage_color::UsageLevel {
    let cycle = match crate::gateway::usage_color::cycle_ms_for_tier(&tier.name) {
        Some(c) => c,
        None => return crate::gateway::usage_color::UsageLevel::Neutral,
    };
    if tier.window_start <= 0 {
        return crate::gateway::usage_color::UsageLevel::Neutral;
    }
    let remain = tier.window_start + cycle - now_ms;
    crate::gateway::usage_color::coding_tier_level(tier.est_utilization, Some(remain), Some(cycle))
}

/// 真查校准：用上游真值覆盖某 tier，并（方案 B）尝试拟合 coef。
///   - has_base（Kimi）：直接记 limit + est_utilization = 真值。
///   - 方案 B：`unit == "prompt_count"` 拟合 `coef_per_request = Δutil/Δ请求数`（分母
///     `requests_since_real`），否则拟合 `coef = Δutil/Δtoken`（分母 `tokens_since_real`）。
///     仅当无跨 reset（util_real >= util_at_last_real）且分母 > 0；
///     reset（util_real < util_at_last_real）→ 丢弃本窗口样本，coef 保留；
///     最后重置基线：util_at_last_real = util_real，分母清零，est = 真值。
#[allow(clippy::too_many_arguments)]
pub fn calibrate_tier(
    prev: &EstTier,
    name: &str,
    util_real: f64,
    has_base: bool,
    limit: Option<f64>,
    resets_at: Option<&str>,
    now_ms: i64,
    unit: &str,
) -> EstTier {
    // window_start 推算：真查若给 resets_at 且 name 有已知周期 → window_start = resets_at - cycle；
    // 否则保留 prev.window_start（首次真查无 resets_at 时退 0 → 配色中性，不误报）。
    let window_start = derive_window_start(name, resets_at, now_ms).unwrap_or(prev.window_start);
    if has_base {
        return EstTier {
            name: name.to_string(),
            est_utilization: util_real,
            coef_per_token: 0.0,
            coef_per_request: 0.0,
            util_at_last_real: util_real,
            tokens_since_real: 0.0,
            requests_since_real: 0.0,
            has_base: true,
            limit: limit.unwrap_or(0.0),
            window_start,
            unit: unit.to_string(),
        };
    }
    // 方案 B 拟合（分母按 unit 口径选）
    let per_request = unit == "prompt_count";
    let (mut coef_token, mut coef_req) = (prev.coef_per_token, prev.coef_per_request);
    let denominator = if per_request {
        prev.requests_since_real
    } else {
        prev.tokens_since_real
    };
    let is_reset = util_real < prev.util_at_last_real;
    if !is_reset && denominator > 0.0 {
        let fitted = (util_real - prev.util_at_last_real) / denominator;
        if fitted > 0.0 {
            if per_request {
                coef_req = fitted;
            } else {
                coef_token = fitted;
            }
        }
    }
    // reset 时丢弃本窗口样本（coef 保留），仅重置基线
    EstTier {
        name: name.to_string(),
        est_utilization: util_real,
        coef_per_token: coef_token,
        coef_per_request: coef_req,
        util_at_last_real: util_real,
        tokens_since_real: 0.0,
        requests_since_real: 0.0,
        has_base: false,
        limit: 0.0,
        window_start,
        unit: unit.to_string(),
    }
}

/// 由真查 resets_at（ISO8601 或 millis 字符串）+ tier name 已知周期推算本周期起点（unix ms）。
/// 无 resets_at / 解析失败 / 未知 name → None（保留旧 window_start）。
fn derive_window_start(name: &str, resets_at: Option<&str>, now_ms: i64) -> Option<i64> {
    let raw = resets_at?;
    let cycle = crate::gateway::usage_color::cycle_ms_for_tier(name)?;
    let resets_ms = parse_resets_to_ms(raw)?;
    let start = resets_ms - cycle;
    // 防御：reset 早于 now 太多 / 异常 → 仍落地（remain 会算成负，配色侧 clamp）。
    let _ = now_ms;
    Some(start)
}

/// 解析 resets_at：先按 ISO8601，再按裸 millis 数字。
pub(super) fn parse_resets_to_ms(raw: &str) -> Option<i64> {
    let raw = raw.trim();
    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(raw) {
        return Some(dt.timestamp_millis());
    }
    if let Ok(ms) = raw.parse::<i64>() {
        // 启发式：>1e12 视作 ms，否则视作秒。
        return Some(if ms > 1_000_000_000_000 {
            ms
        } else {
            ms * 1000
        });
    }
    None
}

#[cfg(test)]
#[path = "test_algo.rs"]
mod test_algo;
