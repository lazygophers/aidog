/// 切页不许吞掉刚改的值。
///
/// 设置页的 `TextRow` / `NumberRow` 是**失焦即提交**（`settings/bits.dart:171-173`）。
/// 点侧栏切页时输入框不会自己失焦，而是连着整页一起被销毁 —— 改动静默丢失。
/// 修法是在 `ShellController.navigate` 里先收一次焦点（`shell/nav.dart:182`）。
///
/// 这条盯的是那一行。删掉它，下面两条当场红。
library;

import 'package:aidog_flutter/i18n.dart';
import 'package:aidog_flutter/pages.dart';
import 'package:aidog_flutter/shell.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import '../pages/harness.dart';

void main() {
  setUp(resetNavGuardForTest);
  tearDown(resetNavGuardForTest);

  /// 一个带输入框的假页面，记下每次「失焦提交」交出来的值。
  Future<(ShellController, List<String>)> mount(
    WidgetTester tester,
    I18nController i18n,
  ) async {
    final nav = ShellController(initial: 'home');
    final committed = <String>[];
    await tester.pumpWidget(
      wrapPage(
        TextRow(
          label: '端口',
          value: '9890',
          onSubmitted: committed.add,
        ),
        i18n,
      ),
    );
    await settle(tester);
    return (nav, committed);
  }

  testWidgets('改了输入框没失焦就切页 → 值仍然落盘', (tester) async {
    final i18n = await makeI18n(tester);
    final (nav, committed) = await mount(tester, i18n);

    await tester.tap(find.byType(TextField));
    await settle(tester);
    await tester.enterText(find.byType(TextField), '9999');
    await settle(tester);
    expect(committed, isEmpty, reason: '还没失焦，这时不该提交');

    nav.navigate('platforms');
    await settle(tester);
    expect(committed, ['9999'], reason: '切页前必须先收焦点，让失焦回调把值交出去');
  });

  testWidgets('没动过输入框的切页不产生多余提交', (tester) async {
    final i18n = await makeI18n(tester);
    final (nav, committed) = await mount(tester, i18n);

    // 压根没聚焦过 → primaryFocus 不是这个框，unfocus 不该凭空触发一次提交。
    nav.navigate('platforms');
    await settle(tester);
    expect(committed, isEmpty);
  });
}
