// ── 使用速率配色（前端唯一事实源）──
// 与后端 src-tauri/src/gateway/usage_color.rs 阈值常量一一对应，禁前端各处各写一套。
// 全部「金额 / 额度」颜色统一按使用速率算红黄绿；后端能算 level 的路径（statusline / group-info）
// 直接消费后端 level，本模块供前端列表页从原始 est 数据本地算 level（同一阈值，无漂移）。

import type { ColorLevel } from "./colorScale";
import { clamp } from "../../utils/formatters";

// ── Coding plan tier：剩余可用时间% 阈值（对齐 usage_color.rs）──
/** 剩余可用时间% < 40 → 红 */
export const CODING_REMAIN_PCT_DANGER = 40;
/** 40 ≤ 剩余 ≤ 60 → 黄；> 60 → 绿 */
export const CODING_REMAIN_PCT_WARN = 60;

// ── 余额：剩余可用天数阈值（对齐 usage_color.rs）──
/** days_remaining < 1 → 红 */
export const BALANCE_DAYS_DANGER = 1;
/** days_remaining < 3 → 黄；否则绿 */
export const BALANCE_DAYS_WARN = 3;

// ── 周期时长（按 tier name 硬编码，单位毫秒，对齐 usage_color.rs）──
const HOUR_MS = 3_600_000;
const DAY_MS = 24 * HOUR_MS;

/** 后端语义级别字符串（group-info / statusline 下发）。 */
export type UsageLevelStr = "red" | "yellow" | "green" | "neutral";

/** 后端 level 字符串 → 前端 ColorLevel。未知 / 空 → neutral。 */
export function usageLevelToColor(level: string | null | undefined): ColorLevel {
  switch (level) {
    case "red":
      return "danger";
    case "yellow":
      return "warning";
    case "green":
      return "success";
    default:
      return "neutral";
  }
}

/** 由 tier name 返回周期时长（ms）。未知 name → null（无周期概念 → 中性）。 */
export function cycleMsForTier(name: string): number | null {
  switch (name) {
    case "five_hour":
      return 5 * HOUR_MS;
    case "weekly_limit":
    case "seven_day":
      return 7 * DAY_MS;
    case "mcp_monthly":
      return 30 * DAY_MS;
    default:
      return null;
  }
}

/**
 * 剩余可用时间% = clamp(100 / pace, 0, 100)；pace = util_ratio / elapsed_ratio。
 * pace < 1（省着用）→ 100% 充足；elapsed_ratio → 0 时 pace → ∞ → 0%。
 */
export function codingRemainPct(utilization: number, remainMs: number, cycleMs: number): number {
  const utilRatio = clamp(utilization / 100, 0, 1);
  const elapsedRatio = clamp((cycleMs - remainMs) / cycleMs, 0, 1);
  if (utilRatio <= 0) return 100;
  if (elapsedRatio <= 0) return 0;
  const pace = utilRatio / elapsedRatio;
  if (pace <= 0) return 100;
  return clamp(100 / pace, 0, 100);
}

/** 剩余可用时间% → ColorLevel。<40 红 / 40-60 黄 / >60 绿。 */
export function colorFromCodingRemainPct(remainPct: number): ColorLevel {
  if (!Number.isFinite(remainPct)) return "neutral";
  if (remainPct < CODING_REMAIN_PCT_DANGER) return "danger";
  if (remainPct <= CODING_REMAIN_PCT_WARN) return "warning";
  return "success";
}

/**
 * Coding plan tier 配色级别。
 *   - utilization：额度已用百分比（0-100）
 *   - remainMs：本周期剩余时间（ms）；null = 无可靠 remain（无 resets_at / 无 window_start）
 *   - cycleMs：周期时长（ms）；null = 未知 name（无周期概念）
 * 缺 remain / cycle / 非法 util → neutral（不静默走旧利用率阈值，不误报）。
 */
export function codingTierLevel(
  utilization: number,
  remainMs: number | null,
  cycleMs: number | null,
): ColorLevel {
  if (!Number.isFinite(utilization) || utilization < 0) return "neutral";
  if (remainMs == null || cycleMs == null || cycleMs <= 0) return "neutral";
  // 配额已耗尽（util≥100，剩余=0）→ danger。pace 算法衡量「撑到周期末」，耗尽后无意义（对齐 usage_color.rs）。
  if (utilization >= 100) return "danger";
  return colorFromCodingRemainPct(codingRemainPct(utilization, remainMs, cycleMs));
}

/** 余额配色级别（按剩余可用天数）。null = 无用量 / 无余额 → neutral（不报警）。 */
export function balanceColorLevel(daysRemaining: number | null | undefined): ColorLevel {
  if (daysRemaining == null || !Number.isFinite(daysRemaining) || daysRemaining < 0) return "neutral";
  if (daysRemaining < BALANCE_DAYS_DANGER) return "danger";
  if (daysRemaining < BALANCE_DAYS_WARN) return "warning";
  return "success";
}
