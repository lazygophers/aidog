/// 序列的公共操作：对齐降采样、域计算、图例。
///
/// React 版的图表收「宽表 + config」（一行一个 x，列是各系列），降采样在行上做，
/// 所以各系列天然共用同一批 x。Dart 这边序列各带各的点集，要保持同一语义就得
/// **按同一批下标取点**（[downsampleAligned]）—— 各系列分别 LTTB 会让堆叠错位。
library;

import 'dart:math' as math;

import 'package:flutter/painting.dart';

import '../../charts/downsample.dart';
import 'tooltip.dart';

/// 对齐降采样：按 [yOf] 给出的「取形序列」跑一次 LTTB，选中的**下标**套用到所有序列。
///
/// - 点数 ≤ [lttbThreshold] → 原列表原样返回（同一实例，与 I05 的 `downsampleLttb` 同约定）
/// - [yOf] 缺省 = 第一条序列的 y（对齐 React `LineChart` 的 `seriesKeys[0]`）；
///   堆叠图传行总量（对齐 React `StackedAreaChart` 的 `rowTotal`）
/// - 各序列点数必须一致（宽表语义），不一致直接 [ArgumentError]，不静默补零
/// 降采样后还剩几个点；没触发降采样返回 null。
///
/// 图表内部照常自己跑 [downsampleAligned]，这里只是给调用方一个「要不要提示」
/// 的答案 —— React 把这句写在图的副标题里（`LineChart.tsx:111-115`），
/// 不说的话用户看到的是抽样曲线而不自知。
int? downsampledPointCount(
  List<ChartSeries> series, {
  double Function(int rowIndex)? yOf,
  int threshold = lttbThreshold,
}) {
  final rows = downsampleAligned(series, yOf: yOf, threshold: threshold);
  if (identical(rows, series) || rows.isEmpty) return null;
  return rows.first.points.length;
}

List<ChartSeries> downsampleAligned(
  List<ChartSeries> series, {
  double Function(int rowIndex)? yOf,
  int threshold = lttbThreshold,
}) {
  if (series.isEmpty) return series;
  final n = series.first.points.length;
  for (final s in series) {
    if (s.points.length != n) {
      throw ArgumentError(
        '序列 ${s.key} 有 ${s.points.length} 个点，与首条序列的 $n 个不一致'
        '（宽表语义要求各系列共用同一批 x）',
      );
    }
  }
  if (n <= threshold) return series;

  final xs = series.first.points;
  final y = yOf ?? (i) => xs[i].y;
  final kept = downsampleLttb<int>(
    List<int>.generate(n, (i) => i, growable: false),
    (i) => xs[i].x,
    y,
    threshold,
  );
  if (kept.length == n) return series;
  return [
    for (final s in series)
      ChartSeries(
        key: s.key,
        label: s.label,
        color: s.color,
        points: [for (final i in kept) s.points[i]],
        format: s.format,
        dashed: s.dashed,
        rightAxis: s.rightAxis,
      ),
  ];
}

/// 行总量取形函数（堆叠图用）：第 i 行各系列 y 之和。
double Function(int) rowTotalOf(List<ChartSeries> series) =>
    (i) => series.fold<double>(
      0,
      (s, e) => e.points[i].missing ? s : s + e.points[i].y,
    );

/// 横轴数据域。空序列 / 全空点集 → null（调用方走空态，不画假轴）。
({double min, double max})? xDomain(List<ChartSeries> series) {
  var lo = double.infinity;
  var hi = double.negativeInfinity;
  for (final s in series) {
    for (final p in s.points) {
      if (!p.x.isFinite) continue;
      lo = math.min(lo, p.x);
      hi = math.max(hi, p.x);
    }
  }
  return lo.isFinite ? (min: lo, max: hi) : null;
}

/// 纵轴数据域（含 React 版的 `Math.min(0, ...)` 起步：有负值才下探，否则从 0 起）。
({double min, double max}) yDomain(List<ChartSeries> series) {
  var lo = 0.0;
  var hi = double.negativeInfinity;
  for (final s in series) {
    for (final p in s.points) {
      // 缺值不参与取值域：它不是 0，只是没有数据。
      if (p.missing || !p.y.isFinite) continue;
      lo = math.min(lo, p.y);
      hi = math.max(hi, p.y);
    }
  }
  return (min: lo, max: hi.isFinite ? hi : 0.0);
}

/// 堆叠纵轴域：0 起步到行总量最大值（React `niceTicks(0, max(...totals, 0))`）。
({double min, double max}) stackedYDomain(List<ChartSeries> series) {
  if (series.isEmpty || series.first.points.isEmpty) return (min: 0, max: 0);
  final total = rowTotalOf(series);
  var hi = 0.0;
  for (var i = 0; i < series.first.points.length; i++) {
    hi = math.max(hi, total(i));
  }
  return (min: 0, max: hi);
}

/// 图例项（喂 `SeriesTile.legend`）。顺序 = 序列顺序 = 堆叠顺序。
List<({Color color, String label})> legendOf(List<ChartSeries> series) =>
    [for (final s in series) (color: s.color, label: s.label)];
