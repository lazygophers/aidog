/// 票 28 ②：中间件那几处输入要能随内容长高，对齐 React 的 `AutoTextarea`
/// （`MiddlewareRules.tsx:70`）。
///
/// 改造前是单行 `PlainTextField`，长正则和多行值只看得到一行。这里测两件事：
///   1. `PlainTextField(maxLines: null)` 真的会随内容变高（高度是实测出来的，不是断言配置值）；
///   2. 中间件那 8 处确实传了 `maxLines: null` —— 不然自增高改了也没人用。
library;

import 'package:aidog_flutter/src/pages/settings/bits.dart' show PlainTextField;
import 'package:aidog_flutter/src/pages/settings/middleware_editor.dart'
    show ActionChainEditor, AppliesToEditor, ConditionTreeEditor;
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import '../pages/harness.dart';

/// 某个 key 下那个输入框的实际高度。
double heightOf(WidgetTester tester, Finder f) => tester.getSize(f).height;

/// 树里每个 [PlainTextField] 的 maxLines。
List<int?> maxLinesOf(WidgetTester tester) => [
  for (final w in tester.widgetList<PlainTextField>(
    find.byType(PlainTextField),
  ))
    w.maxLines,
];

void main() {
  testWidgets('maxLines: null 的输入框随内容长高；单行的不长', (tester) async {
    await useBigSurface(tester);
    final i18n = await makeI18n(tester);
    await tester.pumpWidget(
      wrapPage(
        const Column(
          children: [
            SizedBox(
              width: 300,
              child: PlainTextField(
                key: ValueKey('grow'),
                value: '',
                maxLines: null,
              ),
            ),
            SizedBox(
              width: 300,
              child: PlainTextField(key: ValueKey('fixed'), value: ''),
            ),
          ],
        ),
        i18n,
      ),
    );
    await settle(tester);

    final grow = find.byKey(const ValueKey('grow'));
    final fixed = find.byKey(const ValueKey('fixed'));
    final growBefore = heightOf(tester, grow);
    final fixedBefore = heightOf(tester, fixed);

    await tester.enterText(grow, 'a\nb\nc\nd');
    await tester.enterText(fixed, 'a\nb\nc\nd');
    await settle(tester);

    expect(
      heightOf(tester, grow),
      greaterThan(growBefore),
      reason: '多行内容进来，自增高那个必须真的变高',
    );
    expect(
      heightOf(tester, fixed),
      fixedBefore,
      reason: '单行那个保持原高（它是给 id / 数字这类短值用的）',
    );
  });

  testWidgets('条件树的 field / pattern 两处都是自增高', (tester) async {
    await useBigSurface(tester);
    final i18n = await makeI18n(tester);
    await tester.pumpWidget(
      wrapPage(
        ConditionTreeEditor(
          node: const {'kind': 'leaf', 'field': '', 'pattern': ''},
          onChanged: (_) {},
          onRemove: () {},
          removeLabel: '×',
        ),
        i18n,
      ),
    );
    await settle(tester);
    expect(maxLinesOf(tester), everyElement(isNull));
  });

  testWidgets(
    '动作链的 replacement / value / category / override body 是自增高，数字字段仍是单行',
    (tester) async {
      await useBigSurface(tester);
      final i18n = await makeI18n(tester);
      await tester.pumpWidget(
        wrapPage(
          ActionChainEditor(
            steps: const [
              // 四种 kind 各带不同字段：mask→replacement、inject→target/value、
              // classify→category/override status/override body、budget_gate→金额。
              {'kind': 'mask', 'params': <String, Object?>{}},
              // target 那格只在 body_set 模式下出现，显式挑它才盖得到。
              {
                'kind': 'inject',
                'params': <String, Object?>{'inject_mode': 'body_set'},
              },
              {'kind': 'classify', 'params': <String, Object?>{}},
              {'kind': 'budget_gate', 'params': <String, Object?>{}},
            ],
            onChanged: (_) {},
          ),
          i18n,
        ),
      );
      await settle(tester);
      final lines = maxLinesOf(tester);
      // 自增高：replacement / target / value / category / override body 五处。
      expect(lines.where((l) => l == null).length, greaterThanOrEqualTo(5));
      // 预算金额与 override status 是数字，React 那边也不是 AutoTextarea，保持单行。
      expect(
        lines.where((l) => l == 1).length,
        greaterThanOrEqualTo(2),
        reason: 'budget_usd / override_status 两处仍是单行',
      );
    },
  );

  testWidgets('应用范围的模型清单是自增高（逗号分隔，长列表要看得全）', (tester) async {
    await useBigSurface(tester);
    final i18n = await makeI18n(tester);
    await tester.pumpWidget(
      wrapPage(
        AppliesToEditor(
          value: const {},
          onChanged: (_) {},
          platforms: const [],
          groups: const [],
        ),
        i18n,
      ),
    );
    await settle(tester);
    expect(
      tester
          .widget<PlainTextField>(
            find.byKey(const ValueKey('mw-applies-models')),
          )
          .maxLines,
      isNull,
    );
  });
}
