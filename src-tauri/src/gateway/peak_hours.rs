//! 高峰/低峰时段倍率（peak_hours）：多窗口数组 + first-match 倍率解析。
//!
//! 真值源同 `platform-presets.json`（`include_str!` 编入二进制，禁抄第二份）。
//! `calc_est_cost` 按 `platform.extra.peak_hours`（用户覆盖）→ bundled preset default
//! → 1.0 的混合源拿窗口，再 first-match 命中算 multiplier。

use chrono::{DateTime, Utc};
use serde::Deserialize;
use std::sync::OnceLock;

/// 单个时段窗口（UTC+0 基准）。serde 字段名直接对齐 JSON / TS `PeakWindow`。
///
/// 向后兼容：旧数据无 `start_minute` / `end_minute` / `days_of_month` → None
/// （`start_minute`/`end_minute` None=0，`days_of_month` None=不过滤）。
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct PeakWindow {
    pub start_hour: i32,
    pub end_hour: i32,
    pub multiplier: f64,
    #[serde(default)]
    pub days_of_week: Option<Vec<i32>>,
    /// 分钟精度起点（0-59）；缺省 = 0（仅 hour 精度，向后兼容）。
    #[serde(default)]
    pub start_minute: Option<i32>,
    /// 分钟精度终点（0-59）；缺省 = 0（仅 hour 精度，向后兼容）。
    #[serde(default)]
    pub end_minute: Option<i32>,
    /// 月内日过滤（1-31）；缺省 = 不过滤；与 `days_of_week` 在 UI 层互斥（hit 层同时 Some 时取 AND 兜底）。
    #[serde(default)]
    pub days_of_month: Option<Vec<i32>>,
}

/// bundled preset 缓存：首次访问解析一次 `platform-presets.json`，后续直接索引。
/// 解析失败（不应发生，JSON 已校验）回退空 Map → `default_peak_hours` 返空 → caller 退 1.0。
static PRESETS: OnceLock<serde_json::Value> = OnceLock::new();

const BUNDLED: &str = include_str!("../../defaults/platform-presets.json");

fn presets() -> &'static serde_json::Value {
    PRESETS.get_or_init(|| {
        serde_json::from_str(BUNDLED).unwrap_or_else(|e| {
            tracing::warn!(error = %e, "platform-presets.json parse failed in peak_hours; preset defaults disabled");
            serde_json::Value::Object(serde_json::Map::new())
        })
    })
}

/// t 的 UTC 小时 (0-23) 与 weekday (0=Sun…6=Sat)。
///
/// 历史入口保留（兼容现有 caller）；需 minute / day_of_month 用 `utc_time`。
#[allow(dead_code)]
pub fn utc_hour_weekday(epoch_ms: i64) -> (i32, i32) {
    let (hour, _minute, weekday, _dom) = utc_time(epoch_ms);
    (hour, weekday)
}

/// t 的 UTC 全时间分量：hour (0-23)、minute (0-59)、weekday (0=Sun…6=Sat)、day_of_month (1-31)。
pub fn utc_time(epoch_ms: i64) -> (i32, i32, i32, i32) {
    // ponytail: chrono 已是依赖，直接用，免手算 1970-01-01=Thursday 偏移。
    let dt = DateTime::<Utc>::from_timestamp_millis(epoch_ms)
        .unwrap_or_else(|| DateTime::<Utc>::from_timestamp(0, 0).unwrap());
    use chrono::{Datelike, Timelike};
    let hour = dt.hour() as i32;
    let minute = dt.minute() as i32;
    // chrono weekday(): Mon=0…Sun=6 → 转 0=Sun…6=Sat。
    let wd_chrono = dt.weekday().num_days_from_monday() as i32;
    let weekday = (wd_chrono + 1) % 7;
    let day_of_month = dt.day() as i32;
    (hour, minute, weekday, day_of_month)
}

/// 当前时刻命中窗口？支持 minute 精度 + days_of_week / days_of_month 过滤。
///
/// - 时间比较用绝对分钟半开区间 `[start_min, end_min)`：
///   - 同天 (end_min > start_min)：`start_min <= t_min && t_min < end_min`
///   - 跨天 (end_min <= start_min)：`t_min >= start_min || t_min < end_min`
///   - 退化 (end_min == start_min)：全天命中（兼容旧 hour 精度 start==end 语义）
/// - days_of_week / days_of_month 同时 Some → AND（UI 保证互斥，此为兜底）
pub(crate) fn hit(w: &PeakWindow, hour: i32, minute: i32, weekday: i32, day_of_month: i32) -> bool {
    if let Some(days) = &w.days_of_week {
        if !days.contains(&weekday) {
            return false;
        }
    }
    if let Some(days) = &w.days_of_month {
        if !days.contains(&day_of_month) {
            return false;
        }
    }
    let t_min = hour * 60 + minute;
    let start_min = w.start_hour * 60 + w.start_minute.unwrap_or(0).clamp(0, 59);
    let end_min = w.end_hour * 60 + w.end_minute.unwrap_or(0).clamp(0, 59);
    if end_min > start_min {
        t_min >= start_min && t_min < end_min
    } else {
        // 跨天（含 start==end 的退化情况，按全天命中处理）
        t_min >= start_min || t_min < end_min
    }
}

/// first-match multiplier；空 / 无命中 = 1.0。
pub fn resolve_multiplier(windows: &[PeakWindow], epoch_ms: i64) -> f64 {
    if windows.is_empty() {
        return 1.0;
    }
    let (hour, minute, weekday, day_of_month) = utc_time(epoch_ms);
    for w in windows {
        if hit(w, hour, minute, weekday, day_of_month) {
            return w.multiplier;
        }
    }
    1.0
}

/// first-match 命中任一窗口（跨天 + days_of_week / days_of_month）→ true。空 / 无命中 → false。
/// 与 `resolve_multiplier` 共享 `hit` 判定，仅返回 bool（caller 不关心 multiplier 值）。
/// `disable_during_peak` 开关的路由排除用此函数（不调 multiplier）。
pub fn is_in_peak_window(windows: &[PeakWindow], epoch_ms: i64) -> bool {
    if windows.is_empty() {
        return false;
    }
    let (hour, minute, weekday, day_of_month) = utc_time(epoch_ms);
    windows.iter().any(|w| hit(w, hour, minute, weekday, day_of_month))
}

/// 混合源取某平台 peak_hours 窗口：用户 `extra.peak_hours` 覆盖优先；空/缺 → bundled preset 默认。
/// 等价于 `db::stats_today::platform_peak_hours` 的纯函数版（无 DB 查询），供路由层直接用。
pub fn peak_hours_for(extra: &str, protocol: &str) -> Vec<PeakWindow> {
    let user = parse_platform_peak_hours(extra);
    if !user.is_empty() {
        return user;
    }
    default_peak_hours(protocol)
}

/// 从 `platform.extra` JSON 解析 `disable_during_peak` 字段；缺失/非法/非 bool → false（默认）。
/// 与 `parse_platform_peak_hours` / `parse_breaker` 同模式：extra 是 JSON 字符串 blob，禁加 Rust struct 字段。
pub fn parse_disable_during_peak(extra: &str) -> bool {
    if extra.trim().is_empty() {
        return false;
    }
    let Ok(v) = serde_json::from_str::<serde_json::Value>(extra) else {
        return false;
    };
    v.get("disable_during_peak").and_then(|x| x.as_bool()).unwrap_or(false)
}

/// 按 protocol 名（serde rename 裸名，如 "deepseek"）查 bundled preset 默认窗口。
/// protocol 缺失 / 无 peak_hours 字段 / 解析失败 → 空 Vec（caller 退 1.0）。
pub fn default_peak_hours(protocol: &str) -> Vec<PeakWindow> {
    let doc = presets();
    let Some(proto_obj) = doc.get("protocols").and_then(|p| p.get(protocol)) else {
        return Vec::new();
    };
    let Some(arr) = proto_obj.get("peak_hours") else {
        return Vec::new();
    };
    serde_json::from_value(arr.clone()).unwrap_or_else(|e| {
        tracing::warn!(error = %e, protocol, "peak_hours preset parse failed; skipping");
        Vec::new()
    })
}

/// 从 `platform.extra` JSON 字符串解析 `peak_hours` 字段；非法 / 缺失 → 空。
pub fn parse_platform_peak_hours(extra: &str) -> Vec<PeakWindow> {
    if extra.trim().is_empty() {
        return Vec::new();
    }
    let Ok(v) = serde_json::from_str::<serde_json::Value>(extra) else {
        return Vec::new();
    };
    let Some(arr) = v.get("peak_hours") else {
        return Vec::new();
    };
    serde_json::from_value(arr.clone()).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn w(start: i32, end: i32, mult: f64) -> PeakWindow {
        PeakWindow {
            start_hour: start,
            end_hour: end,
            multiplier: mult,
            days_of_week: None,
            start_minute: None,
            end_minute: None,
            days_of_month: None,
        }
    }

    fn wd(start: i32, end: i32, mult: f64, days: Vec<i32>) -> PeakWindow {
        PeakWindow {
            start_hour: start,
            end_hour: end,
            multiplier: mult,
            days_of_week: Some(days),
            start_minute: None,
            end_minute: None,
            days_of_month: None,
        }
    }

    /// minute 精度窗口构造 helper
    fn w_min(start_h: i32, start_m: i32, end_h: i32, end_m: i32, mult: f64) -> PeakWindow {
        PeakWindow {
            start_hour: start_h,
            end_hour: end_h,
            multiplier: mult,
            days_of_week: None,
            start_minute: Some(start_m),
            end_minute: Some(end_m),
            days_of_month: None,
        }
    }

    /// day_of_month 窗口构造 helper
    fn w_dom(start: i32, end: i32, mult: f64, doms: Vec<i32>) -> PeakWindow {
        PeakWindow {
            start_hour: start,
            end_hour: end,
            multiplier: mult,
            days_of_week: None,
            start_minute: None,
            end_minute: None,
            days_of_month: Some(doms),
        }
    }

    // ── hit / 跨天 / 同天 ──

    #[test]
    fn hit_same_day() {
        let win = w(14, 22, 1.5);
        assert!(hit(&win, 14, 0, 1, 15)); // 含起始
        assert!(hit(&win, 21, 59, 1, 15)); // 不含结束前一刻
        assert!(!hit(&win, 22, 0, 1, 15));
        assert!(!hit(&win, 10, 0, 1, 15));
    }

    #[test]
    fn hit_cross_midnight() {
        let win = w(22, 6, 1.5); // 22:00-06:00 次日
        assert!(hit(&win, 23, 0, 1, 15));
        assert!(hit(&win, 0, 0, 1, 15));
        assert!(hit(&win, 5, 59, 1, 15));
        assert!(!hit(&win, 6, 0, 1, 15));
        assert!(!hit(&win, 12, 0, 1, 15));
    }

    #[test]
    fn hit_days_of_week_filter() {
        let win = wd(0, 24, 0.8, vec![0, 6]); // 周末全天
        assert!(hit(&win, 3, 0, 0, 15)); // Sunday
        assert!(hit(&win, 3, 0, 6, 15)); // Saturday
        assert!(!hit(&win, 3, 0, 1, 15)); // Monday
    }

    // ── minute 精度 ──

    #[test]
    fn hit_minute_precision_same_day() {
        // 01:01:00 - 02:01:00
        let win = w_min(1, 1, 2, 1, 1.5);
        assert!(hit(&win, 1, 1, 1, 15)); // 起点命中
        assert!(hit(&win, 1, 30, 1, 15));
        assert!(hit(&win, 2, 0, 1, 15)); // 02:00 仍命中
        assert!(!hit(&win, 1, 0, 1, 15)); // 01:00 早于起点
        assert!(!hit(&win, 2, 1, 1, 15)); // 终点不命中（半开）
    }

    #[test]
    fn hit_minute_precision_cross_midnight() {
        // 23:30 - 00:30 次日（跨天）
        let win = w_min(23, 30, 0, 30, 1.5);
        assert!(hit(&win, 23, 30, 1, 15));
        assert!(hit(&win, 23, 59, 1, 15));
        assert!(hit(&win, 0, 0, 1, 15));
        assert!(hit(&win, 0, 29, 1, 15));
        assert!(!hit(&win, 0, 30, 1, 15)); // 终点不命中
        assert!(!hit(&win, 12, 0, 1, 15));
    }

    #[test]
    fn hit_minute_zero_default_backward_compat() {
        // 旧数据无 start_minute/end_minute → None=0；窗口 14:00-22:00（hour 精度）
        let win = w(14, 22, 1.5);
        // 14:30 命中（minute 字段被传入但仍按 hour*60+0 边界）
        assert!(hit(&win, 14, 30, 1, 15));
        // 22:30 不命中
        assert!(!hit(&win, 22, 30, 1, 15));
    }

    // ── day_of_month ──

    #[test]
    fn hit_days_of_month_filter() {
        // 每月 1 日 00:00-24:00
        let win = w_dom(0, 24, 0.8, vec![1]);
        assert!(hit(&win, 12, 0, 1, 1)); // 1 日命中
        assert!(!hit(&win, 12, 0, 1, 2)); // 2 日不命中
    }

    #[test]
    fn hit_days_of_month_and_week_mutually_some_and_defense() {
        // 同时 Some（UI 应避免，hit 层 AND 兜底）：周日(0) 且 15 日
        let win = PeakWindow {
            start_hour: 0,
            end_hour: 24,
            multiplier: 1.0,
            days_of_week: Some(vec![0]),
            start_minute: None,
            end_minute: None,
            days_of_month: Some(vec![15]),
        };
        assert!(hit(&win, 12, 0, 0, 15)); // 周日 + 15 日 → 命中
        assert!(!hit(&win, 12, 0, 0, 16)); // 周日 + 16 日 → day_of_month 不过
        assert!(!hit(&win, 12, 0, 1, 15)); // 周一 + 15 日 → weekday 不过
    }

    // ── resolve_multiplier / first-match ──

    #[test]
    fn resolve_first_match_wins() {
        let windows = vec![w(0, 12, 1.5), w(6, 18, 1.2)];
        // hour=8 同时落在两窗口，第一个 (0-12) 命中 → 1.5
        // epoch 2024-01-01T08:00:00Z (Mon) → hour=8
        let ms = DateTime::<Utc>::from_timestamp(1704105600, 0).unwrap().timestamp_millis();
        assert_eq!(resolve_multiplier(&windows, ms), 1.5);
    }

    #[test]
    fn resolve_no_hit_returns_one() {
        let windows = vec![w(0, 6, 0.5)];
        // hour=12 不在 [0,6)
        let ms = DateTime::<Utc>::from_timestamp(1704105600, 0).unwrap().timestamp_millis();
        assert_eq!(resolve_multiplier(&windows, ms), 1.0);
    }

    #[test]
    fn resolve_empty_returns_one() {
        let ms = 1_700_000_000_000;
        assert_eq!(resolve_multiplier(&[], ms), 1.0);
    }

    // ── utc_hour_weekday / utc_time ──

    #[test]
    fn utc_hour_weekday_sunday() {
        // 2024-01-07T02:50:00Z 是 Sunday 02:50 UTC（timestamp 1704595800）
        let ms = DateTime::<Utc>::from_timestamp(1704595800, 0).unwrap().timestamp_millis();
        let (h, wd) = utc_hour_weekday(ms);
        assert_eq!(h, 2);
        assert_eq!(wd, 0); // Sunday
    }

    #[test]
    fn utc_time_returns_minute_and_day_of_month() {
        // 2024-01-07T02:50:00Z → Sunday, hour=2, minute=50, day=7
        let ms = DateTime::<Utc>::from_timestamp(1704595800, 0).unwrap().timestamp_millis();
        let (h, m, wd, dom) = utc_time(ms);
        assert_eq!(h, 2);
        assert_eq!(m, 50);
        assert_eq!(wd, 0); // Sunday
        assert_eq!(dom, 7);
    }

    // ── 向后兼容（旧 JSON 无新字段 → None）──

    #[test]
    fn peak_window_backward_compat_no_new_fields() {
        // 旧数据仅含 start_hour / end_hour / multiplier / days_of_week
        let json = r#"{"start_hour":14,"end_hour":22,"multiplier":1.5}"#;
        let parsed: PeakWindow = serde_json::from_str(json).unwrap();
        assert_eq!(parsed.start_hour, 14);
        assert_eq!(parsed.end_hour, 22);
        assert_eq!(parsed.multiplier, 1.5);
        assert_eq!(parsed.start_minute, None);
        assert_eq!(parsed.end_minute, None);
        assert_eq!(parsed.days_of_month, None);
    }

    #[test]
    fn peak_window_full_schema_with_minute_and_month() {
        let json = r#"{
            "start_hour": 1,
            "end_hour": 2,
            "multiplier": 1.5,
            "start_minute": 1,
            "end_minute": 1,
            "days_of_month": [1, 15]
        }"#;
        let parsed: PeakWindow = serde_json::from_str(json).unwrap();
        assert_eq!(parsed.start_minute, Some(1));
        assert_eq!(parsed.end_minute, Some(1));
        assert_eq!(parsed.days_of_month, Some(vec![1, 15]));
    }

    // ── parse_platform_peak_hours ──

    #[test]
    fn parse_extra_peak_hours_user_override() {
        let extra = r#"{"peak_hours":[{"start_hour":14,"end_hour":22,"multiplier":1.5}]}"#;
        let v = parse_platform_peak_hours(extra);
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].multiplier, 1.5);
    }

    #[test]
    fn parse_extra_empty_returns_empty() {
        assert!(parse_platform_peak_hours("").is_empty());
        assert!(parse_platform_peak_hours("not-json").is_empty());
        assert!(parse_platform_peak_hours("{}").is_empty());
    }

    // ── default_peak_hours ──

    #[test]
    fn default_peak_hours_unknown_protocol_empty() {
        assert!(default_peak_hours("__never_exists__").is_empty());
    }

    #[test]
    fn default_peak_hours_anthropic_currently_empty() {
        // 当前 preset JSON 未手填 peak_hours，absent → 空（向后兼容）
        assert!(default_peak_hours("anthropic").is_empty());
    }

    // ── is_in_peak_window ──

    #[test]
    fn is_in_peak_window_hit() {
        let windows = vec![w(14, 22, 1.5)];
        // 2024-01-01T15:00:00Z → hour=15 落在 [14,22) → 命中（基准 1704067200=2024-01-01T00:00:00Z）
        let ms = DateTime::<Utc>::from_timestamp(1704067200 + 15 * 3600, 0).unwrap().timestamp_millis();
        assert!(is_in_peak_window(&windows, ms));
    }

    #[test]
    fn is_in_peak_window_cross_midnight() {
        let windows = vec![w(22, 6, 1.5)]; // 跨天 22-06
        let ms_in = DateTime::<Utc>::from_timestamp(1704067200 + 23 * 3600, 0).unwrap().timestamp_millis(); // 23:00 UTC → 命中
        let ms_out = DateTime::<Utc>::from_timestamp(1704067200 + 15 * 3600, 0).unwrap().timestamp_millis(); // 15:00 UTC → 不命中
        assert!(is_in_peak_window(&windows, ms_in));
        assert!(!is_in_peak_window(&windows, ms_out));
    }

    #[test]
    fn is_in_peak_window_no_match() {
        let windows = vec![w(0, 6, 0.5)];
        // hour=12 不在 [0,6)（基准 1704067200=2024-01-01T00:00:00Z）
        let ms = DateTime::<Utc>::from_timestamp(1704067200 + 12 * 3600, 0).unwrap().timestamp_millis();
        assert!(!is_in_peak_window(&windows, ms));
    }

    #[test]
    fn is_in_peak_window_empty() {
        let ms = 1_700_000_000_000;
        assert!(!is_in_peak_window(&[], ms));
    }

    // ── peak_hours_for（混合源）──

    #[test]
    fn peak_hours_for_user_override_wins() {
        let user_extra = r#"{"peak_hours":[{"start_hour":14,"end_hour":22,"multiplier":1.5}]}"#;
        // preset 默认 anthropic 当前为空；用户覆盖应优先
        let v = peak_hours_for(user_extra, "anthropic");
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].multiplier, 1.5);
    }

    #[test]
    fn peak_hours_for_empty_extra_falls_back_preset() {
        // anthropic 当前 preset 无 peak_hours → 空 Vec（caller 退 1.0 / is_in_peak_window=false）
        assert!(peak_hours_for("", "anthropic").is_empty());
        assert!(peak_hours_for("{}", "anthropic").is_empty());
    }

    // ── parse_disable_during_peak ──

    #[test]
    fn parse_disable_during_peak_true() {
        assert!(parse_disable_during_peak(r#"{"disable_during_peak":true}"#));
    }

    #[test]
    fn parse_disable_during_peak_false_default() {
        // 缺失 / false / 非法 / 非布尔 → false
        assert!(!parse_disable_during_peak(""));
        assert!(!parse_disable_during_peak("not-json"));
        assert!(!parse_disable_during_peak("{}"));
        assert!(!parse_disable_during_peak(r#"{"disable_during_peak":false}"#));
        // 非布尔值（数字/字符串）→ false，禁把 "true" 字符串误判
        assert!(!parse_disable_during_peak(r#"{"disable_during_peak":"true"}"#));
        assert!(!parse_disable_during_peak(r#"{"disable_during_peak":1}"#));
    }
}
