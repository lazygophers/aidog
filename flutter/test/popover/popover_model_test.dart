// 票 I11 纯数据层：尺寸归一、颜色解析、统计查询构造、二维网格分行。
// 期望值逐条抄自 `src/components/PopoverCards.tsx`，与 React 一字不差。
import 'dart:ui' show Color;

import 'package:aidog_flutter/popover.dart';
import 'package:aidog_flutter/shell.dart' show AidogColors;
import 'package:flutter_test/flutter_test.dart';

Map<String, Object?> item(
  String type, {
  String id = 'i1',
  bool visible = true,
  int order = 0,
  int? row,
  String? size,
  String? scope,
  String? scopeRef,
  String? window,
  Map<String, Object?>? color,
}) => {
  'id': id,
  'item_type': type,
  'visible': visible,
  'order': order,
  'row': ?row,
  'size': ?size,
  'scope': ?scope,
  'scope_ref': ?scopeRef,
  'time_window': ?window,
  'color': ?color,
};

void main() {
  final c = AidogColors.dark;
  // 固定时刻，免得测试在午夜翻篇时飘。
  final now = DateTime(2026, 9, 20, 15, 30);

  group('normPopoverSize', () {
    test('缺省与非法值落到 m（对齐 normSize）', () {
      expect(normPopoverSize('s'), PopoverSize.s);
      expect(normPopoverSize('l'), PopoverSize.l);
      expect(normPopoverSize('m'), PopoverSize.m);
      expect(normPopoverSize(null), PopoverSize.m);
      expect(normPopoverSize('xl'), PopoverSize.m);
      expect(normPopoverSize(3), PopoverSize.m);
    });
  });

  group('颜色', () {
    test('preset 三色走主题 token，不是字面量', () {
      expect(
        popoverValueColor({'mode': 'preset', 'value': 'red'}, c),
        c.bad,
      );
      expect(
        popoverValueColor({'mode': 'preset', 'value': 'green'}, c),
        c.ok,
      );
      expect(
        popoverValueColor({'mode': 'preset', 'value': 'orange'}, c),
        c.peak,
      );
    });

    test('follow / 未知 preset / 非法 hex → null（继承主题前景）', () {
      expect(popoverValueColor({'mode': 'follow', 'value': ''}, c), isNull);
      expect(popoverValueColor({'mode': 'preset', 'value': 'pink'}, c), isNull);
      expect(popoverValueColor({'mode': 'custom', 'value': '#abc'}, c), isNull);
      expect(popoverValueColor({'mode': 'custom', 'value': 'zzzzzz'}, c), isNull);
      expect(popoverValueColor(null, c), isNull);
    });

    test('custom 六位 hex（带不带 # 都认）', () {
      expect(
        popoverValueColor({'mode': 'custom', 'value': '#1A2B3C'}, c),
        const Color(0xFF1A2B3C),
      );
      expect(
        popoverValueColor({'mode': 'custom', 'value': ' 1a2b3c '}, c),
        const Color(0xFF1A2B3C),
      );
    });

    test('entry 色在 follow 时回落主题前景，不是 null', () {
      expect(popoverEntryColor({'mode': 'follow', 'value': ''}, c), c.fg);
      expect(popoverEntryColor({'mode': 'preset', 'value': 'red'}, c), c.bad);
    });
  });

  group('buildPopoverTrendQuery', () {
    test('today → 本地午夜起 + hourly', () {
      final q = buildPopoverTrendQuery(
        item('cost_trend', window: 'today'),
        now: now,
      );
      expect(q['granularity'], 'hourly');
      expect(q['start'], DateTime(2026, 9, 20).millisecondsSinceEpoch);
      expect(q['end'], now.millisecondsSinceEpoch);
    });

    test('缺省 = 7d；30d 取 30 天，都是 daily', () {
      final d = buildPopoverTrendQuery(item('cost_trend'), now: now);
      expect(d['granularity'], 'daily');
      expect(d['start'], now.millisecondsSinceEpoch - 7 * kDayMs);

      final m = buildPopoverTrendQuery(
        item('cost_trend', window: '30d'),
        now: now,
      );
      expect(m['start'], now.millisecondsSinceEpoch - 30 * kDayMs);
    });

    test('scope 带 ref 才落 filter；overall 不落任何 filter', () {
      expect(
        buildPopoverTrendQuery(item('cost_trend'), now: now).containsKey('filter_group'),
        isFalse,
      );
      expect(
        buildPopoverTrendQuery(
          item('cost_trend', scope: 'group', scopeRef: 'gk_1'),
          now: now,
        )['filter_group'],
        'gk_1',
      );
      expect(
        buildPopoverTrendQuery(
          item('cost_trend', scope: 'platform', scopeRef: '7'),
          now: now,
        )['filter_platform'],
        '7',
      );
      // scope 是 group 但没选组 → 不落 filter（否则等于查了个空串分组）
      expect(
        buildPopoverTrendQuery(
          item('cost_trend', scope: 'group', scopeRef: ''),
          now: now,
        ).containsKey('filter_group'),
        isFalse,
      );
    });
  });

  group('buildPopoverItemQuery', () {
    test('非统计卡返 null', () {
      for (final ty in [
        'proxy_status',
        'platform_balance',
        'today_cost',
        'today_cache_rate',
        'today_tokens',
        'platform_today',
        'group_balance',
      ]) {
        expect(buildPopoverItemQuery(item(ty), now: now), isNull, reason: ty);
      }
    });

    test('统计卡都返查询', () {
      for (final ty in kPopoverStatsItemTypes) {
        expect(buildPopoverItemQuery(item(ty), now: now), isNotNull, reason: ty);
      }
    });

    test('三类强制今日窗，即使 item 上写着 30d', () {
      for (final ty in kPopoverTodayOnlyTypes) {
        final q = buildPopoverItemQuery(item(ty, window: '30d'), now: now)!;
        expect(q['granularity'], 'hourly', reason: ty);
        expect(
          q['start'],
          DateTime(2026, 9, 20).millisecondsSinceEpoch,
          reason: ty,
        );
      }
    });

    test('platform_share 加 series_by=platform，其余不加', () {
      expect(
        buildPopoverItemQuery(item('platform_share'), now: now)!['series_by'],
        'platform',
      );
      expect(
        buildPopoverItemQuery(
          item('cost_trend'),
          now: now,
        )!.containsKey('series_by'),
        isFalse,
      );
    });
  });

  group('popoverStatsQueries', () {
    test('只收 visible 的统计卡，itemIds 与 queries 平行', () {
      final config = {
        'items': [
          item('cost_trend', id: 'a', order: 0),
          item('today_cost', id: 'b', order: 1),
          item('hour_heatbar', id: 'c', order: 2, visible: false),
          item('group_cost', id: 'd', order: 3, scope: 'group', scopeRef: 'g'),
        ],
      };
      final q = popoverStatsQueries(config, now: now);
      expect(q.itemIds, ['a', 'd']);
      expect(q.queries.length, 2);
      expect(q.queries[1]['filter_group'], 'g');
    });

    test('没有统计卡 → 空（调用方据此跳过批量 IPC）', () {
      final q = popoverStatsQueries({
        'items': [item('today_cost')],
      }, now: now);
      expect(q.queries, isEmpty);
      expect(q.itemIds, isEmpty);
    });
  });

  group('popoverRows', () {
    test('缺省 row 回退 order：老配置各占一行', () {
      final rows = popoverRows({
        'items': [
          item('today_cost', id: 'a', order: 1),
          item('today_tokens', id: 'b', order: 0),
        ],
      });
      expect(rows.map((r) => r.row), [0, 1]);
      expect(rows[0].items.single['id'], 'b');
      expect(rows.every((r) => r.cols == 1), isTrue);
    });

    test('同一 row 的卡聚成一行，行内按 order 升序', () {
      final rows = popoverRows({
        'items': [
          item('today_cost', id: 'a', order: 2, row: 0),
          item('today_tokens', id: 'b', order: 1, row: 0),
          item('proxy_status', id: 'c', order: 3, row: 1),
        ],
        'rows': [
          {'cols': 2},
          {'cols': 1},
        ],
      });
      expect(rows.length, 2);
      expect(rows[0].cols, 2);
      expect(rows[0].items.map((e) => e['id']), ['b', 'a']);
      expect(rows[1].items.single['id'], 'c');
    });

    test('rows 缺省 / 越界 → cols=1；隐藏项不进网格', () {
      final rows = popoverRows({
        'items': [
          item('today_cost', id: 'a', row: 5),
          item('today_tokens', id: 'b', row: 5, visible: false),
        ],
      });
      expect(rows.single.cols, 1);
      expect(rows.single.items.map((e) => e['id']), ['a']);
    });

    test('全隐藏 → 空（调用方走空态）', () {
      expect(
        popoverRows({
          'items': [item('today_cost', visible: false)],
        }),
        isEmpty,
      );
      expect(popoverRows(const {}), isEmpty);
    });
  });

  group('聚合口径', () {
    test('overview token = input + output，不含 cache（与 today_stats 对齐）', () {
      expect(
        popoverOverviewTokens({
          'total_input_tokens': 10,
          'total_output_tokens': 5,
          'total_cache_tokens': 99,
        }),
        15,
      );
      expect(popoverOverviewTokens(const {}), 0);
    });

    test('占比条目逐序列求和，≤0 的序列滤掉', () {
      final entries = popoverShareEntries({
        'series': [
          {
            'name': 'A',
            'buckets': [
              {'total_cost': 1.5},
              {'total_cost': 0.5},
            ],
          },
          {
            'name': 'B',
            'buckets': [
              {'total_cost': 0},
            ],
          },
        ],
      });
      expect(entries.length, 1);
      expect(entries.single.name, 'A');
      expect(entries.single.value, 2.0);
    });
  });
}
