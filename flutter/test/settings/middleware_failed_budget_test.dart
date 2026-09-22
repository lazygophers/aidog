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

Map<String, Object?> _rule({
  required bool failed,
  bool enabled = true,
  bool builtin = false,
}) => {
  'id': 1,
  'name': '旧规则',
  'description': '',
  'conditions': {'kind': 'leaf', 'field': 'model', 'op': 'eq', 'value': 'x'},
  'actions': <Object?>[],
  'applies_to': <String, Object?>{},
  'priority': 0,
  'enabled': enabled,
  'is_builtin': builtin,
  'failed': failed,
};

Future<(FakeKernel, I18nController)> _mount(
  WidgetTester tester, {
  required bool failed,
  bool enabled = true,
  bool builtin = false,
  List<Object?> budget = const [],
}) async {
  await useBigSurface(tester);
  final k = FakeKernel({
    ...baseResponses(),
    'middleware_list_rules': (_) => [
      _rule(failed: failed, enabled: enabled, builtin: builtin),
    ],
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

  // 第二梯队 2026-09-22：启停原先是个 SmallButton，按钮上写的到底是「现在的
  // 状态」还是「点了会变成什么」本身就有歧义；停用的规则整行也没有任何弱化，
  // 一屏规则里看不出哪几条其实没在跑（`MiddlewareRules.tsx:917,1007-1011`）。
  testWidgets('启停是开关，不是按钮；点一下发 middleware_update_rule', (tester) async {
    final (k, _) = await _mount(tester, failed: false);
    final sw = find.byType(AidogSwitch);
    expect(sw, findsOneWidget);
    expect(tester.widget<AidogSwitch>(sw).value, isTrue);
    await tester.tap(sw);
    await settle(tester);
    expect(k.calls, contains('middleware_update_rule'));
  });

  testWidgets('停用的规则整行弱化到 0.55', (tester) async {
    await _mount(tester, failed: false, enabled: false);
    final op = tester.widget<Opacity>(
      find
          .descendant(
            of: find.byKey(const ValueKey('rule-1')),
            matching: find.byType(Opacity),
          )
          .first,
    );
    expect(op.opacity, 0.55);
  });

  testWidgets('启用的规则不弱化', (tester) async {
    await _mount(tester, failed: false);
    final op = tester.widget<Opacity>(
      find
          .descendant(
            of: find.byKey(const ValueKey('rule-1')),
            matching: find.byType(Opacity),
          )
          .first,
    );
    expect(op.opacity, 1);
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

  testWidgets('内置规则：「查看规则」点得动，开出来是只读表单', (tester) async {
    final (_, t) = await _mount(tester, failed: false, builtin: true);
    final view = find.widgetWithText(SmallButton, t.t('middleware.viewRule'));
    // 原先这颗按钮是禁用的 —— 内置规则的条件和动作在界面上根本打不开看。
    expect(tester.widget<SmallButton>(view).enabled, isTrue);

    await tester.tap(view);
    await settle(tester);

    // 开出来要说清为什么改不动，并且没有保存入口。
    expect(find.text(t.t('middleware.builtinReadonlyHint')), findsOneWidget);
    expect(find.byKey(const ValueKey('rule-save')), findsNothing);
    expect(find.byKey(const ValueKey('rule-readonly-close')), findsOneWidget);
    // 规则内容本身要看得见。
    expect(find.text(t.t('middleware.conditions')), findsOneWidget);

    await tester.tap(find.byKey(const ValueKey('rule-readonly-close')));
    await settle(tester);
    expect(find.text(t.t('middleware.builtinReadonlyHint')), findsNothing);
  });
}
