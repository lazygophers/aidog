// 图表引擎公共层（票 #31）：九组件（#35+）从这里取色板/刻度/降采样/容器/动画/tooltip。
export { PRIMARY_COLOR, seriesColor, seriesColors, heatColor, HEAT_MIN_ALPHA, HEAT_MAX_ALPHA, withDefaultColors } from "./palette";
export { niceTicks, formatTimeTick, bucketMs } from "./ticks";
export { downsampleLTTB, LTTB_THRESHOLD } from "./downsample";
export { useDrawIn, useDrawInPath } from "./drawIn";
export { ChartCard, type ChartCardProps } from "./ChartCard";
export { ChartsTooltip, tooltipValueRows } from "./tooltip";
// 批次一六组件（#35）
export { LineChart, type LineChartProps } from "./LineChart";
export { BarChart, type BarChartProps } from "./BarChart";
export { PieChart, type PieChartProps } from "./PieChart";
export { DonutChart, type DonutChartProps } from "./DonutChart";
export { HourHeatmap, type HourHeatmapProps } from "./HourHeatmap";
export { HourHeatBar, type HourHeatBarProps } from "./HourHeatBar";
export { GaugeChart, type GaugeChartProps, type GaugeTrendPoint } from "./GaugeChart";
// 批次二三组件（#39）：series_by 交叉聚合消费
export { StackedAreaChart, type StackedAreaChartProps } from "./StackedAreaChart";
export { StackedBarChart, type StackedBarChartProps } from "./StackedBarChart";
export { DimensionHeatmap, type DimensionHeatmapProps } from "./DimensionHeatmap";
// T10（#40）：散点直方图（服务端 bin 化矩阵）+ GaugeChart 趋势态
export { ScatterChart, scatterPoints, type ScatterChartProps, type ScatterPoint } from "./ScatterChart";
