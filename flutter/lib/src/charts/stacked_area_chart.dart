/// 堆叠面积图（对应 React 版 `src/components/charts/StackedAreaChart.tsx`）。
///
/// Recharts 有 `stackId`，fl_chart 没有 —— 堆叠是这里自己累加出来的：
/// 第 i 条画的是「第 0..i 条之和」，相邻两条之间的带就是第 i 条本身。
///
/// **绘制顺序必须反过来**（栈顶先画、底层后画），否则上层的填充会盖住下层。
/// 反的是 [ChartSeries] 列表本身，不是另建一份下标映射 —— tooltip 与绘制共用
/// 同一个列表，序列身份跟着对象走（见 tooltip.dart 的说明）。
library;

import 'package:fl_chart/fl_chart.dart';
import 'package:flutter/material.dart';

import '../../charts/ticks.dart';
import '../shell/theme.dart';
import 'axes.dart';
import 'empty.dart';
import 'series.dart';
import 'tooltip.dart';
import 'tooltip_item.dart';

/// 累加到第 [upTo] 条（含）为止的第 [row] 行的值 —— 即该层的栈顶高度。
double stackTop(List<ChartSeries> series, int upTo, int row) {
  var s = 0.0;
  for (var i = 0; i <= upTo; i++) {
    s += series[i].points[row].y;
  }
  return s;
}

class AidogStackedAreaChart extends StatelessWidget {
  const AidogStackedAreaChart({
    super.key,
    required this.series,
    this.emptyText = '',
    this.emptyHint,
    this.tickCount = 5,
  });

  /// 系列序即堆叠序（Stats 喂总量降序 → 最大维度沉底、主色首层）。
  final List<ChartSeries> series;
  final String emptyText;
  final String? emptyHint;
  final int tickCount;

  @override
  Widget build(BuildContext context) {
    assertUniqueKeys(series);
    final t = AidogTheme.of(context);
    // 取形按行总量（对齐 React 的 rowTotal：降采样看栈顶轮廓，堆叠形状不破）。
    final rows = downsampleAligned(series, yOf: rowTotalOf(series));
    final xd = xDomain(rows);
    if (rows.isEmpty || xd == null || rows.first.points.isEmpty) {
      return ChartEmpty(emptyText, hint: emptyHint);
    }

    final xAxis = axisFromTicks(niceTicks(xd.min, xd.max, 6));
    final yd = stackedYDomain(rows);
    final yAxis = axisFromTicks(niceTicks(yd.min, yd.max, tickCount));
    final spanMs = xd.max - xd.min;
    final labelStyle = axisLabelStyle(t.c);

    // 栈顶先画 → 列表整体反转；tooltip 查的也是这一个列表。
    final drawn = rows.reversed.toList(growable: false);
    final n = rows.first.points.length;
    final bars = <LineChartBarData>[
      for (var d = 0; d < drawn.length; d++)
        () {
          // drawn[d] 在原堆叠序里的下标：栈顶（d=0）对应 rows.length-1。
          final layer = rows.length - 1 - d;
          final s = drawn[d];
          return LineChartBarData(
            spots: [
              for (var r = 0; r < n; r++)
                FlSpot(rows.first.points[r].x, stackTop(rows, layer, r)),
            ],
            isCurved: true,
            preventCurveOverShooting: true,
            color: s.color,
            barWidth: 1.5,
            dotData: const FlDotData(show: false),
            belowBarData: BarAreaData(
              show: true,
              gradient: LinearGradient(
                begin: Alignment.topCenter,
                end: Alignment.bottomCenter,
                colors: [
                  s.color.withValues(alpha: 0.45),
                  s.color.withValues(alpha: 0.08),
                ],
              ),
            ),
          );
        }(),
    ];

    // React `margin={{top:8,right:12,bottom:0,left:0}}`
    //（`StackedAreaChart.tsx:92`）—— fl_chart 没有外边距字段，包一层 Padding。
    return Padding(
      padding: const EdgeInsets.only(top: 8, right: 12),
      child: LineChart(
        LineChartData(
          minX: xAxis.min,
          maxX: xAxis.max,
          minY: yAxis.min,
          maxY: yAxis.max,
          lineBarsData: bars,
          gridData: FlGridData(
            show: true,
            drawVerticalLine: false,
            horizontalInterval: yAxis.interval,
            getDrawingHorizontalLine: (_) => FlLine(
              // `stroke-border/50`（`ui/chart.tsx:67`）。
              color: t.c.line.withValues(alpha: .5),
              strokeWidth: 1,
              dashArray: const [3, 3],
            ),
          ),
          borderData: FlBorderData(show: false),
          titlesData: FlTitlesData(
            topTitles: const AxisTitles(),
            rightTitles: const AxisTitles(),
            bottomTitles: AxisTitles(
              sideTitles: SideTitles(
                showTitles: true,
                interval: xAxis.interval,
                reservedSize: 24,
                getTitlesWidget: (v, meta) => Text(
                  formatTimeTick(v, spanMs),
                  style: labelStyle,
                  maxLines: 1,
                  softWrap: false,
                ),
              ),
            ),
            leftTitles: AxisTitles(
              sideTitles: SideTitles(
                showTitles: true,
                interval: yAxis.interval,
                reservedSize: 48,
                getTitlesWidget: (v, meta) => Text(
                  rows.first.format(v),
                  style: labelStyle,
                  maxLines: 1,
                  softWrap: false,
                ),
              ),
            ),
          ),
          lineTouchData: LineTouchData(
            // 盒样式对齐 `ui/chart.tsx:186` + `charts/tooltip.tsx:30`（同折线图）。
            touchTooltipData: LineTouchTooltipData(
              getTooltipColor: (_) => t.c.surface,
              tooltipBorder: BorderSide(color: t.c.line.withValues(alpha: .5)),
              tooltipPadding: const EdgeInsets.symmetric(
                horizontal: 10,
                vertical: 6,
              ),
              tooltipBorderRadius: BorderRadius.circular(AidogRadius.sm),
              maxContentWidth: 256,
              fitInsideHorizontally: true,
              fitInsideVertically: true,
              // 触点 y 是**累加后**的栈顶高度，不是该层自己的值；
              // tooltipRowAtSpot 回到序列点集取原值。
              getTooltipItems: (spots) => [
                for (var i = 0; i < spots.length; i++)
                  chartTooltipItem(
                    tooltipRowAtSpot(
                      drawn,
                      spots[i].barIndex,
                      spots[i].spotIndex,
                    ),
                    header: i == 0 ? formatTimeTick(spots[i].x, spanMs) : null,
                    c: t.c,
                  ),
              ],
            ),
          ),
        ),
      ),
    );
  }
}
