/// 中间件的两个「后端发了、前端没接」的字段。
///
/// 与日志详情的 `attempts` 同一类问题：**逐元素比对表照不出来**（它比的是渲染出来的
/// 东西），得往模型层查。两条都出自 `generated/*.ts`：
///   - `MiddlewareRule.failed`（`:12-16`）：旧模型残留、引擎跳过，前端该引导手删
///   - `MiddlewareBudgetStatus.budget_usd`（`:7-11`）：预算上限
library;

import 'package:aidog_flutter/i18n.dart';
import 'package:aidog_flutter/pages.dart';
import 'package:aidog_flutter/shell.dart';
import 'package:aidog_flutter/utils/formatters.dart';
import 'package:aidog_flutter/src/pages/settings/middleware_logic.dart'
    show MiddlewareRule;
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import '../pages/harness.dart';
import 'pages_c_widget_test.dart' show baseResponses;

Map<String, Object?> _rule({required bool failed}) => {
  'id': 1,
  'name': '旧规则',
  'description': '',
  'conditions': {'kind': 'leaf', 'field': 'model', 'op': 'eq', 'value': 'x'},
  'actions': <Object?>[],
  'applies_to': <String, Object?>{},
  'priority': 0,
  'enabled': true,
  'is_builtin': false,
  'failed': failed,
};

Future<(FakeKernel, I18nController)> _mount(
  WidgetTester tester, {
  required bool failed,
  List<Object?> budget = const [],
}) async {
  await useBigSurface(tester);
  final k = FakeKernel({
    ...baseResponses(),
    'middleware_list_rules': (_) => [_rule(failed: failed)],
    'middleware_budget_status': (_) => budget,
  });
  final i18n = await makeI18n(tester);
  await tester.pumpWidget(
    wrapPage(MiddlewareSettingsPage(invoke: k.invoke), i18n),
  );
  await settle(tester);
  return (k, i18n);
}

void main() {
  setUp(resetNavGuardForTest);
  tearDown(resetNavGuardForTest);

  test('MiddlewareRule.failed 解析得出来（原先模型里压根没这个字段）', () {
    expect(MiddlewareRule(_rule(failed: true)).failed, isTrue);
    expect(MiddlewareRule(_rule(failed: false)).failed, isFalse);
    // 后端没发这个字段时按「没失效」处理，不要把旧数据全判成失效。
    final noField = Map<String, Object?>.from(_rule(failed: false))
      ..remove('failed');
    expect(MiddlewareRule(noField).failed, isFalse);
  });

  testWidgets('失效规则：出「失效」徽标，且编辑按钮点不动', (tester) async {
    final (_, t) = await _mount(tester, failed: true);
    expect(find.text(t.t('middleware.failed')), findsOneWidget);
    expect(
      tester
          .widget<SmallButton>(
            find.widgetWithText(SmallButton, t.t('action.edit')),
          )
          .enabled,
      isFalse,
      reason: '引擎翻译不了那份条件，改它没有意义 —— React 干脆不渲染编辑按钮',
    );
  });

  testWidgets('正常规则不出失效徽标，编辑按钮可点', (tester) async {
    final (_, t) = await _mount(tester, failed: false);
    expect(find.text(t.t('middleware.failed')), findsNothing);
    expect(
      tester
          .widget<SmallButton>(
            find.widgetWithText(SmallButton, t.t('action.edit')),
          )
          .enabled,
      isTrue,
    );
  });

  testWidgets('预算闸门：画出上限，超限换成「已超预算」', (tester) async {
    final (_, t) = await _mount(
      tester,
      failed: false,
      budget: [
        {
          'rule_id': 1,
          'rule_name': '旧规则',
          'budget_usd': 10.0,
          'spent_usd': 12.0,
          'remaining_usd': -2.0,
        },
      ],
    );
    // 上限必须出现在界面上 —— 原先只画「已用 X · 剩余 Y」，看不到上限是多少。
    expect(find.textContaining(formatCostUsd(10)), findsWidgets);
    expect(find.text(t.t('middleware.budgetExceeded')), findsOneWidget);
    // 超限时不该再显「剩余 -2」这种读不懂的数。
    expect(find.textContaining(t.t('middleware.budgetRemaining')), findsNothing);
  });
}
