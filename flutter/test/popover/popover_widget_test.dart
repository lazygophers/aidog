// 票 I11 小窗渲染：14 种卡片 × 三态（加载 / 失败 / 有数据）× 三尺寸，
// 加装载器的分路失败兜底。不起内核（假 invoke 顶掉传输层），不碰 9890 端口。
import 'package:aidog_flutter/i18n.dart';
import 'package:aidog_flutter/popover.dart';
import 'package:aidog_flutter/charts.dart' show HourHeatBar;
import 'package:aidog_flutter/shell.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import '../pages/harness.dart';

Map<String, Object?> cfgItem(
  String type, {
  String id = 'i1',
  bool visible = true,
  int order = 0,
  int? row,
  String size = 'm',
  String? scope,
  String? scopeRef,
  Map<String, Object?>? color,
}) => {
  'id': id,
  'item_type': type,
  'visible': visible,
  'order': order,
  'row': ?row,
  'size': size,
  'scope': ?scope,
  'scope_ref': ?scopeRef,
  'color': color ?? {'mode': 'follow', 'value': ''},
};

Map<String, Object?> popoverData(
  List<Map<String, Object?>> items, {
  List<Map<String, Object?>> rows = const [],
  bool running = true,
  int port = 9890,
  List<Map<String, Object?>> entries = const [],
  List<Map<String, Object?>> platformToday = const [],
  Map<String, Object?>? today,
}) => {
  'config': {'items': items, 'rows': rows},
  'entries': entries,
  'today_stats':
      today ??
      {
        'tokens': 12345,
        'input_tokens': 10000,
        'output_tokens': 2345,
        'cache_tokens': 500,
        'cache_rate': 12.5,
        'cost': 3.25,
      },
  'platform_today': platformToday,
  'proxy_running': running,
  'proxy_port': port,
};

Future<void> pumpGrid(
  WidgetTester tester,
  I18nController i18n,
  PopoverFrame frame,
) async {
  await tester.pumpWidget(
    MaterialApp(
      theme: aidogThemeData(AidogMode.dark),
      home: AidogI18n(
        controller: i18n,
        child: Scaffold(
          body: SingleChildScrollView(
            child: SizedBox(width: 340, child: PopoverGrid(frame: frame)),
          ),
        ),
      ),
    ),
  );
  await settle(tester);
}

void main() {
  group('网格骨架', () {
    testWidgets('没有可见卡片 → 空态文案，不画空壳', (tester) async {
      final t = await makeI18n(tester);
      await pumpGrid(
        tester,
        t,
        PopoverFrame(data: popoverData([cfgItem('today_cost', visible: false)])),
      );
      expect(find.text(t.t('popover.empty')), findsOneWidget);
    });

    testWidgets('同一行两列并排，行内按 order 升序', (tester) async {
      final t = await makeI18n(tester);
      await pumpGrid(
        tester,
        t,
        PopoverFrame(
          data: popoverData(
            [
              cfgItem('today_cost', id: 'a', order: 2, row: 0),
              cfgItem('today_tokens', id: 'b', order: 1, row: 0),
            ],
            rows: [
              {'cols': 2},
            ],
          ),
        ),
      );
      final cards = tester.widgetList<PopoverCard>(find.byType(PopoverCard));
      expect(cards.map((e) => e.item['id']), ['b', 'a']);
      // 两张卡在同一个 Row 里（并排），不是上下两行。
      final aY = tester.getTopLeft(find.text(t.t('popover.todayCost'))).dy;
      final bY = tester.getTopLeft(find.text(t.t('popover.todayTokens'))).dy;
      expect(aY, bY);
    });

    testWidgets('一行里的卡多于列数 → 折到下一排，不挤成一排', (tester) async {
      final t = await makeI18n(tester);
      await pumpGrid(
        tester,
        t,
        PopoverFrame(
          data: popoverData(
            [
              cfgItem('today_cost', id: 'a', order: 0, row: 0),
              cfgItem('today_tokens', id: 'b', order: 1, row: 0),
              cfgItem('today_cache_rate', id: 'c', order: 2, row: 0),
            ],
            rows: [
              {'cols': 2},
            ],
          ),
        ),
      );
      final third = tester.getTopLeft(find.text(t.t('popover.todayCacheRate'))).dy;
      final first = tester.getTopLeft(find.text(t.t('popover.todayCost'))).dy;
      expect(third, greaterThan(first));
    });

    testWidgets('未知类型 → 什么都不画（对齐 React 的 default: null）', (tester) async {
      final t = await makeI18n(tester);
      await pumpGrid(
        tester,
        t,
        PopoverFrame(data: popoverData([cfgItem('not_a_real_type')])),
      );
      expect(find.byType(PopoverCard), findsOneWidget);
      expect(find.byType(Text), findsNothing);
    });
  });

  group('不走统计查询的四张卡', () {
    testWidgets('proxy_status：跑着显端口，停了显 Stopped', (tester) async {
      final t = await makeI18n(tester);
      await pumpGrid(
        tester,
        t,
        PopoverFrame(data: popoverData([cfgItem('proxy_status')])),
      );
      expect(find.textContaining('9890'), findsOneWidget);

      await pumpGrid(
        tester,
        t,
        PopoverFrame(
          data: popoverData([cfgItem('proxy_status')], running: false),
        ),
      );
      expect(find.text('Stopped'), findsOneWidget);
    });

    testWidgets('platform_balance：s 尺寸只出值不出名', (tester) async {
      final t = await makeI18n(tester);
      const entries = [
        {
          'name': 'GLM',
          'value': r'$12.00',
          'color': {'mode': 'preset', 'value': 'green'},
        },
      ];
      await pumpGrid(
        tester,
        t,
        PopoverFrame(
          data: popoverData([cfgItem('platform_balance')], entries: entries),
        ),
      );
      expect(find.text('GLM'), findsOneWidget);

      await pumpGrid(
        tester,
        t,
        PopoverFrame(
          data: popoverData([
            cfgItem('platform_balance', size: 's'),
          ], entries: entries),
        ),
      );
      expect(find.text('GLM'), findsNothing);
      expect(find.textContaining('12.00'), findsOneWidget);
    });

    testWidgets('platform_balance 没有条目 → 整卡不画', (tester) async {
      final t = await makeI18n(tester);
      await pumpGrid(
        tester,
        t,
        PopoverFrame(data: popoverData([cfgItem('platform_balance')])),
      );
      expect(find.byType(Text), findsNothing);
    });

    testWidgets('platform_today：空态 / 平台名兜底 / 尺寸逐级加信息', (tester) async {
      final t = await makeI18n(tester);
      await pumpGrid(
        tester,
        t,
        PopoverFrame(data: popoverData([cfgItem('platform_today')])),
      );
      expect(find.text(t.t('popover.noUsageToday')), findsOneWidget);

      const rows = [
        {
          'platform_id': 7,
          'platform_name': '',
          'tokens': 2000,
          'cost': 1.5,
          'requests': 9,
        },
      ];
      // s：只有平台名 + 金额
      await pumpGrid(
        tester,
        t,
        PopoverFrame(
          data: popoverData([
            cfgItem('platform_today', size: 's'),
          ], platformToday: rows),
        ),
      );
      expect(find.text(t.t('popover.unknownPlatform')), findsOneWidget);
      expect(find.textContaining('tok'), findsNothing);

      // m：+ token
      await pumpGrid(
        tester,
        t,
        PopoverFrame(
          data: popoverData([cfgItem('platform_today')], platformToday: rows),
        ),
      );
      expect(find.textContaining('tok'), findsOneWidget);
      expect(find.textContaining(t.t('popover.reqUnit')), findsNothing);

      // l：+ 请求数
      await pumpGrid(
        tester,
        t,
        PopoverFrame(
          data: popoverData([
            cfgItem('platform_today', size: 'l'),
          ], platformToday: rows),
        ),
      );
      expect(find.textContaining(t.t('popover.reqUnit')), findsOneWidget);
    });

    testWidgets('today_* 三张：s 无标签，l 多一行副标', (tester) async {
      final t = await makeI18n(tester);
      for (final (ty, labelKey, subKey) in [
        ('today_cost', 'popover.todayCost', 'popover.todayCostSub'),
        (
          'today_cache_rate',
          'popover.todayCacheRate',
          'popover.todayCacheRateSub',
        ),
        ('today_tokens', 'popover.todayTokens', 'popover.todayTokensSub'),
      ]) {
        await pumpGrid(
          tester,
          t,
          PopoverFrame(data: popoverData([cfgItem(ty, size: 's')])),
        );
        expect(find.text(t.t(labelKey)), findsNothing, reason: '$ty s');

        await pumpGrid(
          tester,
          t,
          PopoverFrame(data: popoverData([cfgItem(ty)])),
        );
        expect(find.text(t.t(labelKey)), findsOneWidget, reason: '$ty m');
        expect(find.text(t.t(subKey)), findsNothing, reason: '$ty m');

        await pumpGrid(
          tester,
          t,
          PopoverFrame(data: popoverData([cfgItem(ty, size: 'l')])),
        );
        expect(find.text(t.t(subKey)), findsOneWidget, reason: '$ty l');
      }
    });

    testWidgets('today_cost 显示的就是 today_stats.cost', (tester) async {
      final t = await makeI18n(tester);
      await pumpGrid(
        tester,
        t,
        PopoverFrame(data: popoverData([cfgItem('today_cost')])),
      );
      expect(find.textContaining('3.25'), findsOneWidget);
    });

    testWidgets('group_balance：加载中 / 无此组 / 求和', (tester) async {
      final t = await makeI18n(tester);
      final items = [cfgItem('group_balance', scopeRef: 'gk_1', size: 'l')];

      // groupDetails == null → 加载中
      await pumpGrid(tester, t, PopoverFrame(data: popoverData(items)));
      expect(find.text(t.t('common.loading')), findsOneWidget);

      // 已加载但没这个组
      await pumpGrid(
        tester,
        t,
        PopoverFrame(data: popoverData(items), groupDetails: const []),
      );
      expect(find.text(t.t('popover.trendNoGroup')), findsOneWidget);

      // 组内平台余额求和
      await pumpGrid(
        tester,
        t,
        PopoverFrame(
          data: popoverData(items),
          groups: const [
            {'group_key': 'gk_1', 'name': '主组'},
          ],
          groupDetails: const [
            {
              'group': {'group_key': 'gk_1', 'name': '主组'},
              'platforms': [
                {
                  'platform': {'est_balance_remaining': 10.0},
                },
                {
                  'platform': {'est_balance_remaining': 2.5},
                },
              ],
            },
          ],
        ),
      );
      expect(find.textContaining('12.50'), findsOneWidget);
      expect(
        find.text(t.t('popover.groupBalanceTitle', {'name': '主组'})),
        findsOneWidget,
      );
      expect(
        find.text('2 ${t.t('popover.platformsUnit')}'),
        findsOneWidget,
      );
    });
  });

  group('走统计查询的卡：三态', () {
    testWidgets('未加载完 → 加载中；加载完但缺结果 → 加载失败', (tester) async {
      final t = await makeI18n(tester);
      for (final ty in kPopoverStatsItemTypes) {
        final items = [cfgItem(ty, scopeRef: 'gk_1')];
        await pumpGrid(
          tester,
          t,
          PopoverFrame(data: popoverData(items)),
        );
        expect(find.text(t.t('common.loading')), findsOneWidget, reason: ty);

        await pumpGrid(
          tester,
          t,
          PopoverFrame(data: popoverData(items), statsLoaded: true),
        );
        expect(
          find.text(t.t('popover.trendLoadError')),
          findsOneWidget,
          reason: ty,
        );
      }
    });

    testWidgets('cost_trend：空桶空态 / 有桶出曲线 / l 出合计 / 标题跟着 scope 变', (
      tester,
    ) async {
      final t = await makeI18n(tester);
      final items = [cfgItem('cost_trend', size: 'l')];

      await pumpGrid(
        tester,
        t,
        PopoverFrame(
          data: popoverData(items),
          stats: {'i1': statsResult()},
          statsLoaded: true,
        ),
      );
      expect(find.text(t.t('popover.trendOverallTitle')), findsOneWidget);
      expect(find.text(t.t('popover.noUsageToday')), findsOneWidget);

      await pumpGrid(
        tester,
        t,
        PopoverFrame(
          data: popoverData(items),
          stats: {
            'i1': statsResult(
              buckets: [
                statsBucket('2026-09-19', 1, cost: 1.0),
                statsBucket('2026-09-20', 2, cost: 2.0),
              ],
            ),
          },
          statsLoaded: true,
        ),
      );
      expect(find.textContaining(t.t('popover.trendTotal')), findsOneWidget);
      expect(find.textContaining('3.00'), findsOneWidget);
    });

    testWidgets('cost_trend 标题：platform scope 取平台名，group scope 用分组文案', (
      tester,
    ) async {
      final t = await makeI18n(tester);
      await pumpGrid(
        tester,
        t,
        PopoverFrame(
          data: popoverData(
            [cfgItem('cost_trend', scope: 'platform', scopeRef: '7')],
            platformToday: const [
              {
                'platform_id': 7,
                'platform_name': '智谱',
                'tokens': 0,
                'cost': 0,
                'requests': 0,
              },
            ],
          ),
        ),
      );
      expect(
        find.text(t.t('popover.trendPlatformTitle', {'name': '智谱'})),
        findsOneWidget,
      );

      await pumpGrid(
        tester,
        t,
        PopoverFrame(
          data: popoverData([
            cfgItem('cost_trend', scope: 'group', scopeRef: 'gk_1'),
          ]),
        ),
      );
      expect(find.text(t.t('popover.trendGroupTitle')), findsOneWidget);
    });

    testWidgets('platform_share：不足两个有效扇区 → 诚实空态，不画假环', (tester) async {
      final t = await makeI18n(tester);
      final items = [cfgItem('platform_share')];
      await pumpGrid(
        tester,
        t,
        PopoverFrame(
          data: popoverData(items),
          stats: {
            'i1': statsResult(
              series: [
                {
                  'name': 'A',
                  'buckets': [statsBucket('2026-09-20', 1, cost: 1)],
                },
              ],
            ),
          },
          statsLoaded: true,
        ),
      );
      expect(find.text(t.t('charts.noData')), findsOneWidget);

      await pumpGrid(
        tester,
        t,
        PopoverFrame(
          data: popoverData(items),
          stats: {
            'i1': statsResult(
              series: [
                {
                  'name': 'A',
                  'buckets': [statsBucket('2026-09-20', 1, cost: 1)],
                },
                {
                  'name': 'B',
                  'buckets': [statsBucket('2026-09-20', 1, cost: 2)],
                },
              ],
            ),
          },
          statsLoaded: true,
        ),
      );
      expect(find.text(t.t('charts.noData')), findsNothing);
    });

    testWidgets('hour_heatbar：空桶空态 / 有桶画 24 格', (tester) async {
      final t = await makeI18n(tester);
      final items = [cfgItem('hour_heatbar')];
      await pumpGrid(
        tester,
        t,
        PopoverFrame(
          data: popoverData(items),
          stats: {'i1': statsResult()},
          statsLoaded: true,
        ),
      );
      expect(find.text(t.t('popover.noUsageToday')), findsOneWidget);

      await pumpGrid(
        tester,
        t,
        PopoverFrame(
          data: popoverData(items),
          stats: {
            'i1': statsResult(
              buckets: [statsBucket('2026-09-20 14:00:00', 5)],
            ),
          },
          statsLoaded: true,
        ),
      );
      expect(find.byType(HourHeatBar), findsOneWidget);
    });

    testWidgets('platform_metric：金额 + token 口径 input+output，l 拆进出', (
      tester,
    ) async {
      final t = await makeI18n(tester);
      final stats = {
        'i1': statsResult(
          overview: statsOverview(input: 100, output: 20, cache: 999, cost: 4.5),
        ),
      };
      await pumpGrid(
        tester,
        t,
        PopoverFrame(
          data: popoverData([cfgItem('platform_metric', scopeRef: '7')]),
          stats: stats,
          statsLoaded: true,
        ),
      );
      expect(find.textContaining('4.50'), findsOneWidget);
      expect(find.textContaining('120 tok'), findsOneWidget);

      await pumpGrid(
        tester,
        t,
        PopoverFrame(
          data: popoverData([
            cfgItem('platform_metric', scopeRef: '7', size: 'l'),
          ]),
          stats: stats,
          statsLoaded: true,
        ),
      );
      expect(
        find.textContaining('${t.t('popover.tokenIn')} 100'),
        findsOneWidget,
      );
    });

    testWidgets('group_cost / group_tokens / group_requests：值与 l 副标', (
      tester,
    ) async {
      final t = await makeI18n(tester);
      final stats = {
        'i1': statsResult(
          overview: statsOverview(
            requests: 42,
            successRate: 95.0,
            input: 100,
            output: 20,
            cost: 7.75,
          ),
        ),
      };
      const groups = [
        {'group_key': 'gk_1', 'name': '主组'},
      ];

      await pumpGrid(
        tester,
        t,
        PopoverFrame(
          data: popoverData([
            cfgItem('group_cost', scope: 'group', scopeRef: 'gk_1', size: 'l'),
          ]),
          groups: groups,
          stats: stats,
          statsLoaded: true,
        ),
      );
      expect(
        find.text(t.t('popover.groupCostTitle', {'name': '主组'})),
        findsOneWidget,
      );
      expect(find.textContaining('7.75'), findsOneWidget);
      expect(find.textContaining('120 tok'), findsOneWidget);

      await pumpGrid(
        tester,
        t,
        PopoverFrame(
          data: popoverData([
            cfgItem('group_tokens', scope: 'group', scopeRef: 'gk_1'),
          ]),
          groups: groups,
          stats: stats,
          statsLoaded: true,
        ),
      );
      expect(find.textContaining('120 tok'), findsOneWidget);

      await pumpGrid(
        tester,
        t,
        PopoverFrame(
          data: popoverData([
            cfgItem(
              'group_requests',
              scope: 'group',
              scopeRef: 'gk_1',
              size: 'l',
            ),
          ]),
          groups: groups,
          stats: stats,
          statsLoaded: true,
        ),
      );
      expect(find.textContaining('42'), findsOneWidget);
      expect(
        find.textContaining(t.t('popover.successRate')),
        findsOneWidget,
      );
    });

    testWidgets('分组名查不到就退回 scope_ref，再退回「分组」', (tester) async {
      final t = await makeI18n(tester);
      await pumpGrid(
        tester,
        t,
        PopoverFrame(
          data: popoverData([
            cfgItem('group_cost', scope: 'group', scopeRef: 'gk_x'),
          ]),
          stats: {'i1': statsResult()},
          statsLoaded: true,
        ),
      );
      expect(
        // 卡片标题走 `.popover-stats-title` 的 `text-transform: uppercase`
        //（`src/styles/popover.css:191`），拉丁字母会被顶成大写。
        find.text(t.t('popover.groupCostTitle', {'name': 'gk_x'}).toUpperCase()),
        findsOneWidget,
      );

      await pumpGrid(
        tester,
        t,
        PopoverFrame(
          data: popoverData([cfgItem('group_cost', scope: 'group')]),
          stats: {'i1': statsResult()},
          statsLoaded: true,
        ),
      );
      expect(
        find.text(
          t.t('popover.groupCostTitle', {'name': t.t('popover.trendScopeGroup')}),
        ),
        findsOneWidget,
      );
    });

    testWidgets('s 尺寸不出标题（省一行给数字）', (tester) async {
      final t = await makeI18n(tester);
      await pumpGrid(
        tester,
        t,
        PopoverFrame(
          data: popoverData([cfgItem('cost_trend', size: 's')]),
          stats: {
            'i1': statsResult(
              buckets: [statsBucket('2026-09-20', 1, cost: 1)],
            ),
          },
          statsLoaded: true,
        ),
      );
      expect(find.text(t.t('popover.trendOverallTitle')), findsNothing);
    });
  });

  group('自定义颜色落到数值上', () {
    testWidgets('custom hex 直接上色；follow 用主题前景', (tester) async {
      final t = await makeI18n(tester);
      await pumpGrid(
        tester,
        t,
        PopoverFrame(
          data: popoverData([
            cfgItem(
              'today_cost',
              size: 's',
              color: {'mode': 'custom', 'value': '#FF0000'},
            ),
          ]),
        ),
      );
      final styled = tester.widget<Text>(find.textContaining('3.25'));
      expect(styled.style?.color, const Color(0xFFFF0000));
    });
  });

  group('PopoverDataController', () {
    test('主数据拿不到 → 保持上一帧，不把内容擦空', () async {
      var fail = false;
      final c = PopoverDataController(
        invoke: (cmd, [args]) async {
          if (cmd == 'popover_data') {
            if (fail) throw StateError('boom');
            return popoverData([cfgItem('today_cost')]);
          }
          return const [];
        },
      );
      await c.reload();
      expect(c.frame, isNotNull);
      fail = true;
      await c.reload();
      expect(c.frame, isNotNull, reason: '第二轮失败不该把帧清掉');
    });

    test('统计批量结果按 itemIds 顺序映射回 item.id', () async {
      final c = PopoverDataController(
        invoke: (cmd, [args]) async => switch (cmd) {
          'popover_data' => popoverData([
            cfgItem('cost_trend', id: 'a', order: 0),
            cfgItem('today_cost', id: 'b', order: 1),
            cfgItem('hour_heatbar', id: 'c', order: 2),
          ]),
          'group_list' => const [],
          'group_detail_list' => const [],
          'stats_query_batch' => [
            statsResult(overview: statsOverview(cost: 1)),
            statsResult(overview: statsOverview(cost: 2)),
          ],
          _ => null,
        },
      );
      await c.reload();
      expect(c.frame!.statsLoaded, isTrue);
      expect(c.frame!.stats.keys.toList()..sort(), ['a', 'c']);
      expect(
        (c.frame!.stats['c']!['overview']! as Map)['total_cost'],
        2,
      );
    });

    test('分组两路失败各自兜底，不拖垮整帧', () async {
      final c = PopoverDataController(
        invoke: (cmd, [args]) async {
          if (cmd == 'popover_data') {
            return popoverData([cfgItem('today_cost')]);
          }
          throw StateError('down');
        },
      );
      await c.reload();
      expect(c.frame!.groups, isEmpty);
      expect(c.frame!.groupDetails, isEmpty);
      expect(c.frame!.statsLoaded, isTrue);
    });

    test('没有统计卡就不发 stats_query_batch', () async {
      final calls = <String>[];
      final c = PopoverDataController(
        invoke: (cmd, [args]) async {
          calls.add(cmd);
          return cmd == 'popover_data'
              ? popoverData([cfgItem('today_cost')])
              : const [];
        },
      );
      await c.reload();
      expect(calls, isNot(contains('stats_query_batch')));
    });
  });
}
