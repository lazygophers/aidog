/// 折线图（对应 React 版 `src/components/charts/LineChart.tsx`）。
///
/// 覆盖 React 版的全部能力：nice-ticks 双轴、LTTB 对齐降采样、首个左轴系列下方渐变面积、
/// 虚线辅线、mini 裸渲染、tooltip。
///
/// **双轴的实现差异（fl_chart 无第二标尺，上游 issue #429 仍 open）**：右轴系列的值被
/// 线性映射进左轴坐标系再画，右轴刻度按原值标注。tooltip **不读绘图坐标**，走
/// [tooltipRowAtSpot] 回到序列自己的点集取原值 —— 否则显示的是换算过的假数。
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

/// 把右轴原值映射进左轴坐标系（两轴域都退化时原样返回）。
double mapToLeft(double y, AxisSpec right, AxisSpec left) {
  final rSpan = right.max - right.min;
  final lSpan = left.max - left.min;
  if (rSpan == 0 || lSpan == 0) return y;
  return left.min + (y - right.min) / rSpan * lSpan;
}

class AidogLineChart extends StatefulWidget {
  const AidogLineChart({
    super.key,
    required this.series,
    this.emptyText = '',
    this.emptyHint,
    this.tickCount = 5,
    this.mini = false,
    this.area = false,
  });

  /// 每条一条线。左右轴由 [ChartSeries.rightAxis] 决定，虚线由 [ChartSeries.dashed]。
  /// 数值格式化挂在序列自己身上（见 tooltip.dart 的说明）。
  final List<ChartSeries> series;

  /// 空态文案（i18n 归票 I03，这里只收成串）。
  final String emptyText;
  final String? emptyHint;

  /// Y 轴 nice-ticks 目标刻度数，默认 5（与 React 版同默认）。
  final int tickCount;

  /// 迷你模式（托盘浮窗）：无网格、无轴、无点，只有线 + tooltip。
  final bool mini;

  /// 首个左轴系列下方渐变面积填充。
  final bool area;

  @override
  State<AidogLineChart> createState() => _AidogLineChartState();
}

/// build 里的重活（LTTB 降采样 + niceTicks×2 + FlSpot 列表）算一次的产物。
/// silent 刷新（代理事件，500ms 一轮）和 fl_chart 触摸状态变化都会整图重建，
/// 输入没变时必须复用（性能审计 #8）。
class _ChartDerived {
  _ChartDerived({
    required this.rows,
    required this.xAxis,
    required this.leftAxis,
    required this.rightAxis,
    required this.bars,
    required this.left,
    required this.right,
    required this.empty,
  });

  /// 无可画数据时的空标记：axes / bars 全是占位，build 直接走 ChartEmpty。
  final bool empty;

  final List<ChartSeries> rows;
  final AxisSpec xAxis, leftAxis, rightAxis;
  final List<LineChartBarData> bars;
  final List<ChartSeries> left, right;
}

class _AidogLineChartState extends State<AidogLineChart> {
  List<ChartSeries>? _derivedInput;
  _ChartDerived? _derived;

  _ChartDerived _ensureDerived() {
    final input = widget.series;
    if (identical(_derivedInput, input) &&
        _derived != null &&
        widget.tickCount == _tickCountOf &&
        widget.mini == _miniOf &&
        widget.area == _areaOf) {
      return _derived!;
    }
    final rows = downsampleAligned(input);
    final xd = xDomain(rows);
    _ChartDerived emptyOut() {
      _derivedInput = input;
      _tickCountOf = widget.tickCount;
      _miniOf = widget.mini;
      _areaOf = widget.area;
      return _derived = _ChartDerived(
        rows: rows,
        xAxis: axisFromTicks(const []),
        leftAxis: axisFromTicks(const []),
        rightAxis: axisFromTicks(const []),
        bars: const [],
        left: const [],
        right: const [],
        empty: true,
      );
    }

    if (rows.isEmpty || xd == null) return emptyOut();
    final left = rows.where((s) => !s.rightAxis).toList(growable: false);
    final right = rows.where((s) => s.rightAxis).toList(growable: false);

    final xAxis = axisFromTicks(niceTicks(xd.min, xd.max, 6));
    final ld = yDomain(left.isEmpty ? rows : left);
    final leftAxis = axisFromTicks(niceTicks(ld.min, ld.max, widget.tickCount));
    final rd = yDomain(right);
    final rightAxis = right.isEmpty
        ? leftAxis
        : axisFromTicks(niceTicks(rd.min, rd.max, widget.tickCount));

    final bars = <LineChartBarData>[
      for (final s in rows)
        LineChartBarData(
          spots: [
            for (final p in s.points)
              // 缺值画成 `nullSpot`：fl_chart 在这里断线，与 recharts 的
              // `connectNulls={false}` 同行为（`LineChart.tsx:201`）。
              // 落到 0 会把「那个时段没有数据」画成「那个时段是 0」。
              if (p.missing)
                FlSpot.nullSpot
              else
                FlSpot(
                  p.x,
                  s.rightAxis ? mapToLeft(p.y, rightAxis, leftAxis) : p.y,
                ),
          ],
          isCurved: true,
          preventCurveOverShooting: true,
          color: s.color,
          barWidth: 2,
          dashArray: s.dashed ? const [3, 3] : null,
          dotData: FlDotData(
            show: !widget.mini && rows.first.points.length <= 60,
          ),
          belowBarData: BarAreaData(
            show: widget.area && !s.rightAxis && identical(s, left.firstOrNull),
            gradient: LinearGradient(
              begin: Alignment.topCenter,
              end: Alignment.bottomCenter,
              colors: [
                s.color.withValues(alpha: 0.2),
                s.color.withValues(alpha: 0),
              ],
            ),
          ),
        ),
    ];

    _derivedInput = input;
    _tickCountOf = widget.tickCount;
    _miniOf = widget.mini;
    _areaOf = widget.area;
    return _derived = _ChartDerived(
      rows: rows,
      xAxis: xAxis,
      leftAxis: leftAxis,
      rightAxis: rightAxis,
      bars: bars,
      left: left,
      right: right,
      empty: false,
    );
  }

  // 缓存键的另外三个标量；series 用 identical 判（调用方每次 setState 新建列表
  // 就是缓存 miss，静态数据是同一引用就命中）。
  int _tickCountOf = -1;
  bool _miniOf = false;
  bool _areaOf = false;

  @override
  Widget build(BuildContext context) {
    assertUniqueKeys(widget.series);
    final t = AidogTheme.of(context);
    final d = _ensureDerived();
    if (d.empty) return ChartEmpty(widget.emptyText, hint: widget.emptyHint);
    final rows = d.rows;
    final xAxis = d.xAxis;
    final leftAxis = d.leftAxis;
    final rightAxis = d.rightAxis;
    final left = d.left;
    final right = d.right;

    final spanMs = xAxis.max - xAxis.min;
    final labelStyle = axisLabelStyle(t.c);

    // 图表自持重绘边界：hover 指示线 / tooltip 不再触发整页重绘（性能审计 #8）。
    return RepaintBoundary(
      child: LineChart(
        LineChartData(
          minX: xAxis.min,
          maxX: xAxis.max,
          minY: leftAxis.min,
          maxY: leftAxis.max,
          lineBarsData: d.bars,
          gridData: FlGridData(
            show: !widget.mini,
            drawVerticalLine: false,
            horizontalInterval: leftAxis.interval,
            getDrawingHorizontalLine: (_) => FlLine(
              color: t.c.line,
              strokeWidth: 1,
              dashArray: const [3, 3],
            ),
          ),
          borderData: FlBorderData(show: false),
          titlesData: FlTitlesData(
            show: !widget.mini,
            topTitles: const AxisTitles(),
            bottomTitles: AxisTitles(
              sideTitles: SideTitles(
                showTitles: !widget.mini,
                interval: xAxis.interval,
                reservedSize: 24,
                getTitlesWidget: (v, meta) =>
                    _tick(formatTimeTick(v, spanMs), labelStyle),
              ),
            ),
            leftTitles: AxisTitles(
              sideTitles: SideTitles(
                showTitles: !widget.mini,
                interval: leftAxis.interval,
                reservedSize: 48,
                getTitlesWidget: (v, meta) => _tick(
                  (left.firstOrNull ?? rows.first).format(v),
                  labelStyle,
                ),
              ),
            ),
            rightTitles: AxisTitles(
              sideTitles: SideTitles(
                // 右轴刻度标**原值**：位置在左轴坐标系里，文字反解回右轴域。
                showTitles: !widget.mini && right.isNotEmpty,
                interval: leftAxis.interval,
                reservedSize: 48,
                getTitlesWidget: (v, meta) => _tick(
                  right.isEmpty
                      ? ''
                      : right.first.format(mapToLeft(v, leftAxis, rightAxis)),
                  labelStyle,
                ),
              ),
            ),
          ),
          lineTouchData: LineTouchData(
            touchTooltipData: LineTouchTooltipData(
              getTooltipColor: (_) => t.c.surface2,
              tooltipBorder: BorderSide(color: t.c.line),
              maxContentWidth: 260,
              fitInsideHorizontally: true,
              fitInsideVertically: true,
              getTooltipItems: (spots) => [
                for (var i = 0; i < spots.length; i++)
                  chartTooltipItem(
                    tooltipRowAtSpot(
                      rows,
                      spots[i].barIndex,
                      spots[i].spotIndex,
                    ),
                    // 表头只挂第一行（fl_chart 每个触点一行，没有独立表头槽）。
                    header: i == 0 ? formatTimeTick(spots[i].x, spanMs) : null,
                    c: t.c,
                  ),
              ],
            ),
            getTouchedSpotIndicator: (bar, indexes) => [
              for (final _ in indexes)
                TouchedSpotIndicatorData(
                  FlLine(
                    color: t.c.line,
                    strokeWidth: 1,
                    dashArray: const [4, 3],
                  ),
                  FlDotData(show: true),
                ),
            ],
          ),
        ),
      ),
    );
  }
}

Widget _tick(String text, TextStyle style) =>
    Text(text, style: style, maxLines: 1, softWrap: false);
