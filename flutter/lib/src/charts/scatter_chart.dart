/// 散点直方图（对应 React 版 `src/components/charts/ScatterChart.tsx`）。
///
/// 数据层（bin 矩阵 → 点集）是票 I05 的 `charts/scatter.dart::scatterPoints`，这里只画。
/// 点大小 ∝ count（Recharts 的 `ZAxis range` 是**面积**区间 24–400，
/// fl_chart 收的是半径 → `r = sqrt(area / π)`，同一视觉语义）。
library;

import 'dart:math' as math;

import 'package:fl_chart/fl_chart.dart';
import 'package:flutter/material.dart';

import '../../charts/scatter.dart';
import '../../charts/ticks.dart';
import '../../stats/models.dart';
import '../../utils/formatters.dart';
import '../shell/theme.dart';
import 'axes.dart';
import 'empty.dart';
import 'palette.dart';

/// Recharts `ZAxis range=[24, 400]` 的面积下/上限。下限保「单请求也看得见」，上限防盖格。
const double kScatterMinArea = 24;
const double kScatterMaxArea = 400;

/// count → 半径。[maxCount] ≤ 最小值时全部落在下限（单一计数不该放大成满格）。
double scatterRadius(int count, int maxCount) {
  final t = maxCount <= 1 ? 0.0 : (count - 1) / (maxCount - 1);
  final area = kScatterMinArea + (kScatterMaxArea - kScatterMinArea) * t;
  return math.sqrt(area / math.pi);
}

class AidogScatterChart extends StatelessWidget {
  const AidogScatterChart({
    super.key,
    required this.histogram,
    required this.xLabel,
    required this.yLabel,
    this.formatX = formatDurationMs,
    this.formatY = formatCostUsd,
    required this.countLabel,
    this.emptyText = '',
    this.emptyHint,
  });

  /// 服务端 bin 化结果（`statsApi.scatterHistogram`）。
  final ScatterHistogram histogram;

  /// 轴标签（x = 延迟，y = 成本）；i18n 归票 I03。
  final String xLabel;
  final String yLabel;
  final String Function(double) formatX;
  final String Function(double) formatY;

  /// tooltip 第三行：请求数文案（收 count 出整句，如 `(n) => '$n 次请求'`）。
  final String Function(int) countLabel;
  final String emptyText;
  final String? emptyHint;

  @override
  Widget build(BuildContext context) {
    final t = AidogTheme.of(context);
    final points = scatterPoints(histogram);
    if (points.isEmpty) return ChartEmpty(emptyText, hint: emptyHint);

    // 轴域钉在 bin 边界（服务端算好的 nice 边界），空矩阵退化 [0,1] —— 与 React 版同规则。
    final xAxis = axisFromTicks(
      niceTicks(
        histogram.durationBins.first,
        histogram.durationBins.last,
        5,
      ),
      fallbackMax: 1,
    );
    final yAxis = axisFromTicks(
      niceTicks(histogram.costBins.first, histogram.costBins.last, 5),
      fallbackMax: 1,
    );
    final maxCount = points.map((p) => p.count).reduce(math.max);
    final color = ChartPalette(t.c).primary.withValues(alpha: 0.55);
    final labelStyle = axisLabelStyle(t.c);

    return ScatterChart(
      ScatterChartData(
        minX: xAxis.min,
        maxX: xAxis.max,
        minY: yAxis.min,
        maxY: yAxis.max,
        scatterSpots: [
          for (final p in points)
            ScatterSpot(
              p.x,
              p.y,
              dotPainter: FlDotCirclePainter(
                radius: scatterRadius(p.count, maxCount),
                color: color,
              ),
            ),
        ],
        gridData: FlGridData(
          show: true,
          horizontalInterval: yAxis.interval,
          verticalInterval: xAxis.interval,
          getDrawingHorizontalLine: (_) =>
              FlLine(color: t.c.line, strokeWidth: 1, dashArray: const [3, 3]),
          getDrawingVerticalLine: (_) =>
              FlLine(color: t.c.line, strokeWidth: 1, dashArray: const [3, 3]),
        ),
        borderData: FlBorderData(show: false),
        titlesData: FlTitlesData(
          topTitles: const AxisTitles(),
          rightTitles: const AxisTitles(),
          bottomTitles: AxisTitles(
            axisNameSize: 16,
            axisNameWidget: Text(
              xLabel,
              style: AidogType.caption.copyWith(color: t.c.fg3),
            ),
            sideTitles: SideTitles(
              showTitles: true,
              interval: xAxis.interval,
              reservedSize: 24,
              getTitlesWidget: (v, meta) =>
                  Text(formatX(v), style: labelStyle, maxLines: 1),
            ),
          ),
          leftTitles: AxisTitles(
            axisNameSize: 16,
            axisNameWidget: Text(
              yLabel,
              style: AidogType.caption.copyWith(color: t.c.fg3),
            ),
            sideTitles: SideTitles(
              showTitles: true,
              interval: yAxis.interval,
              reservedSize: 56,
              getTitlesWidget: (v, meta) =>
                  Text(formatY(v), style: labelStyle, maxLines: 1),
            ),
          ),
        ),
        scatterTouchData: ScatterTouchData(
          touchTooltipData: ScatterTouchTooltipData(
            getTooltipColor: (_) => t.c.surface2,
            tooltipBorder: BorderSide(color: t.c.line),
            fitInsideHorizontally: true,
            fitInsideVertically: true,
            // 触点自带坐标（不是位置索引），按坐标回查 count。
            // 查不到不静默显示 0 —— 那是数据层出了问题，该炸。
            getTooltipItems: (spot) => scatterTooltipItem(
              points.firstWhere(
                (p) => p.x == spot.x && p.y == spot.y,
                orElse: () => throw StateError('散点 (${spot.x}, ${spot.y}) 不在点集里'),
              ),
              xLabel: xLabel,
              yLabel: yLabel,
              formatX: formatX,
              formatY: formatY,
              countLabel: countLabel,
              c: t.c,
            ),
          ),
        ),
      ),
    );
  }
}

/// tooltip 三行：`延迟 · 1.2 s` / `成本 · $0.003` / `12 次请求`（与 React 版同三行）。
ScatterTooltipItem scatterTooltipItem(
  ScatterPoint p, {
  required String xLabel,
  required String yLabel,
  required String Function(double) formatX,
  required String Function(double) formatY,
  required String Function(int) countLabel,
  required AidogColors c,
}) {
  final style = AidogType.numSm.copyWith(
    color: c.fg,
    fontFeatures: const [FontFeature.tabularFigures()],
  );
  return ScatterTooltipItem(
    '$xLabel · ${formatX(p.x)}',
    textStyle: style,
    textAlign: TextAlign.left,
    children: [
      TextSpan(text: '\n$yLabel · ${formatY(p.y)}', style: style),
      TextSpan(text: '\n${countLabel(p.count)}', style: style),
    ],
  );
}
