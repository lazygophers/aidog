import 'package:aidog_flutter/src/utils/contrast.dart';
import 'package:aidog_flutter/shell.dart' show AidogColors;
import 'package:flutter_test/flutter_test.dart';

/// accent 系三个 token 的**可用性**护栏。
///
/// 背景：`accent.dark` 是近黑 `#101012`（用户 2026-09-23 定），它对窗口底只有
/// 1.05:1、对格子表面 1.01:1 —— 当填充色可以，当文字色或描边色就是不可见。
/// 所以强调被拆成三个 token：`accent` 只做填充，`accent-text` 做文字，
/// `accent-edge` 做描边。这里守的是后两个**真的够亮**，改 token 时别退回去。
///
/// 判据是 WCAG 2.1：
/// - 正文 4.5:1（1.4.3 AA）→ `accent-text`
/// - 界面控件与图形 3:1（1.4.11 AA）→ `accent-edge`
void main() {
  test('accentText 当文字用，两种模式对底与表面都过 4.5:1', () {
    for (final (name, c) in [
      ('dark', AidogColors.dark),
      ('light', AidogColors.light),
    ]) {
      expect(
        contrastRatio(c.accentText, c.bg),
        greaterThanOrEqualTo(4.5),
        reason: '$name: accentText 压在窗口底上要能读',
      );
      expect(
        contrastRatio(c.accentText, c.surface),
        greaterThanOrEqualTo(4.5),
        reason: '$name: accentText 压在格子表面上要能读',
      );
    }
  });

  test('accentEdge 当描边用，两种模式对底与表面都过 3:1', () {
    for (final (name, c) in [
      ('dark', AidogColors.dark),
      ('light', AidogColors.light),
    ]) {
      // 半透明的边要先合成到它压着的底色上再判 —— WCAG 只对实色定义。
      for (final (baseName, base) in [('bg', c.bg), ('surface', c.surface)]) {
        expect(
          contrastRatio(compositeOver(c.accentEdge, base), base),
          greaterThanOrEqualTo(3.0),
          reason: '$name: accentEdge 画在 $baseName 上要划得出轮廓',
        );
      }
      // 实心按钮的边是画在 accent 填充上的，那一面也要过。
      expect(
        contrastRatio(compositeOver(c.accentEdge, c.accent), c.bg),
        greaterThanOrEqualTo(3.0),
        reason: '$name: 实心控件的边对窗口底要划得出轮廓',
      );
    }
  });

  test('accent 只当填充：深色下它对底不足 3:1，所以不许再拿它画字或边', () {
    // 这条不是「要求它暗」，是把「为什么需要另外两个 token」钉在测试里。
    //
    // 🔴 哪天 accent 被改回一个够亮的颜色，这条会红。**那时该做的是删掉这条断言
    // 并回退描边层（accent-edge 那一整套），不是改数字让它变绿** —— 数字变绿只会
    // 留下一层没人记得为什么存在的描边，下一个人照着它继续加层。
    expect(
      contrastRatio(AidogColors.dark.accent, AidogColors.dark.bg),
      lessThan(3.0),
    );
  });

  test('accentWash 比 hover 亮：选中不能比鼠标划过还不显眼', () {
    final c = AidogColors.dark;
    expect(
      compositeOver(c.accentWash, c.surface).computeLuminance(),
      greaterThan(c.surface2.computeLuminance()),
    );
  });
}
