// 八张在用图表的 widget 测试。React 侧 __tests__ 里的断言逐条翻译，**数据与期望值不改**
// （React 断言 DOM 节点数 / CSS 串，这里断言 fl_chart 的数据模型 / painter 参数 —— 同一件事）。
//
// 没迁的三张（BarChart / PieChart / StackedBarChart，共 276 行）在 React 侧零页面调用者，
// 所以这里也没有它们的测试。

import 'package:aidog_flutter/charts.dart';
import 'package:aidog_flutter/shell.dart';
import 'package:aidog_flutter/stats/models.dart';
import 'package:fl_chart/fl_chart.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

/// 最小宿主：注入 AidogTheme（色板从主题取，没有它 ChartPalette.of 会 assert）。
Widget host(Widget child, {AidogMode mode = AidogMode.dark}) => MaterialApp(
      theme: aidogThemeData(mode),
      home: Scaffold(body: SizedBox(width: 600, height: 300, child: child)),
    );

const _noData = 'charts.noData';

// React LineChart.test.tsx 的数据：T0 起每小时一点，cost=(i%7)+0.5，tokens=i*10
final t0 = DateTime.parse('2026-09-13T00:00:00').millisecondsSinceEpoch;
const hour = 3600000;
const day = 86400000;

List<ChartPoint> pts(int n, double Function(int) y, {int step = hour}) =>
    [for (var i = 0; i < n; i++) ChartPoint((t0 + i * step).toDouble(), y(i))];

ChartSeries series(
  String key,
  List<ChartPoint> points, {
  Color color = const Color(0xFFAA0000),
  String Function(double)? format,
  bool rightAxis = false,
  bool dashed = false,
}) =>
    ChartSeries(
      key: key,
      label: key,
      color: color,
      points: points,
      format: format ?? ((n) => '$n'),
      rightAxis: rightAxis,
      dashed: dashed,
    );

LineChartData lineData(WidgetTester tester) =>
    tester.widget<LineChart>(find.byType(LineChart)).data;

void main() {
  group('AidogLineChart', () {
    final cfg = [
      series('cost', pts(24, (i) => (i % 7) + 0.5)),
      series('tokens', pts(24, (i) => i * 10.0)),
    ];

    testWidgets('正常数据画出两条线与刻度', (tester) async {
      await tester.pumpWidget(host(AidogLineChart(series: cfg)));
      final d = lineData(tester);
      // React: .recharts-line-curve 两条
      expect(d.lineBarsData.length, 2);
      expect(d.lineBarsData[0].spots.length, 24);
      // React: 刻度文本 > 0 —— 这里断言轴刻度被打开且 interval 有效
      expect(d.titlesData.leftTitles.sideTitles.showTitles, isTrue);
      expect(d.titlesData.leftTitles.sideTitles.interval, greaterThan(0));
      expect(d.titlesData.bottomTitles.sideTitles.interval, greaterThan(0));
    });

    testWidgets('空数据 → 诚实空态，不画零值假图', (tester) async {
      await tester.pumpWidget(
        host(const AidogLineChart(series: [], emptyText: _noData)),
      );
      expect(find.text(_noData), findsOneWidget);
      expect(find.byType(LineChart), findsNothing);
    });

    testWidgets('超 500 点降采样到 500', (tester) async {
      await tester.pumpWidget(host(AidogLineChart(series: [
        series('cost', pts(600, (i) => (i % 7) + 0.5)),
        series('tokens', pts(600, (i) => i * 10.0)),
      ])));
      // React: 600 点 → LTTB 降到 500，副题出现降采样提示
      expect(lineData(tester).lineBarsData[0].spots.length, lttbThreshold);
    });

    testWidgets('Y 轴刻度走本序列的格式化', (tester) async {
      await tester.pumpWidget(host(AidogLineChart(series: [
        series('cost', pts(12, (i) => i.toDouble()),
            format: (n) => '\$${n.toStringAsFixed(2)}'),
      ])));
      // React: 刻度文本里出现 "$"
      expect(find.textContaining('\$'), findsWidgets);
    });

    testWidgets('mini 裸渲染：无网格、无刻度、无点', (tester) async {
      await tester.pumpWidget(host(AidogLineChart(series: cfg, mini: true)));
      final d = lineData(tester);
      expect(d.lineBarsData.length, 2);
      expect(d.gridData.show, isFalse);
      expect(d.titlesData.show, isFalse);
      expect(d.lineBarsData[0].dotData.show, isFalse);
    });

    testWidgets('双轴：右轴序列映射进左轴坐标系，主线下方有面积，辅线虚线', (tester) async {
      final req = series('req', pts(24, (i) => i * 10.0));
      final cost = series(
        'cost',
        pts(24, (i) => (i % 7) + 0.5),
        rightAxis: true,
        dashed: true,
        format: (n) => '\$${n.toStringAsFixed(2)}',
      );
      await tester.pumpWidget(
        host(AidogLineChart(series: [req, cost], area: true)),
      );
      final d = lineData(tester);
      // React: 左右系列各一条折线
      expect(d.lineBarsData.length, 2);
      // React: 主系列下方面积填充一处
      expect(d.lineBarsData.where((b) => b.belowBarData.show).length, 1);
      expect(d.lineBarsData[0].belowBarData.show, isTrue);
      // React 的 dasharray 断言在 jsdom 下不可行，Flutter 这边直接能看
      expect(d.lineBarsData[1].dashArray, const [3, 3]);
      // 右轴刻度打开（React: x + 左右两条 Y 轴 = 3 条轴）
      expect(d.titlesData.rightTitles.sideTitles.showTitles, isTrue);
      // 右轴值被映射进左轴域：cost 原值 ≤ 6.5，画出来的 y 落在 req 的量纲上
      expect(d.lineBarsData[1].spots.map((s) => s.y).reduce((a, b) => a > b ? a : b),
          greaterThan(10));
      // 但 tooltip 取的是原值（不是映射后的假数）
      expect(tooltipRowAtSpot([req, cost], 1, 0).value, '\$0.50');
    });

    testWidgets('tooltip 回调按位置索引取行，取到的是那条序列自己的东西', (tester) async {
      await tester.pumpWidget(host(AidogLineChart(series: cfg)));
      final d = lineData(tester);
      final items = d.lineTouchData.touchTooltipData.getTooltipItems([
        LineBarSpot(d.lineBarsData[1], 1, d.lineBarsData[1].spots[3]),
      ]);
      expect(items.single!.children!.any((s) => s.text!.contains('tokens')), isTrue);
    });
  });

  group('AidogStackedAreaChart', () {
    // React StackedAreaChart.test.tsx 的数据：每天一点，claude=(i%5)+1，glm=(i%3)+1
    List<ChartSeries> cfg(int n) => [
          series('claude', pts(n, (i) => (i % 5) + 1.0, step: day)),
          series('glm', pts(n, (i) => (i % 3) + 1.0, step: day)),
        ];

    testWidgets('每条序列一层，同一个栈', (tester) async {
      await tester.pumpWidget(
        host(AidogStackedAreaChart(series: cfg(14))),
      );
      final d = lineData(tester);
      // React: .recharts-area 两层
      expect(d.lineBarsData.length, 2);
      expect(d.lineBarsData.every((b) => b.belowBarData.show), isTrue);
    });

    testWidgets('空数据 → 诚实空态', (tester) async {
      await tester.pumpWidget(
        host(const AidogStackedAreaChart(series: [], emptyText: _noData)),
      );
      expect(find.text(_noData), findsOneWidget);
    });

    testWidgets('Y 轴按堆叠总量定，不按单序列最大值', (tester) async {
      await tester.pumpWidget(
        host(AidogStackedAreaChart(series: cfg(14), tickCount: 4)),
      );
      // React: 单序列 max=5，堆叠 max=7，Y 顶刻度须 ≥ 7
      expect(lineData(tester).maxY, greaterThanOrEqualTo(7));
    });

    testWidgets('超 500 点降采样', (tester) async {
      await tester.pumpWidget(
        host(AidogStackedAreaChart(series: cfg(600))),
      );
      expect(lineData(tester).lineBarsData[0].spots.length, lttbThreshold);
    });

    testWidgets('栈顶先画（顺序反转），tooltip 仍报各层自己的原值', (tester) async {
      final s = cfg(14);
      await tester.pumpWidget(host(AidogStackedAreaChart(series: s)));
      final d = lineData(tester);
      // 0 号 bar 是栈顶（glm + claude 的累加），末号是底层（claude 自己）
      expect(d.lineBarsData[0].spots[0].y, stackTop(s, 1, 0));
      expect(d.lineBarsData[1].spots[0].y, stackTop(s, 0, 0));
      // 绘图 y 是累加值，tooltip 要给该层自己的值
      final drawn = s.reversed.toList();
      expect(tooltipRowAtSpot(drawn, 0, 0).key, 'glm');
      expect(tooltipRowAtSpot(drawn, 0, 0).value, '${s[1].points[0].y}');
    });
  });

  group('AidogDonutChart', () {
    // React DonutChart.test.tsx 的 entries(n)：p1..pn，value = n-i（降序）
    List<({String name, double value})> entries(int n) =>
        [for (var i = 0; i < n; i++) (name: 'p${i + 1}', value: (n - i).toDouble())];

    test('topN 之外合并成一块（纯函数，与 React 的 slices useMemo 同规则）', () {
      final p = ChartPalette(AidogColors.dark);
      final s = donutSlices(entries(6), p, topN: 4, restLabel: 'stats.donutRest');
      expect(s.map((e) => e.name).toList(),
          ['p1', 'p2', 'p3', 'p4', 'stats.donutRest']);
      // 其余 p5(2) + p6(1) = 3
      expect(s.last.value, 3);
      // React: 首位主色，其后灰阶
      expect(s[0].color, p.series(0));
      expect(s[1].color, p.series(1));
      expect(s[2].color, p.series(2));
      // 百分比按总量算（6+5+4+3+2+1 = 21）
      expect(s[0].percent, closeTo(6 / 21 * 100, 1e-9));
    });

    test('value ≤ 0 的项被剔除', () {
      final s = donutSlices(
        [(name: 'a', value: 5), (name: 'b', value: 0), (name: 'c', value: -1)],
        ChartPalette(AidogColors.dark),
        restLabel: 'rest',
      );
      expect(s.map((e) => e.name).toList(), ['a']);
    });

    testWidgets('中央总值与说明字', (tester) async {
      await tester.pumpWidget(host(AidogDonutChart(
        data: entries(3),
        restLabel: 'rest',
        formatValue: (n) => '\$${n.toStringAsFixed(2)}',
        centerLabel: 'total',
      )));
      // React: 3+2+1 = 6
      expect(find.text('\$6.00'), findsOneWidget);
      expect(find.text('total'), findsOneWidget);
      expect(find.text('p1'), findsOneWidget);
    });

    testWidgets('零 / 单块有效扇区 → 诚实空态', (tester) async {
      await tester.pumpWidget(host(AidogDonutChart(
        data: const [],
        restLabel: 'rest',
        formatValue: (n) => '$n',
        emptyText: _noData,
      )));
      expect(find.text(_noData), findsOneWidget);

      await tester.pumpWidget(host(AidogDonutChart(
        data: const [(name: 'only', value: 5)],
        restLabel: 'rest',
        formatValue: (n) => '$n',
        emptyText: _noData,
      )));
      expect(find.text(_noData), findsOneWidget);
    });

    testWidgets('mini 不足两块时什么都不画（不是空态卡）', (tester) async {
      await tester.pumpWidget(host(AidogDonutChart(
        mini: true,
        data: const [(name: 'only', value: 5)],
        restLabel: 'rest',
        formatValue: (n) => '$n',
        emptyText: _noData,
      )));
      expect(find.text(_noData), findsNothing);
      expect(find.byType(PieChart), findsNothing);
    });

    testWidgets('showLegend=false 收掉侧列', (tester) async {
      await tester.pumpWidget(host(AidogDonutChart(
        data: entries(3),
        restLabel: 'rest',
        formatValue: (n) => '$n',
        showLegend: false,
      )));
      expect(find.text('p1'), findsNothing);
      expect(find.byType(PieChart), findsOneWidget);
    });
  });

  group('AidogScatterChart', () {
    // React ScatterChart.test.tsx 的 3×3 网格，一字不改
    const hist = ScatterHistogram(
      durationBins: [0, 1000, 2000, 3000],
      costBins: [0, 0.02, 0.04, 0.06],
      counts: [
        [10, 0, 5],
        [0, 3, 0],
        [0, 0, 1],
      ],
    );

    testWidgets('4 个非零 bin → 4 个散点，大小随 count', (tester) async {
      await tester.pumpWidget(host(const AidogScatterChart(
        histogram: hist,
        xLabel: 'charts.scatterX',
        yLabel: 'charts.scatterY',
        countLabel: _count,
      )));
      final d = tester.widget<ScatterChart>(find.byType(ScatterChart)).data;
      expect(d.scatterSpots.length, 4);
      final r = [
        for (final s in d.scatterSpots)
          (s.dotPainter as FlDotCirclePainter).radius
      ];
      // count 10 最大 → 半径最大；count 1 最小 → 半径落在下限
      expect(r[0], greaterThan(r[3]));
      expect(r[3], closeTo(scatterRadius(1, 10), 1e-12));
      // 轴标签（React: charts.scatterX / charts.scatterY 两个 key）
      expect(find.text('charts.scatterX'), findsOneWidget);
      expect(find.text('charts.scatterY'), findsOneWidget);
    });

    testWidgets('空矩阵 → 诚实空态', (tester) async {
      await tester.pumpWidget(host(const AidogScatterChart(
        histogram: ScatterHistogram(
          durationBins: [],
          costBins: [],
          counts: [],
        ),
        xLabel: 'x',
        yLabel: 'y',
        countLabel: _count,
        emptyText: _noData,
      )));
      expect(find.text(_noData), findsOneWidget);
    });

    test('scatterRadius：单一计数不放大成满格', () {
      expect(scatterRadius(1, 1), closeTo(scatterRadius(1, 10), 1e-12));
      expect(scatterRadius(10, 10), greaterThan(scatterRadius(1, 10)));
    });
  });

  group('GaugeChart', () {
    testWidgets('中央百分比 + 格式化值 + 说明字', (tester) async {
      await tester.pumpWidget(host(GaugeChart(
        value: 30,
        max: 120,
        formatValue: (n) => '${n.toInt()}h',
        label: 'glm',
      )));
      // React: 25%（整数位）/ 30h / glm
      expect(find.text('25%'), findsOneWidget);
      expect(find.text('30h'), findsOneWidget);
      expect(find.text('glm'), findsOneWidget);
    });

    // 第三梯队 2026-09-22：平台名原先塞在环心里当说明字，React 那边是
    // 画在环**上方**的标题（`Stats.tsx:853-860` 传的就是平台名）。
    testWidgets('title 画在环上方，与环心的 label 各是各的', (tester) async {
      await tester.pumpWidget(host(GaugeChart(
        value: 30,
        max: 120,
        formatValue: (n) => '${n.toInt()}h',
        title: 'GLM 平台',
        label: 'glm',
      )));
      expect(find.text('GLM 平台'), findsOneWidget);
      expect(find.text('glm'), findsOneWidget);
      // 标题在百分比上方。
      expect(
        tester.getTopLeft(find.text('GLM 平台')).dy,
        lessThan(tester.getTopLeft(find.text('25%')).dy),
      );
    });

    testWidgets('没给 title 就不占位', (tester) async {
      await tester.pumpWidget(host(GaugeChart(
        value: 30,
        max: 120,
        formatValue: (n) => '${n.toInt()}h',
      )));
      expect(find.text('25%'), findsOneWidget);
    });

    testWidgets('max ≤ 0 → 诚实空态', (tester) async {
      await tester.pumpWidget(host(GaugeChart(
        value: 5,
        max: 0,
        formatValue: (n) => '$n',
        emptyText: _noData,
      )));
      expect(find.text(_noData), findsOneWidget);
    });

    testWidgets('占比映到弧角并 clamp 溢出', (tester) async {
      GaugeRingPainter ring() => tester
          .widgetList<CustomPaint>(find.byType(CustomPaint))
          .map((w) => w.painter)
          .whereType<GaugeRingPainter>()
          .single;

      await tester.pumpWidget(host(GaugeChart(
        value: 50,
        max: 100,
        formatValue: (n) => '$n',
      )));
      // React: 0.5 → 180deg
      expect(ring().fraction, 0.5);

      await tester.pumpWidget(host(GaugeChart(
        value: 999,
        max: 100,
        formatValue: (n) => '$n',
      )));
      // React: 满配额 → 360deg
      expect(ring().fraction, 1.0);
    });

    testWidgets('趋势 ≥2 点画线，单点画点，无 trend 不画', (tester) async {
      SparklinePainter? spark() => tester
          .widgetList<CustomPaint>(find.byType(CustomPaint))
          .map((w) => w.painter)
          .whereType<SparklinePainter>()
          .firstOrNull;

      await tester.pumpWidget(host(GaugeChart(
        value: 3,
        max: 10,
        formatValue: (n) => '$n',
        trend: const [
          GaugeTrendPoint(at: 0, fraction: 1),
          GaugeTrendPoint(at: 60, fraction: 0.3),
        ],
      )));
      expect(spark()!.points.length, 2);

      await tester.pumpWidget(host(GaugeChart(
        value: 3,
        max: 10,
        formatValue: (n) => '$n',
        trend: const [GaugeTrendPoint(at: 0, fraction: 0.3)],
      )));
      expect(spark()!.points.length, 1);

      await tester.pumpWidget(
        host(GaugeChart(value: 3, max: 10, formatValue: (n) => '$n')),
      );
      expect(spark(), isNull);
    });

    test('sparkline 坐标：x 按 at 归一，y 上 = 1 下 = 0；单点不除零', () {
      const size = Size(100, kSparklineHeight);
      const p = SparklinePainter(
        points: [
          GaugeTrendPoint(at: 0, fraction: 1),
          GaugeTrendPoint(at: 60, fraction: 0),
        ],
        color: Color(0xFF000000),
      );
      expect(p.at(0, size).dx, kSparklinePad);
      expect(p.at(1, size).dx, 100 - kSparklinePad);
      expect(p.at(0, size).dy, kSparklinePad); // fraction 1 在顶
      expect(p.at(1, size).dy, kSparklineHeight - kSparklinePad);

      const one = SparklinePainter(
        points: [GaugeTrendPoint(at: 5, fraction: 0.5)],
        color: Color(0xFF000000),
      );
      expect(one.at(0, size).dx.isFinite, isTrue);
    });
  });

  group('HourHeatmap', () {
    String dayLabel(int d) => 'D$d';

    testWidgets('24×7 = 168 格，顶部刻度 00/06/12/18', (tester) async {
      await tester.pumpWidget(host(HourHeatmap(
        data: const [(day: 1, hour: 10, value: 3.0)],
        dayLabel: dayLabel,
      )));
      expect(find.byType(HeatCell), findsNWidgets(168));
      for (final h in ['00', '06', '12', '18']) {
        expect(find.text(h), findsOneWidget);
      }
    });

    testWidgets('空数据 → 诚实空态', (tester) async {
      await tester.pumpWidget(host(HourHeatmap(
        data: const [],
        dayLabel: dayLabel,
        emptyText: _noData,
      )));
      expect(find.text(_noData), findsOneWidget);
    });

    testWidgets('热度映到色带，满值打到顶', (tester) async {
      await tester.pumpWidget(host(HourHeatmap(
        data: const [
          (day: 2, hour: 9, value: 1.0),
          (day: 2, hour: 14, value: 10.0),
        ],
        dayLabel: dayLabel,
      )));
      final cells = tester.widgetList<HeatCell>(find.byType(HeatCell)).toList();
      // 行序周一…周日 → day 2 在第 2 行（0-based 1），每行 24 格
      final cold = cells[1 * 24 + 9];
      final hot = cells[1 * 24 + 14];
      expect(cold.t, lessThan(hot.t));
      // React: 满值格 alpha ≈ 0.92
      expect(ChartPalette.heatAlpha(hot.t), closeTo(0.92, 0.01));
    });

    testWidgets('全零 → 回落色带最低档 0.06', (tester) async {
      await tester.pumpWidget(host(HourHeatmap(
        data: const [(day: 0, hour: 0, value: 0.0)],
        dayLabel: dayLabel,
      )));
      final cells = tester.widgetList<HeatCell>(find.byType(HeatCell)).toList();
      expect(ChartPalette.heatAlpha(cells.first.t), 0.06);
    });
  });

  group('HourHeatBar', () {
    testWidgets('恒 24 格，缺省小时按 0 落最低档', (tester) async {
      await tester.pumpWidget(
        host(const HourHeatBar(data: [(hour: 9, value: 5.0)])),
      );
      final cells = tester.widgetList<HeatCell>(find.byType(HeatCell)).toList();
      expect(cells.length, 24);
      expect(ChartPalette.heatAlpha(cells[0].t), 0.06);
    });

    testWidgets('热度映到色带，最大格打到顶', (tester) async {
      await tester.pumpWidget(host(const HourHeatBar(
        data: [(hour: 3, value: 1.0), (hour: 20, value: 10.0)],
      )));
      final cells = tester.widgetList<HeatCell>(find.byType(HeatCell)).toList();
      expect(cells[3].t, lessThan(cells[20].t));
      expect(ChartPalette.heatAlpha(cells[20].t), closeTo(0.92, 0.01));
    });

    testWidgets('格子提示走 formatValue', (tester) async {
      await tester.pumpWidget(host(HourHeatBar(
        data: const [(hour: 14, value: 1234.0)],
        formatValue: (n) => '${n.toInt()}r',
        semanticLabel: 'heat',
      )));
      final cells = tester.widgetList<HeatCell>(find.byType(HeatCell)).toList();
      // React: "14:00 · 1234r"
      expect(cells[14].tooltip, '14:00 · 1234r');
    });

    testWidgets('越界小时被忽略而不是崩', (tester) async {
      await tester.pumpWidget(host(const HourHeatBar(
        data: [(hour: 25, value: 9.0), (hour: -1, value: 9.0)],
      )));
      expect(find.byType(HeatCell), findsNWidgets(24));
    });
  });

  group('DimensionHeatmap', () {
    // React DimensionHeatmap.test.tsx 的 d(i)
    final day0 = DateTime.parse('2026-09-10T00:00:00').millisecondsSinceEpoch;
    double d(int i) => (day0 + i * day).toDouble();

    testWidgets('行 × 列网格 + 列刻度', (tester) async {
      await tester.pumpWidget(host(DimensionHeatmap(
        data: [
          (name: 'claude', day: d(0), value: 1.0),
          (name: 'claude', day: d(1), value: 2.0),
          (name: 'glm', day: d(0), value: 3.0),
        ],
      )));
      // React: 2 维度行 × 2 日列 = 4 格
      expect(find.byType(HeatCell), findsNWidgets(4));
      expect(find.text('claude'), findsOneWidget);
      expect(find.text('glm'), findsOneWidget);
      expect(find.text('09-10'), findsOneWidget);
      expect(find.text('09-11'), findsOneWidget);
    });

    testWidgets('空数据 → 诚实空态', (tester) async {
      await tester.pumpWidget(
        host(const DimensionHeatmap(data: [], emptyText: _noData)),
      );
      expect(find.text(_noData), findsOneWidget);
    });

    testWidgets('热度映到色带，零格回落最低档', (tester) async {
      await tester.pumpWidget(host(DimensionHeatmap(
        data: [
          (name: 'claude', day: d(0), value: 1.0),
          (name: 'claude', day: d(1), value: 10.0),
          (name: 'glm', day: d(0), value: 0.0),
          (name: 'glm', day: d(1), value: 0.0),
        ],
      )));
      final cells = tester.widgetList<HeatCell>(find.byType(HeatCell)).toList();
      expect(cells[0].t, lessThan(cells[1].t));
      expect(ChartPalette.heatAlpha(cells[1].t), closeTo(0.92, 0.01));
      expect(ChartPalette.heatAlpha(cells[3].t), closeTo(0.06, 0.001));
    });

    testWidgets('自定义 formatDay / formatValue 落进格子提示', (tester) async {
      await tester.pumpWidget(host(DimensionHeatmap(
        data: [(name: 'claude', day: d(0), value: 7.0)],
        formatDay: (ms) =>
            'day-${DateTime.fromMillisecondsSinceEpoch(ms.toInt()).day}',
        formatValue: (n) => '${n.toInt()} req',
      )));
      final cell = tester.widget<HeatCell>(find.byType(HeatCell));
      // React: "claude day-10 · 7 req"
      expect(cell.tooltip, 'claude day-10 · 7 req');
    });
  });
}

String _count(int n) => 'charts.scatterCount:$n';
