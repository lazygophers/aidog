// 逐条翻自 React 版 src/components/charts/__tests__/ScatterChart.test.tsx 的 scatterPoints 段
// （数据与期望值一字不改；ScatterChart 渲染断言属票 I04）。
import 'package:aidog_flutter/charts/scatter.dart';
import 'package:aidog_flutter/stats/models.dart';
import 'package:flutter_test/flutter_test.dart';

// 3×3 网格（duration 0-3000ms step1000，cost 0-0.06 step0.02；counts 行=duration bin 列=cost bin）
const hist = ScatterHistogram(
  durationBins: [0, 1000, 2000, 3000],
  costBins: [0, 0.02, 0.04, 0.06],
  counts: [
    [10, 0, 5],
    [0, 3, 0],
    [0, 0, 1],
  ],
);

void main() {
  group('scatterPoints', () {
    test('非零格 → bin 中心点，零格剔除', () {
      expect(scatterPoints(hist), [
        (x: 500.0, y: 0.01, count: 10),
        (x: 500.0, y: 0.05, count: 5),
        (x: 1500.0, y: 0.03, count: 3),
        (x: 2500.0, y: 0.05, count: 1),
      ]);
    });
    test('全空矩阵（空窗口）→ 空数组不抛', () {
      expect(
        scatterPoints(
          const ScatterHistogram(durationBins: [], costBins: [], counts: []),
        ),
        <ScatterPoint>[],
      );
      expect(
        scatterPoints(
          const ScatterHistogram(
            durationBins: [0, 1000],
            costBins: [0, 1],
            counts: [
              [0],
            ],
          ),
        ),
        <ScatterPoint>[],
      );
    });
  });
}
