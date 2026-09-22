// 序列公共层：对齐降采样、域计算、堆叠累加、双轴映射、轴域翻译。
//
// React 那边这些逻辑散在各组件的 useMemo 里，断言藏在组件测试的「Y 轴顶刻度 ≥ 7」
// 一类间接观察中（StackedAreaChart.test.tsx:45-54）。这里直接测函数，期望值与那些
// 间接断言同源。

import 'package:aidog_flutter/charts.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';

const _black = Color(0xFF000000);

ChartSeries mk(String key, List<double> ys, {double x0 = 0, double step = 1}) =>
    ChartSeries(
      key: key,
      label: key,
      color: _black,
      points: [
        for (var i = 0; i < ys.length; i++) ChartPoint(x0 + i * step, ys[i]),
      ],
      format: (n) => '$n',
    );

void main() {
  group('downsampleAligned', () {
    test('点数 ≤ 阈值 → 原列表原样返回（同一实例）', () {
      final s = [mk('a', List.filled(500, 1))];
      expect(identical(downsampleAligned(s), s), isTrue);
    });

    test('超阈值 → 降到阈值，各序列共用同一批下标', () {
      final ys = [for (var i = 0; i < 600; i++) (i % 7) + 0.5];
      final zs = [for (var i = 0; i < 600; i++) i * 10.0];
      final out = downsampleAligned([mk('cost', ys), mk('tokens', zs)]);
      expect(out.length, 2);
      expect(out[0].points.length, lttbThreshold);
      expect(out[1].points.length, lttbThreshold);
      // 同一批下标 ⇒ 两条序列的 x 序列完全一致（堆叠不会错位）
      expect(
        out[0].points.map((p) => p.x).toList(),
        out[1].points.map((p) => p.x).toList(),
      );
      // 首尾必留（LTTB 约定）
      expect(out[0].points.first.x, 0);
      expect(out[0].points.last.x, 599);
    });

    test('取形函数换成行总量 → 选点跟着栈顶轮廓走', () {
      final a = mk('a', [for (var i = 0; i < 600; i++) 1.0]);
      final b = mk('b', [for (var i = 0; i < 600; i++) i == 300 ? 999.0 : 0.0]);
      final byTotal = downsampleAligned([a, b], yOf: rowTotalOf([a, b]));
      // 总量取形会把那根尖峰留下；只看 a（恒 1）则完全看不见它
      expect(byTotal[1].points.any((p) => p.y == 999), isTrue);
    });

    test('序列点数不一致 → 抛，不静默补零', () {
      expect(
        () => downsampleAligned([mk('a', [1, 2]), mk('b', [1])]),
        throwsArgumentError,
      );
    });

    test('空序列列表原样返回', () {
      expect(downsampleAligned(const []), isEmpty);
    });
  });

  group('域', () {
    test('xDomain 取全部序列的 x 极值；全空 → null', () {
      expect(xDomain([mk('a', [1, 2, 3])]), (min: 0.0, max: 2.0));
      expect(xDomain(const []), isNull);
      expect(xDomain([mk('a', const [])]), isNull);
    });

    test('yDomain 从 0 起步（无负值时），有负值才下探', () {
      expect(yDomain([mk('a', [3, 7])]), (min: 0.0, max: 7.0));
      expect(yDomain([mk('a', [-3, 7])]), (min: -3.0, max: 7.0));
    });

    test('stackedYDomain 看堆叠总量，不看单序列最大值', () {
      // React StackedAreaChart.test.tsx:45 的那组数据：claude=(i%5)+1，glm=(i%3)+1
      final claude = mk('claude', [for (var i = 0; i < 14; i++) (i % 5) + 1.0]);
      final glm = mk('glm', [for (var i = 0; i < 14; i++) (i % 3) + 1.0]);
      // 单序列 max = 5，堆叠总量 max = 7
      expect(yDomain([claude]).max, 5);
      expect(stackedYDomain([claude, glm]).max, 7);
      // React 那条断言是「Y 顶刻度 ≥ 7」，这里同样成立
      expect(axisFromTicks(niceTicks(0, 7, 4)).max, greaterThanOrEqualTo(7));
    });

    test('stackedYDomain 空序列 → (0, 0)', () {
      expect(stackedYDomain(const []), (min: 0.0, max: 0.0));
      expect(stackedYDomain([mk('a', const [])]), (min: 0.0, max: 0.0));
    });
  });

  group('stackTop', () {
    test('第 i 层的栈顶 = 第 0..i 层之和', () {
      final s = [mk('a', [1, 2]), mk('b', [10, 20]), mk('c', [100, 200])];
      expect(stackTop(s, 0, 0), 1);
      expect(stackTop(s, 1, 0), 11);
      expect(stackTop(s, 2, 0), 111);
      expect(stackTop(s, 2, 1), 222);
    });
  });

  group('axisFromTicks', () {
    test('等距刻度 → min/max/interval', () {
      final a = axisFromTicks(const [0, 2, 4, 6]);
      expect((a.min, a.max, a.interval), (0.0, 6.0, 2.0));
    });

    test('空刻度（数据域含 NaN）→ 退化区间，interval 恒 > 0', () {
      final a = axisFromTicks(const []);
      expect((a.min, a.max), (0.0, 1.0));
      expect(a.interval, greaterThan(0));
    });

    test('单刻度（min == max 的退化域）→ 上下各留 0.5', () {
      final a = axisFromTicks(const [5]);
      expect((a.min, a.max, a.interval), (4.5, 5.5, 1.0));
    });
  });

  group('mapToLeft（fl_chart 无第二 Y 轴，右轴靠线性映射）', () {
    final right = axisFromTicks(const [0, 0.5, 1]);
    final left = axisFromTicks(const [0, 50, 100]);

    test('端点对端点，中点对中点', () {
      expect(mapToLeft(0, right, left), 0);
      expect(mapToLeft(1, right, left), 100);
      expect(mapToLeft(0.5, right, left), 50);
    });

    test('反向映射能还原（右轴刻度标原值靠这条）', () {
      expect(mapToLeft(mapToLeft(0.25, right, left), left, right),
          closeTo(0.25, 1e-12));
    });

    test('轴域退化 → 原样返回，不除零', () {
      final flat = axisFromTicks(const []);
      expect(mapToLeft(3, flat, flat), 3);
    });
  });

  test('legendOf 顺序 = 序列顺序', () {
    final s = [mk('a', [1]), mk('b', [1])];
    expect(legendOf(s).map((l) => l.label).toList(), ['a', 'b']);
  });

  // 第二梯队 2026-09-22：缺值原先一律填 0，「那个时段没有这个平台的数据」被画成
  // 「那个时段是 0 次请求」，走势看起来与 React 不同
  //（recharts 默认 connectNulls={false}，`LineChart.tsx:201`）。
  group('缺值（ChartPoint.missing）', () {
    test('不参与取值域：全缺的一条不会把上界拉到 0', () {
      final s = ChartSeries(
        key: 'a',
        label: 'a',
        color: _black,
        format: (v) => '$v',
        points: [
          const ChartPoint(0, 5),
          const ChartPoint.missing(1),
          const ChartPoint(2, 9),
        ],
      );
      final d = yDomain([s]);
      expect(d.max, 9);
      expect(d.min, 0);
    });

    test('堆叠累加跳过缺值，不把它当 0 之外的东西', () {
      final a = ChartSeries(
        key: 'a',
        label: 'a',
        color: _black,
        format: (v) => '$v',
        points: [const ChartPoint(0, 3), const ChartPoint.missing(1)],
      );
      final b = ChartSeries(
        key: 'b',
        label: 'b',
        color: _black,
        format: (v) => '$v',
        points: [const ChartPoint(0, 4), const ChartPoint(1, 2)],
      );
      final total = rowTotalOf([a, b]);
      expect(total(0), 7);
      expect(total(1), 2);
    });

    test('缺值点与 y=0 的点不相等 —— 两者语义不同', () {
      expect(const ChartPoint.missing(1) == const ChartPoint(1, 0), isFalse);
    });
  });

  group('降采样提示', () {
    test('点数没超阈值 → null（不提示）', () {
      expect(downsampledPointCount([mk('a', List.filled(10, 1))]), isNull);
    });

    test('点数超阈值 → 返回降采样后还剩几个点', () {
      final n = downsampledPointCount([
        mk('a', List.generate(lttbThreshold * 2, (i) => i.toDouble())),
      ]);
      expect(n, isNotNull);
      expect(n, lessThanOrEqualTo(lttbThreshold));
    });
  });
}
