// ── 轴刻度公共层（对应 React 版 src/components/charts/ticks.ts）──
// nice-ticks + 时间轴标签 + 桶串解析，图表组件不各自实现。
// 数值补零复用 utils/formatters.dart 的 pad，禁本文件重复定义。
import 'dart:math' as math;

import '../utils/formatters.dart';

/// 1/2/5×10^n 好看步长：norm = rawStep / 10^floor(log10) 落档取整倍率。
double _niceStep(double norm, double mag) {
  final mult = norm <= 1
      ? 1
      : norm <= 2
      ? 2
      : norm <= 5
      ? 5
      : 10;
  return mult * mag;
}

/// nice-ticks：给数据域 [min, max] 生成含两端的「好看」均匀刻度（1/2/5 步长）。
/// - 刻度域覆盖整个数据域（末刻度 ≥ max：步长跨过 max 时补一档，防调用方按刻度定轴域后顶值被裁）
/// - min == max 或 max < min → 单刻度 [min]（退化域，调用方自行加 padding）
/// - 非有限值（NaN/Infinity，脏数据漏到轴上）→ []
List<double> niceTicks(num min, num max, [int tickCount = 5]) {
  if (!min.isFinite || !max.isFinite) return [];
  if (!(max > min)) return [min.toDouble()];
  final rawStep = (max - min) / math.max(2, tickCount - 1);
  // JS 的 Math.log10 在 Dart 无对应，log(x)/ln10 是同一算式。
  final mag = math.pow(10, (math.log(rawStep) / math.ln10).floor()).toDouble();
  final step = _niceStep(rawStep / mag, mag);
  final ticks = <double>[];
  // + step/2 容差吃掉浮点累加误差，保证闭区间右端点入列；
  // toStringAsPrecision(12)（= JS toPrecision(12)）去 0.30000000000000004 噪声。
  for (var v = (min / step).floor() * step; v <= max + step / 2; v += step) {
    ticks.add(double.parse(v.toStringAsPrecision(12)));
  }
  // 末刻度仍够不到 max（步长整跨数据域）→ 再补一档，保证覆盖。
  // isNotEmpty 对应 JS 里 `ticks[len-1]` 为 undefined 时比较为 false 的分支。
  if (ticks.isNotEmpty && ticks.last < max) {
    ticks.add(double.parse((ticks.last + step).toStringAsPrecision(12)));
  }
  return ticks;
}

const int _hourMs = 3600000;

final RegExp _dateOnly = RegExp(r'^\d{4}-\d{2}-\d{2}$');

/// 后端 time_bucket 串 → 本地时区 ms：
/// daily `YYYY-MM-DD` | minute/5min `YYYY-MM-DD HH:MM` | hourly `YYYY-MM-DD HH:00:00`。
/// 含时间段补 T 走本地解析；纯日期补 T00:00:00 同样按本地解析。
/// 解析失败 → NaN（对应 JS `Date.parse` 的 NaN，调用方按 `isNaN` 跳过）。
double bucketMs(String tb) {
  final s = tb.contains(' ') ? tb.replaceFirst(' ', 'T') : '${tb}T00:00:00';
  return DateTime.tryParse(s)?.millisecondsSinceEpoch.toDouble() ?? double.nan;
}

/// x 值归一为 ms 时间戳：num 原样，DateTime 取时间戳，串归一后解析（NaN 交轴自动回落）。
///
/// 注意：**纯日期串按 UTC 午夜解析**，与 [bucketMs] 的本地午夜不同 —— 这是 JS
/// `Date.parse` 的语义，React 版 ticks.test.ts 有断言固定住它。Dart 的
/// `DateTime.parse` 对纯日期串按本地解析，故这里显式补 `T00:00:00Z` 复刻。
double xNum(Object? v) {
  if (v is num) return v.toDouble();
  if (v is DateTime) return v.millisecondsSinceEpoch.toDouble();
  final s = v.toString().replaceFirst(' ', 'T');
  final iso = _dateOnly.hasMatch(s) ? '${s}T00:00:00Z' : s;
  return DateTime.tryParse(iso)?.millisecondsSinceEpoch.toDouble() ?? double.nan;
}

/// 时间轴刻度标签：按横轴总跨度（spanMs = max - min）选粒度，全部本地时区。
/// - ≤ 48h → `HH:MM`（含跨日的日内窗）
/// - ≤ 60 天 → `MM-DD`
/// - 更长 → `YYYY-MM`
/// value 无效（NaN / Infinity）→ ""。
String formatTimeTick(num value, num spanMs) {
  if (!value.isFinite) return '';
  final d = DateTime.fromMillisecondsSinceEpoch(value.toInt());
  if (spanMs <= 48 * _hourMs) return '${pad(d.hour)}:${pad(d.minute)}';
  if (spanMs <= 60 * 24 * _hourMs) return '${pad(d.month)}-${pad(d.day)}';
  return '${d.year}-${pad(d.month)}';
}
