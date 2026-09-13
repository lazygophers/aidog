// 图表引擎公共层（票 #31）：九组件（#35+）从这里取色板/刻度/降采样/容器/动画/tooltip。
export { PRIMARY_COLOR, seriesColor, seriesColors, heatColor, HEAT_MIN_ALPHA, HEAT_MAX_ALPHA } from "./palette";
export { niceTicks, formatTimeTick } from "./ticks";
export { downsampleLTTB, LTTB_THRESHOLD } from "./downsample";
export { drawInProps, useDrawInPath } from "./drawIn";
export { ChartCard, type ChartCardProps } from "./ChartCard";
export { ChartsTooltip } from "./tooltip";
