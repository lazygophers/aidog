//! 高峰/低峰时段倍率（peak）：多窗口数组 + first-match 倍率解析。
//!
//! 真值源同 `platform-presets.json`（`include_str!` 编入二进制，禁抄第二份）。
//! `calc_est_cost` 按 `platform.extra.peak`（用户覆盖）→ bundled preset default
//! → 1.0 的混合源拿窗口，再 first-match 命中算 multiplier。

pub use aidog_db::models::TimeWindow;
use chrono::{DateTime, Utc};


/// registry preset 读取入口：`aidog_db::registry::effective_presets`（DB 同步值优先，
/// 未同步过回落编译期内置那份）。缓存句柄 clone 一次即用，热路径不做 DB IO、不重建文档。
/// 协议缺失 / 解析失败 → `default_peak` 返空 → caller 退 1.0。
use super::registry::effective_presets;

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
    wall_time(epoch_ms, None)
}

/// t 在 `tz`（IANA 名，None/非法 = UTC）的**本地**全时间分量（DST 由 tz 数据库处理）。
/// 窗口时区解析的统一入口：`hit` 的 hour/weekday/day_of_month 均以此值比较。
/// 与前端 `utils/timeWindow.ts::wallTimeInTz` 对称（跨层一致）。
pub fn wall_time(epoch_ms: i64, tz: Option<&str>) -> (i32, i32, i32, i32) {
    // ponytail: chrono 已是依赖，直接用，免手算 1970-01-01=Thursday 偏移。
    let dt = DateTime::<Utc>::from_timestamp_millis(epoch_ms)
        .unwrap_or_else(|| DateTime::<Utc>::from_timestamp(0, 0).unwrap());
    // naive_local 保留目标时区的墙面时钟分量（hour/weekday/dom），两分支统一形状。
    let local = match tz {
        Some(name) => match name.parse::<chrono_tz::Tz>() {
            Ok(z) => dt.with_timezone(&z).naive_local(),
            Err(_) => dt.naive_utc(), // 非法时区名回落 UTC（数据脏不炸热路径）
        },
        None => dt.naive_utc(),
    };
    use chrono::{Datelike, Timelike};
    let hour = local.hour() as i32;
    let minute = local.minute() as i32;
    // chrono weekday(): Mon=0…Sun=6 → 转 0=Sun…6=Sat。
    let wd_chrono = local.weekday().num_days_from_monday() as i32;
    let weekday = (wd_chrono + 1) % 7;
    let day_of_month = local.day() as i32;
    (hour, minute, weekday, day_of_month)
}

/// 当前时刻命中窗口？支持 minute 精度 + days_of_week / days_of_month 过滤。
///
/// - 时间比较用绝对分钟半开区间 `[start_min, end_min)`：
///   - 同天 (end_min > start_min)：`start_min <= t_min && t_min < end_min`
///   - 跨天 (end_min <= start_min)：`t_min >= start_min || t_min < end_min`
///   - 退化 (end_min == start_min)：全天命中（兼容旧 hour 精度 start==end 语义）
/// - days_of_week / days_of_month 同时 Some → AND（UI 保证互斥，此为兜底）
pub(crate) fn hit(w: &TimeWindow, hour: i32, minute: i32, weekday: i32, day_of_month: i32) -> bool {
    if let Some(days) = &w.days_of_week
        && !days.contains(&weekday) {
            return false;
        }
    if let Some(days) = &w.days_of_month
        && !days.contains(&day_of_month) {
            return false;
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
///
/// `request_model`：请求模型名（用于 model scope 过滤，PRD 07-09 D2）。
/// - `""`（空串）= 调用方无 model 上下文 → 跳过 model 过滤（兼容旧行为，向后兼容）
/// - 窗口 `models` = None → 全平台生效（向后兼容）
/// - 窗口 `models` = Some(patterns) → request_model 须匹配某 pattern（exact 或 `prefix*` 通配）
pub fn resolve_multiplier(windows: &[TimeWindow], epoch_ms: i64, request_model: &str) -> f64 {
    if windows.is_empty() {
        return 1.0;
    }
    let epoch_sec = epoch_ms / 1000;
    for w in windows {
        // 生效期判定优先（start_at/end_at，Unix 秒；未启用 / 已失效 → 跳过此窗口）
        if !period_active(w, epoch_sec) {
            continue;
        }
        // 每窗口各自时区的本地时刻（timezone 缺省 = UTC，向后兼容）
        let (hour, minute, weekday, day_of_month) = wall_time(epoch_ms, w.timezone.as_deref());
        if !hit(w, hour, minute, weekday, day_of_month) {
            continue;
        }
        if !window_models_hit(w, request_model) {
            continue;
        }
        return w.multiplier;
    }
    1.0
}

/// first-match 命中任一窗口（跨天 + days_of_week / days_of_month + model scope）→ true。
/// 空 / 无命中 → false。与 `resolve_multiplier` 共享 `hit` + `window_models_hit` 判定，
/// 仅返回 bool（caller 不关心 multiplier 值）。`disable_during_peak` 开关的路由排除用此函数。
///
/// `request_model` 语义同 `resolve_multiplier`：空串 = 无 model 上下文（跳过 model 过滤）。
pub fn is_in_peak_window(windows: &[TimeWindow], epoch_ms: i64, request_model: &str) -> bool {
    if windows.is_empty() {
        return false;
    }
    let epoch_sec = epoch_ms / 1000;
    windows.iter().any(|w| {
        let (hour, minute, weekday, day_of_month) = wall_time(epoch_ms, w.timezone.as_deref());
        period_active(w, epoch_sec)
            && hit(w, hour, minute, weekday, day_of_month)
            && window_models_hit(w, request_model)
    })
}

/// 窗口生效期判定（PRD 07-09 D2 福利期自动切换）：
/// - `start_at` Some 且 `epoch_sec < start_at` → 未启用 → false
/// - `end_at` Some 且 `epoch_sec >= end_at` → 已失效 → false
/// - 否则（含二者均 None = 永久/立即可用）→ true
///
/// 判定顺序：生效期 → 时间 → model（见 design §1.2，生效期优先级最高）。
fn period_active(w: &TimeWindow, epoch_sec: i64) -> bool {
    if let Some(s) = w.start_at
        && epoch_sec < s {
            return false;
        }
    if let Some(e) = w.end_at
        && epoch_sec >= e {
            return false;
        }
    true
}

/// 窗口 model scope 是否覆盖 `request_model`：
/// - `request_model == ""` → true（调用方无上下文，跳过过滤，兼容旧行为）
/// - `w.models == None` → true（窗口未限定，全平台生效）
/// - `w.models == Some(patterns)` → 任一 pattern 命中（exact 或通配）
fn window_models_hit(w: &TimeWindow, request_model: &str) -> bool {
    if request_model.is_empty() {
        return true;
    }
    match &w.models {
        None => true,
        Some(patterns) => patterns.iter().any(|p| model_match(p, request_model)),
    }
}

/// 单 pattern 与请求模型匹配：exact OR 前缀通配（`"glm-5.2*"` 覆盖 `glm-5.2` / `glm-5.2-turbo`）。
/// exact-first：非 `*` 结尾走精确匹配；`*` 结尾取前缀，`request_model == prefix` 或 `starts_with(prefix)`。
fn model_match(pattern: &str, request_model: &str) -> bool {
    if let Some(prefix) = pattern.strip_suffix('*') {
        request_model == prefix || request_model.starts_with(prefix)
    } else {
        request_model == pattern
    }
}

/// 混合源取某平台 peak 窗口：用户 `extra.peak` 覆盖优先；空/缺 → bundled preset 默认。
/// 等价于 `db::stats_today::platform_peak` 的纯函数版（无 DB 查询），供路由层直接用。
pub fn peak_for(extra: &str, protocol: &str) -> Vec<TimeWindow> {
    let user = parse_platform_peak(extra);
    if !user.is_empty() {
        return user;
    }
    default_peak(protocol)
}

/// 从 `platform.extra` JSON 解析 `disable_during_peak` 字段；缺失/非法/非 bool → false（默认）。
pub fn parse_disable_during_peak(extra: &str) -> bool {
    crate::gateway::models::PlatformExtra::parse(extra).disable_during_peak
}

/// 按 protocol 名（serde rename 裸名，如 "deepseek"）查生效 preset（DB 优先）的默认窗口。
/// protocol 缺失 / 无 peak 字段 / 解析失败 → 空 Vec（caller 退 1.0）。
pub fn default_peak(protocol: &str) -> Vec<TimeWindow> {
    let doc = effective_presets();
    peak_in(&doc, protocol)
}

/// [`default_peak`] 的纯函数核心：从任意一篇 presets 文档取某协议的窗口。
pub fn peak_in(doc: &serde_json::Value, protocol: &str) -> Vec<TimeWindow> {
    let Some(proto_obj) = doc.get("protocols").and_then(|p| p.get(protocol)) else {
        return Vec::new();
    };
    let Some(arr) = proto_obj.get("peak") else {
        return Vec::new();
    };
    serde_json::from_value(arr.clone()).unwrap_or_else(|e| {
        tracing::warn!(error = %e, protocol, "peak preset parse failed; skipping");
        Vec::new()
    })
}

/// 按 protocol 名查生效 preset（DB 优先）的 `models.peak` 分支（PRD 07-11）。
/// 返回解析后的 PlatformModels；protocol 缺失 / 无 models.peak 字段 / 解析失败 → None
/// （caller 退 platform.models 默认）。仅 glm_coding 等少数协议 preset 带 peak 分支。
///
/// 与 `default_peak` 同源：`effective_presets()`（DB 同步值优先，禁抄第二份）——
/// 否则前端徽标按新同步的窗口显示「当前: 高峰」，后端仍按二进制里的旧窗口路由计价。
/// 路由层 candidates.rs 命中高峰窗口且 preset 提供本协议 peak 分支时用此替换 effective_models。
pub fn default_peak_models(protocol: &str) -> Option<crate::gateway::models::PlatformModels> {
    peak_models_in(&effective_presets(), protocol)
}

/// [`default_peak_models`] 的纯函数核心。
pub fn peak_models_in(doc: &serde_json::Value, protocol: &str) -> Option<crate::gateway::models::PlatformModels> {
    let proto_obj = doc.get("protocols").and_then(|p| p.get(protocol))?;
    let models_obj = proto_obj.get("models")?;
    let peak_val = models_obj.get("peak")?;
    serde_json::from_value(peak_val.clone()).ok()
}

/// 从 `platform.extra` JSON 字符串解析 `peak` 字段；非法 / 缺失 → 空。
pub fn parse_platform_peak(extra: &str) -> Vec<TimeWindow> {
    crate::gateway::models::PlatformExtra::parse(extra).peak
}

#[cfg(test)]
mod tests {
    use super::*;

    fn w(start: i32, end: i32, mult: f64) -> TimeWindow {
        TimeWindow {
            start_hour: start,
            end_hour: end,
            multiplier: mult,
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

    fn wd(start: i32, end: i32, mult: f64, days: Vec<i32>) -> TimeWindow {
        TimeWindow {
            start_hour: start,
            end_hour: end,
            multiplier: mult,
            timezone: None,
            days_of_week: Some(days),
            start_minute: None,
            end_minute: None,
            days_of_month: None,
            models: None,
            start_at: None,
            end_at: None,
        }
    }

    /// minute 精度窗口构造 helper
    fn w_min(start_h: i32, start_m: i32, end_h: i32, end_m: i32, mult: f64) -> TimeWindow {
        TimeWindow {
            start_hour: start_h,
            end_hour: end_h,
            multiplier: mult,
            timezone: None,
            days_of_week: None,
            start_minute: Some(start_m),
            end_minute: Some(end_m),
            days_of_month: None,
            models: None,
            start_at: None,
            end_at: None,
        }
    }

    /// day_of_month 窗口构造 helper
    fn w_dom(start: i32, end: i32, mult: f64, doms: Vec<i32>) -> TimeWindow {
        TimeWindow {
            start_hour: start,
            end_hour: end,
            multiplier: mult,
            timezone: None,
            days_of_week: None,
            start_minute: None,
            end_minute: None,
            days_of_month: Some(doms),
            models: None,
            start_at: None,
            end_at: None,
        }
    }

    /// 带 model scope 的窗口构造 helper（PRD 07-09 D2）
    fn w_models(start: i32, end: i32, mult: f64, models: Vec<String>) -> TimeWindow {
        TimeWindow {
            start_hour: start,
            end_hour: end,
            multiplier: mult,
            timezone: None,
            days_of_week: None,
            start_minute: None,
            end_minute: None,
            days_of_month: None,
            models: Some(models),
            start_at: None,
            end_at: None,
        }
    }

    /// 带生效期窗口构造 helper（PRD 07-09 D2 福利期切换）
    fn w_period(start: i32, end: i32, mult: f64, start_at: Option<i64>, end_at: Option<i64>) -> TimeWindow {
        TimeWindow {
            start_hour: start,
            end_hour: end,
            multiplier: mult,
            timezone: None,
            days_of_week: None,
            start_minute: None,
            end_minute: None,
            days_of_month: None,
            models: None,
            start_at,
            end_at,
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
        let win = TimeWindow {
            start_hour: 0,
            end_hour: 24,
            multiplier: 1.0,
            timezone: None,
            days_of_week: Some(vec![0]),
            start_minute: None,
            end_minute: None,
            days_of_month: Some(vec![15]),
            models: None,
            start_at: None,
            end_at: None,
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
        assert_eq!(resolve_multiplier(&windows, ms, ""), 1.5);
    }

    #[test]
    fn resolve_no_hit_returns_one() {
        let windows = vec![w(0, 6, 0.5)];
        // hour=12 不在 [0,6)
        let ms = DateTime::<Utc>::from_timestamp(1704105600, 0).unwrap().timestamp_millis();
        assert_eq!(resolve_multiplier(&windows, ms, ""), 1.0);
    }

    #[test]
    fn resolve_empty_returns_one() {
        let ms = 1_700_000_000_000;
        assert_eq!(resolve_multiplier(&[], ms, ""), 1.0);
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

    // ── wall_time / timezone ──

    #[test]
    fn wall_time_shanghai_shifts_eight_hours() {
        // 2024-01-07T02:50:00Z = 北京时间 10:50 同日（+8，无跨日）
        let ms = DateTime::<Utc>::from_timestamp(1704595800, 0).unwrap().timestamp_millis();
        let (h, m, wd, dom) = wall_time(ms, Some("Asia/Shanghai"));
        assert_eq!(h, 10);
        assert_eq!(m, 50);
        assert_eq!(wd, 0); // 同日仍 Sunday
        assert_eq!(dom, 7);
        // 缺省 / None = UTC；非法名回落 UTC
        assert_eq!(wall_time(ms, None), wall_time(ms, Some("UTC")));
        assert_eq!(wall_time(ms, None), wall_time(ms, Some("not-a-tz")));
    }

    #[test]
    fn wall_time_shanghai_weekday_rolls() {
        // 2024-01-06T18:00:00Z（Saturday）= 北京时间 1 月 7 日 02:00（Sunday）
        let ms = DateTime::<Utc>::from_timestamp(1704564000, 0).unwrap().timestamp_millis();
        let (h, _m, wd, dom) = wall_time(ms, Some("Asia/Shanghai"));
        assert_eq!(h, 2);
        assert_eq!(wd, 0); // Sunday
        assert_eq!(dom, 7);
    }

    #[test]
    fn resolve_multiplier_respects_window_timezone() {
        // 窗口按北京时间 9-12 高峰 ×2：UTC 01:00（北京 09:00）命中；UTC 05:00（北京 13:00）不命中
        let win = TimeWindow {
            start_hour: 9,
            end_hour: 12,
            multiplier: 2.0,
            days_of_week: None,
            start_minute: None,
            end_minute: None,
            days_of_month: None,
            timezone: Some("Asia/Shanghai".to_string()),
            models: None,
            start_at: None,
            end_at: None,
        };
        let ms_hit = DateTime::<Utc>::from_timestamp(1704590800, 0).unwrap().timestamp_millis(); // 2024-01-07T01:26:40Z
        let ms_miss = DateTime::<Utc>::from_timestamp(1704605200, 0).unwrap().timestamp_millis(); // 2024-01-07T05:26:40Z
        assert_eq!(resolve_multiplier(std::slice::from_ref(&win), ms_hit, ""), 2.0);
        assert_eq!(resolve_multiplier(&[win], ms_miss, ""), 1.0);
    }

    #[test]
    fn resolve_multiplier_absent_timezone_uses_utc_backward_compat() {
        // 旧数据无 timezone：窗口 6-10 按 UTC 解释，行为与迁移前完全一致
        let win = TimeWindow {
            start_hour: 6,
            end_hour: 10,
            multiplier: 3.0,
            days_of_week: None,
            start_minute: None,
            end_minute: None,
            days_of_month: None,
            timezone: None,
            models: None,
            start_at: None,
            end_at: None,
        };
        let ms = DateTime::<Utc>::from_timestamp(1704590800, 0).unwrap().timestamp_millis(); // UTC 01:26
        assert_eq!(resolve_multiplier(&[win], ms, ""), 1.0); // UTC 1 点不在 [6,10)
    }

    // ── 向后兼容（旧 JSON 无新字段 → None）──

    #[test]
    fn peak_window_backward_compat_no_new_fields() {
        // 旧数据仅含 start_hour / end_hour / multiplier / days_of_week
        let json = r#"{"start_hour":14,"end_hour":22,"multiplier":1.5}"#;
        let parsed: TimeWindow = serde_json::from_str(json).unwrap();
        assert_eq!(parsed.start_hour, 14);
        assert_eq!(parsed.end_hour, 22);
        assert_eq!(parsed.multiplier, 1.5);
        assert_eq!(parsed.start_minute, None);
        assert_eq!(parsed.end_minute, None);
        assert_eq!(parsed.days_of_month, None);
        assert_eq!(parsed.models, None, "旧 JSON 无 models 字段 → None（全平台生效）");
        assert_eq!(parsed.start_at, None, "旧 JSON 无 start_at → None（立即可用）");
        assert_eq!(parsed.end_at, None, "旧 JSON 无 end_at → None（永久）");
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
        let parsed: TimeWindow = serde_json::from_str(json).unwrap();
        assert_eq!(parsed.start_minute, Some(1));
        assert_eq!(parsed.end_minute, Some(1));
        assert_eq!(parsed.days_of_month, Some(vec![1, 15]));
    }

    #[test]
    fn peak_window_models_field_parse() {
        // 显式 models 字段：精确 + 通配 pattern 混合
        let json = r#"{
            "start_hour": 6,
            "end_hour": 10,
            "multiplier": 3.0,
            "models": ["glm-5.2", "glm-5-turbo"]
        }"#;
        let parsed: TimeWindow = serde_json::from_str(json).unwrap();
        assert_eq!(parsed.models, Some(vec!["glm-5.2".to_string(), "glm-5-turbo".to_string()]));
    }

    #[test]
    fn peak_window_models_null_treated_as_none() {
        // 显式 null → None（serde default + Option 双重保险）
        let json = r#"{"start_hour":6,"end_hour":10,"multiplier":3.0,"models":null}"#;
        let parsed: TimeWindow = serde_json::from_str(json).unwrap();
        assert_eq!(parsed.models, None);
    }

    // ── 生效期 start_at / end_at（PRD 07-09 D2 福利期自动切换）──

    #[test]
    fn peak_window_start_at_end_at_parse() {
        let json = r#"{
            "start_hour": 0,
            "end_hour": 24,
            "multiplier": 2.0,
            "models": ["glm-5.2"],
            "start_at": 1759276800,
            "end_at": 1800000000
        }"#;
        let parsed: TimeWindow = serde_json::from_str(json).unwrap();
        assert_eq!(parsed.start_at, Some(1759276800));
        assert_eq!(parsed.end_at, Some(1800000000));
    }

    #[test]
    fn resolve_multiplier_period_not_yet_started_skips() {
        // start_at 在未来（2000000000 = 2033-05-18）→ 当前 ts 1704067200（2024-01-01）尚未启用 → 跳过
        let windows = vec![w_period(0, 24, 2.0, Some(2_000_000_000), None)];
        let ms = DateTime::<Utc>::from_timestamp(1704067200, 0).unwrap().timestamp_millis();
        assert_eq!(resolve_multiplier(&windows, ms, ""), 1.0, "epoch < start_at → 未启用，返默认 1.0");
    }

    #[test]
    fn resolve_multiplier_period_expired_skips() {
        // end_at 在过去（1700000000 = 2023-11-14）→ 当前 ts 1704067200 已失效 → 跳过
        let windows = vec![w_period(0, 24, 2.0, None, Some(1_700_000_000))];
        let ms = DateTime::<Utc>::from_timestamp(1704067200, 0).unwrap().timestamp_millis();
        assert_eq!(resolve_multiplier(&windows, ms, ""), 1.0, "epoch >= end_at → 已失效，返默认 1.0");
    }

    #[test]
    fn resolve_multiplier_period_active_hits() {
        // 生效中：start_at=1700000000（过去）+ end_at=2000000000（未来），时间窗口命中 → 命中 2.0
        let windows = vec![w_period(0, 24, 2.0, Some(1_700_000_000), Some(2_000_000_000))];
        let ms = DateTime::<Utc>::from_timestamp(1704067200, 0).unwrap().timestamp_millis();
        assert_eq!(resolve_multiplier(&windows, ms, ""), 2.0, "start_at <= epoch < end_at → 生效中，命中");
    }

    #[test]
    fn resolve_multiplier_period_absent_permanent() {
        // start_at / end_at 均无 → 永久 / 立即可用（向后兼容），命中
        let windows = vec![w_period(0, 24, 2.0, None, None)];
        let ms = DateTime::<Utc>::from_timestamp(1704067200, 0).unwrap().timestamp_millis();
        assert_eq!(resolve_multiplier(&windows, ms, ""), 2.0, "absent = 永久，命中");
    }

    #[test]
    fn resolve_multiplier_period_first_match_with_fallback() {
        // GLM 实战场景：高峰窗口（永久 3 倍）+ 非高峰兜底窗口（start_at=10-01 才启用）
        // 当前 9 月（福利期）+ 时间 7 点（落高峰 6-10）：高峰排前 first-match → 命中 3.0
        let windows = vec![
            w_models(6, 10, 3.0, vec!["glm-5.2".into()]),
            TimeWindow {
                start_hour: 0,
                end_hour: 24,
                multiplier: 2.0,
                timezone: None,
                days_of_week: None,
                start_minute: None,
                end_minute: None,
                days_of_month: None,
                models: Some(vec!["glm-5.2".into()]),
                start_at: Some(1_759_276_800), // 2026-10-01 UTC+8
                end_at: None,
            },
        ];
        // 2024-01-01T07:00:00Z → hour=7 落 [6,10)
        let ms = DateTime::<Utc>::from_timestamp(1704067200 + 7 * 3600, 0).unwrap().timestamp_millis();
        assert_eq!(resolve_multiplier(&windows, ms, "glm-5.2"), 3.0, "高峰窗口 first-match 命中");
    }

    #[test]
    fn resolve_multiplier_period_non_peak_before_activation_defaults_to_one() {
        // 非高峰时段 + 兜底窗口未启用（start_at 在未来）→ 跳过兜底 → 返默认 1.0（福利期 1 倍抵扣）
        let windows = vec![
            w_models(6, 10, 3.0, vec!["glm-5.2".into()]),
            TimeWindow {
                start_hour: 0,
                end_hour: 24,
                multiplier: 2.0,
                timezone: None,
                days_of_week: None,
                start_minute: None,
                end_minute: None,
                days_of_month: None,
                models: Some(vec!["glm-5.2".into()]),
                start_at: Some(1_759_276_800),
                end_at: None,
            },
        ];
        // hour=12（非高峰 6-10 外）→ 高峰窗口时间不命中 → 跳过；兜底 start_at 未到 → 跳过 → 1.0
        let ms = DateTime::<Utc>::from_timestamp(1704067200 + 12 * 3600, 0).unwrap().timestamp_millis();
        assert_eq!(resolve_multiplier(&windows, ms, "glm-5.2"), 1.0, "非高峰 + 兜底未启用 → 福利 1.0");
    }

    #[test]
    fn is_in_peak_window_period_filter() {
        // disable_during_peak + 生效期：未启用 / 已失效的窗口不应触发排除
        let windows_active = vec![w_period(14, 22, 1.5, Some(1_700_000_000), Some(2_000_000_000))];
        let windows_future = vec![w_period(14, 22, 1.5, Some(2_000_000_000), None)];
        let windows_expired = vec![w_period(14, 22, 1.5, None, Some(1_700_000_000))];
        let ms = DateTime::<Utc>::from_timestamp(1704067200 + 15 * 3600, 0).unwrap().timestamp_millis();
        assert!(is_in_peak_window(&windows_active, ms, ""), "生效中 + 时间命中 → 命中");
        assert!(!is_in_peak_window(&windows_future, ms, ""), "未启用 → 不命中（不触发排除）");
        assert!(!is_in_peak_window(&windows_expired, ms, ""), "已失效 → 不命中");
    }

    // ── parse_platform_peak ──

    #[test]
    fn parse_extra_peak_user_override() {
        let extra = r#"{"peak":[{"start_hour":14,"end_hour":22,"multiplier":1.5}]}"#;
        let v = parse_platform_peak(extra);
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].multiplier, 1.5);
    }

    #[test]
    fn parse_extra_empty_returns_empty() {
        assert!(parse_platform_peak("").is_empty());
        assert!(parse_platform_peak("not-json").is_empty());
        assert!(parse_platform_peak("{}").is_empty());
    }

    // ── default_peak ──

    #[test]
    fn default_peak_unknown_protocol_empty() {
        assert!(default_peak("__never_exists__").is_empty());
    }

    #[test]
    fn default_peak_anthropic_currently_empty() {
        // 当前 preset JSON 未手填 peak，absent → 空（向后兼容）
        assert!(default_peak("anthropic").is_empty());
    }

    /// 票 13-D：路由/计费读的窗口必须跟着 DB 同步下来的 preset 走，不能钉死在二进制内置那份
    /// （否则前端徽标按新窗口显示「当前: 高峰」，后端仍按旧窗口计价，跨层判定分裂）。
    #[test]
    fn peak_windows_follow_db_synced_preset() {
        // bundled 里 anthropic 没有窗口
        assert!(peak_in(aidog_db::registry::presets(), "anthropic").is_empty());
        // 上游新增了 anthropic 的高峰窗口并同步入库 → 同一读取链立刻给出新窗口
        let synced = aidog_db::registry::merge_presets_doc(
            [(
                "anthropic",
                r#"{"last_updated":9999999999,
                    "peak":[{"start_hour":6,"end_hour":10,"multiplier":3.0}],
                    "models":{"peak":{"default":"claude-peak"}}}"#,
            )],
            Some(9_999_999_999),
        );
        let w = peak_in(&synced, "anthropic");
        assert_eq!(w.len(), 1);
        assert_eq!(w[0].multiplier, 3.0);
        assert_eq!(
            peak_models_in(&synced, "anthropic").and_then(|m| m.default),
            Some("claude-peak".to_string())
        );
        // 未被 DB 覆盖的协议照旧走 bundled 值
        assert_eq!(peak_in(&synced, "glm_coding").len(), peak_in(aidog_db::registry::presets(), "glm_coding").len());
    }

    // ── is_in_peak_window ──

    #[test]
    fn is_in_peak_window_hit() {
        let windows = vec![w(14, 22, 1.5)];
        // 2024-01-01T15:00:00Z → hour=15 落在 [14,22) → 命中（基准 1704067200=2024-01-01T00:00:00Z）
        let ms = DateTime::<Utc>::from_timestamp(1704067200 + 15 * 3600, 0).unwrap().timestamp_millis();
        assert!(is_in_peak_window(&windows, ms, ""));
    }

    #[test]
    fn is_in_peak_window_cross_midnight() {
        let windows = vec![w(22, 6, 1.5)]; // 跨天 22-06
        let ms_in = DateTime::<Utc>::from_timestamp(1704067200 + 23 * 3600, 0).unwrap().timestamp_millis(); // 23:00 UTC → 命中
        let ms_out = DateTime::<Utc>::from_timestamp(1704067200 + 15 * 3600, 0).unwrap().timestamp_millis(); // 15:00 UTC → 不命中
        assert!(is_in_peak_window(&windows, ms_in, ""));
        assert!(!is_in_peak_window(&windows, ms_out, ""));
    }

    #[test]
    fn is_in_peak_window_no_match() {
        let windows = vec![w(0, 6, 0.5)];
        // hour=12 不在 [0,6)（基准 1704067200=2024-01-01T00:00:00Z）
        let ms = DateTime::<Utc>::from_timestamp(1704067200 + 12 * 3600, 0).unwrap().timestamp_millis();
        assert!(!is_in_peak_window(&windows, ms, ""));
    }

    #[test]
    fn is_in_peak_window_empty() {
        let ms = 1_700_000_000_000;
        assert!(!is_in_peak_window(&[], ms, ""));
    }

    // ── model scope（PRD 07-09 D2）──

    #[test]
    fn model_match_exact_and_wildcard() {
        // exact
        assert!(model_match("glm-5.2", "glm-5.2"));
        assert!(!model_match("glm-5.2", "glm-5.2-turbo"));
        assert!(!model_match("glm-5.2", "glm-5-turbo"));
        // 通配：`prefix*` 覆盖 prefix 自身 + prefix 开头任意后缀
        assert!(model_match("glm-5.2*", "glm-5.2"));
        assert!(model_match("glm-5.2*", "glm-5.2-turbo"));
        assert!(model_match("glm-5.2*", "glm-5.2x"));
        assert!(!model_match("glm-5.2*", "glm-5-turbo"));
        // 大小写敏感（不做了 casefold，pattern 须精确大小写）
        assert!(!model_match("GLM-5.2", "glm-5.2"));
    }

    #[test]
    fn resolve_multiplier_models_scope_hit_in_list() {
        // GLM 规则：高峰 3 倍 仅 glm-5.2 / glm-5-turbo
        let windows = vec![w_models(6, 10, 3.0, vec!["glm-5.2".into(), "glm-5-turbo".into()])];
        // 2024-01-01T07:00:00Z → hour=7 落在 [6,10) → 时间命中
        let ms = DateTime::<Utc>::from_timestamp(1704067200 + 7 * 3600, 0).unwrap().timestamp_millis();
        assert_eq!(resolve_multiplier(&windows, ms, "glm-5.2"), 3.0, "model 在列表 → 命中 3 倍");
        assert_eq!(resolve_multiplier(&windows, ms, "glm-5-turbo"), 3.0);
    }

    #[test]
    fn resolve_multiplier_models_scope_not_in_list_skips() {
        let windows = vec![w_models(6, 10, 3.0, vec!["glm-5.2".into(), "glm-5-turbo".into()])];
        let ms = DateTime::<Utc>::from_timestamp(1704067200 + 7 * 3600, 0).unwrap().timestamp_millis();
        // model 不在列表 → 该窗口不适用 → 跳过 → 返默认 1.0
        assert_eq!(resolve_multiplier(&windows, ms, "glm-4"), 1.0);
        assert_eq!(resolve_multiplier(&windows, ms, "claude-opus-4"), 1.0);
    }

    #[test]
    fn resolve_multiplier_models_wildcard_covers_variants() {
        // 通配 pattern：`glm-5.2*` 覆盖 glm-5.2 / glm-5.2-turbo
        let windows = vec![w_models(6, 10, 3.0, vec!["glm-5.2*".into()])];
        let ms = DateTime::<Utc>::from_timestamp(1704067200 + 7 * 3600, 0).unwrap().timestamp_millis();
        assert_eq!(resolve_multiplier(&windows, ms, "glm-5.2"), 3.0);
        assert_eq!(resolve_multiplier(&windows, ms, "glm-5.2-turbo"), 3.0);
        assert_eq!(resolve_multiplier(&windows, ms, "glm-5-turbo"), 1.0, "通配不覆盖 glm-5-turbo");
    }

    #[test]
    fn resolve_multiplier_models_absent_all_match() {
        // window.models = None → 全平台生效（向后兼容）
        let windows = vec![w(6, 10, 2.0)];
        let ms = DateTime::<Utc>::from_timestamp(1704067200 + 7 * 3600, 0).unwrap().timestamp_millis();
        assert_eq!(resolve_multiplier(&windows, ms, "glm-5.2"), 2.0);
        assert_eq!(resolve_multiplier(&windows, ms, "anything-else"), 2.0);
        assert_eq!(resolve_multiplier(&windows, ms, ""), 2.0, "空 model 也命中（无 model scope 限制）");
    }

    #[test]
    fn resolve_multiplier_empty_request_model_bypasses_filter() {
        // request_model="" 视为无上下文 → 跳过 model 过滤（即便 window.models 限定也命中）
        // 语义：caller 暂无 model 上下文时（ST1 适配期）的兼容行为
        let windows = vec![w_models(6, 10, 3.0, vec!["glm-5.2".into()])];
        let ms = DateTime::<Utc>::from_timestamp(1704067200 + 7 * 3600, 0).unwrap().timestamp_millis();
        assert_eq!(resolve_multiplier(&windows, ms, ""), 3.0, "空 model → 跳过过滤，命中窗口");
    }

    #[test]
    fn resolve_multiplier_models_first_match_with_scope() {
        // 两窗口：第一个限定 glm-5.2（时间也命中），第二个不限 model
        // request=claude-opus → 跳过第一个 → 命中第二个
        let windows = vec![
            w_models(0, 24, 3.0, vec!["glm-5.2".into()]),
            w(0, 24, 1.5),
        ];
        let ms = DateTime::<Utc>::from_timestamp(1704067200 + 7 * 3600, 0).unwrap().timestamp_millis();
        assert_eq!(resolve_multiplier(&windows, ms, "glm-5.2"), 3.0, "glm-5.2 命中第一个");
        assert_eq!(resolve_multiplier(&windows, ms, "claude-opus-4"), 1.5, "claude-opus 跳过第一个，命中第二个");
    }

    #[test]
    fn is_in_peak_window_models_scope() {
        // disable_during_peak + model scope：仅当 model 在 scope 内且时间命中才算「命中该窗口」
        let windows = vec![w_models(6, 10, 3.0, vec!["glm-5.2".into(), "glm-5-turbo".into()])];
        let ms = DateTime::<Utc>::from_timestamp(1704067200 + 7 * 3600, 0).unwrap().timestamp_millis();
        assert!(is_in_peak_window(&windows, ms, "glm-5.2"), "model 在 scope → 命中");
        assert!(!is_in_peak_window(&windows, ms, "claude-opus-4"), "model 不在 scope → 不命中 → 不排除");
        // 空 model → 跳过过滤 → 命中（与 disable_during_peak 默认行为一致：caller 无 model 上下文仍排除）
        assert!(is_in_peak_window(&windows, ms, ""));
    }

    // ── peak_for（混合源）──

    #[test]
    fn peak_for_user_override_wins() {
        let user_extra = r#"{"peak":[{"start_hour":14,"end_hour":22,"multiplier":1.5}]}"#;
        // preset 默认 anthropic 当前为空；用户覆盖应优先
        let v = peak_for(user_extra, "anthropic");
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].multiplier, 1.5);
    }

    #[test]
    fn peak_for_empty_extra_falls_back_preset() {
        // anthropic 当前 preset 无 peak → 空 Vec（caller 退 1.0 / is_in_peak_window=false）
        assert!(peak_for("", "anthropic").is_empty());
        assert!(peak_for("{}", "anthropic").is_empty());
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

    // ── default_peak_models（PRD 07-11 models.peak 分支）──

    #[test]
    fn default_peak_models_glm_coding_present() {
        // glm_coding preset 带 models.{default,peak} 双分支（PRD 07-11）
        let m = default_peak_models("glm_coding").expect("glm_coding preset has models.peak");
        // peak 分支值（platform-presets.json 真值，禁硬编码第二份 — 此处仅锁 schema 解析正确）
        assert_eq!(m.default.as_deref(), Some("glm-5.3-flash"));
        assert_eq!(m.opus.as_deref(), Some("glm-5.3-flash"));
        assert_eq!(m.sonnet.as_deref(), Some("glm-5.3-flash"));
        assert_eq!(m.gpt.as_deref(), Some("glm-5.3-flash"));
        assert_eq!(m.haiku.as_deref(), Some("glm-5.3-flash"));
    }

    #[test]
    fn default_peak_models_absent_for_protocol_without_peak_branch() {
        // 无 models.peak 分支的协议（如 anthropic / deepseek）→ None（向后兼容，caller 退 platform.models）
        assert!(default_peak_models("anthropic").is_none());
        assert!(default_peak_models("deepseek").is_none());
    }

    #[test]
    fn default_peak_models_unknown_protocol_none() {
        assert!(default_peak_models("__never_exists__").is_none());
    }
}
