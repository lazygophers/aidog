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

/// 成功率百分比（0–100）。对应 `formatters.ts::successRate`：总数 ≤ 0 → 0。
double successRate(num successCount, num totalRequests) =>
    totalRequests <= 0 ? 0 : (successCount / totalRequests) * 100;
