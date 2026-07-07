//! 高峰/低峰时段倍率（peak_hours）：多窗口数组 + first-match 倍率解析。
//!
//! 真值源同 `platform-presets.json`（`include_str!` 编入二进制，禁抄第二份）。
//! `calc_est_cost` 按 `platform.extra.peak_hours`（用户覆盖）→ bundled preset default
//! → 1.0 的混合源拿窗口，再 first-match 命中算 multiplier。

use chrono::{DateTime, Utc};
use serde::Deserialize;
use std::sync::OnceLock;

/// 单个时段窗口（UTC+0 基准）。serde 字段名直接对齐 JSON / TS `PeakWindow`。
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct PeakWindow {
    pub start_hour: i32,
    pub end_hour: i32,
    pub multiplier: f64,
    #[serde(default)]
    pub days_of_week: Option<Vec<i32>>,
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
pub fn utc_hour_weekday(epoch_ms: i64) -> (i32, i32) {
    // ponytail: chrono 已是依赖，直接用，免手算 1970-01-01=Thursday 偏移。
    let dt = DateTime::<Utc>::from_timestamp_millis(epoch_ms)
        .unwrap_or_else(|| DateTime::<Utc>::from_timestamp(0, 0).unwrap());
    use chrono::{Datelike, Timelike};
    let hour = dt.hour() as i32;
    // chrono weekday(): Mon=0…Sun=6 → 转 0=Sun…6=Sat。
    let wd_chrono = dt.weekday().num_days_from_monday() as i32;
    let weekday = (wd_chrono + 1) % 7;
    (hour, weekday)
}

/// hour 命中窗口？days_of_week 过滤 + 跨天 (end<start) / 同天 [start,end) 半开判定。
fn hit(w: &PeakWindow, hour: i32, weekday: i32) -> bool {
    if let Some(days) = &w.days_of_week {
        if !days.contains(&weekday) {
            return false;
        }
    }
    if w.end_hour > w.start_hour {
        hour >= w.start_hour && hour < w.end_hour
    } else {
        // 跨天（含 start==end 的退化情况，按全天命中处理）
        hour >= w.start_hour || hour < w.end_hour
    }
}

/// first-match multiplier；空 / 无命中 = 1.0。
pub fn resolve_multiplier(windows: &[PeakWindow], epoch_ms: i64) -> f64 {
    if windows.is_empty() {
        return 1.0;
    }
    let (hour, weekday) = utc_hour_weekday(epoch_ms);
    for w in windows {
        if hit(w, hour, weekday) {
            return w.multiplier;
        }
    }
    1.0
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
        PeakWindow { start_hour: start, end_hour: end, multiplier: mult, days_of_week: None }
    }

    fn wd(start: i32, end: i32, mult: f64, days: Vec<i32>) -> PeakWindow {
        PeakWindow { start_hour: start, end_hour: end, multiplier: mult, days_of_week: Some(days) }
    }

    // ── hit / 跨天 / 同天 ──

    #[test]
    fn hit_same_day() {
        let win = w(14, 22, 1.5);
        assert!(hit(&win, 14, 1)); // 含起始
        assert!(hit(&win, 21, 1)); // 不含结束前一刻
        assert!(!hit(&win, 22, 1));
        assert!(!hit(&win, 10, 1));
    }

    #[test]
    fn hit_cross_midnight() {
        let win = w(22, 6, 1.5); // 22:00-06:00 次日
        assert!(hit(&win, 23, 1));
        assert!(hit(&win, 0, 1));
        assert!(hit(&win, 5, 1));
        assert!(!hit(&win, 6, 1));
        assert!(!hit(&win, 12, 1));
    }

    #[test]
    fn hit_days_of_week_filter() {
        let win = wd(0, 24, 0.8, vec![0, 6]); // 周末全天
        assert!(hit(&win, 3, 0)); // Sunday
        assert!(hit(&win, 3, 6)); // Saturday
        assert!(!hit(&win, 3, 1)); // Monday
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

    // ── utc_hour_weekday ──

    #[test]
    fn utc_hour_weekday_sunday() {
        // 2024-01-07T02:50:00Z 是 Sunday 02:50 UTC（timestamp 1704595800）
        let ms = DateTime::<Utc>::from_timestamp(1704595800, 0).unwrap().timestamp_millis();
        let (h, wd) = utc_hour_weekday(ms);
        assert_eq!(h, 2);
        assert_eq!(wd, 0); // Sunday
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
}
