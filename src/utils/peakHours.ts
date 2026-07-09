/** 前端高峰时段判定 helper（与 Rust `gateway::peak_hours::is_in_peak_window` 对称）。
 *  跨层一致：minute 精度 + days_of_week / days_of_month 过滤 + 跨天 end<start 半开 [start,end)；
 *  days_of_week 0=Sun…6=Sat 缺省=每天；空/无命中=false。
 *  for: 平台列表徽标 + 编辑表单预览 + Groups 指示（D7/D8/D9 共用此 helper）。 */
import type { PeakWindow } from "../domains/platforms/defaults";

/** 当前 UTC 时刻命中窗口？
 *  与 Rust `peak_hours::hit` 逐行对称：
 *   - days_of_week 过滤（含则需在列表里；双 Some 与 days_of_month 取 AND 兜底）
 *   - days_of_month 过滤（含则当前 day_of_month 需在列表里）
 *   - 绝对分钟半开区间：t_min = hour*60 + minute；
 *     start_min = start_hour*60 + (start_minute ?? 0)；end_min = end_hour*60 + (end_minute ?? 0)；
 *     同天 (end_min > start_min): t_min >= start_min && t_min < end_min；
 *     跨天 (end_min <= start_min，含 start==end 退化): t_min >= start_min || t_min < end_min。
 */
function hit(
  w: PeakWindow,
  hour: number,
  minute: number,
  weekday: number,
  dayOfMonth: number,
): boolean {
  if (w.days_of_week && !w.days_of_week.includes(weekday)) return false;
  if (w.days_of_month && !w.days_of_month.includes(dayOfMonth)) return false;
  const tMin = hour * 60 + minute;
  const startMin = w.start_hour * 60 + clampMinute(w.start_minute ?? 0);
  const endMin = w.end_hour * 60 + clampMinute(w.end_minute ?? 0);
  if (endMin > startMin) {
    return tMin >= startMin && tMin < endMin;
  }
  // 跨天（含 start==end 的退化情况，按全天命中处理）
  return tMin >= startMin || tMin < endMin;
}

function clampMinute(m: number): number {
  if (m < 0) return 0;
  if (m > 59) return 59;
  return m;
}

/** first-match 命中任一窗口 → true（不关心 multiplier 值）；空/无命中 → false。
 *  使用 UTC 小时 / 分钟 / weekday (0=Sun) / day_of_month (1-31)，与 Rust `utc_time` 对齐。
 *  weekday 归一：JS `Date.getUTCDay()` 返 0=Sun…6=Sat，与 Rust 目标 `(num_days_from_monday + 1) % 7`
 *  完全一致，无需转换。day_of_month 用 `Date.getUTCDate()` (1-31)。
 *  ponytail: 复用 Date.getUTC* 免手算 epoch 偏移。 */
export function isCurrentlyPeak(windows: PeakWindow[] | undefined | null, nowMs: number): boolean {
  if (!windows || windows.length === 0) return false;
  const d = new Date(nowMs);
  const hour = d.getUTCHours();
  const minute = d.getUTCMinutes();
  const weekday = d.getUTCDay(); // 0=Sun…6=Sat
  const dayOfMonth = d.getUTCDate(); // 1-31
  return windows.some((w) => hit(w, hour, minute, weekday, dayOfMonth));
}
