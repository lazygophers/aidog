//! 使用速率配色（唯一事实源）。
//!
//! 全部「金额 / 额度」颜色统一按**使用速率**算红黄绿，阈值常量集中于此，
//! 前端 / statusline / 后端只消费 `UsageLevel`，禁各写一套阈值（避免漂移）。
//!
//! 语义级别（下发前端，前端映射到 `var(--color-*)`）：
//!   - `red`    → 速率过快 / 快不够用
//!   - `yellow` → 临界
//!   - `green`  → 充足
//!   - `neutral`→ 无数据 / 无法判定（不报警）

// ── Coding plan tier：进度差（百分点）阈值 ───────────────────
// 差额 = 额度已用% − 时间已过%（同为 0-100 的百分比，直接相减取百分点）。
// 负 = 用得比时间慢（省）；正 = 烧得比时间快（超支）。
/// |差额| ≤ 3 个百分点 → 黄（跟得上时间进度）；< -3 → 绿；> +3 → 红。
pub const CODING_PACE_DELTA_PP: f64 = 3.0;

// ── 余额：剩余可用天数阈值 ──────────────────────────────────
/// days_remaining < 1 → 红
pub const BALANCE_DAYS_DANGER: f64 = 1.0;
/// days_remaining < 3 → 黄；否则绿
pub const BALANCE_DAYS_WARN: f64 = 3.0;

// ── 周期时长（按 tier name 硬编码，单位毫秒）─────────────────
const HOUR_MS: i64 = 3_600_000;
const DAY_MS: i64 = 24 * HOUR_MS;

/// 由 tier name 返回周期时长（ms）。未知 name → None（无周期概念，回退中性 / 利用率阈值）。
pub fn cycle_ms_for_tier(name: &str) -> Option<i64> {
    match name {
        "five_hour" => Some(5 * HOUR_MS),
        "weekly_limit" | "seven_day" => Some(7 * DAY_MS),
        "mcp_monthly" => Some(30 * DAY_MS),
        _ => None,
    }
}

/// 语义级别。序列化为小写字符串下发前端 / statusline。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UsageLevel {
    Red,
    Yellow,
    Green,
    Neutral,
}

impl UsageLevel {
    pub fn as_str(self) -> &'static str {
        match self {
            UsageLevel::Red => "red",
            UsageLevel::Yellow => "yellow",
            UsageLevel::Green => "green",
            UsageLevel::Neutral => "neutral",
        }
    }
}

/// Coding plan tier 配色（按「额度已用% − 时间已过%」的百分点差）。
///
/// 入参：
///   - `utilization`：额度已用百分比（0-100）
///   - `remain_ms`：本周期剩余时间（ms）；None = 无可靠 remain（无 resets_at / 无 window_start）
///   - `cycle_ms`：周期时长（ms）；None = 未知 name（无周期概念）
///
/// 缺 remain / cycle / 利用率非法 → `Neutral`（不静默走旧利用率阈值，不误报）。
pub fn coding_tier_level(
    utilization: f64,
    remain_ms: Option<i64>,
    cycle_ms: Option<i64>,
) -> UsageLevel {
    if !utilization.is_finite() || utilization < 0.0 {
        return UsageLevel::Neutral;
    }
    let (remain, cycle) = match (remain_ms, cycle_ms) {
        (Some(r), Some(c)) if c > 0 => (r as f64, c as f64),
        _ => return UsageLevel::Neutral,
    };
    // 配额已耗尽（util≥100）→ 直接 Red。差额算法衡量「用量进度 vs 时间进度」，
    // 但配额耗尽后该语义失效（已无可用，进度快慢无意义），按进度判绿会与现实矛盾。
    if utilization >= 100.0 {
        return UsageLevel::Red;
    }
    level_from_pace_delta(coding_pace_delta(utilization, remain, cycle))
}

/// 进度差（百分点）= 额度已用% − 时间已过%。负 = 省着用，正 = 超支。
pub fn coding_pace_delta(utilization: f64, remain_ms: f64, cycle_ms: f64) -> f64 {
    let used_pct = utilization.clamp(0.0, 100.0);
    let elapsed_pct = ((cycle_ms - remain_ms) / cycle_ms * 100.0).clamp(0.0, 100.0);
    used_pct - elapsed_pct
}

/// 进度差 → 级别。< -3 绿 / -3~+3 黄 / > +3 红。
pub fn level_from_pace_delta(delta_pp: f64) -> UsageLevel {
    if !delta_pp.is_finite() {
        return UsageLevel::Neutral;
    }
    if delta_pp > CODING_PACE_DELTA_PP {
        UsageLevel::Red
    } else if delta_pp >= -CODING_PACE_DELTA_PP {
        UsageLevel::Yellow
    } else {
        UsageLevel::Green
    }
}

/// 余额配色（按剩余可用天数）。None = 无用量 / 无余额 → Neutral（不报警）。
pub fn balance_level(days_remaining: Option<f64>) -> UsageLevel {
    match days_remaining {
        Some(d) if d.is_finite() && d >= 0.0 => {
            if d < BALANCE_DAYS_DANGER {
                UsageLevel::Red
            } else if d < BALANCE_DAYS_WARN {
                UsageLevel::Yellow
            } else {
                UsageLevel::Green
            }
        }
        _ => UsageLevel::Neutral,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cycle_lookup() {
        assert_eq!(cycle_ms_for_tier("five_hour"), Some(5 * HOUR_MS));
        assert_eq!(cycle_ms_for_tier("weekly_limit"), Some(7 * DAY_MS));
        assert_eq!(cycle_ms_for_tier("seven_day"), Some(7 * DAY_MS));
        assert_eq!(cycle_ms_for_tier("mcp_monthly"), Some(30 * DAY_MS));
        assert_eq!(cycle_ms_for_tier("unknown"), None);
    }

    // 差额 = 已用% − 已过时间%。验收：+2→黄 / +8→红 / -36→绿。
    #[test]
    fn coding_delta_within_band_is_yellow() {
        // 5h 周期过半（elapsed 50%），用了 52% → +2pp → 黄
        let cycle = 5.0 * 3600.0 * 1000.0;
        let remain = cycle * 0.5;
        let d = coding_pace_delta(52.0, remain, cycle);
        assert!((d - 2.0).abs() < 1e-6, "delta = {d}");
        assert_eq!(level_from_pace_delta(d), UsageLevel::Yellow);
        // 边界 ±3 含在黄区
        assert_eq!(level_from_pace_delta(3.0), UsageLevel::Yellow);
        assert_eq!(level_from_pace_delta(-3.0), UsageLevel::Yellow);
    }

    #[test]
    fn coding_overspend_is_red() {
        // elapsed 50%，用了 58% → +8pp → 红
        let cycle = 5.0 * 3600.0 * 1000.0;
        let remain = cycle * 0.5;
        let d = coding_pace_delta(58.0, remain, cycle);
        assert!((d - 8.0).abs() < 1e-6, "delta = {d}");
        assert_eq!(level_from_pace_delta(d), UsageLevel::Red);
    }

    #[test]
    fn coding_under_budget_is_green() {
        // 用户实例：5h 周期剩 17m（elapsed 94.33%），配额剩 42%（已用 58%）→ -36.3pp → 绿
        let cycle = 5.0 * 3600.0 * 1000.0;
        let remain = 17.0 * 60.0 * 1000.0;
        let d = coding_pace_delta(58.0, remain, cycle);
        assert!((d + 36.333_333).abs() < 1e-3, "delta = {d}");
        assert_eq!(level_from_pace_delta(d), UsageLevel::Green);
        assert_eq!(
            coding_tier_level(58.0, Some(17 * 60 * 1000), Some(5 * HOUR_MS)),
            UsageLevel::Green
        );
    }

    #[test]
    fn coding_window_start_is_yellow_not_red() {
        // 周期刚开 3 分钟（elapsed 1%）随手用了 2% → +1pp → 黄（旧 pace 算法会判红）
        let cycle = 5 * HOUR_MS;
        let remain = cycle - cycle / 100;
        assert_eq!(
            coding_tier_level(2.0, Some(remain), Some(cycle)),
            UsageLevel::Yellow
        );
    }

    #[test]
    fn coding_no_data_is_neutral() {
        assert_eq!(
            coding_tier_level(50.0, None, Some(1000)),
            UsageLevel::Neutral
        );
        assert_eq!(
            coding_tier_level(50.0, Some(500), None),
            UsageLevel::Neutral
        );
        assert_eq!(
            coding_tier_level(-1.0, Some(500), Some(1000)),
            UsageLevel::Neutral
        );
    }

    #[test]
    fn coding_depleted_is_red() {
        // 配额耗尽（util≥100）→ Red，绕过差额算法。weekly 剩 2d（elapsed 71%）按差额会判绿。
        let cycle = 7 * 24 * 3_600_000; // weekly ms
        let remain = 2 * 24 * 3_600_000; // 剩 2d
        assert_eq!(
            coding_tier_level(100.0, Some(remain), Some(cycle)),
            UsageLevel::Red
        );
        // 上溢（异常但不可用）也红
        assert_eq!(
            coding_tier_level(150.0, Some(remain), Some(cycle)),
            UsageLevel::Red
        );
        // 边界：99.9 < 100，不走短路，仍按差额算。时间也快用完（剩 1h，elapsed 99.4%）
        // → 差额 +0.5pp → 黄，证明短路只在 util≥100 时接管。
        assert_eq!(
            coding_tier_level(99.9, Some(HOUR_MS), Some(cycle)),
            UsageLevel::Yellow
        );
    }

    #[test]
    fn balance_thresholds() {
        assert_eq!(balance_level(Some(0.5)), UsageLevel::Red);
        assert_eq!(balance_level(Some(2.0)), UsageLevel::Yellow);
        assert_eq!(balance_level(Some(5.0)), UsageLevel::Green);
        assert_eq!(balance_level(None), UsageLevel::Neutral);
    }
}
