// ── 指标色编码工具 ──
// 成功率 / 错误率 / 成本等指标 → 语义色（CSS 变量，明暗双模式对比度由 globals.css `:root` 语义色保证）。
// 禁直接返回硬编码十六进制主题色；全部走 var(--color-*)。

/** 语义级别。 */
export type ColorLevel = "success" | "warning" | "danger" | "neutral";

/** 级别 → 前景文字 CSS 变量。 */
export function levelColor(level: ColorLevel): string {
  return `var(--color-${level})`;
}

/** 级别 → 浅色背景 CSS 变量（用于 chip / 进度条底色）。 */
export function levelBg(level: ColorLevel): string {
  return `var(--color-${level}-bg)`;
}

/**
 * 成功率（0–100）→ 级别。
 * >= 99 success / >= 95 warning / < 95 danger。total 为 0 时 neutral（无数据不强行着色）。
 */
export function successRateLevel(rate: number, totalRequests = 1): ColorLevel {
  if (totalRequests <= 0) return "neutral";
  if (rate >= 99) return "success";
  if (rate >= 95) return "warning";
  return "danger";
}

/**
 * 错误率（0–100）→ 级别。successRateLevel 的反向。
 */
export function errorRateLevel(rate: number, totalRequests = 1): ColorLevel {
  if (totalRequests <= 0) return "neutral";
  if (rate <= 1) return "success";
  if (rate <= 5) return "warning";
  return "danger";
}

/**
 * 成本 → 级别（相对阈值，调用方可传 thresholds 调整）。
 * 默认：< warnAt success，< dangerAt warning，否则 danger。不对成本好坏做绝对判断，仅做视觉分档。
 */
export function costLevel(cost: number, warnAt = 1, dangerAt = 10): ColorLevel {
  if (cost <= 0) return "neutral";
  if (cost < warnAt) return "success";
  if (cost < dangerAt) return "warning";
  return "danger";
}

/** 便捷：成功率 → 前景色。 */
export function successRateColor(rate: number, totalRequests = 1): string {
  return levelColor(successRateLevel(rate, totalRequests));
}
