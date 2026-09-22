/// 票 28 ③ 的护栏：导入差异弹窗选中一项后，能看到**完整的**「当前 / 导入」两栏。
///
/// 改造前只有一行截断 60 字符的摘要，对象干脆只写「对象」二字。导入是覆盖操作，
/// 看不清要把什么覆盖成什么就按不下去 —— 这条测的就是「看得清」。
///
/// 差异树的**计算**由 `import_diff_test.dart` 守，这里只挂卡片测渲染。
library;

import 'package:aidog_flutter/i18n.dart';
import 'package:aidog_flutter/pages.dart';
import 'package:aidog_flutter/src/pages/settings/import_diff.dart';
import 'package:aidog_flutter/src/pages/settings/schema_config_logic.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import '../pages/harness.dart';

/// 一条超过旧截断阈值（60 字符）的值，用来证明现在不截断了。
final String longVal = 'x' * 80;

/// 一棵两叶子的差异树：一个标量改动 + 一个对象改动（对象那条是改造前最看不清的）。
PendingImportDiff pending() => PendingImportDiff(
  source: const {'model': 'opus'},
  recommended: false,
  diff: [
    DiffNode(
      path: 'model',
      label: 'model',
      current: 'sonnet',
      incoming: 'opus',
    ),
    DiffNode(
      path: 'env',
      label: 'env',
      current: {'A': '1', 'LONG': longVal},
      incoming: const {'A': '2'},
    ),
  ],
);

Future<(List<Set<String>>, I18nController)> mount(WidgetTester tester) async {
  await useBigSurface(tester);
  final applied = <Set<String>>[];
  final i18n = await makeI18n(tester);
  await tester.pumpWidget(
    wrapPage(
      ImportDiffCard(pending: pending(), onCancel: () {}, onApply: applied.add),
      i18n,
    ),
  );
  await settle(tester);
  return (applied, i18n);
}

/// 某一栏里的正文。
String paneText(WidgetTester tester, String key) =>
    tester.widget<SelectableText>(find.byKey(ValueKey(key))).data!;

void main() {
  testWidgets('选中项展开「当前 / 导入」两栏，标量值原样给全', (tester) async {
    final (_, t) = await mount(tester);

    // 默认全选 → 两条都展开。
    expect(find.text(t.t('settings.editor.diffCurrent')), findsNWidgets(2));
    expect(find.text(t.t('settings.editor.diffIncoming')), findsNWidgets(2));

    expect(paneText(tester, 'diff-current-model'), 'sonnet');
    expect(paneText(tester, 'diff-incoming-model'), 'opus');
  });

  testWidgets('对象值给完整 JSON，不再是「对象」二字，也不截断到 60 字符', (tester) async {
    await mount(tester);
    final current = paneText(tester, 'diff-current-env');
    // 缩进两格的 JSON（React `formatValue` 的 `JSON.stringify(v, null, 2)`）。
    expect(current, contains('"A": "1"'));
    // 80 字符的长值要完整留着——改造前这里会被砍到 60 字符加省略号。
    expect(current, contains(longVal));
    expect(current.contains('…'), isFalse);
    expect(paneText(tester, 'diff-incoming-env'), contains('"A": "2"'));
  });

  testWidgets('收起某项 → 它的两栏消失，只剩一行摘要', (tester) async {
    final (_, t) = await mount(tester);
    await tester.tap(find.byKey(const ValueKey('diff-model')));
    await settle(tester);

    expect(find.byKey(const ValueKey('diff-current-model')), findsNothing);
    // 另一条仍展开着。
    expect(find.byKey(const ValueKey('diff-current-env')), findsOneWidget);
    expect(find.text(t.t('settings.editor.diffCurrent')), findsOneWidget);
    // 收起那条回到一行摘要（摘要走 jsonEncode，标量带引号）。
    expect(find.text('"sonnet" → "opus"'), findsOneWidget);
  });

  testWidgets('缺值的一侧写「(无)」而不是空白', (tester) async {
    await useBigSurface(tester);
    final i18n = await makeI18n(tester);
    await tester.pumpWidget(
      wrapPage(
        ImportDiffCard(
          pending: PendingImportDiff(
            source: const {},
            recommended: false,
            diff: [
              DiffNode(
                path: 'added',
                label: 'added',
                current: null,
                incoming: 'v',
              ),
            ],
          ),
          onCancel: () {},
          onApply: (_) {},
        ),
        i18n,
      ),
    );
    await settle(tester);
    expect(
      paneText(tester, 'diff-current-added'),
      i18n.t('settings.editor.none'),
    );
    expect(paneText(tester, 'diff-incoming-added'), 'v');
  });
}
