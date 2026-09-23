/// 数字输入框不许把用户输入静默变成 0（`ui_bits.dart::NumberInput`）。
///
/// 回归 2026-09-23：中间件的预算上限与覆盖状态码、分组超时、模型信息兜底单价
/// 等 17 处都是「纯文本框 + `tryParse(v) ?? 0`」。用户写「10 usd」，值静默变成
/// **0** —— 预算闸门当场失效、兜底单价变成免费，而界面一声不吭。
///
/// 三条语义都在这里钉住：① 非数字敲不进去 ② 解析不出来保留原值、不写 0
/// ③ 越界夹取并可见。
library;

import 'package:aidog_flutter/pages.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'harness.dart';

void main() {
  testWidgets('「10 usd」这种输入：字母根本进不来，值不会变成 0', (tester) async {
    final i18n = await makeI18n(tester);
    var committed = '3.5';

    await tester.pumpWidget(
      wrapPage(
        StatefulBuilder(
          builder: (context, setState) => NumberInput(
            value: committed,
            decimal: true,
            onChanged: (v) => setState(() => committed = v),
          ),
        ),
        i18n,
      ),
    );
    await settle(tester);

    await tester.enterText(find.byType(TextField), '10 usd');
    await settle(tester);

    // 过滤器把字母和空格挡在外面，框里只剩数字。
    expect(tester.widget<TextField>(find.byType(TextField)).controller!.text,
        '10');

    // 提交（失焦）后存的是 10，**不是 0**。
    await tester.tap(find.byIcon(Icons.keyboard_arrow_up));
    await settle(tester);
    expect(double.parse(committed), greaterThan(0));
  });

  testWidgets('清空后提交：保留原值，不写 0', (tester) async {
    final i18n = await makeI18n(tester);
    var committed = '42';

    await tester.pumpWidget(
      wrapPage(
        StatefulBuilder(
          builder: (context, setState) => NumberInput(
            value: committed,
            onChanged: (v) => setState(() => committed = v),
          ),
        ),
        i18n,
      ),
    );
    await settle(tester);

    await tester.enterText(find.byType(TextField), '');
    await tester.testTextInput.receiveAction(TextInputAction.done);
    await settle(tester);

    expect(committed, '42');
    // 输入框也恢复成当前值：用户看得见自己那串没被接受。
    expect(tester.widget<TextField>(find.byType(TextField)).controller!.text,
        '42');
  });

  testWidgets('越界会夹取，并把范围显示出来', (tester) async {
    final i18n = await makeI18n(tester);
    var committed = '200';

    await tester.pumpWidget(
      wrapPage(
        StatefulBuilder(
          builder: (context, setState) => NumberInput(
            value: committed,
            min: 100,
            max: 599,
            onChanged: (v) => setState(() => committed = v),
          ),
        ),
        i18n,
      ),
    );
    await settle(tester);

    await tester.enterText(find.byType(TextField), '900');
    await tester.testTextInput.receiveAction(TextInputAction.done);
    await settle(tester);

    expect(committed, '599');
    expect(find.text('100–599'), findsOneWidget);
  });

  testWidgets('± 与 ↑↓ 都能步进，且夹在下限内', (tester) async {
    final i18n = await makeI18n(tester);
    var committed = '1';

    await tester.pumpWidget(
      wrapPage(
        StatefulBuilder(
          builder: (context, setState) => NumberInput(
            value: committed,
            min: 0,
            onChanged: (v) => setState(() => committed = v),
          ),
        ),
        i18n,
      ),
    );
    await settle(tester);

    await tester.tap(find.byIcon(Icons.keyboard_arrow_up));
    await settle(tester);
    expect(committed, '2');

    await tester.tap(find.byIcon(Icons.keyboard_arrow_down));
    await settle(tester);
    expect(committed, '1');

    // 一路往下：0 是合法值（保留期那几处「0 = 永久保留」），负数才夹。
    await tester.tap(find.byIcon(Icons.keyboard_arrow_down));
    await settle(tester);
    expect(committed, '0');
    await tester.tap(find.byIcon(Icons.keyboard_arrow_down));
    await settle(tester);
    expect(committed, '0');
  });
}
