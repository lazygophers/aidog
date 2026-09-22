/// 票 24 的护栏：分组展开区渲染的是**完整平台卡**，不是一行文字。
///
/// 挂的是整个 [PlatformsPage]（`showGroups: true`），因为这张票的要害就是
/// 「分组区怎么拿到平台卡要的数据」—— 卡片由平台页注入闭包构建，单独挂
/// `GroupsSection` 测不到那条线。
library;

import 'package:aidog_flutter/i18n.dart';
import 'package:aidog_flutter/pages.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import '../settings/fake_invoke.dart';
import 'harness.dart';

Map<String, dynamic> plat(int id, String name) => {
  'id': id,
  'name': name,
  'platform_type': 'openai',
  'base_url': 'https://u$id',
  'api_key': 'k',
  'status': 'enabled',
  'enabled': true,
  'models': {'default': 'm'},
  'available_models': ['m'],
  'endpoints': <Object?>[],
  'est_balance_remaining': 0,
};

/// 一个分组（G10）里装着两个平台（P1 / P2），另有一个未分组平台（P3）。
/// 两个组内平台是为了让「上下移」两端都有可点的一侧。
FakeInvoke pageFake() {
  final detail = {
    'group': {'id': 10, 'name': 'G10', 'group_key': 'gk10'},
    'platforms': [
      {'platform': plat(1, 'P1'), 'level_priority': 5},
      {'platform': plat(2, 'P2'), 'level_priority': 5},
    ],
    'model_mappings': <Object?>[],
  };
  return FakeInvoke({
    'platform_list': [plat(1, 'P1'), plat(2, 'P2'), plat(3, 'P3')],
    'group_detail_list_paged': [detail],
    'group_detail_list': [detail],
    'group_detail': detail,
    'all_group_usage_stats': <String, Object?>{},
    'all_platform_usage_stats': <String, Object?>{},
    'get_last_test_result': null,
    'scheduling_settings_get': null,
    'platform_usage_stats': null,
    'proxy_get_settings': {'port': 9999},
    'get_defaults_json': '{"protocols":{}}',
    'set_ui_extra': null,
    'group_platform_set_level_priority': null,
    'group_platform_reorder': null,
  });
}

Future<I18nController> mount(WidgetTester tester, FakeInvoke k) async {
  await useBigSurface(tester);
  final c = await makeI18n(tester);
  await tester.pumpWidget(
    wrapPage(
      PlatformsPage(invoke: k.fn, logUpdates: const Stream<void>.empty()),
      c,
    ),
  );
  await settle(tester);
  return c;
}

/// 组内那两张卡（`draggable: false`）。未分组那张是 true，靠这个区分。
Finder groupCards() =>
    find.byWidgetPredicate((w) => w is PlatformCard && !w.draggable);

void main() {
  testWidgets('展开分组 → 组内渲染完整 PlatformCard，且不带拖拽手柄', (tester) async {
    final c = await mount(tester, pageFake());

    // 组内两张 + 未分组一张 = 三张真卡。
    expect(find.byType(PlatformCard), findsNWidgets(3));
    expect(groupCards(), findsNWidgets(2));

    // 组内的卡没有拖拽手柄；未分组那张有（票 24 约束 3 的直接投影）。
    for (final card in tester.widgetList<PlatformCard>(groupCards())) {
      expect(card.draggable, isFalse);
    }
    expect(
      find.byTooltip(c.t('platform.dragReorder')),
      findsOneWidget,
      reason: '只有未分组列表那张卡带手柄',
    );

    // 卡片真的是「完整」的：平台页那一份控制器的数据流到了组内
    // —— 快操作区的开关与图标按钮在组内也画得出来。
    expect(find.byType(AidogSwitch), findsNWidgets(3));
    expect(find.byTooltip(c.t('page.logs')), findsNWidgets(3));
  });

  testWidgets('组内控件保留：上下移、优先级加减、移除', (tester) async {
    final k = pageFake();
    final c = await mount(tester, k);

    // 上下移：第一行的「上移」禁用、第二行的「下移」禁用，中间两颗可点。
    // 按图标找而不是按 tooltip —— `group.dragToReorder` 这个 key 被分组卡自己的
    // 拖拽把手（`groups.dart:493`）共用，按 tooltip 找会多命中它一个。
    final ups = find.widgetWithIcon(IconButton, Icons.arrow_upward);
    final downs = find.widgetWithIcon(IconButton, Icons.arrow_downward);
    expect(ups, findsNWidgets(2));
    expect(downs, findsNWidgets(2));
    final upBtns = tester.widgetList<IconButton>(ups).toList();
    final downBtns = tester.widgetList<IconButton>(downs).toList();
    expect(upBtns[0].onPressed, isNull, reason: '第一行不能再上移');
    expect(upBtns[1].onPressed, isNotNull);
    expect(downBtns[0].onPressed, isNotNull);
    expect(downBtns[1].onPressed, isNull, reason: '最后一行不能再下移');

    // 优先级加减：点一下「+」发命令。
    final up = find.byTooltip(c.t('group.levelPriorityUp'));
    expect(up, findsNWidgets(2));
    await tester.tap(up.first);
    await settle(tester);
    expect(k.callsTo('group_platform_set_level_priority'), isNotEmpty);
    expect(find.byTooltip(c.t('group.levelPriorityDown')), findsNWidgets(2));

    // 「移除」仍在（每个组内平台一颗）。
    expect(
      find.widgetWithText(SmallButton, c.t('group.deletePlatformTitle')),
      findsNWidgets(2),
    );
  });

  testWidgets('多选态：勾选框仍在，组内控件让位', (tester) async {
    final c = await mount(tester, pageFake());

    await tester.tap(find.text(c.t('group.batchOps')));
    await settle(tester);

    // 每个组内平台一个勾选框；卡片本身仍是完整卡。
    expect(find.byType(Checkbox), findsNWidgets(2));
    expect(groupCards(), findsNWidgets(2));
    // 多选态下上下移与优先级收起（与改造前同一条规矩）。
    expect(find.widgetWithIcon(IconButton, Icons.arrow_upward), findsNothing);
    expect(find.byTooltip(c.t('group.levelPriorityUp')), findsNothing);

    // 勾一个，选中态进到控制器里。
    await tester.tap(find.byType(Checkbox).first);
    await settle(tester);
    expect(tester.widget<Checkbox>(find.byType(Checkbox).first).value, isTrue);
  });

  testWidgets('组内卡与未分组卡共用同一份数据源：不为分组区多发一轮取数', (tester) async {
    final k = pageFake();
    await mount(tester, k);
    // 票 24 的选型理由就落在这条上：分组区若自起一份 PlatformsController，
    // 每平台统计会被整查两轮，两份缓存也会各自过期，同一个平台在页面上下两处
    // 显示不同余额。共用一份 → 只查一轮。
    expect(k.callsTo('all_platform_usage_stats').length, 1);
    // `platform_list` 是 2 轮，但那**不是**本票引入的：GroupsController 早就
    // 自己要一份平台清单给「关联平台」选择器用（`groups_logic.dart:545/614/642`），
    // 与卡片数据无关。这里钉住它别再涨。
    expect(k.callsTo('platform_list').length, 2);
  });
}
