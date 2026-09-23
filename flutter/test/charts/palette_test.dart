// 色板单测。React 版 palette.test.ts 的结构逐条翻译。
//
// 差异只在**色值怎么表达**：React 断言的是 CSS 变量串（"var(--primary)" / "var(--chart-2)"），
// Flutter 没有 CSS 变量，色值从主题取。所以断言改成「取的是主题里的哪个 token」与
// 「循环 / 回绕的次序」—— 语义一字不改。
// 热力色带那一组的 alpha 期望值（0.06 / 0.92 / 中点）与 React 版完全相同。

import 'package:aidog_flutter/charts.dart';
import 'package:aidog_flutter/shell.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  final p = ChartPalette(AidogColors.dark);

  group('series', () {
    // 主系列色读的是 `dataPrimary` 不是 `accent`：深色强调色是近黑
    // （用户 2026-09-23 定），近黑折线画在卡片上 1.01:1，数据本身看不见。
    test('index 0 与负数 → 主色', () {
      expect(p.series(0), AidogColors.dark.dataPrimary);
      expect(p.series(-3), p.primary);
    });

    test('index 1..4 按序取四级灰阶', () {
      for (var i = 1; i <= 4; i++) {
        expect(
          p.series(i),
          AidogColors.dark.fg2.withValues(alpha: kAuxAlphas[i - 1]),
          reason: 'series($i)',
        );
      }
      // 四档互不相同，否则「灰阶」名存实亡
      expect({for (var i = 1; i <= 4; i++) p.series(i)}.length, 4);
    });

    test('index 超过 4 回绕', () {
      expect(p.series(5), p.series(1));
      expect(p.series(8), p.series(4));
      expect(p.series(9), p.series(1));
    });
  });

  group('seriesColors', () {
    test('返回 count 个，映射与 series 一致', () {
      expect(p.seriesColors(6), [
        p.series(0),
        p.series(1),
        p.series(2),
        p.series(3),
        p.series(4),
        p.series(1), // 回绕
      ]);
    });

    test('count 为 0 → 空列表', () {
      expect(p.seriesColors(0), isEmpty);
    });
  });

  group('heat（alpha 期望值与 React 版逐字相同）', () {
    test('t=0 → 最低档 0.06，t=1 → 最高档 0.92', () {
      expect(ChartPalette.heatAlpha(0), kHeatMinAlpha);
      expect(ChartPalette.heatAlpha(1), kHeatMaxAlpha);
      expect(kHeatMinAlpha, 0.06);
      expect(kHeatMaxAlpha, 0.92);
    });

    test('中间值线性插值', () {
      final mid = kHeatMinAlpha + (kHeatMaxAlpha - kHeatMinAlpha) * 0.5;
      expect(ChartPalette.heatAlpha(0.5), closeTo(mid, 1e-12));
    });

    test('区间外 clamp 到两端', () {
      expect(ChartPalette.heatAlpha(-1), ChartPalette.heatAlpha(0));
      expect(ChartPalette.heatAlpha(2), ChartPalette.heatAlpha(1));
    });

    test('色相取主色，只有 alpha 在动', () {
      final c = p.heat(0.5);
      expect(c.r, p.primary.r);
      expect(c.g, p.primary.g);
      expect(c.b, p.primary.b);
      expect(c.a, closeTo(ChartPalette.heatAlpha(0.5), 1 / 255));
    });
  });

  test('浅色模式取浅色 token，不写死一套色值', () {
    final light = ChartPalette(AidogColors.light);
    expect(light.primary, AidogColors.light.dataPrimary);
    expect(light.primary, isNot(p.primary));
  });
}
