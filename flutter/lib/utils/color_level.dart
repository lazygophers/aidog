/// 色编码分级（对应 React 版 `src/components/shared/colorScale.ts`）。
///
/// 阈值逐字照抄，**色值只来自主题** —— React 版返回 `var(--color-success)` 这类 CSS
/// 变量名，A′ 主题没有 CSS 变量，所以这里返回分级本身，取色走 [levelColor]。
///
/// A′ token 表没有 `warning` 这一档（只有 `ok` / `peak` / `bad` / `fg3`），
/// warning 映到 `peak`（琥珀，语义同「注意」），neutral 映到 `fg3`（不强行着色）。
library;

import 'package:flutter/widgets.dart';

import '../src/shell/theme.dart';

enum ColorLevel { success, warning, danger, neutral }

/// 分级 → 主题色。
Color levelColor(ColorLevel level, AidogColors c) => switch (level) {
  ColorLevel.success => c.ok,
  ColorLevel.warning => c.peak,
  ColorLevel.danger => c.bad,
  ColorLevel.neutral => c.fg3,
};

/// 成功率（0–100）→ 分级。`>= 99` success / `>= 95` warning / 其余 danger；
/// 总数 ≤ 0 时 neutral（无数据不强行着色）。
ColorLevel successRateLevel(double rate, [num totalRequests = 1]) {
  if (totalRequests <= 0) return ColorLevel.neutral;
  if (rate >= 99) return ColorLevel.success;
  if (rate >= 95) return ColorLevel.warning;
  return ColorLevel.danger;
}

/// 成本（$）→ 分级。`<= 0` neutral / `< warnAt` success / `< dangerAt` warning / 其余 danger。
ColorLevel costLevel(double cost, [double warnAt = 1, double dangerAt = 10]) {
  if (cost <= 0) return ColorLevel.neutral;
  if (cost < warnAt) return ColorLevel.success;
  if (cost < dangerAt) return ColorLevel.warning;
  return ColorLevel.danger;
}
