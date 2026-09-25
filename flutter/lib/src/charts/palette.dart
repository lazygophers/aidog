/// 图表色板（对应 React 版 `src/components/charts/palette.ts`）。
///
/// React 版把主色写成 `var(--primary)` 交给 CSS 解析；Flutter 没有 CSS 变量，
/// 主色从主题取（`AidogTheme.of(context).c`）。辅线灰阶与热力琥珀在 React 侧
/// 是明暗同值的固定色（globals.css / palette.ts），这里直接写死常量，不跟主题。
library;

import 'package:flutter/widgets.dart';

import '../shell/theme.dart';

/// 辅线灰阶四档 = React 的 `--chart-2..5`（globals.css:106-109,811-814），
/// 原值 oklch(0.556/0.439/0.371/0.269 0 0)，明暗两模式同值。
/// oklch→sRGB 换算（2026-09-25 批次一）：零色度 → a=b=0 → 线性值恰为 L³，
/// gamma 编码 1.055·c^(1/2.4)−0.055 后 ×255 四舍五入：
///   0.556³=0.17188 → 0.4515 → 115 → #737373
///   0.439³=0.08460 → 0.3220 →  82 → #525252
///   0.371³=0.05106 → 0.2505 →  64 → #404040
///   0.269³=0.01947 → 0.1493 →  38 → #262626
/// （恰为 Tailwind neutral-500..800，互为佐证。）
const List<Color> kAuxColors = [
  Color(0xFF737373),
  Color(0xFF525252),
  Color(0xFF404040),
  Color(0xFF262626),
];

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
    return kAuxColors[(index - 1) % kAuxColors.length];
  }

  /// 前 count 个系列的颜色（批量取色）。
  List<Color> seriesColors(int count) =>
      List.generate(count, series, growable: false);

  /// 热力色带色源：固定琥珀 `#e8c547`（React `palette.ts:33` 的 HEAT_RGB），不跟主题。
  static const Color kHeatAmber = Color(0xFFE8C547);

  /// 热力格子颜色：t ∈ [0,1] → 琥珀的 alpha 阶梯，区间外 clamp 到两端。
  /// alpha 公式与 React 版逐字对应（min + (max-min) × clamp(t)）。
  Color heat(double t) => kHeatAmber.withValues(alpha: heatAlpha(t));

  /// 热力格子的 alpha（暴露出来是因为测试按 alpha 断言，与 React 版同一套期望值）。
  ///
  /// 取 3 位小数：React 版落到 CSS 时写的是 `alpha.toFixed(3)`，那一步同时也吃掉了
  /// 浮点累加噪声（0.92 会算成 0.9200000000000002）。这里复刻同一个精度。
  static double heatAlpha(double t) => double.parse(
    (kHeatMinAlpha + (kHeatMaxAlpha - kHeatMinAlpha) * _clamp01(t))
        .toStringAsFixed(3),
  );
}
