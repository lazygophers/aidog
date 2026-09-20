// ── 数值/时间格式化公共层（对应 React 版 src/utils/formatters.ts）──
// 其余格式化函数（formatNumber / formatCostUsd / formatDurationMs …）由页面票按需补进本文件，
// 禁在页面内重复定义（CLAUDE.md「数值格式化统一走 utils/formatters」）。

import 'dart:math' as math;

/// 两位补零：`pad(5) == "05"`。对应 `formatters.ts::pad`。
String pad(int n) => n.toString().padLeft(2, '0');

/// 夹逼到 [min, max]。对应 `formatters.ts::clamp`。
double clamp(double value, double min, double max) =>
    math.min(max, math.max(min, value));

/// 成本格式化（定点小数，非科学记数）。对应 `formatters.ts::formatCost`：
/// ≤0 → "0"；≥1 → 2 位；≥0.01 → 3 位；更小按「保 2 位有效数字」算小数位，下限 5 上限 12。
String formatCost(double n) {
  if (!(n > 0)) return '0';
  if (n >= 1) return n.toStringAsFixed(2);
  if (n >= 0.01) return n.toStringAsFixed(3);
  final decimals = math.min(
    12,
    math.max(5, -(math.log(n) / math.ln10).floor() + 1),
  );
  return n.toStringAsFixed(decimals);
}

/// `$` + [formatCost]。对应 `formatters.ts::formatCostUsd`。
String formatCostUsd(double n) => '\$${formatCost(n)}';

/// 延迟格式化（ms → 人读单位）。对应 `formatters.ts::formatDurationMs`：
/// <1s 用 ms（取整），<1min 用秒（1 位小数），其余用分钟（1 位小数）。
String formatDurationMs(double ms) {
  final abs = ms.abs();
  if (abs < 1000) return '${ms.round()} ms';
  if (abs < 60000) return '${(ms / 1000).toStringAsFixed(1)} s';
  return '${(ms / 60000).toStringAsFixed(1)} min';
}

/// 百分比格式化：值已是 0–100 的百分数，保留 [digits] 位（默认 1）。
/// 对应 `formatters.ts::formatPercent`。
String formatPercent(double n, [int digits = 1]) =>
    '${n.toStringAsFixed(digits)}%';

/// 大数缩写。对应 `formatters.ts::formatNumber`：
/// ≥1e6 → `x.xM`；≥1e3 → `x.xK`；否则整数不带小数、非整数保 1 位。
String formatNumber(num n) {
  if (n >= 1000000) return '${(n / 1000000).toStringAsFixed(1)}M';
  if (n >= 1000) return '${(n / 1000).toStringAsFixed(1)}K';
  return n.toStringAsFixed(n % 1 == 0 ? 0 : 1);
}

/// 字节数。对应 `formatters.ts::formatBytes`（票 I16 补，设置页的日志清理预估用）：
/// 非正数 → `0 B`；B 档四舍五入取整，其余保 1 位小数。
String formatBytes(num n) {
  if (!(n > 0)) return '0 B';
  const units = ['B', 'KB', 'MB', 'GB', 'TB'];
  var v = n.toDouble();
  var i = 0;
  while (v >= 1024 && i < units.length - 1) {
    v /= 1024;
    i += 1;
  }
  return '${i == 0 ? v.round() : v.toStringAsFixed(1)} ${units[i]}';
}

/// 成功率百分比（0–100）。对应 `formatters.ts::successRate`：总数 ≤ 0 → 0。
double successRate(num successCount, num totalRequests) =>
    totalRequests <= 0 ? 0 : (successCount / totalRequests) * 100;

/// 毫秒戳 → 可读的本地时刻。对应 `formatters.ts::formatDateTime`（票 I07 补）。
/// 0 / 负数 = 没有时间，返回空串（React 那边这种输入返回 `null`，调用处一律当假值用）。
///
/// **与 React 有一处形态差异**：那边是 `Date.toLocaleString()`，跟浏览器 locale 走，
/// 中文环境出「2026/9/20 14:03:05」、英文环境出「9/20/2026, 2:03:05 PM」。
/// 这里固定成 `YYYY/M/D HH:MM:SS` 一种。要跟 locale 走得先
/// `initializeDateFormatting()`（`intl` 包），漏调会在非英文 locale 抛
/// `LocaleDataException` —— 与 I06 不用 `DateFormat.E` 画星期标签是同一个理由。
String formatDateTime(int msSinceEpoch) {
  if (msSinceEpoch <= 0) return '';
  final d = DateTime.fromMillisecondsSinceEpoch(msSinceEpoch);
  return '${d.year}/${d.month}/${d.day} '
      '${pad(d.hour)}:${pad(d.minute)}:${pad(d.second)}';
}
