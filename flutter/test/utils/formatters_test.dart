// 本票新增的格式化函数（clamp / formatCost / formatCostUsd / formatDurationMs /
// formatPercent），断言与 React 版 src/utils/formatters.ts 的行为逐条对齐。
// 散点图轴与仪表盘中央读数默认走它们，跑偏就是图上显示错的数。

import 'package:aidog_flutter/utils/formatters.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  test('pad 两位补零', () {
    expect(pad(5), '05');
    expect(pad(14), '14');
  });

  test('clamp 夹逼到区间', () {
    expect(clamp(-1, 0, 1), 0);
    expect(clamp(2, 0, 1), 1);
    expect(clamp(0.5, 0, 1), 0.5);
  });

  group('formatCost（定点小数，非科学记数）', () {
    test('≤0 → "0"', () {
      expect(formatCost(0), '0');
      expect(formatCost(-1), '0');
    });
    test('≥1 → 2 位', () => expect(formatCost(12.345), '12.35'));
    test('≥0.01 → 3 位', () => expect(formatCost(0.0345), '0.035'));
    test('更小 → 保 2 位有效数字，不出科学记数', () {
      // 小数位有下限 5，所以 0.0034 会补成 "0.00340"（React 版同此）
      expect(formatCost(0.0034), '0.00340');
      expect(formatCost(0.00000045), '0.00000045');
    });
    test('极小值小数位封顶 12', () {
      expect(formatCost(1e-20).split('.')[1].length, 12);
    });
  });

  test('formatCostUsd 带 \$ 前缀', () {
    expect(formatCostUsd(1.5), '\$1.50');
  });

  group('formatDurationMs', () {
    test('<1s 用 ms（取整）', () => expect(formatDurationMs(850.4), '850 ms'));
    test('<1min 用秒（1 位小数）', () => expect(formatDurationMs(1500), '1.5 s'));
    test('其余用分钟（1 位小数）', () => expect(formatDurationMs(90000), '1.5 min'));
    test('负值按绝对值选单位', () => expect(formatDurationMs(-1500), '-1.5 s'));
  });

  group('formatPercent', () {
    test('缺省 1 位', () => expect(formatPercent(98.7), '98.7%'));
    test('指定 0 位', () => expect(formatPercent(98.7, 0), '99%'));
  });
}
