// ── 图表系列色板（spec §A3 定案）──
// 主线琥珀：首位/主系列 = var(--primary)（mono 萤火虫主题 dark #e8c547 / light #c49a3c）。
// 辅线灰阶：var(--chart-2..5) 循环。globals.css 的 --chart-N 定义保持纯灰阶不动，
// 色板语义只在这里表达。
import { clamp } from "@/utils/formatters";

export const PRIMARY_COLOR = "var(--primary)" as const;

const AUX_COLORS = [
  "var(--chart-2)",
  "var(--chart-3)",
  "var(--chart-4)",
  "var(--chart-5)",
] as const;

/**
 * 第 index 个系列的颜色：0（及负数）→ 主系列琥珀，1 起循环灰阶 --chart-2..5。
 * 例：seriesColor(0)="var(--primary)"，seriesColor(1)="var(--chart-2)"，
 * seriesColor(5)= 回绕 "var(--chart-2)"。
 */
export function seriesColor(index: number): string {
  if (index <= 0) return PRIMARY_COLOR;
  return AUX_COLORS[(index - 1) % AUX_COLORS.length];
}

/** 前 count 个系列的颜色数组（ChartConfig / Cell fill 批量取色用）。 */
export function seriesColors(count: number): string[] {
  return Array.from({ length: count }, (_, i) => seriesColor(i));
}

// ── 热力图色带：琥珀 alpha 阶梯（原型验证值 .06 → .92，spec §A3）──
const HEAT_RGB = "232, 197, 71"; // 琥珀 #e8c547
export const HEAT_MIN_ALPHA = 0.06;
export const HEAT_MAX_ALPHA = 0.92;

/**
 * 热力格子颜色：t ∈ [0,1]（格子值 / 色带最大值）→ 琥珀 rgba alpha 阶梯。
 * t 越大越热（越不透明越亮）；区间外 clamp 到两端。
 */
export function heatColor(t: number): string {
  const alpha = HEAT_MIN_ALPHA + (HEAT_MAX_ALPHA - HEAT_MIN_ALPHA) * clamp(t, 0, 1);
  return `rgba(${HEAT_RGB}, ${alpha.toFixed(3)})`;
}
