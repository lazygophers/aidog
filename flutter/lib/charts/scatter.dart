// ── 散点直方图数据层（对应 React 版 src/components/charts/ScatterChart.tsx 的纯函数部分）──
// 渲染留给票 I04，这里只出点集。
import '../stats/models.dart';

/// 一个散点：x = duration bin 中心（ms），y = cost bin 中心（$），count = 该格请求数。
typedef ScatterPoint = ({double x, double y, int count});

/// 纯函数：bin 矩阵 → 散点点集（bin 中心，count == 0 的格剔除）。全空矩阵 → 空列表。
List<ScatterPoint> scatterPoints(ScatterHistogram h) {
  final pts = <ScatterPoint>[];
  for (var i = 0; i < h.counts.length; i++) {
    final row = h.counts[i];
    // 边界数组比矩阵行短（脏数据）→ 该行整行跳过，对应 JS 的 `x1 == null` 分支
    if (i + 1 >= h.durationBins.length) continue;
    final x0 = h.durationBins[i];
    final x1 = h.durationBins[i + 1];
    for (var j = 0; j < row.length; j++) {
      final count = row[j];
      if (count == 0) continue;
      if (j + 1 >= h.costBins.length) continue;
      pts.add((
        x: (x0 + x1) / 2,
        y: (h.costBins[j] + h.costBins[j + 1]) / 2,
        count: count,
      ));
    }
  }
  return pts;
}
