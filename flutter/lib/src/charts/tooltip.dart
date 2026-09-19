/// 图表 tooltip 层（对应 React 版 `src/components/charts/tooltip.tsx`）。
///
/// ## 为什么这一层要先于任何一张图存在
///
/// React 那边一个 `tooltipValueRows` 服务四张图；`fl_chart` 不给这个位置 ——
/// 每种图各有一套 `XxxTouchTooltipData`，回调拿到的是**位置索引**
/// （`barIndex` / `spotIndex`），要还原「这是哪条序列」得靠调用方自己按位置去查。
///
/// 而 Stats 的序列顺序是**按总量动态排**的（`src/pages/Stats.tsx:114`
/// `ordered = [...series].sort((a,b) => reqSum(b.buckets) - reqSum(a.buckets))`）：
/// 同一个 `s0` 今天是「深度求索」，明天可能是「智谱」。位置索引 + 动态排序 =
/// 格式化静默走偏，显示一个**错的数**而不是报错。commit `a7e665c8` 修的就是这个形状。
///
/// ## 这一层怎么让它不可能再发生
///
/// **身份、展示名、颜色、格式化函数全挂在同一个 [ChartSeries] 对象上。**
/// 没有第二份按位置对齐的平行数组可以漂移 —— 序列重排时这四样一起动。
/// [tooltipRowAt] 只做一件事：`series[barIndex]` 取出那个对象，四样全从它身上读。
///
/// 越界不静默：`barIndex` 超出范围直接 [RangeError]，不回落到别的序列。
library;

import 'package:flutter/widgets.dart';

/// 一条序列：**身份（[key]）与它的展示名、颜色、格式化函数绑死在一起**。
///
/// [key] 是稳定身份（Stats 的 `s0` / `s1`，或业务键 `cost` / `tokens`），
/// 不是位置。构造图表时的列表顺序可以随便变，行内容跟着对象走。
@immutable
class ChartSeries {
  const ChartSeries({
    required this.key,
    required this.label,
    required this.color,
    required this.points,
    required this.format,
    this.dashed = false,
    this.rightAxis = false,
  });

  /// 稳定身份。同一张图内必须唯一（[assertUniqueKeys] 会拦）。
  final String key;

  /// 展示名（tooltip / 图例）。React 版的 `ChartConfig.label`。
  final String label;

  final Color color;

  /// 数据点，按 x 升序（LTTB 的前提）。
  final List<ChartPoint> points;

  /// **本序列自己的**数值格式化。双轴时左右轴各带各的，
  /// 不再有「按 name 查右轴集合」那一步 —— 那一步正是 `a7e665c8` 出错的地方。
  final String Function(double) format;

  /// 虚线渲染（React 版的 `dashedKeys`）。
  final bool dashed;

  /// 挂右轴独立标尺（React 版的 `rightConfig`）。
  final bool rightAxis;

  /// 本序列 y 值之和（堆叠 / 排序用）。
  double get total => points.fold(0, (s, p) => s + p.y);
}

/// 一个数据点。x 一般是毫秒时间戳（走 `charts/ticks.dart::xNum` 归一）。
@immutable
class ChartPoint {
  const ChartPoint(this.x, this.y);
  final double x;
  final double y;

  @override
  bool operator ==(Object other) =>
      other is ChartPoint && other.x == x && other.y == y;

  @override
  int get hashCode => Object.hash(x, y);

  @override
  String toString() => 'ChartPoint($x, $y)';
}

/// tooltip 的一行（纯数据模型，可脱离 widget 单测 —— 这是钉死序列身份的地方）。
@immutable
class TooltipRow {
  const TooltipRow({
    required this.key,
    required this.label,
    required this.color,
    required this.value,
  });

  /// 这一行属于哪条序列。断言写在它上面，不写在位置上。
  final String key;
  final String label;
  final Color color;

  /// 已格式化好的数值串（来自 [ChartSeries.format]，即**本序列自己的**格式化）。
  final String value;

  @override
  bool operator ==(Object other) =>
      other is TooltipRow &&
      other.key == key &&
      other.label == label &&
      other.color == color &&
      other.value == value;

  @override
  int get hashCode => Object.hash(key, label, color, value);

  @override
  String toString() => 'TooltipRow($key: $label = $value)';
}

/// 同一张图内序列 key 必须唯一：重复 key 会让「按 key 找行」变得有歧义。
/// 图表构造时调一次，release 下被 assert 抹掉（开发期拦住就够）。
void assertUniqueKeys(List<ChartSeries> series) {
  assert(() {
    final seen = <String>{};
    for (final s in series) {
      if (!seen.add(s.key)) {
        throw ArgumentError('图表序列 key 重复：${s.key}');
      }
    }
    return true;
  }());
}

/// fl_chart 的位置索引 → tooltip 行。
///
/// 四样（key / label / color / format）全从 `series[barIndex]` **同一个对象**上读，
/// 所以重排序列列表时它们一起动，不可能出现「名字对了、数字用了别人的格式」。
/// 越界抛 [RangeError]，绝不回落到相邻序列。
TooltipRow tooltipRowAt(List<ChartSeries> series, int barIndex, double value) {
  RangeError.checkValidIndex(barIndex, series, 'barIndex', series.length);
  final s = series[barIndex];
  return TooltipRow(
    key: s.key,
    label: s.label,
    color: s.color,
    value: s.format(value),
  );
}

/// 按 key 取行（调用方 / 测试要定位某条序列时用这个，**别记位置**）。
/// key 不存在 → [ArgumentError]（静默返 null 就等于把错误推到显示层）。
TooltipRow tooltipRowFor(List<ChartSeries> series, String key, double value) {
  final i = series.indexWhere((s) => s.key == key);
  if (i < 0) throw ArgumentError('序列 key 不存在：$key');
  return tooltipRowAt(series, i, value);
}

/// 按位置索引取行，**数值从序列自己的点集里读**，不信任绘图坐标。
///
/// 双轴图把右轴序列归一化进左轴坐标系后画（fl_chart 没有第二标尺），
/// 绘图 y 已经不是原值；照着触点的 y 显示就是显示一个换算过的假数。
/// 这里回到 `series[barIndex].points[spotIndex].y` 取原值。
TooltipRow tooltipRowAtSpot(
  List<ChartSeries> series,
  int barIndex,
  int spotIndex,
) {
  RangeError.checkValidIndex(barIndex, series, 'barIndex', series.length);
  final s = series[barIndex];
  RangeError.checkValidIndex(spotIndex, s.points, 'spotIndex', s.points.length);
  return tooltipRowAt(series, barIndex, s.points[spotIndex].y);
}

/// 色点字符：`fl_chart` 的 tooltip 只收文本模型（`LineTooltipItem` 等），
/// 塞不进任意 widget，所以色块用一个着色的方块字符表达。
const String kTooltipSwatch = '■';
