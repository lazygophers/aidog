// ── shared 通用展示组件 barrel ──
// 供三页（Platforms / Groups / Stats）统一 import。

export { CompactCard, type CompactCardProps } from "./CompactCard";
export { StatChip, type StatChipProps } from "./StatChip";
export { BalanceBar, type BalanceBarProps } from "./BalanceBar";
export { CostTrendChart, type CostTrendChartProps } from "./CostTrendChart";
export {
  type ColorLevel,
  levelColor,
  levelBg,
  successRateLevel,
  errorRateLevel,
  costLevel,
  successRateColor,
} from "./colorScale";
export {
  type UsageLevelStr,
  usageLevelToColor,
  cycleMsForTier,
  codingRemainPct,
  colorFromCodingRemainPct,
  codingTierLevel,
  balanceColorLevel,
} from "./usageColor";
