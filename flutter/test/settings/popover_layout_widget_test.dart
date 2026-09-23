/// I17 浮窗二维布局编辑器的 widget 测试：行列数、长按拖拽跨行 / 行内、
/// 添加落新行、删除。语义部分（normalize / moveItemToRow / makeItem）在
/// `popover_layout_test.dart`，这里只测接线上。
library;

import 'package:aidog_flutter/i18n.dart';
import 'package:aidog_flutter/pages.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import '../pages/harness.dart';
import 'pages_c_widget_test.dart' show baseResponses;

Map<String, Object?> _item(String id, String ty, int row, int order) => {
  'id': id,
  'item_type': ty,
  'visible': true,
  'order': order,
  'row': row,
  'size': 'm',
};

void main() {
  Future<(FakeKernel, I18nController)> mount(
    WidgetTester tester, {
    Map<String, Object?> config = const {},
  }) async {
    await useBigSurface(tester);
    final k = FakeKernel({
      ...baseResponses(),
      'popover_config_get': (_) => config,
      'popover_config_set': (_) => null,
    });
    final i18n = await makeI18n(tester);
    await tester.pumpWidget(
      wrapPage(PopoverSettingsPage(invoke: k.invoke), i18n),
    );
    await settle(tester);
    return (k, i18n);
  }

  testWidgets('两行两卡：行容器 + 列数按钮就位，cols 落盘', (tester) async {
    final (k, _) = await mount(
      tester,
      config: {
        'items': [
          _item('a', 'today_cost', 0, 0),
          _item('b', 'today_tokens', 1, 0),
        ],
        'rows': [
          {'cols': 1},
          {'cols': 2},
        ],
      },
    );
    expect(find.byKey(const ValueKey('popover-row-0')), findsOneWidget);
    expect(find.byKey(const ValueKey('popover-row-1')), findsOneWidget);

    await tester.tap(find.byKey(const ValueKey('popover-cols-0-3')));
    await settle(tester);
    final cfg = k.lastArgsOf('popover_config_set')!['config']! as Map;
    final rows = (cfg['rows']! as List).cast<Map>();
    expect(rows[0]['cols'], 3);
    expect(rows[1]['cols'], 2); // 没动的那行保留
  });

  testWidgets('长按拖到另一行：跨行搬移落盘', (tester) async {
    final (k, t) = await mount(
      tester,
      config: {
        'items': [
          _item('a', 'today_cost', 0, 0),
          _item('b', 'today_tokens', 1, 0),
        ],
        'rows': [
          {'cols': 1},
          {'cols': 1},
        ],
      },
    );
    // 拖 a 卡（从它的卡片标题起手，长按超过 250ms 阈值再移动到第 1 行容器）。
    final aTitle = find.descendant(
      of: find.byKey(const ValueKey('popover-row-0')),
      matching: find.text(t.t('popover.todayCost')),
    );
    final gesture = await tester.startGesture(tester.getCenter(aTitle));
    await tester.pump(const Duration(milliseconds: 400));
    await gesture.moveTo(
      tester.getCenter(find.byKey(const ValueKey('popover-row-1'))),
    );
    await tester.pump();
    await gesture.up();
    await settle(tester);

    final cfg = k.lastArgsOf('popover_config_set')!['config']! as Map;
    final byId = {
      for (final i in (cfg['items']! as List).whereType<Map>()) '${i['id']}': i,
    };
    // 拖过去后两卡同排、a 在 b 前。行号本身会被规整重编（原行空了就前移），
    // 断言落在「同排 + 次序」上，与渲染层 `popoverRows` 的读取口径一致。
    expect(byId['a']!['row'], byId['b']!['row']);
    expect(byId['a']!['order'], 0);
    expect(byId['b']!['order'], 1);
    expect((cfg['rows']! as List).length, 1); // 空行收掉
  });

  testWidgets('添加一项：落到新的一行，缺省带类型默认值', (tester) async {
    final (k, _) = await mount(
      tester,
      config: {
        'items': [_item('a', 'today_cost', 0, 0)],
        'rows': [
          {'cols': 2},
        ],
      },
    );
    // 「添加项」现在是按钮 + 菜单（不再是一排平铺按钮），先点开再选。
    await tester.tap(
      find.descendant(
        of: find.byKey(const ValueKey('popover-add')),
        matching: find.byType(SmallButton),
      ),
    );
    await settle(tester);
    await tester.tap(
      find.byKey(const ValueKey('popover-add-today_cache_rate')),
    );
    await settle(tester);
    final cfg = k.lastArgsOf('popover_config_set')!['config']! as Map;
    final items = (cfg['items']! as List).cast<Map>();
    expect(items.length, 2);
    final added = items.firstWhere((i) => '${i['id']}' != 'a');
    expect(added['row'], 1); // 新行
    expect(added['visible'], true);
    final rows = (cfg['rows']! as List).cast<Map>();
    expect(rows.length, 2);
    expect(rows[1]['cols'], 1);
    expect(rows[0]['cols'], 2); // 原行列数保留
  });

  testWidgets('删除一张卡：从行里移除并落盘', (tester) async {
    final (k, _) = await mount(
      tester,
      config: {
        'items': [
          _item('a', 'today_cost', 0, 0),
          _item('b', 'today_tokens', 0, 1),
        ],
        'rows': [
          {'cols': 2},
        ],
      },
    );
    await tester.tap(find.byKey(const ValueKey('popover-del-a')));
    await settle(tester);
    final cfg = k.lastArgsOf('popover_config_set')!['config']! as Map;
    final items = (cfg['items']! as List).cast<Map>();
    expect(items.map((i) => '${i['id']}'), ['b']);
    expect(items[0]['order'], 0); // 重排
  });

  // 第二梯队 2026-09-22：切到 platform / group 原先一律把 scope_ref 清成 null，
  // 切完这张卡是空的，还得再点一次选具体对象（`ScopeConfig.tsx:29-37` 是预填第一个）。
  testWidgets('趋势卡切范围：切到平台预填第一个平台，切回总体清空', (tester) async {
    await useBigSurface(tester);
    final k = FakeKernel({
      ...baseResponses(),
      'popover_config_get': (_) => {
        'items': [_item('a', 'cost_trend', 0, 0)],
        'rows': [
          {'cols': 1},
        ],
      },
      'popover_config_set': (_) => null,
      'platform_list': (_) => [
        {'id': 7, 'name': 'P7', 'platform_type': 'openai'},
        {'id': 8, 'name': 'P8', 'platform_type': 'openai'},
      ],
      // 这一页的分组来自 group_list（`popover_logic.dart:135`），
      // 不是 group_detail_list。
      'group_list': (_) => [
        {'id': 1, 'group_key': 'gk1', 'name': 'G1'},
      ],
    });
    final i18n = await makeI18n(tester);
    await tester.pumpWidget(
      wrapPage(PopoverSettingsPage(invoke: k.invoke), i18n),
    );
    await settle(tester);

    // 范围是下拉（对齐 React `ScopeConfig.tsx:27-50`）：点开再选。
    // 选中某个范围后会多出一个同名标签的选择器行，所以选项 tap 用
    // `.last`（菜单浮在最上层）。
    Future<void> pickScope(String label) async {
      await tester.tap(
        find.descendant(
          of: find.byKey(const ValueKey('popover-scope-a')),
          matching: find.byWidgetPredicate((w) => w is DropdownButton),
        ),
      );
      await tester.pumpAndSettle();
      await tester.tap(find.text(label).last);
      await settle(tester);
    }

    await pickScope(i18n.t('popover.trendScopePlatform'));
    var cfg = k.lastArgsOf('popover_config_set')!['config']! as Map;
    var item = (cfg['items']! as List).first as Map;
    expect(item['scope'], 'platform');
    expect(item['scope_ref'], '7', reason: '预填第一个平台，不是留空');

    await pickScope(i18n.t('popover.trendScopeGroup'));
    cfg = k.lastArgsOf('popover_config_set')!['config']! as Map;
    item = (cfg['items']! as List).first as Map;
    expect(item['scope'], 'group');
    expect(item['scope_ref'], 'gk1');

    await pickScope(i18n.t('popover.trendScopeOverall'));
    await settle(tester);
    cfg = k.lastArgsOf('popover_config_set')!['config']! as Map;
    item = (cfg['items']! as List).first as Map;
    expect(item['scope'], 'overall');
    expect(item['scope_ref'], isNull, reason: '总体范围没有具体对象');
  });
}
