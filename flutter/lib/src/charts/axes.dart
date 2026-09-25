/// 轴域：把 `charts/ticks.dart::niceTicks` 的刻度列表翻成 fl_chart 要的
/// `min / max / interval` 三元组。
///
/// 为什么要这一步：Recharts 直接收任意 `ticks` 数组，fl_chart 的 `SideTitles`
/// 只认**等距 interval**。niceTicks 本来就是等距的（1/2/5 步长），所以
/// interval = 相邻两刻度之差，信息零损失。
library;

import 'package:flutter/widgets.dart';

import '../shell/theme.dart';
import '../shell/tiles.dart';

/// 一条轴的可画参数。[ticks] 保留原列表，供测试逐值断言（与 React 版同一套期望）。
@immutable
class AxisSpec {
  const AxisSpec({
    required this.min,
    required this.max,
    required this.interval,
    required this.ticks,
  });

  final double min;
  final double max;

  /// fl_chart 的 `SideTitles.interval`，恒 > 0。
  final double interval;
  final List<double> ticks;
}

/// 刻度列表 → 轴域。
/// - 空列表（数据域含 NaN/Infinity）→ 退化到 [fallbackMin, fallbackMax]，interval = 跨度
/// - 单刻度（min == max 的退化域）→ 上下各留 0.5 的可视范围，不画出一条压在边框上的线
AxisSpec axisFromTicks(
  List<double> ticks, {
  double fallbackMin = 0,
  double fallbackMax = 1,
}) {
  if (ticks.isEmpty) {
    final span = fallbackMax - fallbackMin;
    return AxisSpec(
      min: fallbackMin,
      max: fallbackMax,
      interval: span > 0 ? span : 1,
      ticks: const [],
    );
  }
  if (ticks.length == 1) {
    final v = ticks.first;
    return AxisSpec(min: v - 0.5, max: v + 0.5, interval: 1, ticks: ticks);
  }
  return AxisSpec(
    min: ticks.first,
    max: ticks.last,
    interval: ticks[1] - ticks[0],
    ticks: ticks,
  );
}

/// 轴刻度文字样式。
///
/// React 的刻度继承 `ChartContainer` 的 `text-xs` = 12 **系统 sans** +
/// `fill-muted-foreground`（`src/components/ui/chart.tsx:67`），只是 tabular-nums；
/// 用等宽族会让整屏字体观感与 React 不同 —— 所以走 [counterStyle] 而不是 numSm。
TextStyle axisLabelStyle(AidogColors c) =>
    counterStyle(fontSize: 12, color: c.fg2, fontWeight: FontWeight.w400);
