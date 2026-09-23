// ── 图表系列色板（spec §A3 定案）──
// 主线 = var(--data-primary)（token `data-primary`，蓝紫 dark #5E6AD2 / light #4E59C4）。
// **不是 var(--primary)**：那是界面填充色，2026-09-23 之后深色下是近黑，
// 画成折线等于看不见。数据色与界面强调色从此是两档，别再合并。
// 辅线灰阶：var(--chart-2..5) 循环。globals.css 的 --chart-N 定义保持纯灰阶不动，
// 色板语义只在这里表达。
import { clamp } from "@/utils/formatters";

export const PRIMARY_COLOR = "var(--data-primary)" as const;

const AUX_COLORS = [
  "var(--chart-2)",
  "var(--chart-3)",
  "var(--chart-4)",
  "var(--chart-5)",
] as const;

/**
 * 第 index 个系列的颜色：0（及负数）→ 主系列琥珀，1 起循环灰阶 --chart-2..5。
 * 例：seriesColor(0)="var(--data-primary)"，seriesColor(1)="var(--chart-2)"，
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

// ── ChartConfig 默认色注入（spec §A3：色板语义在 ChartConfig 层表达）──
import type { ChartConfig } from "@/components/ui/chart";

/**
 * 按 config 键序补默认系列色：第 i 个键未显式给 color（且无 theme 双色定义）时
 * 补 seriesColor(i)（首位琥珀，其余灰阶）。组件统一走这里，再交 ChartStyle 落
 * `--color-<key>`，系列引用 `var(--color-<key>)`。
 */
export function withDefaultColors(config: ChartConfig): ChartConfig {
  return Object.fromEntries(
    Object.entries(config).map(([key, item], i) => [
      key,
      item.color ?? item.theme ? item : { ...item, color: seriesColor(i) },
    ]),
  );
}
