/// 颜色对比度（WCAG 2.x）。**判「看不看得见」用算的，不靠眼估。**
///
/// 2026-09-23 深色强调色改成近黑 `#101012` 之后，一批「一看就有」的东西其实
/// 已经不可见了（实心徽标近黑字压近黑底 1.05:1、选中边对表面 1.01:1、开关开态
/// 对关态 1.06:1）。这些都不是靠观感能发现的，得有个地方算。
///
/// 放在 `utils/` 而不是某个页面文件里：生产代码（`MiniBadge` 的 `needsEdge`）与
/// 护栏测试（`test/accent_contrast_test.dart`）用的必须是**同一份**实现，
/// 否则两边会各自漂。
library;

import 'dart:ui' show Color;

/// 两色对比度，范围 1.0（同色）~ 21.0（纯黑对纯白）。
///
/// 常用判据（WCAG 2.2）：
/// - 正文文字 ≥ 4.5:1（1.4.3）
/// - 大字 / 图形 / 界面控件边界 ≥ 3:1（1.4.11）
///
/// 半透明色**不能直接传进来**：WCAG 只对实色定义，先用 [compositeOver] 把它
/// 合成到实际压着的底色上再算。
double contrastRatio(Color a, Color b) {
  final la = a.computeLuminance();
  final lb = b.computeLuminance();
  final hi = la > lb ? la : lb;
  final lo = la > lb ? lb : la;
  return (hi + 0.05) / (lo + 0.05);
}

/// 把半透明的 [fg] 按 src-over 合成到不透明的 [bg] 上，得到实际看到的颜色。
///
/// `accent-wash`（白 6%）、`accent-edge`（白 34%）这类 token 都是半透明的，
/// 直接拿去算对比度得到的是「它自己有多亮」，不是「它压在那块底上有多亮」。
Color compositeOver(Color fg, Color bg) {
  final a = fg.a;
  double mix(double f, double b) => f * a + b * (1 - a);
  return Color.from(
    alpha: 1,
    red: mix(fg.r, bg.r),
    green: mix(fg.g, bg.g),
    blue: mix(fg.b, bg.b),
  );
}
