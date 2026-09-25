// 回调路径的测试：轴刻度 widget、tooltip 文本、触点指示器、空态副行、浅色模式。
//
// 这些是 fl_chart 只在真实触摸 / 布局时才调的闭包，widget 树断言够不着，
// 所以直接从图表的数据模型上取出来调一遍 —— 出错的正是这些闭包（tooltip 走偏就在这里）。

import 'package:aidog_flutter/charts.dart';
import 'package:aidog_flutter/shell.dart';
import 'package:aidog_flutter/stats/models.dart';
import 'package:aidog_flutter/utils/formatters.dart';
import 'package:fl_chart/fl_chart.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

Widget host(Widget child, {AidogMode mode = AidogMode.dark}) => MaterialApp(
  theme: aidogThemeData(mode),
  home: Scaffold(body: SizedBox(width: 600, height: 300, child: child)),
);

final t0 = DateTime.parse('2026-09-13T00:00:00').millisecondsSinceEpoch;
const hour = 3600000;
const day = 86400000;

List<ChartPoint> pts(int n, double Function(int) y, {int step = hour}) => [
  for (var i = 0; i < n; i++) ChartPoint((t0 + i * step).toDouble(), y(i)),
];

ChartSeries series(
  String key,
  List<ChartPoint> points, {
  String Function(double)? format,
  bool rightAxis = false,
}) => ChartSeries(
  key: key,
  label: key,
  color: const Color(0xFFAA0000),
  points: points,
  format: format ?? ((n) => '$n'),
  rightAxis: rightAxis,
);

LineChartData lineData(WidgetTester tester) =>
    tester.widget<LineChart>(find.byType(LineChart)).data;

/// SideTitles 回调要一个 TitleMeta；这里只有 interval 与轴域被用到。
TitleMeta meta(SideTitles s) => TitleMeta(
  min: 0,
  max: 1,
  parentAxisSize: 100,
  axisPosition: 0,
  appliedInterval: s.interval ?? 1,
  sideTitles: s,
  formattedValue: '',
  axisSide: AxisSide.left,
  rotationQuarterTurns: 0,
);

String tickText(SideTitles s, double v) =>
    (s.getTitlesWidget(v, meta(s)) as Text).data!;

String countLabel(int n) => 'charts.scatterCount:$n';

void main() {
  group('散点图回调', () {
    const hist = ScatterHistogram(
      durationBins: [0, 1000, 2000],
      costBins: [0, 0.02, 0.04],
      counts: [
        [10, 0],
        [0, 3],
      ],
    );

    testWidgets('轴刻度走 formatters 的默认实现，tooltip 出三行', (tester) async {
      await tester.pumpWidget(
        host(
          AidogScatterChart(
            histogram: hist,
            xLabel: 'x',
            yLabel: 'y',
            countLabel: countLabel,
          ),
        ),
      );
      final d = tester.widget<ScatterChart>(find.byType(ScatterChart)).data;
      expect(
        tickText(d.titlesData.bottomTitles.sideTitles, 1500),
        formatDurationMs(1500),
      );
      expect(
        tickText(d.titlesData.leftTitles.sideTitles, 0.02),
        formatCostUsd(0.02),
      );
      final item = d.scatterTouchData.touchTooltipData.getTooltipItems(
        d.scatterSpots.first,
      )!;
      // React: 延迟 · … / 成本 · … / n 次请求
      expect(item.text, 'x · ${formatDurationMs(500)}');
      final rest = item.children!.map((c) => c.text).join();
      expect(rest, contains('y · ${formatCostUsd(0.01)}'));
      expect(rest, contains(countLabel(10)));
      // tooltip 盒底是 `bg-background`（`src/components/ui/chart.tsx:186`）
      // = Flutter 的 surface，不是 surface2。
      expect(
        d.scatterTouchData.touchTooltipData.getTooltipColor(
          d.scatterSpots.first,
        ),
        AidogColors.dark.surface,
      );
    });

    testWidgets('触点不在点集里 → 抛，不静默显示 0', (tester) async {
      await tester.pumpWidget(
        host(
          AidogScatterChart(
            histogram: hist,
            xLabel: 'x',
            yLabel: 'y',
            countLabel: countLabel,
          ),
        ),
      );
      final d = tester.widget<ScatterChart>(find.byType(ScatterChart)).data;
      expect(
        () => d.scatterTouchData.touchTooltipData.getTooltipItems(
          ScatterSpot(999, 999),
        ),
        throwsStateError,
      );
    });
  });

  group('堆叠面积回调', () {
    testWidgets('轴刻度 + tooltip 报各层自己的原值（不是累加值）', (tester) async {
      final s = [
        series('claude', pts(14, (i) => (i % 5) + 1.0, step: day)),
        series('glm', pts(14, (i) => (i % 3) + 1.0, step: day)),
      ];
      await tester.pumpWidget(host(AidogStackedAreaChart(series: s)));
      final d = lineData(tester);
      expect(tickText(d.titlesData.leftTitles.sideTitles, 4), '4.0');
      expect(
        tickText(d.titlesData.bottomTitles.sideTitles, d.minX),
        isNotEmpty,
      );

      final spot = LineBarSpot(
        d.lineBarsData[0],
        0,
        d.lineBarsData[0].spots[0],
      );
      final text = d.lineTouchData.touchTooltipData
          .getTooltipItems([spot])
          .single!
          .children!
          .map((c) => c.text)
          .join();
      // 0 号 bar 是栈顶 glm；绘图 y 是 claude+glm 的和，tooltip 必须只报 glm 的值
      expect(text, contains('glm'));
      expect(text, contains('${s[1].points[0].y}'));
      expect(d.lineBarsData[0].spots[0].y, stackTop(s, 1, 0));
      // 同上：`bg-background` = surface。
      expect(
        d.lineTouchData.touchTooltipData.getTooltipColor(spot),
        AidogColors.dark.surface,
      );
    });
  });

  group('折线图回调', () {
    testWidgets('触点指示器 + 右轴刻度反解回原值', (tester) async {
      final req = series('req', pts(24, (i) => i * 10.0));
      final cost = series(
        'cost',
        pts(24, (i) => (i % 7) + 0.5),
        rightAxis: true,
        format: (n) => n.toStringAsFixed(2),
      );
      await tester.pumpWidget(host(AidogLineChart(series: [req, cost])));
      final d = lineData(tester);
      expect(
        d.lineTouchData.getTouchedSpotIndicator(d.lineBarsData[0], const [
          0,
        ]).length,
        1,
      );
      // 左轴顶 → 反解回右轴域的顶（cost 原值量纲，不是 req 的量纲）
      final top = double.parse(
        tickText(d.titlesData.rightTitles.sideTitles, d.maxY),
      );
      expect(top, closeTo(7, 1)); // cost 最大 6.5 → nice 刻度顶 7
      expect(top, lessThan(50));
    });

    testWidgets('单序列时左轴刻度用那条序列的格式化', (tester) async {
      await tester.pumpWidget(
        host(
          AidogLineChart(
            series: [
              series('a', pts(5, (i) => i.toDouble()), format: (n) => 'v$n'),
            ],
          ),
        ),
      );
      expect(
        tickText(lineData(tester).titlesData.leftTitles.sideTitles, 2),
        'v2.0',
      );
    });
  });

  testWidgets('空态副行显示出来', (tester) async {
    await tester.pumpWidget(
      host(
        const AidogLineChart(
          series: [],
          emptyText: 'charts.noData',
          emptyHint: 'charts.filtered',
        ),
      ),
    );
    expect(find.text('charts.noData'), findsOneWidget);
    expect(find.text('charts.filtered'), findsOneWidget);
  });

  testWidgets('浅色模式照样画得出（色值不写死一套）', (tester) async {
    await tester.pumpWidget(
      host(
        AidogLineChart(series: [series('a', pts(5, (i) => i.toDouble()))]),
        mode: AidogMode.light,
      ),
    );
    expect(find.byType(LineChart), findsOneWidget);
  });

  group('模型的相等性与重绘判定', () {
    test('ChartPoint', () {
      expect(const ChartPoint(1, 2), const ChartPoint(1, 2));
      expect(const ChartPoint(1, 2).hashCode, const ChartPoint(1, 2).hashCode);
      expect(const ChartPoint(1, 2) == const ChartPoint(1, 3), isFalse);
      expect(const ChartPoint(1, 2).toString(), contains('1.0'));
    });

    test('TooltipRow', () {
      final a = tooltipRowAt(
        [
          series('k', const [ChartPoint(0, 1)]),
        ],
        0,
        1,
      );
      final b = tooltipRowAt(
        [
          series('k', const [ChartPoint(0, 1)]),
        ],
        0,
        1,
      );
      expect(a, b);
      expect(a.hashCode, b.hashCode);
      expect(a.toString(), contains('k'));
    });

    test('ChartSeries.total 是 y 之和', () {
      expect(series('k', const [ChartPoint(0, 1), ChartPoint(1, 2)]).total, 3);
    });

    test('两个 painter 的 shouldRepaint', () {
      const a = GaugeRingPainter(
        fraction: .5,
        ringWidth: 10,
        arc: Color(0xFF000001),
        track: Color(0xFF000002),
      );
      const b = GaugeRingPainter(
        fraction: .6,
        ringWidth: 10,
        arc: Color(0xFF000001),
        track: Color(0xFF000002),
      );
      expect(a.shouldRepaint(b), isTrue);
      expect(a.shouldRepaint(a), isFalse);

      const p = [GaugeTrendPoint(at: 0, fraction: 0)];
      const s1 = SparklinePainter(points: p, color: Color(0xFF000001));
      const s2 = SparklinePainter(points: p, color: Color(0xFF000002));
      expect(s1.shouldRepaint(s2), isTrue);
      expect(s1.shouldRepaint(s1), isFalse);
    });
  });
}
