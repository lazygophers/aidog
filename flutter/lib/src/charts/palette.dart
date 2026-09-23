/// 图表色板（对应 React 版 `src/components/charts/palette.ts`）。
///
/// React 版把色值写成 `var(--primary)` / `var(--chart-2..5)` 字符串交给 CSS 解析；
/// Flutter 没有 CSS 变量，所以这里**从主题取色**（`AidogTheme.of(context).c`），
/// 一个色值字面量都不写 —— 改色请改 `design/tokens/tokens.json`。
library;

import 'package:flutter/widgets.dart';

import '../shell/theme.dart';

/// 辅线灰阶的四级不透明度：React 版的 `--chart-2..5` 是纯灰阶四档，
/// 这里用 fg-2 的透明度阶梯表达同一语义（A′ 主题没有 chart-N token）。
const List<double> kAuxAlphas = [1.0, 0.72, 0.50, 0.34];

/// 热力色带端点：与 React 版 `HEAT_MIN_ALPHA` / `HEAT_MAX_ALPHA` 一字不差。
const double kHeatMinAlpha = 0.06;
const double kHeatMaxAlpha = 0.92;

double _clamp01(double t) => t < 0 ? 0 : (t > 1 ? 1 : t);

/// 一套模式下的系列取色。页面不自己算色，一律 `ChartPalette.of(context)`。
@immutable
class ChartPalette {
  const ChartPalette(this.c);

  factory ChartPalette.of(BuildContext context) =>
      ChartPalette(AidogTheme.of(context).c);

  final AidogColors c;

  /// 主系列色（React 版 `PRIMARY_COLOR` = `var(--primary)`）。
  ///
  /// 读 `dataPrimary` 而不是 `accent`：两者曾经同值，直到深色强调色改成近黑
  /// （`#101012`，用户 2026-09-23 定）—— 近黑折线画在 `#111214` 的卡片上是
  /// 1.01:1，**数据本身看不见**。图表是唯一不跟随强调色单色化的地方：多条线
  /// 必须靠色相分开，灰阶只够分 2~3 条（用户同日拍板）。
  Color get primary => c.dataPrimary;

  /// 第 index 个系列的颜色：0（及负数）→ 主色，1 起循环四级灰阶。
  /// 例：`series(0)` = accent，`series(1)` = fg2，`series(5)` 回绕 = `series(1)`。
  Color series(int index) {
    if (index <= 0) return primary;
    return c.fg2.withValues(alpha: kAuxAlphas[(index - 1) % kAuxAlphas.length]);
  }

  /// 前 count 个系列的颜色（批量取色）。
  List<Color> seriesColors(int count) =>
      List.generate(count, series, growable: false);

  /// 热力格子颜色：t ∈ [0,1] → 主色的 alpha 阶梯，区间外 clamp 到两端。
  /// alpha 公式与 React 版逐字对应（min + (max-min) × clamp(t)）。
  Color heat(double t) =>
      primary.withValues(alpha: heatAlpha(t));

  /// 热力格子的 alpha（暴露出来是因为测试按 alpha 断言，与 React 版同一套期望值）。
  ///
  /// 取 3 位小数：React 版落到 CSS 时写的是 `alpha.toFixed(3)`，那一步同时也吃掉了
  /// 浮点累加噪声（0.92 会算成 0.9200000000000002）。这里复刻同一个精度。
  static double heatAlpha(double t) => double.parse(
        (kHeatMinAlpha + (kHeatMaxAlpha - kHeatMinAlpha) * _clamp01(t))
            .toStringAsFixed(3),
      );
}
