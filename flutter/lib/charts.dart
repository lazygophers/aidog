/// 图表层（票 I04）。页面票（I06 首页/统计、I07 平台/分组/日志）只从这里 import。
///
/// ```dart
/// import 'package:aidog_flutter/charts.dart';
///
/// final p = ChartPalette.of(context);
/// AidogLineChart(series: [
///   ChartSeries(key: 'cost', label: '成本', color: p.series(0),
///               points: rows, format: formatCostUsd),
/// ]);
/// ```
///
/// ## 三条不能破的
///
/// 1. **序列身份跟着 [ChartSeries] 对象走，不跟位置走。** fl_chart 的 tooltip 回调只给
///    `barIndex`，而 Stats 的序列顺序按总量动态排（`src/pages/Stats.tsx:114`）——
///    位置 + 动态排序 = 显示一个错的数还不报错（commit `a7e665c8` 修的就是这个）。
///    取行一律走 [tooltipRowAtSpot] / [tooltipRowFor]，别自己按下标查平行数组。
/// 2. **色值只来自 [ChartPalette]**：主色跟主题，辅线灰阶与热力琥珀是固定常量。
/// 3. **卡片壳、标题、图例归 `SeriesTile`**（票 I02 的四种格子之一）。本层只出图表本体
///    与诚实空态（[ChartEmpty]），不造第五种格子。
///
/// ## 迁了什么、没迁什么
///
/// 迁了 8 个（React 侧有页面消费者的全部）：折线 / 堆叠面积 / 环形 / 散点走 fl_chart（MIT，
/// 与本项目 AGPL-3.0-or-later 相容；Syncfusion 专有许可不可用）；三张热力图是 GridView
/// 语义的格子布局（React 那边本来就零 Recharts）；仪表盘环 + 趋势 sparkline 走 CustomPainter。
///
/// **没迁 3 个**：`BarChart` / `PieChart` / `StackedBarChart`（共 276 行）在 React 侧
/// 零页面调用者，只被 `index.ts` 和自己的测试引用。没有消费者就不迁。
library;

export 'charts/downsample.dart';
export 'charts/scatter.dart';
export 'charts/ticks.dart';
export 'src/charts/axes.dart';
export 'src/charts/donut_chart.dart';
export 'src/charts/empty.dart';
export 'src/charts/gauge_chart.dart';
export 'src/charts/heatmaps.dart';
export 'src/charts/line_chart.dart';
export 'src/charts/palette.dart';
export 'src/charts/scatter_chart.dart';
export 'src/charts/series.dart';
export 'src/charts/stacked_area_chart.dart';
export 'src/charts/tooltip.dart';
export 'src/charts/tooltip_item.dart';
