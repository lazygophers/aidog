//! 手动预算限额：窗口重置 / 扣减 / 耗尽阻断（仅无上游 quota 自动支持平台）。
//!
//! 设计要点：
//!   - 纯函数（maybe_reset / remaining / is_depleted）无副作用，可单测，覆盖 4 种窗口语义。
//!   - DB 集成 `apply_manual_budgets` 在**同一持锁临界区**内 SELECT manual_budgets →
//!     惰性窗口重置 → consumed 累加 → UPDATE 回写（JSON read-modify-write，禁持锁跨 await）。
//!   - 阻断判定 `evaluate_depletion`（proxy 转发前调用）采**惰性只读判定**——
//!     不写库（避免转发前短临界区写竞争），落库统一交给请求后的 apply_manual_budgets。
//!     纯函数对 budget 的克隆做窗口重置后判 remaining，不影响 DB 状态。
//!
//! 窗口语义（4 kind）：
//!   - total：永不重置（总额）。
//!   - rolling：首用起算，`now - window_start >= window_hours` → 满额重置，window_start=now。
//!   - fixed：钟点对齐——锚点为「本地日 00:00」，每 window_hours 一段；
//!     跨入新段（now 所在段起点 != 上次记录段起点）→ 重置，window_start=段起点。
//!   - daily：跨本地自然日（00:00 local）→ 重置，window_start=今日 00:00。

use chrono::{Local, TimeZone, Timelike};
use rusqlite::params;

use super::db::Db;
use super::models::{parse_manual_budgets, serialize_manual_budgets, ManualBudget, WindowUnit};

/// 窗口数值 + 单位 → 毫秒时长（纯函数，锁安全）。
/// month 固定按 30 天换算（无歧义需求）。负值/0 由调用方判定（返回非正值表示「无窗口」）。
fn window_ms(value: f64, unit: WindowUnit) -> f64 {
    let unit_ms = match unit {
        WindowUnit::Minute => 60_000.0,
        WindowUnit::Hour => 3_600_000.0,
        WindowUnit::Day => 86_400_000.0,
        WindowUnit::Week => 604_800_000.0,
        WindowUnit::Month => 2_592_000_000.0, // 30 天
    };
    value * unit_ms
}

/// 本地自然日 00:00 的毫秒戳（包含 now_ms 的那一天）
fn local_day_start_ms(now_ms: i64) -> i64 {
    let dt = Local.timestamp_millis_opt(now_ms).single();
    match dt {
        Some(dt) => {
            let day_start = dt
                .with_hour(0)
                .and_then(|d| d.with_minute(0))
                .and_then(|d| d.with_second(0))
                .and_then(|d| d.with_nanosecond(0))
                .unwrap_or(dt);
            day_start.timestamp_millis()
        }
        None => now_ms,
    }
}

/// fixed 窗口：now_ms 所在「本地日 00:00 + k×窗口时长」段的起点毫秒戳。
/// 窗口时长 <= 0 时退化为当日 00:00（不分段）。
fn fixed_segment_start_ms(now_ms: i64, win_ms_f: f64) -> i64 {
    let day_start = local_day_start_ms(now_ms);
    if win_ms_f <= 0.0 {
        return day_start;
    }
    let seg_ms = win_ms_f as i64;
    if seg_ms <= 0 {
        return day_start;
    }
    let elapsed = now_ms - day_start;
    // elapsed 理论 >= 0；防御性处理跨日负值
    let seg_index = if elapsed >= 0 { elapsed / seg_ms } else { 0 };
    day_start + seg_index * seg_ms
}

/// 惰性窗口重置（纯函数，原地修改 budget）。total 不重置。
/// 首次使用（window_start_at 为 None）→ 初始化为当前窗口起点（不清 consumed）。
pub fn maybe_reset(budget: &mut ManualBudget, now_ms: i64) {
    match budget.kind.as_str() {
        "rolling" => {
            let win_ms = window_ms(budget.window_hours.unwrap_or(0.0), budget.window_unit) as i64;
            match budget.window_start_at {
                None => budget.window_start_at = Some(now_ms),
                Some(start) => {
                    if win_ms > 0 && now_ms - start >= win_ms {
                        budget.consumed = 0.0;
                        budget.window_start_at = Some(now_ms);
                    }
                }
            }
        }
        "fixed" => {
            let seg_start = fixed_segment_start_ms(
                now_ms,
                window_ms(budget.window_hours.unwrap_or(0.0), budget.window_unit),
            );
            match budget.window_start_at {
                None => budget.window_start_at = Some(seg_start),
                Some(start) => {
                    if seg_start > start {
                        budget.consumed = 0.0;
                        budget.window_start_at = Some(seg_start);
                    }
                }
            }
        }
        "daily" => {
            let day_start = local_day_start_ms(now_ms);
            match budget.window_start_at {
                None => budget.window_start_at = Some(day_start),
                Some(start) => {
                    if day_start > start {
                        budget.consumed = 0.0;
                        budget.window_start_at = Some(day_start);
                    }
                }
            }
        }
        // "total" 或未知 kind：不重置
        _ => {}
    }
}

/// 当前剩余额度 = amount - consumed（可为负）
pub fn remaining(budget: &ManualBudget) -> f64 {
    budget.amount - budget.consumed
}

/// 是否耗尽（剩余 <= 0）
pub fn is_depleted(budget: &ManualBudget) -> bool {
    remaining(budget) <= 0.0
}

/// 阻断判定结果：耗尽的限额信息（供 402 响应体）
#[derive(Debug, Clone, PartialEq)]
pub struct DepletionInfo {
    pub kind: String,
    pub unit: String,
    pub amount: f64,
}

/// 惰性只读判定：对每个 enabled 限额做窗口重置（仅作用于克隆）后判耗尽。
/// 返回首个耗尽限额（None = 全部有余 / 无限额 → 放行）。不写库。
pub fn evaluate_depletion(budgets: &[ManualBudget], now_ms: i64) -> Option<DepletionInfo> {
    for b in budgets {
        if !b.enabled {
            continue;
        }
        let mut cloned = b.clone();
        maybe_reset(&mut cloned, now_ms);
        if is_depleted(&cloned) {
            return Some(DepletionInfo {
                kind: cloned.kind.clone(),
                unit: cloned.unit.clone(),
                amount: cloned.amount,
            });
        }
    }
    None
}

/// 单条限额扣减（纯函数）：先窗口重置，再按 unit 累加。
pub fn apply_one(budget: &mut ManualBudget, est_cost: f64, total_tokens: f64, now_ms: i64) {
    if !budget.enabled {
        return;
    }
    maybe_reset(budget, now_ms);
    let delta = if budget.unit == "usd" { est_cost } else { total_tokens };
    budget.consumed += delta;
}

/// DB 集成：同一持锁临界区 SELECT manual_budgets → 各 enabled 限额扣减 → UPDATE 回写。
/// 禁持锁跨 .await（本函数全同步）。无限额 → 跳过不写。
pub fn apply_manual_budgets(
    db: &Db,
    platform_id: u64,
    est_cost: f64,
    total_tokens: f64,
    now_ms: i64,
) -> Result<(), String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let json: String = conn
        .query_row(
            "SELECT manual_budgets FROM platform WHERE id = ?1",
            params![platform_id as i64],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;
    let mut budgets = parse_manual_budgets(&json);
    if budgets.is_empty() {
        return Ok(());
    }
    for b in budgets.iter_mut() {
        apply_one(b, est_cost, total_tokens, now_ms);
    }
    conn.execute(
        "UPDATE platform SET manual_budgets = ?1 WHERE id = ?2",
        params![serialize_manual_budgets(&budgets), platform_id as i64],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mk(kind: &str, unit: &str, amount: f64, window_hours: Option<f64>) -> ManualBudget {
        ManualBudget {
            id: "b1".into(),
            kind: kind.into(),
            unit: unit.into(),
            amount,
            window_hours,
            window_unit: WindowUnit::Hour,
            consumed: 0.0,
            window_start_at: None,
            enabled: true,
        }
    }

    // ── total：不重置，consumed 持续累加 ──
    #[test]
    fn total_never_resets() {
        let mut b = mk("total", "usd", 10.0, None);
        let t0 = 1_700_000_000_000;
        apply_one(&mut b, 4.0, 0.0, t0);
        apply_one(&mut b, 4.0, 0.0, t0 + 100 * 3_600_000); // 100h 后
        assert!((b.consumed - 8.0).abs() < 1e-9);
        assert!((remaining(&b) - 2.0).abs() < 1e-9);
        assert!(!is_depleted(&b));
        apply_one(&mut b, 3.0, 0.0, t0 + 200 * 3_600_000);
        assert!(is_depleted(&b), "11>10 应耗尽");
    }

    // ── rolling：window_hours 后满额重置 ──
    #[test]
    fn rolling_resets_after_window() {
        let mut b = mk("rolling", "usd", 10.0, Some(5.0)); // 滑动 5h
        let t0 = 1_700_000_000_000;
        apply_one(&mut b, 8.0, 0.0, t0); // 首用，window_start=t0
        assert!((b.consumed - 8.0).abs() < 1e-9);
        // 3h 后再扣，窗口未到期 → 累加
        apply_one(&mut b, 1.0, 0.0, t0 + 3 * 3_600_000);
        assert!((b.consumed - 9.0).abs() < 1e-9);
        // 6h 后（>=5h）→ 重置后扣
        apply_one(&mut b, 2.0, 0.0, t0 + 6 * 3_600_000);
        assert!((b.consumed - 2.0).abs() < 1e-9, "应重置后 = 2, got {}", b.consumed);
        assert_eq!(b.window_start_at, Some(t0 + 6 * 3_600_000));
    }

    // ── token 单位扣总 token ──
    #[test]
    fn token_unit_decrements_tokens() {
        let mut b = mk("total", "token", 100_000.0, None);
        apply_one(&mut b, 5.0, 30_000.0, 1_700_000_000_000); // est_cost 忽略
        assert!((b.consumed - 30_000.0).abs() < 1e-9);
    }

    // ── daily：跨本地自然日重置 ──
    #[test]
    fn daily_resets_next_day() {
        let mut b = mk("daily", "usd", 10.0, None);
        let day0 = local_day_start_ms(1_700_000_000_000);
        let noon = day0 + 12 * 3_600_000;
        apply_one(&mut b, 8.0, 0.0, noon);
        assert!((b.consumed - 8.0).abs() < 1e-9);
        // 次日（+24h）→ 重置
        let next_noon = noon + 24 * 3_600_000;
        apply_one(&mut b, 3.0, 0.0, next_noon);
        assert!((b.consumed - 3.0).abs() < 1e-9, "次日应重置, got {}", b.consumed);
    }

    // ── fixed：钟点对齐分段重置（6h 段）──
    #[test]
    fn fixed_resets_on_segment_boundary() {
        let mut b = mk("fixed", "usd", 10.0, Some(6.0));
        let day0 = local_day_start_ms(1_700_000_000_000);
        // 第一段 [0,6h)：02:00 扣
        apply_one(&mut b, 7.0, 0.0, day0 + 2 * 3_600_000);
        assert!((b.consumed - 7.0).abs() < 1e-9);
        // 同段 04:00 → 累加
        apply_one(&mut b, 1.0, 0.0, day0 + 4 * 3_600_000);
        assert!((b.consumed - 8.0).abs() < 1e-9);
        // 进入第二段 [6h,12h) 的 07:00 → 重置
        apply_one(&mut b, 2.0, 0.0, day0 + 7 * 3_600_000);
        assert!((b.consumed - 2.0).abs() < 1e-9, "跨段应重置, got {}", b.consumed);
    }

    // ── disabled 限额不扣不阻断 ──
    #[test]
    fn disabled_budget_skipped() {
        let mut b = mk("total", "usd", 1.0, None);
        b.enabled = false;
        apply_one(&mut b, 100.0, 0.0, 1_700_000_000_000);
        assert!((b.consumed - 0.0).abs() < 1e-9, "disabled 不应扣");
        assert!(evaluate_depletion(&[b], 1_700_000_000_000).is_none(), "disabled 不阻断");
    }

    // ── 耗尽判定：任一限额耗尽即阻断，返回该限额信息 ──
    #[test]
    fn evaluate_blocks_on_any_depleted() {
        let mut full = mk("total", "usd", 10.0, None);
        full.consumed = 2.0; // 有余
        let mut empty = mk("daily", "token", 1000.0, None);
        empty.consumed = 1000.0; // 耗尽
        empty.id = "b2".into();
        let now = 1_700_000_000_000;
        let info = evaluate_depletion(&[full, empty], now);
        assert!(info.is_some());
        let info = info.unwrap();
        assert_eq!(info.kind, "daily");
        assert_eq!(info.unit, "token");
    }

    // ── 全部有余 → 放行 ──
    #[test]
    fn evaluate_passes_when_all_remain() {
        let b = mk("total", "usd", 10.0, None);
        assert!(evaluate_depletion(&[b], 1_700_000_000_000).is_none());
        assert!(evaluate_depletion(&[], 1_700_000_000_000).is_none(), "空 → 放行");
    }

    // ── 阻断判定含惰性窗口重置：耗尽限额在新窗口恢复放行 ──
    #[test]
    fn evaluate_recovers_after_window() {
        let mut b = mk("rolling", "usd", 10.0, Some(5.0));
        b.consumed = 10.0; // 耗尽
        let t0 = 1_700_000_000_000;
        b.window_start_at = Some(t0);
        // 窗口内仍耗尽
        assert!(evaluate_depletion(&[b.clone()], t0 + 3_600_000).is_some());
        // 窗口到期（+5h）→ 惰性重置判定 → 放行
        assert!(evaluate_depletion(&[b], t0 + 5 * 3_600_000).is_none());
    }

    // ── window_ms 各单位换算表 ──
    #[test]
    fn window_ms_conversion_table() {
        assert!((window_ms(1.0, WindowUnit::Minute) - 60_000.0).abs() < 1e-6);
        assert!((window_ms(1.0, WindowUnit::Hour) - 3_600_000.0).abs() < 1e-6);
        assert!((window_ms(1.0, WindowUnit::Day) - 86_400_000.0).abs() < 1e-6);
        assert!((window_ms(1.0, WindowUnit::Week) - 604_800_000.0).abs() < 1e-6);
        assert!((window_ms(1.0, WindowUnit::Month) - 2_592_000_000.0).abs() < 1e-6, "month=30d");
        // 复合数值：7 天、90 分钟
        assert!((window_ms(7.0, WindowUnit::Day) - 604_800_000.0).abs() < 1e-6, "7d == 1week");
        assert!((window_ms(90.0, WindowUnit::Minute) - 5_400_000.0).abs() < 1e-6, "90min");
    }

    // ── 向后兼容：旧 JSON {window_hours:2} 无 window_unit → 解析为 2 小时窗口 ──
    #[test]
    fn legacy_json_defaults_to_hour() {
        let json = r#"[{"id":"b1","kind":"rolling","unit":"usd","amount":10,"window_hours":2}]"#;
        let budgets = parse_manual_budgets(json);
        assert_eq!(budgets.len(), 1);
        let b = &budgets[0];
        assert_eq!(b.window_unit, WindowUnit::Hour, "缺 window_unit → 默认 hour");
        assert_eq!(b.window_hours, Some(2.0));
        // 行为不变：2 小时窗口
        let mut bb = b.clone();
        let t0 = 1_700_000_000_000;
        apply_one(&mut bb, 8.0, 0.0, t0); // 首用
        apply_one(&mut bb, 1.0, 0.0, t0 + 1 * 3_600_000); // 1h 后未到期 → 累加
        assert!((bb.consumed - 9.0).abs() < 1e-9);
        apply_one(&mut bb, 2.0, 0.0, t0 + 2 * 3_600_000); // 2h 后到期 → 重置
        assert!((bb.consumed - 2.0).abs() < 1e-9, "2h 后应重置, got {}", bb.consumed);
    }

    // ── rolling 以「天」为单位：7 天窗口 ──
    #[test]
    fn rolling_day_unit_7days() {
        let mut b = mk("rolling", "usd", 10.0, Some(7.0));
        b.window_unit = WindowUnit::Day;
        let t0 = 1_700_000_000_000;
        apply_one(&mut b, 8.0, 0.0, t0); // 首用
        apply_one(&mut b, 1.0, 0.0, t0 + 6 * 86_400_000); // 6 天后未到期 → 累加
        assert!((b.consumed - 9.0).abs() < 1e-9);
        apply_one(&mut b, 2.0, 0.0, t0 + 7 * 86_400_000); // 7 天后到期 → 重置
        assert!((b.consumed - 2.0).abs() < 1e-9, "7 天后应重置, got {}", b.consumed);
    }

    // ── rolling 以「分钟」为单位：90 分钟窗口 ──
    #[test]
    fn rolling_minute_unit_90min() {
        let mut b = mk("rolling", "usd", 10.0, Some(90.0));
        b.window_unit = WindowUnit::Minute;
        let t0 = 1_700_000_000_000;
        apply_one(&mut b, 8.0, 0.0, t0); // 首用
        apply_one(&mut b, 1.0, 0.0, t0 + 60 * 60_000); // 60 分钟后未到期 → 累加
        assert!((b.consumed - 9.0).abs() < 1e-9);
        apply_one(&mut b, 2.0, 0.0, t0 + 90 * 60_000); // 90 分钟后到期 → 重置
        assert!((b.consumed - 2.0).abs() < 1e-9, "90 分钟后应重置, got {}", b.consumed);
    }
}
