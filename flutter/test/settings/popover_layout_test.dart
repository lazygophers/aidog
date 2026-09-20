/// `lib/src/pages/settings/popover_layout.dart` 的单测 —— 语义对齐
/// `src/pages/PopoverConfigTab/utils.ts::normalizeConfig / effRow / makeItem`
/// 与 `usePopoverConfig.ts::moveItemToRow`（React 侧行为逐条翻译）。
library;

import 'package:aidog_flutter/src/pages/settings/popover_layout.dart';
import 'package:flutter_test/flutter_test.dart';

Map<String, Object?> _item(String id, Object? row, int order) =>
    {'id': id, 'item_type': 'today_cost', 'row': row, 'order': order};

void main() {
  group('normalizePopoverConfig（utils.ts::normalizeConfig）', () {
    test('row 缺省回退 order（老配置各占一行）', () {
      final cfg = normalizePopoverConfig(
        [_item('a', null, 0), _item('b', null, 1)],
        null,
      );
      final items = (cfg['items']! as List).cast<Map>();
      // row 为 null 的老条目：effRow = order → 各自成行。
      expect(items.map((i) => i['row']), [0, 1]);
      expect(items.map((i) => i['order']), [0, 0]);
      expect((cfg['rows']! as List).length, 2);
    });

    test('行号重排为连续、行内按 order、rows 列数保留', () {
      final cfg = normalizePopoverConfig(
        [
          _item('c', 5, 0),
          _item('a', 2, 1),
          _item('b', 2, 0),
          _item('d', 2, 5),
        ],
        [
          {'cols': 2},
        ],
      );
      final items = (cfg['items']! as List).cast<Map>();
      expect(items.map((i) => '${i['id']}'), ['b', 'a', 'd', 'c']);
      expect(items.map((i) => i['row']), [0, 0, 0, 1]);
      expect(items.map((i) => i['order']), [0, 1, 2, 0]);
      final rows = (cfg['rows']! as List).cast<Map>();
      expect(rows.length, 2);
      expect(rows[0]['cols'], 2); // 旧行号 2 → 新行 0，已有列数保留
      expect(rows[1]['cols'], 1); // 缺省 1
    });

    test('幂等：规整结果再规整一次不变', () {
      final once = normalizePopoverConfig(
        [_item('a', 0, 0), _item('b', 1, 0)],
        const [],
      );
      final twice = normalizePopoverConfig(
        ((once['items']! as List).whereType<Map>()).map(Map<String, Object?>.from).toList(),
        ((once['rows']! as List).whereType<Map>()).map(Map<String, Object?>.from).toList(),
      );
      expect(twice, once);
    });
  });

  group('movePopoverItemToRow（usePopoverConfig.ts::moveItemToRow）', () {
    test('跨行搬移到行尾', () {
      final next = movePopoverItemToRow(
        [_item('a', 0, 0), _item('b', 1, 0), _item('c', 1, 1)],
        'a',
        1,
        null,
      );
      final byId = {for (final i in next) '${i['id']}': i};
      expect(byId['a']!['row'], 1);
      expect(byId['a']!['order'], 2); // 追加到行尾
      expect(byId['b']!['order'], 0);
      expect(byId['c']!['order'], 1);
    });

    test('插到某张卡之前', () {
      final next = movePopoverItemToRow(
        [_item('a', 0, 0), _item('b', 1, 0), _item('c', 1, 1)],
        'a',
        1,
        'c',
      );
      final byId = {for (final i in next) '${i['id']}': i};
      expect(byId['a']!['row'], 1);
      expect(byId['a']!['order'], 1); // b, a, c
      expect(byId['c']!['order'], 2);
    });

    test('行内换位（beforeId = 同行后一张卡）', () {
      final next = movePopoverItemToRow(
        [_item('a', 0, 0), _item('b', 0, 1), _item('c', 0, 2)],
        'c',
        0,
        'a',
      );
      final byId = {for (final i in next) '${i['id']}': i};
      expect(byId['c']!['order'], 0);
      expect(byId['a']!['order'], 1);
      expect(byId['b']!['order'], 2);
    });

    test('同行 + beforeId null → 原样返回（dnd 落在行容器的 noop 分支）', () {
      final items = [_item('a', 0, 0), _item('b', 0, 1)];
      expect(movePopoverItemToRow(items, 'a', 0, null), same(items));
    });

    test('beforeId 是自己 → 原样返回', () {
      final items = [_item('a', 0, 0)];
      expect(movePopoverItemToRow(items, 'a', 0, 'a'), same(items));
    });

    test('activeId 不存在 / beforeId 不在目标行 → 行尾', () {
      final items = [_item('a', 0, 0), _item('b', 1, 0)];
      expect(movePopoverItemToRow(items, 'zz', 1, null), same(items));
      final next = movePopoverItemToRow(items, 'a', 1, 'missing');
      final byId = {for (final i in next) '${i['id']}': i};
      expect(byId['a']!['order'], 1); // 找不到 beforeId → 行尾
    });
  });

  group('makePopoverItem（utils.ts::makeItem）', () {
    test('按类型带缺省值', () {
      final ps = [(id: 7, name: 'p7')];
      final gs = [(id: 1, name: 'g', groupKey: 'gk')];
      expect(makePopoverItem('today_cost', 0, 0)['scope'], isNull);
      final trend = makePopoverItem('cost_trend', 0, 1);
      expect(trend['scope'], 'overall');
      expect(trend['time_window'], '7d');
      expect(makePopoverItem('platform_share', 0, 0)['time_window'], '7d');
      expect(makePopoverItem('hour_heatbar', 0, 0)['time_window'], 'today');
      final metric = makePopoverItem('platform_metric', 0, 0, platforms: ps);
      expect(metric['scope'], 'platform');
      expect(metric['scope_ref'], '7');
      expect(metric['time_window'], 'today');
      final gc = makePopoverItem('group_cost', 0, 0, groups: gs);
      expect(gc['scope'], 'group');
      expect(gc['scope_ref'], 'gk');
      expect(gc['time_window'], '7d');
      expect(makePopoverItem('group_balance', 0, 0, groups: gs)['time_window'],
          isNull);
      expect(
        makePopoverItem('group_requests', 0, 0, groups: gs)['time_window'],
        'today',
      );
    });

    test('id 含类型与时间戳，重复添加不撞', () {
      final a = makePopoverItem('today_cost', 0, 0);
      final b = makePopoverItem('today_cost', 0, 0);
      expect(a['id'], isNot(b['id']));
    });
  });
}
