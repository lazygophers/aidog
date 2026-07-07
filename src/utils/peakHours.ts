/** 前端高峰时段判定 helper（与 Rust `gateway::peak_hours::is_in_peak_window` 对称）。
 *  跨层一致：跨天 end<start 半开 [start,end)；days_of_week 0=Sun…6=Sat 缺省=每天；空/无命中=false。
 *  for: 平台列表徽标 + 编辑表单预览 + Groups 指示（D7/D8/D9 共用此 helper）。 */
import type { PeakWindow } from "../domains/platforms/defaults";

/** hour 命中窗口？days_of_week 过滤 + 跨天 (end<start) / 同天 [start,end) 半开判定。
 *  与 Rust `peak_hours::hit` 逐行对称（含 start==end 退化全天命中）。 */
function hit(w: PeakWindow, hour: number, weekday: number): boolean {
  if (w.days_of_week && !w.days_of_week.includes(weekday)) return false;
  if (w.end_hour > w.start_hour) {
    return hour >= w.start_hour && hour < w.end_hour;
  }
  // 跨天（含 start==end 退化情况，按全天命中处理）
  return hour >= w.start_hour || hour < w.end_hour;
}

/** first-match 命中任一窗口 → true（不关心 multiplier 值）；空/无命中 → false。
 *  使用 UTC 小时与 UTC weekday（0=Sun），与 Rust `utc_hour_weekday` 对齐。
 *  ponytail: 复用 Date.getUTC* 免手算 epoch 偏移。 */
export function isCurrentlyPeak(windows: PeakWindow[] | undefined | null, nowMs: number): boolean {
  if (!windows || windows.length === 0) return false;
  const d = new Date(nowMs);
  const hour = d.getUTCHours();
  const weekday = d.getUTCDay(); // 0=Sun…6=Sat
  return windows.some((w) => hit(w, hour, weekday));
}
