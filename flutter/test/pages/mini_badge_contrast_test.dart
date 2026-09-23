import 'package:aidog_flutter/pages.dart';
import 'package:aidog_flutter/shell.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

/// 实心徽标在深色下必须读得出来：近黑的 accent 当底色时，文字不能也用近黑。
void main() {
  testWidgets('实心 MiniBadge 的文字与底色对比 ≥ 4.5:1（深色近黑 accent）', (tester) async {
    await tester.pumpWidget(
      MaterialApp(
        theme: aidogThemeData(AidogMode.dark),
        home: Builder(
          builder: (context) => Scaffold(
            body: MiniBadge(
              text: '默认',
              color: AidogTheme.of(context).c.accent,
              solid: true,
            ),
          ),
        ),
      ),
    );
    final fill = AidogColors.dark.accent;
    final txt = tester
        .widget<Text>(find.byType(Text).first)
        .style!
        .color!;
    double lum(Color c) => c.computeLuminance();
    final hi = lum(fill) > lum(txt) ? lum(fill) : lum(txt);
    final lo = lum(fill) > lum(txt) ? lum(txt) : lum(fill);
    expect((hi + 0.05) / (lo + 0.05), greaterThan(4.5));
  });
}
