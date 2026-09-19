// 逐条翻自 React 版 src/components/charts/__tests__/downsample.test.ts（数据与期望值一字不改）。
import 'dart:math' as math;

import 'package:aidog_flutter/charts/downsample.dart';
import 'package:flutter_test/flutter_test.dart';

/// 可比身份的点（JS 测试用 `toBe` 断言的是对象引用，Dart 对应 `identical`，
/// 故这里用 class 而非 record —— record 是值类型，没有稳定身份）。
class XY {
  const XY(this.x, this.y);
  final double x;
  final double y;
}

double xOf(XY p) => p.x;
double yOf(XY p) => p.y;
XY xy(double x, double y) => XY(x, y);

void main() {
  group('downsampleLttb', () {
    test('keeps the 500-point threshold constant (spec §F1)', () {
      expect(lttbThreshold, 500);
    });

    test('input at or below threshold returns the same reference untouched', () {
      final pts = [xy(0, 1), xy(1, 2), xy(2, 3)];
      expect(identical(downsampleLttb(pts, xOf, yOf, 5), pts), true);
      expect(identical(downsampleLttb(pts, xOf, yOf, 3), pts), true);
    });

    test('empty input → empty output', () {
      expect(downsampleLttb(<XY>[], xOf, yOf), <XY>[]);
    });

    test('threshold < 3 cannot bucket → input returned as-is', () {
      final pts = [for (var i = 0; i < 100; i++) xy(i.toDouble(), i.toDouble())];
      expect(identical(downsampleLttb(pts, xOf, yOf, 2), pts), true);
    });

    test('output has exactly threshold points, first/last preserved, x ascending', () {
      final pts = [
        for (var i = 0; i < 1000; i++) xy(i.toDouble(), math.sin(i / 50) + 1),
      ];
      final out = downsampleLttb(pts, xOf, yOf, 100);
      expect(out, hasLength(100));
      expect(identical(out[0], pts[0]), true);
      expect(identical(out[out.length - 1], pts[999]), true);
      final xs = out.map(xOf).toList();
      for (var i = 1; i < xs.length; i++) {
        expect(xs[i], greaterThan(xs[i - 1]));
      }
    });

    test('keeps the dominant spike inside its bucket (visual fidelity)', () {
      final pts = [
        for (var i = 0; i < 1000; i++) xy(i.toDouble(), i < 500 ? 0 : 1),
      ];
      // index 499 是阶跃尖峰，LTTB 应选中而非丢失
      final out = downsampleLttb(pts, xOf, yOf, 100);
      expect(out.any((p) => identical(p, pts[499])), true);
    });

    test('bucket averaging hits the last-bucket clamp (rangeEnd capped at n)', () {
      // n=999, threshold=100：every 非整除，末段 bucket rangeEnd 被 clamp 到 n
      final pts = [
        for (var i = 0; i < 999; i++) xy(i.toDouble(), (i % 7) * 0.1),
      ];
      final out = downsampleLttb(pts, xOf, yOf, 100);
      expect(out, hasLength(100));
      expect(identical(out[99], pts[998]), true);
    });
  });
}
