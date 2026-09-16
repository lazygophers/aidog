// ── 使用速率配色（前端唯一事实源）──
// 与后端 src-tauri/crates/aidog_core/src/gateway/usage_color.rs 阈值常量一一对应，禁前端各处各写一套。
// 全部「金额 / 额度」颜色统一按使用速率算红黄绿；后端能算 level 的路径（statusline / group-info）
// 直接消费后端 level，本模块供前端列表页从原始 est 数据本地算 level（同一阈值，无漂移）。

import type { ColorLevel } from "./colorScale";
import { clamp } from "../../utils/formatters";

// ── Coding plan tier：进度差（百分点）阈值（对齐 usage_color.rs）──
// 差额 = 额度已用% − 时间已过%（同为 0-100 的百分比，直接相减取百分点）。
// 负 = 用得比时间慢（省）；正 = 烧得比时间快（超支）。
/** |差额| ≤ 3 个百分点 → 黄；< -3 → 绿；> +3 → 红。 */
export const CODING_PACE_DELTA_PP = 3;

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

/** 进度差（百分点）= 额度已用% − 时间已过%。负 = 省着用，正 = 超支。 */
export function codingPaceDelta(utilization: number, remainMs: number, cycleMs: number): number {
  const usedPct = clamp(utilization, 0, 100);
  const elapsedPct = clamp(((cycleMs - remainMs) / cycleMs) * 100, 0, 100);
  return usedPct - elapsedPct;
}

/** 进度差 → ColorLevel。< -3 绿 / -3~+3 黄 / > +3 红。 */
export function colorFromPaceDelta(deltaPp: number): ColorLevel {
  if (!Number.isFinite(deltaPp)) return "neutral";
  if (deltaPp > CODING_PACE_DELTA_PP) return "danger";
  if (deltaPp >= -CODING_PACE_DELTA_PP) return "warning";
  return "success";
}

/**
 * Coding plan tier 配色级别（按「额度已用% − 时间已过%」的百分点差）。
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
  // 配额已耗尽（util≥100，剩余=0）→ danger。差额算法衡量「用量进度 vs 时间进度」，耗尽后无意义（对齐 usage_color.rs）。
  if (utilization >= 100) return "danger";
  return colorFromPaceDelta(codingPaceDelta(utilization, remainMs, cycleMs));
}

/** 余额配色级别（按剩余可用天数）。null = 无用量 / 无余额 → neutral（不报警）。 */
export function balanceColorLevel(daysRemaining: number | null | undefined): ColorLevel {
  if (daysRemaining == null || !Number.isFinite(daysRemaining) || daysRemaining < 0) return "neutral";
  if (daysRemaining < BALANCE_DAYS_DANGER) return "danger";
  if (daysRemaining < BALANCE_DAYS_WARN) return "warning";
  return "success";
}
