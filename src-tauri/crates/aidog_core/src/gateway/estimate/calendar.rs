//! 固定北京时间的套餐周期边界。
//!
//! 短月取月末、下一个含订阅原日的月份恢复原日为 AiDog 本地约定，官方未定义。

use chrono::{Datelike, Duration, FixedOffset, NaiveDate, TimeZone, Utc};

/// 返回本周期起止时间（毫秒）。月周期缺有效订阅日期时返回 None。
pub(super) fn bounds(name: &str, subscription: Option<&str>, now_ms: i64) -> Option<(i64, i64)> {
    let tz = FixedOffset::east_opt(8 * 3600)?;
    let now = chrono::DateTime::<Utc>::from_timestamp_millis(now_ms)?.with_timezone(&tz);
    let today = now.date_naive();
    let midnight = |d: NaiveDate| {
        tz.from_local_datetime(&d.and_hms_opt(0, 0, 0)?)
            .single()
            .map(|t| t.timestamp_millis())
    };
    if name == "weekly_limit" {
        let start = today
            .checked_sub_signed(Duration::days(today.weekday().num_days_from_monday() as i64))?;
        return Some((
            midnight(start)?,
            midnight(start.checked_add_signed(Duration::days(7))?)?,
        ));
    }
    if name != "monthly" {
        return None;
    }
    let anchor = NaiveDate::parse_from_str(subscription?, "%Y-%m-%d").ok()?;
    if today < anchor {
        return None;
    }
    let at_month = |year: i32, month: u32| -> Option<NaiveDate> {
        (1..=anchor.day())
            .rev()
            .find_map(|day| NaiveDate::from_ymd_opt(year, month, day))
    };
    let shift = |year: i32, month: u32, delta: i32| -> (i32, u32) {
        let n = year * 12 + month as i32 - 1 + delta;
        (n.div_euclid(12), (n.rem_euclid(12) + 1) as u32)
    };
    let current = at_month(today.year(), today.month())?;
    let (sy, sm) = if today < current {
        shift(today.year(), today.month(), -1)
    } else {
        (today.year(), today.month())
    };
    let (ey, em) = shift(sy, sm, 1);
    Some((midnight(at_month(sy, sm)?)?, midnight(at_month(ey, em)?)?))
}

/// 更新日历档，返回该档本次是否可使用。
pub(super) fn refresh_tier(
    tier: &mut super::model::EstTier,
    subscription: Option<&str>,
    now_ms: i64,
) -> bool {
    let Some((start, end)) = bounds(&tier.name, subscription, now_ms) else {
        tier.window_start = 0;
        tier.window_end = 0;
        tier.est_utilization = 0.0;
        tier.requests_since_real = 0.0;
        return false;
    };
    let same_legacy_window =
        tier.window_end == 0 && tier.window_start >= start && tier.window_start < end;
    if !same_legacy_window && (tier.window_start != start || tier.window_end != end) {
        tier.est_utilization = 0.0;
        tier.util_at_last_real = 0.0;
        tier.requests_since_real = 0.0;
        tier.tokens_since_real = 0.0;
    }
    tier.window_start = start;
    tier.window_end = end;
    true
}

/// 千帆旧档没有 reset_mode，按固定档名称补上；其他平台原样返回。
pub(crate) fn current_plan(
    json: &str,
    extra: &str,
    is_qianfan: bool,
    now_ms: i64,
) -> super::model::EstCodingPlan {
    let mut plan = super::model::EstCodingPlan::from_json(json);
    if !is_qianfan {
        return plan;
    }
    let extra: serde_json::Value = serde_json::from_str(extra).unwrap_or_default();
    let date = extra
        .get("subscription_start_date")
        .and_then(|v| v.as_str());
    plan.tiers.retain_mut(|tier| {
        match tier.name.as_str() {
            "weekly_limit" => tier.reset_mode = "calendar_week".to_owned(),
            "monthly" => tier.reset_mode = "subscription_month".to_owned(),
            _ => return true,
        }
        refresh_tier(tier, date, now_ms)
    });
    plan
}

#[cfg(test)]
mod tests {
    use super::*;
    fn ts(s: &str) -> i64 {
        chrono::DateTime::parse_from_rfc3339(s)
            .unwrap()
            .timestamp_millis()
    }
    #[test]
    fn legacy_window_in_current_period_keeps_usage() {
        let now = ts("2026-02-28T12:00:00+08:00");
        let json = format!(
            r#"{{"tiers":[{{"name":"weekly_limit","est_utilization":42,"window_start":{}}}]}}"#,
            ts("2026-02-24T12:00:00+08:00")
        );
        let plan = current_plan(&json, "{}", true, now);
        assert_eq!(plan.tiers[0].est_utilization, 42.0);
        assert_eq!(plan.tiers[0].window_start, ts("2026-02-23T00:00:00+08:00"));
        assert_eq!(plan.tiers[0].window_end, ts("2026-03-02T00:00:00+08:00"));
    }
    #[test]
    fn old_tiers_reset_and_missing_date_skips_month() {
        let json = r#"{"tiers":[{"name":"weekly_limit","est_utilization":75,"window_start":1,"limit":100,"has_base":true,"unit":"prompt_count"},{"name":"monthly","est_utilization":80,"window_start":1,"limit":100,"has_base":true,"unit":"prompt_count"}]}"#;
        let now = ts("2026-02-28T00:00:00+08:00");
        let plan = current_plan(json, "{}", true, now);
        assert_eq!(plan.tiers.len(), 1);
        assert_eq!(plan.tiers[0].name, "weekly_limit");
        assert_eq!(plan.tiers[0].est_utilization, 0.0);
        assert_eq!(plan.tiers[0].reset_mode, "calendar_week");
        let plan = current_plan(
            json,
            r#"{"subscription_start_date":"2026-01-31"}"#,
            true,
            now,
        );
        assert_eq!(plan.tiers.len(), 2);
        assert_eq!(plan.tiers[1].est_utilization, 0.0);
        assert_eq!(plan.tiers[1].window_end, ts("2026-03-31T00:00:00+08:00"));
        let unchanged = current_plan(json, "{}", false, now);
        assert_eq!(unchanged.tiers[0].est_utilization, 75.0);
        assert_eq!(unchanged.tiers.len(), 2);
    }

    #[test]
    fn week_and_month_boundaries() {
        assert_eq!(
            bounds("weekly_limit", None, ts("2026-11-30T00:00:00+08:00")),
            Some((
                ts("2026-11-30T00:00:00+08:00"),
                ts("2026-12-07T00:00:00+08:00")
            ))
        );
        assert_eq!(
            bounds(
                "monthly",
                Some("2026-01-31"),
                ts("2026-02-28T00:00:00+08:00")
            ),
            Some((
                ts("2026-02-28T00:00:00+08:00"),
                ts("2026-03-31T00:00:00+08:00")
            ))
        );
        assert_eq!(
            bounds("monthly", None, ts("2026-02-28T00:00:00+08:00")),
            None
        );
    }
}
