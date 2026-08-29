/** 前端高峰时段判定 helper（与 Rust `gateway::peak_hours::is_in_peak_window` 对称）。
 *  跨层一致：minute 精度 + days_of_week / days_of_month 过滤 + model scope 过滤 + 跨天 end<start 半开 [start,end)；
 *  days_of_week 0=Sun…6=Sat 缺省=每天；model scope 缺省=全平台；空/无命中=false。
 *  for: 平台列表徽标 + 编辑表单预览 + Groups 指示（D7/D8/D9 共用此 helper）。 */
import type { PeakWindow } from "../domains/platforms/defaults";

/** 时区展示模式：本地 or UTC+0。存储永远 UTC+0，仅展示/输入层换算。 */
export type TzMode = "local" | "utc";

/** 窗口时区下拉候选（IANA 名）。存储即 IANA 串；__utc__ 哨兵 = 无 timezone（= UTC，向后兼容）。
 *  覆盖主流市场 + 用户所在地（瑞士）；完整 IANA 列表过长，收窄为常用集。
 *  formSections.tsx（peak_hours）与 WindowsEditModal.tsx（time_models）两编辑器共用。 */
export const WINDOW_TIMEZONES = [
  "__utc__", "Asia/Shanghai", "Asia/Tokyo", "Asia/Singapore", "Asia/Kolkata",
  "America/New_York", "America/Chicago", "America/Los_Angeles",
  "Europe/London", "Europe/Berlin", "Europe/Zurich", "Europe/Moscow",
] as const;

/** 本地时区相对 UTC 的分钟偏移（东区为正）。模块加载时取值，沿用既有时机（不做 DST 跨时刻重算）。 */
export const LOCAL_OFFSET_MINUTES = -new Date().getTimezoneOffset();

/** 选中时区模式对应的分钟偏移（UTC = 0 / 本地 = LOCAL_OFFSET_MINUTES）。 */
export function tzOffsetMinutes(mode: TzMode): number {
  return mode === "local" ? LOCAL_OFFSET_MINUTES : 0;
}

/** 时钟平移的纯函数内核 —— offset 显式入参，可被单测覆盖任意时区（含 +5:30 / 负偏移）。
 *  ⚠️ 单测必须打这一层：LOCAL_OFFSET_MINUTES 在模块加载时求值，
 *  vi.spyOn(Date.prototype, "getTimezoneOffset") 对它无效。 */
export function shiftClock(hour: number, minute: number, offsetMinutes: number): { hour: number; minute: number } {
  const m = (((hour * 60 + minute + offsetMinutes) % 1440) + 1440) % 1440;
  return { hour: Math.floor(m / 60), minute: m % 60 };
}

/** UTC 存值 → 选中时区显示值。按绝对分钟换算，半时区（+5:30）精确到分钟。 */
export function utcToDisplay(hour: number, minute: number, mode: TzMode): { hour: number; minute: number } {
  return shiftClock(hour, minute, tzOffsetMinutes(mode));
}

/** 选中时区输入值 → UTC 存值。 */
export function displayToUtc(hour: number, minute: number, mode: TzMode): { hour: number; minute: number } {
  return shiftClock(hour, minute, -tzOffsetMinutes(mode));
}

/** 存量非整数 start_hour/end_hour（半时区旧逻辑产物，如 8.5）拆为 hour+minute。整数值原样不动。 */
export function normalizeWindow(w: PeakWindow): PeakWindow {
  let result = w;
  if (!Number.isInteger(w.start_hour)) {
    const { hour, minute } = splitFraction(w.start_hour, w.start_minute);
    result = { ...result, start_hour: hour, start_minute: minute };
  }
  if (!Number.isInteger(w.end_hour)) {
    const { hour, minute } = splitFraction(w.end_hour, w.end_minute);
    result = { ...result, end_hour: hour, end_minute: minute };
  }
  return result;
}

function splitFraction(h: number, existingMinute: number | undefined): { hour: number; minute: number } {
  const hour = Math.floor(h);
  const extraMinutes = Math.round((h - hour) * 60);
  return shiftClock(hour, (existingMinute ?? 0) + extraMinutes, 0);
}

/** t 在 tz（IANA 名，缺省 / 非法 = UTC）的**本地**全时间分量（DST 由浏览器 tz 数据处理）。
 *  与 Rust `peak_hours::wall_time` 对称（跨层一致）。非法时区名回落 UTC（数据脏不炸 UI）。
 *  ponytail: Intl.DateTimeFormat formatToParts 免手算 epoch 偏移。 */
export function wallTimeInTz(
  ms: number,
  timeZone: string | undefined,
): { hour: number; minute: number; weekday: number; dayOfMonth: number } {
  const fmt = (() => {
    try {
      return new Intl.DateTimeFormat("en-US", {
        timeZone: timeZone || "UTC",
        hour12: false,
        hour: "2-digit",
        minute: "2-digit",
        weekday: "short",
        day: "2-digit",
      });
    } catch {
      return undefined; // 非法时区名 → 回落 UTC
    }
  })();
  if (!fmt) return wallTimeInTz(ms, "UTC");
  const parts = fmt.formatToParts(new Date(ms));
  const get = (t: string): string => parts.find((p) => p.type === t)?.value ?? "";
  const WD: Record<string, number> = { Sun: 0, Mon: 1, Tue: 2, Wed: 3, Thu: 4, Fri: 5, Sat: 6 };
  let hour = parseInt(get("hour"), 10);
  if (hour === 24) hour = 0; // hour12:false 某些引擎给 24:xx
  const wd = WD[get("weekday")] ?? 0;
  if (Number.isNaN(hour) || Number.isNaN(wd)) {
    // 防御：parts 异常缺字段 → 回落 UTC（不静默给 0 值）
    return wallTimeInTz(ms, "UTC");
  }
  return { hour, minute: parseInt(get("minute"), 10), weekday: wd, dayOfMonth: parseInt(get("day"), 10) };
}

/** 当前 UTC 时刻命中窗口？
 *  与 Rust `peak_hours::hit` + `window_models_hit` + `period_active` 逐行对称：
 *   - 生效期判定（PRD 07-09 D2，优先级最高）：start_at Some 且 epoch_sec < start_at → 未启用跳过；
 *     end_at Some 且 epoch_sec >= end_at → 已失效跳过；二者均 absent = 永久/立即可用。
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
  if (w.start_at !== undefined && epochSec < w.start_at) return false;
  if (w.end_at !== undefined && epochSec >= w.end_at) return false;
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
 *  时段基准：每窗口各自 timezone（缺省 = UTC，向后兼容），hour/weekday/day_of_month
 *  按该窗口时区本地时刻取 —— 与 Rust `peak_hours::is_in_peak_window` 每窗口
 *  `wall_time(epoch_ms, w.timezone)` 对称。
 *
 *  requestModel（PRD 07-09 D2）：请求模型名，用于 model scope 过滤；
 *  缺省 / 空串 = 无 model 上下文 → 跳过 model 过滤（兼容旧行为，向后兼容）。 */
export function isCurrentlyPeak(
  windows: PeakWindow[] | undefined | null,
  nowMs: number,
  requestModel: string = "",
): boolean {
  if (!windows || windows.length === 0) return false;
  const epochSec = Math.floor(nowMs / 1000);
  return windows.some((w) => {
    const { hour, minute, weekday, dayOfMonth } = wallTimeInTz(nowMs, w.timezone);
    return hit(w, hour, minute, weekday, dayOfMonth, requestModel, epochSec);
  });
}
