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

// ── Coding plan tier：剩余可用时间% 阈值 ──────────────────────
// 剩余可用时间% = clamp(100 / pace, 0, 100)，pace = 额度已用比 / 时间已过比。
/// 剩余可用时间% < 40 → 红（pace > 2.5，烧太快撑不到重置）
pub const CODING_REMAIN_PCT_DANGER: f64 = 40.0;
/// 40 ≤ 剩余 ≤ 60 → 黄（pace 1.67~2.5）；> 60 → 绿（pace < 1.67）
pub const CODING_REMAIN_PCT_WARN: f64 = 60.0;

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

/// Coding plan tier 配色（按 pace / 剩余可用时间%）。
///
/// 入参：
///   - `utilization`：额度已用百分比（0-100）
///   - `remain_ms`：本周期剩余时间（ms）；None = 无可靠 remain（无 resets_at / 无 window_start）
///   - `cycle_ms`：周期时长（ms）；None = 未知 name（无周期概念）
///
/// 缺 remain / cycle / 利用率非法 → `Neutral`（不静默走旧利用率阈值，不误报）。
pub fn coding_tier_level(utilization: f64, remain_ms: Option<i64>, cycle_ms: Option<i64>) -> UsageLevel {
    if !utilization.is_finite() || utilization < 0.0 {
        return UsageLevel::Neutral;
    }
    let (remain, cycle) = match (remain_ms, cycle_ms) {
        (Some(r), Some(c)) if c > 0 => (r as f64, c as f64),
        _ => return UsageLevel::Neutral,
    };
    // 配额已耗尽（util≥100，剩余=0）→ 直接 Red。pace 算法衡量「按当前燃烧速度能否撑到周期末」，
    // 但配额耗尽后该语义失效（已无可用，撑不撑得到无意义），按时间维度判绿会与现实矛盾。
    if utilization >= 100.0 {
        return UsageLevel::Red;
    }
    let remain_pct = coding_remain_pct(utilization, remain, cycle);
    level_from_coding_remain_pct(remain_pct)
}

/// 剩余可用时间% = clamp(100 / pace, 0, 100)；pace = util_ratio / elapsed_ratio。
/// pace < 1（省着用）→ 100% 充足；elapsed_ratio → 0 时 pace → ∞ → 0%。
pub fn coding_remain_pct(utilization: f64, remain_ms: f64, cycle_ms: f64) -> f64 {
    let util_ratio = (utilization / 100.0).clamp(0.0, 1.0);
    let elapsed_ratio = ((cycle_ms - remain_ms) / cycle_ms).clamp(0.0, 1.0);
    if util_ratio <= 0.0 {
        return 100.0; // 未消耗 → 充足
    }
    if elapsed_ratio <= 0.0 {
        return 0.0; // 周期刚开始却已有消耗 → pace → ∞ → 0% 充足
    }
    let pace = util_ratio / elapsed_ratio;
    if pace <= 0.0 {
        return 100.0;
    }
    (100.0 / pace).clamp(0.0, 100.0)
}

/// 剩余可用时间% → 级别。<40 红 / 40-60 黄 / >60 绿。
pub fn level_from_coding_remain_pct(remain_pct: f64) -> UsageLevel {
    if !remain_pct.is_finite() {
        return UsageLevel::Neutral;
    }
    if remain_pct < CODING_REMAIN_PCT_DANGER {
        UsageLevel::Red
    } else if remain_pct <= CODING_REMAIN_PCT_WARN {
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

    // pace = util_ratio / elapsed_ratio。验收：pace=2.0→黄 / 3.0→红 / 1.2→绿。
    #[test]
    fn coding_pace_2_is_yellow() {
        // util=50%, 时间过半 (elapsed 0.5) → pace = 0.5/0.5 = ... 需 pace=2:
        // util_ratio=0.5, elapsed_ratio=0.25 → pace=2.0 → remain%=50 → 黄
        let cycle = 168.0 * 3600.0 * 1000.0;
        let remain = cycle * 0.75; // elapsed 0.25
        let pct = coding_remain_pct(50.0, remain, cycle);
        assert!((pct - 50.0).abs() < 1e-6, "remain% = {pct}");
        assert_eq!(level_from_coding_remain_pct(pct), UsageLevel::Yellow);
    }

    #[test]
    fn coding_pace_3_is_red() {
        // util_ratio=0.6, elapsed_ratio=0.2 → pace=3.0 → remain%≈33.3 → 红
        let cycle = 5.0 * 3600.0 * 1000.0;
        let remain = cycle * 0.8;
        let pct = coding_remain_pct(60.0, remain, cycle);
        assert!((pct - 100.0 / 3.0).abs() < 1e-6, "remain% = {pct}");
        assert_eq!(level_from_coding_remain_pct(pct), UsageLevel::Red);
    }

    #[test]
    fn coding_pace_1_2_is_green() {
        // util_ratio=0.6, elapsed_ratio=0.5 → pace=1.2 → remain%≈83.3 → 绿
        let cycle = 5.0 * 3600.0 * 1000.0;
        let remain = cycle * 0.5;
        let pct = coding_remain_pct(60.0, remain, cycle);
        assert!((pct - 100.0 / 1.2).abs() < 1e-6, "remain% = {pct}");
        assert_eq!(level_from_coding_remain_pct(pct), UsageLevel::Green);
    }

    #[test]
    fn coding_under_budget_is_green() {
        // pace<1（省着用）→ remain% clamp 100 → 绿
        let cycle = 5.0 * 3600.0 * 1000.0;
        let remain = cycle * 0.1; // elapsed 0.9
        let pct = coding_remain_pct(10.0, remain, cycle);
        assert!((pct - 100.0).abs() < 1e-6, "remain% = {pct}");
        assert_eq!(level_from_coding_remain_pct(pct), UsageLevel::Green);
    }

    #[test]
    fn coding_no_data_is_neutral() {
        assert_eq!(coding_tier_level(50.0, None, Some(1000)), UsageLevel::Neutral);
        assert_eq!(coding_tier_level(50.0, Some(500), None), UsageLevel::Neutral);
        assert_eq!(coding_tier_level(-1.0, Some(500), Some(1000)), UsageLevel::Neutral);
    }

    #[test]
    fn coding_depleted_is_red() {
        // 配额耗尽（util≥100）→ Red，绕过 pace。weekly 剩 2d（elapsed≈0.71）按 pace 会判绿。
        let cycle = 7 * 24 * 3_600_000; // weekly ms
        let remain = 2 * 24 * 3_600_000; // 剩 2d
        assert_eq!(coding_tier_level(100.0, Some(remain), Some(cycle)), UsageLevel::Red);
        // 上溢（异常但不可用）也红
        assert_eq!(coding_tier_level(150.0, Some(remain), Some(cycle)), UsageLevel::Red);
        // 边界：99.9 仍走 pace（非本修复目标）
        assert_ne!(coding_tier_level(99.9, Some(remain), Some(cycle)), UsageLevel::Red);
    }

    #[test]
    fn balance_thresholds() {
        assert_eq!(balance_level(Some(0.5)), UsageLevel::Red);
        assert_eq!(balance_level(Some(2.0)), UsageLevel::Yellow);
        assert_eq!(balance_level(Some(5.0)), UsageLevel::Green);
        assert_eq!(balance_level(None), UsageLevel::Neutral);
    }
}
