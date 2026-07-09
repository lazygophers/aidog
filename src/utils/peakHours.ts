/** 前端高峰时段判定 helper（与 Rust `gateway::peak_hours::is_in_peak_window` 对称）。
 *  跨层一致：minute 精度 + days_of_week / days_of_month 过滤 + model scope 过滤 + 跨天 end<start 半开 [start,end)；
 *  days_of_week 0=Sun…6=Sat 缺省=每天；model scope 缺省=全平台；空/无命中=false。
 *  for: 平台列表徽标 + 编辑表单预览 + Groups 指示（D7/D8/D9 共用此 helper）。 */
import type { PeakWindow } from "../domains/platforms/defaults";

/** 当前 UTC 时刻命中窗口？
 *  与 Rust `peak_hours::hit` + `window_models_hit` + `period_active` 逐行对称：
 *   - 生效期判定（PRD 07-09 D2，优先级最高）：starts_at Some 且 epoch_sec < starts_at → 未启用跳过；
 *     expires_at Some 且 epoch_sec >= expires_at → 已失效跳过；二者均 absent = 永久/立即可用。
 *   - days_of_week 过滤（含则需在列表里；双 Some 与 days_of_month 取 AND 兜底）
 *   - days_of_month 过滤（含则当前 day_of_month 需在列表里）
 *   - 绝对分钟半开区间：t_min = hour*60 + minute；
 *     start_min = start_hour*60 + (start_minute ?? 0)；end_min = end_hour*60 + (end_minute ?? 0)；
 *     同天 (end_min > start_min): t_min >= start_min && t_min < end_min；
 *     跨天 (end_min <= start_min，含 start==end 退化): t_min >= start_min || t_min < end_min。
 *   - model scope 过滤（PRD 07-09 D2）：window.models 缺省/undefined → 全平台；
 *     否则 requestModel 须匹配某 pattern（exact 或 `prefix*` 通配）。
 *     requestModel 空串 = 调用方无 model 上下文 → 跳过 model 过滤（兼容旧行为）。
 */
function hit(
  w: PeakWindow,
  hour: number,
  minute: number,
  weekday: number,
  dayOfMonth: number,
  requestModel: string,
  epochSec: number,
): boolean {
  // 生效期判定（与 Rust period_active 对称，优先级最高）
  if (w.starts_at !== undefined && epochSec < w.starts_at) return false;
  if (w.expires_at !== undefined && epochSec >= w.expires_at) return false;
  if (w.days_of_week && !w.days_of_week.includes(weekday)) return false;
  if (w.days_of_month && !w.days_of_month.includes(dayOfMonth)) return false;
  const tMin = hour * 60 + minute;
  const startMin = w.start_hour * 60 + clampMinute(w.start_minute ?? 0);
  const endMin = w.end_hour * 60 + clampMinute(w.end_minute ?? 0);
  let timeHit: boolean;
  if (endMin > startMin) {
    timeHit = tMin >= startMin && tMin < endMin;
  } else {
    // 跨天（含 start==end 的退化情况，按全天命中处理）
    timeHit = tMin >= startMin || tMin < endMin;
  }
  if (!timeHit) return false;
  return windowModelsHit(w, requestModel);
}

function clampMinute(m: number): number {
  if (m < 0) return 0;
  if (m > 59) return 59;
  return m;
}

/** 窗口 model scope 是否覆盖 requestModel（与 Rust `peak_hours::window_models_hit` 对称）。
 *  - requestModel === "" → true（调用方无上下文，跳过过滤，兼容旧行为）
 *  - w.models undefined → true（窗口未限定，全平台生效）
 *  - w.models 定义 → 任一 pattern 命中（exact 或 `prefix*` 通配）
 */
function windowModelsHit(w: PeakWindow, requestModel: string): boolean {
  if (requestModel === "") return true;
  if (!w.models || w.models.length === 0) return true;
  return w.models.some((p) => modelMatch(p, requestModel));
}

/** 单 pattern 与请求模型匹配（与 Rust `peak_hours::model_match` 对称）：
 *  exact OR 前缀通配（`"glm-5.2*"` 覆盖 `glm-5.2` / `glm-5.2-turbo`）。
 *  exact-first：非 `*` 结尾走精确匹配；`*` 结尾取前缀，`requestModel === prefix || startsWith(prefix)`。
 */
function modelMatch(pattern: string, requestModel: string): boolean {
  if (pattern.endsWith("*")) {
    const prefix = pattern.slice(0, -1);
    return requestModel === prefix || requestModel.startsWith(prefix);
  }
  return requestModel === pattern;
}

/** first-match 命中任一窗口 → true（不关心 multiplier 值）；空/无命中 → false。
 *  使用 UTC 小时 / 分钟 / weekday (0=Sun) / day_of_month (1-31)，与 Rust `utc_time` 对齐。
 *  weekday 归一：JS `Date.getUTCDay()` 返 0=Sun…6=Sat，与 Rust 目标 `(num_days_from_monday + 1) % 7`
 *  完全一致，无需转换。day_of_month 用 `Date.getUTCDate()` (1-31)。
 *
 *  requestModel（PRD 07-09 D2）：请求模型名，用于 model scope 过滤；
 *  缺省 / 空串 = 无 model 上下文 → 跳过 model 过滤（兼容旧行为，向后兼容）。
 *  ponytail: 复用 Date.getUTC* 免手算 epoch 偏移。 */
export function isCurrentlyPeak(
  windows: PeakWindow[] | undefined | null,
  nowMs: number,
  requestModel: string = "",
): boolean {
  if (!windows || windows.length === 0) return false;
  const d = new Date(nowMs);
  const hour = d.getUTCHours();
  const minute = d.getUTCMinutes();
  const weekday = d.getUTCDay(); // 0=Sun…6=Sat
  const dayOfMonth = d.getUTCDate(); // 1-31
  const epochSec = Math.floor(nowMs / 1000);
  return windows.some((w) => hit(w, hour, minute, weekday, dayOfMonth, requestModel, epochSec));
}
