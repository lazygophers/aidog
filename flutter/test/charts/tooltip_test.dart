// tooltip 层单测。**本文件的第一等目的是钉死序列身份**：
//
// fl_chart 的 tooltip 回调只给位置索引（barIndex），而 Stats 的序列顺序按总量动态排
// （src/pages/Stats.tsx:114）。位置 + 动态排序 = 格式化静默走偏，显示一个错的数还不报错
// —— commit a7e665c8 刚修掉的就是这个形状。下面「按总量重排后」那一组就是复发闸。
//
// React 版 tooltip.test.tsx 的三条断言（默认显示原始 name / labelOf 映射 / fmt 第二参
// 拿到原始 dataKey）逐条翻在下面，数据与期望值一字不改。

import 'package:aidog_flutter/charts.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';

ChartSeries s(
  String key,
  String label,
  String Function(double) format, {
  List<ChartPoint> points = const [ChartPoint(0, 1), ChartPoint(1, 2)],
  Color color = const Color(0xFF000000),
}) =>
    ChartSeries(
      key: key,
      label: label,
      color: color,
      points: points,
      format: format,
    );

void main() {
  group('tooltipValueRows 的三条（React tooltip.test.tsx 逐条翻译）', () {
    test('默认显示原始 name（dataKey）', () {
      // React: tooltipValueRows((n) => `$${n}`)(12.5, "s0") → 含 "s0" 与 "$12.5"
      final row = tooltipRowAt([s('s0', 's0', (n) => '\$$n')], 0, 12.5);
      expect(row.label, 's0');
      expect(row.value, '\$12.5');
    });

    test('label 映射 dataKey → 展示名（安全键不外露）', () {
      // React: labelOf 把 s0 映成「深度求索」，且 tooltip 里不再出现 s0
      final row = tooltipRowAt(
        [s('s0', '深度求索', (n) => '$n'), s('s1', '智谱', (n) => '$n')],
        0,
        3,
      );
      expect(row.label, '深度求索');
      expect(row.value, '3.0');
      expect(row.key, 's0'); // 身份仍是 s0，只是不拿它当展示名
    });

    test('格式化拿的是本序列自己的（双轴按序列分派）', () {
      // React: rightKeys = {"cost"}，fmt 按 name 分派 → tooltipValueRows(fmt)(8,"cost") = "$8"
      // Dart：格式化直接挂在序列上，没有「按 name 查右轴集合」这一步。
      final series = [
        s('req', 'req', (n) => '${n.toInt()}'),
        ChartSeries(
          key: 'cost',
          label: 'cost',
          color: const Color(0xFF000000),
          points: const [ChartPoint(0, 8)],
          format: (n) => '\$${n.toInt()}',
          rightAxis: true,
        ),
      ];
      expect(tooltipRowFor(series, 'cost', 8).value, '\$8');
    });
  });

  group('序列身份不随位置漂移（a7e665c8 复发闸）', () {
    // 两条序列，展示名与格式化都不同 —— 串行的话一眼看得出来。
    List<ChartSeries> build() => [
          ChartSeries(
            key: 's0',
            label: '深度求索',
            color: const Color(0xFFAA0000),
            points: const [ChartPoint(0, 8)],
            format: (n) => '${n.toInt()} 次',
            rightAxis: false,
          ),
          ChartSeries(
            key: 's1',
            label: '智谱',
            color: const Color(0xFF00BB00),
            points: const [ChartPoint(0, 8)],
            format: (n) => '\$${n.toStringAsFixed(2)}',
            rightAxis: true,
          ),
        ];

    test('按总量重排后，每条序列的名字 / 颜色 / 格式化仍是它自己的', () {
      final before = build();
      // Stats 的 `[...series].sort((a,b) => reqSum(b) - reqSum(a))` 那一步：顺序整个反过来。
      final after = before.reversed.toList();

      // 位置变了：s1 从 1 号位变到 0 号位。
      expect(before[0].key, 's0');
      expect(after[0].key, 's1');

      // 行内容没变：四样（key/label/color/format）跟着对象一起搬。
      final fromBefore = tooltipRowAt(before, 1, 8);
      final fromAfter = tooltipRowAt(after, 0, 8);
      expect(fromAfter, fromBefore);
      expect(fromAfter.label, '智谱');
      expect(fromAfter.value, '\$8.00'); // 不是 s0 的「8 次」
      expect(fromAfter.color, const Color(0xFF00BB00));
    });

    test('按 key 取行与顺序无关', () {
      final before = build();
      final after = before.reversed.toList();
      for (final k in ['s0', 's1']) {
        expect(tooltipRowFor(after, k, 8), tooltipRowFor(before, k, 8));
      }
    });

    test('越界抛 RangeError，不回落到相邻序列', () {
      final series = build();
      expect(() => tooltipRowAt(series, 2, 1), throwsRangeError);
      expect(() => tooltipRowAt(series, -1, 1), throwsRangeError);
      expect(() => tooltipRowAtSpot(series, 0, 5), throwsRangeError);
    });

    test('key 不存在抛 ArgumentError，不静默返 null', () {
      expect(() => tooltipRowFor(build(), 's9', 1), throwsArgumentError);
    });

    test('重复 key 在构图时就被拦住', () {
      expect(
        () => assertUniqueKeys([s('a', 'A', (n) => ''), s('a', 'B', (n) => '')]),
        throwsArgumentError,
      );
    });
  });

  group('tooltipRowAtSpot 取原值', () {
    test('数值来自序列自己的点集，不是传进来的绘图坐标', () {
      // 双轴图里绘图 y 被归一化过；这条保证 tooltip 显示的是原值。
      final series = [
        s(
          'cost',
          'cost',
          (n) => n.toStringAsFixed(3),
          points: const [ChartPoint(0, 0.125), ChartPoint(1, 9.5)],
        ),
      ];
      expect(tooltipRowAtSpot(series, 0, 0).value, '0.125');
      expect(tooltipRowAtSpot(series, 0, 1).value, '9.500');
    });
  });
}
