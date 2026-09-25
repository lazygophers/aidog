// 使用统计页交互 widget 测试：四 tab 的按需查询、筛选联动、排序、分页、
// 加载态 / 空态 / 失败保留旧数据 / 禁用态 / 并发刷新 / 超长维度名。
import 'dart:async';

import 'package:aidog_flutter/pages.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'harness.dart';

/// 4 个非空 hourly 桶。少于 4 个会命中「自动降级到 minute」那条真分支，
/// 多出一次查询，把别的断言搅乱（那条分支自己有专门的用例）。
final denseBuckets = [
  for (var h = 0; h < 4; h++) statsBucket('2026-09-13 0$h:00:00', 1),
];

/// 统计页的六条命令，全都摆上默认载荷。
Map<String, Object? Function(Map<String, Object?>?)> statsResponses({
  Map<String, dynamic>? result,
  List<Map<String, dynamic>> groups = const [],
  List<Map<String, dynamic>> platforms = const [],
  List<Map<String, dynamic>> quota = const [],
}) => {
  // 缺省给 4 个非空 hourly 桶：少于 4 个会触发「自动降级到 minute」那条真分支，
  // 多出一次查询，把别的断言搅乱（那条分支自己有专门的用例）。
  'stats_query': (_) =>
      result ??
      statsResult(
        overview: statsOverview(requests: 10),
        buckets: denseBuckets,
      ),
  'group_detail_list': (_) => groups,
  'platform_list': (_) => platforms,
  'get_defaults_json': (_) => defaultsJson(),
  'scatter_histogram': (_) => {
    'duration_bins': <double>[],
    'cost_bins': <double>[],
    'counts': <List<int>>[],
  },
  'quota_snapshots': (_) => quota,
};

Map<String, dynamic> groupJson(String key, String name) => {
  'group': {'id': 1, 'name': name, 'group_key': key},
  'platforms': <Object>[],
  'model_mappings': <Object>[],
};

Future<(FakeKernel, dynamic)> mountStats(
  WidgetTester tester, {
  Map<String, Object? Function(Map<String, Object?>?)>? responses,
  Stream<void>? logUpdates,
  int? initialPlatformId,
}) async {
  await useBigSurface(tester);
  final k = FakeKernel(responses ?? statsResponses());
  final c = await makeI18n(tester);
  await tester.pumpWidget(
    wrapPage(
      StatsPage(
        invoke: k.invoke,
        logUpdates: logUpdates,
        now: () => DateTime(2026, 9, 13, 14, 30),
        initialPlatformId: initialPlatformId,
      ),
      c,
    ),
  );
  await settle(tester);
  return (k, c);
}

void main() {
  testWidgets('首屏：主查询 + 环比查询 + 分组 / 平台 / 协议词表，六条命令全到位', (tester) async {
    final (k, _) = await mountStats(tester);
    expect(
      k.commandSetSignature,
      'get_defaults_json,group_detail_list,platform_list,stats_query',
    );
    // 主查询 + 上一等长周期各一次
    expect(k.countOf('stats_query'), 2);
  });

  testWidgets('环比查询窗口 = 当前窗口整体前移一段', (tester) async {
    final (k, _) = await mountStats(tester);
    final qs = [
      for (var i = 0; i < k.calls.length; i++)
        if (k.calls[i] == 'stats_query')
          k.argsLog[i]!['query']! as Map<String, Object?>,
    ];
    final cur = qs[0];
    final prev = qs[1];
    expect(prev['end'], cur['start']);
    expect(
      (cur['end']! as int) - (cur['start']! as int),
      (prev['end']! as int) - (prev['start']! as int),
    );
    // 主查询带 series_by，环比查询不带（只用 overview 做对比）
    expect(cur['series_by'], 'platform');
    expect(prev.containsKey('series_by'), isFalse);
  });

  testWidgets('加载态显「加载中...」，回数据后换成 Overview 卡', (tester) async {
    final gate = Completer<void>();
    final r = statsResponses();
    r['stats_query'] = (_) => gate.future.then(
      (_) => statsResult(overview: statsOverview(requests: 7)),
    );
    final k = FakeKernel(r);
    final c = await makeI18n(tester);
    await tester.pumpWidget(
      wrapPage(StatsPage(invoke: k.invoke), c),
    );
    await tester.pump();
    expect(find.text(c.t('stats.loading')), findsOneWidget);

    gate.complete();
    await settle(tester);
    expect(find.text(c.t('stats.loading')), findsNothing);
    expect(find.text(c.t('stats.totalRequests')), findsOneWidget);
    expect(find.text('7'), findsOneWidget);
  });

  testWidgets('主查询失败 → 显「暂无统计数据」，不崩不弹窗', (tester) async {
    final r = statsResponses();
    r['stats_query'] = (_) => throw StateError('db down');
    final k = FakeKernel(r);
    final c = await makeI18n(tester);
    await tester.pumpWidget(wrapPage(StatsPage(invoke: k.invoke), c));
    await settle(tester);
    expect(find.text(c.t('stats.noData')), findsOneWidget);
    expect(tester.takeException(), isNull);
  });

  testWidgets('维度为空 → 不出排行表', (tester) async {
    final (_, c) = await mountStats(tester);
    expect(find.textContaining(c.t('stats.dimensionRank')), findsNothing);
  });

  testWidgets('维度表：默认按请求降序，点表头换列 / 反向', (tester) async {
    final (k, c) = await mountStats(
      tester,
      responses: statsResponses(
        result: statsResult(
          buckets: denseBuckets,
          overview: statsOverview(requests: 10),
          dimensions: [
            dimensionEntry('low', req: 1, cost: 9),
            dimensionEntry('high', req: 5, cost: 1),
          ],
        ),
      ),
    );
    expect(k.countOf('stats_query'), 2);
    expect(find.textContaining(c.t('stats.dimensionRank')), findsOneWidget);

    // 默认 total_requests desc → high 在前
    var names = tester
        .widgetList<Text>(find.byType(Text))
        .map((w) => w.data)
        .where((s) => s == 'high' || s == 'low')
        .toList();
    expect(names.first, 'high');

    // 点「预估成本」表头 → 换成 total_cost desc → low 在前
    await tester.tap(find.text(c.t('stats.totalCost').toUpperCase()).last);
    await settle(tester);
    names = tester
        .widgetList<Text>(find.byType(Text))
        .map((w) => w.data)
        .where((s) => s == 'high' || s == 'low')
        .toList();
    expect(names.first, 'low');
  });

  testWidgets('超过 50 条：分页器出现，首页「上一页」是禁用态', (tester) async {
    final (_, c) = await mountStats(
      tester,
      responses: statsResponses(
        result: statsResult(
          buckets: denseBuckets,
          overview: statsOverview(requests: 10),
          dimensions: [
            for (var i = 0; i < 60; i++) dimensionEntry('d$i', req: 60 - i),
          ],
        ),
      ),
    );
    expect(find.text(c.t('stats.prevPage')), findsOneWidget);
    expect(find.textContaining('1 / 2'), findsOneWidget);

    await tester.tap(find.text(c.t('stats.nextPage')));
    await settle(tester);
    expect(find.textContaining('2 / 2'), findsOneWidget);
    // 末页「下一页」禁用：再点一次页码不动
    await tester.tap(find.text(c.t('stats.nextPage')));
    await settle(tester);
    expect(find.textContaining('2 / 2'), findsOneWidget);
  });

  testWidgets('密度 tab：按需拉固定 hourly 的桶', (tester) async {
    final (k, c) = await mountStats(tester);
    final before = k.countOf('stats_query');
    await tester.tap(find.text(c.t('stats.tabDensity')));
    await settle(tester);
    expect(k.countOf('stats_query'), before + 1);
    final q = k.lastArgsOf('stats_query')!['query']! as Map<String, Object?>;
    expect(q['granularity'], 'hourly');
  });

  testWidgets('密度 tab 切到散点视图 → 才发 scatter_histogram', (tester) async {
    final (k, c) = await mountStats(tester);
    await tester.tap(find.text(c.t('stats.tabDensity')));
    await settle(tester);
    expect(k.countOf('scatter_histogram'), 0);

    await tester.tap(find.text(c.t('stats.densityViewScatter')));
    await settle(tester);
    expect(k.countOf('scatter_histogram'), 1);
  });

  testWidgets('配额 tab：拉快照；无快照 → 诚实空态提示', (tester) async {
    final (k, c) = await mountStats(tester);
    await tester.tap(find.text(c.t('stats.tabQuota')));
    await settle(tester);
    expect(k.countOf('quota_snapshots'), 1);
    expect(find.text(c.t('stats.quotaEmptyHint')), findsOneWidget);
  });

  testWidgets('配额 tab + 「无平台」筛选 → 不发请求，直接空态', (tester) async {
    final (k, c) = await mountStats(tester, initialPlatformId: 0);
    await tester.tap(find.text(c.t('stats.tabQuota')));
    await settle(tester);
    expect(k.countOf('quota_snapshots'), 0);
    expect(find.text(c.t('stats.quotaEmptyHint')), findsOneWidget);
  });

  testWidgets('配额 tab：有快照 → 按平台出仪表盘，标题用平台名', (tester) async {
    final (_, c) = await mountStats(
      tester,
      responses: statsResponses(
        platforms: [platformJson(7, '深度求索')],
        quota: [
          {
            'platform_id': 7,
            'est_balance_remaining': 10.0,
            'created_at': 60000,
          },
          {'platform_id': 7, 'est_balance_remaining': 4.0, 'created_at': 180000},
        ],
      ),
    );
    await tester.tap(find.text(c.t('stats.tabQuota')));
    await settle(tester);
    expect(find.text('深度求索'), findsOneWidget);
    expect(find.text(c.t('stats.quotaEmptyHint')), findsNothing);
  });

  testWidgets('占比 tab：成本占比标题带当前维度', (tester) async {
    final (_, c) = await mountStats(
      tester,
      responses: statsResponses(
        result: statsResult(
          buckets: denseBuckets,
          overview: statsOverview(requests: 3),
          dimensions: [dimensionEntry('glm', req: 3, cost: 1.5)],
        ),
      ),
    );
    await tester.tap(find.text(c.t('stats.tabShare')));
    await settle(tester);
    expect(
      find.text('${c.t('stats.costShare')} — ${c.t('stats.byPlatform')}'),
      findsOneWidget,
    );
  });

  testWidgets('单序列不出「折线 / 堆叠」切换', (tester) async {
    final (_, c) = await mountStats(tester);
    expect(find.text(c.t('stats.viewStacked')), findsNothing);
  });

  testWidgets('多序列出「折线 / 堆叠」切换，能切到堆叠视图', (tester) async {
    final (_, c) = await mountStats(
      tester,
      responses: statsResponses(
        result: statsResult(
          buckets: denseBuckets,
          overview: statsOverview(requests: 9),
          series: [
            {
              'name': 'a',
              'buckets': [statsBucket('2026-09-13 10:00:00', 3)],
            },
            {
              'name': 'b',
              'buckets': [statsBucket('2026-09-13 10:00:00', 6)],
            },
          ],
        ),
      ),
    );
    expect(find.text(c.t('stats.viewStacked')), findsOneWidget);
    await tester.tap(find.text(c.t('stats.viewStacked')));
    await settle(tester);
    expect(tester.takeException(), isNull);
  });

  testWidgets('切时间预设联动粒度：7 天 → 按天，并重发查询', (tester) async {
    final (k, c) = await mountStats(tester);
    final before = k.countOf('stats_query');
    await tester.tap(find.text(c.t('stats.7d')));
    await settle(tester);
    expect(k.countOf('stats_query'), greaterThan(before));
    final q = k.lastArgsOf('stats_query')!['query']! as Map<String, Object?>;
    expect(q['granularity'], 'daily');
  });

  testWidgets('自动降级：hourly + 当天 + 非空桶 < 4 → 追加一次 minute 查询', (tester) async {
    var n = 0;
    final r = statsResponses();
    r['stats_query'] = (args) {
      n++;
      return statsResult(
        overview: statsOverview(requests: 1),
        buckets: [statsBucket('2026-09-13 10:00:00', 1)],
      );
    };
    final k = FakeKernel(r);
    final c = await makeI18n(tester);
    await tester.pumpWidget(
      wrapPage(
        StatsPage(invoke: k.invoke, now: () => DateTime(2026, 9, 13, 14, 30)),
        c,
      ),
    );
    await settle(tester);
    // 主 + 环比 + 降级后的 minute = 3
    expect(n, 3);
    final q = k.lastArgsOf('stats_query')!['query']! as Map<String, Object?>;
    expect(q['granularity'], 'minute');
  });

  testWidgets('分组筛选：选「无分组」→ filter_group 传空串', (tester) async {
    final (k, c) = await mountStats(
      tester,
      responses: statsResponses(groups: [groupJson('g1', '默认分组')]),
    );
    await tester.tap(find.text(c.t('stats.allGroups')));
    await settle(tester);
    await tester.tap(find.text(c.t('stats.noGroup')));
    await settle(tester);
    final q = k.lastArgsOf('stats_query')!['query']! as Map<String, Object?>;
    expect(q['filter_group'], '');
  });

  testWidgets('平台筛选：搜索框按 registry 词条跨语言过滤，无匹配显空态', (tester) async {
    final (_, c) = await mountStats(
      tester,
      responses: statsResponses(platforms: [platformJson(1, 'My OpenAI')]),
    );
    await tester.tap(find.text(c.t('stats.allPlatforms')));
    await settle(tester);

    // registry 的 keywords 里有 gpt → 命中 platform_type=openai 的那条
    await tester.enterText(find.byType(TextField), 'gpt');
    await settle(tester);
    expect(find.text('My OpenAI'), findsOneWidget);

    await tester.enterText(find.byType(TextField), 'zzz-nope');
    await settle(tester);
    expect(find.text(c.t('stats.noMatch')), findsOneWidget);
  });

  testWidgets('proxy-log-updated → 静默重查，不掀「加载中...」', (tester) async {
    final ticker = LogTicker();
    addTearDown(ticker.close);
    final (k, c) = await mountStats(tester, logUpdates: ticker.stream);
    final before = k.countOf('stats_query');

    ticker.fire();
    await tester.pump();
    expect(find.text(c.t('stats.loading')), findsNothing);
    await settle(tester);
    expect(k.countOf('stats_query'), greaterThan(before));
    // 筛选选项不跟着重拉（票 11 病灶 B）
    expect(k.countOf('group_detail_list'), 1);
    expect(k.countOf('platform_list'), 1);
  });

  testWidgets('并发刷新去重：在途时的事件不发第二轮', (tester) async {
    final gate = Completer<void>();
    var n = 0;
    final r = statsResponses();
    r['stats_query'] = (_) {
      n++;
      return n == 1
          ? gate.future.then((_) => statsResult())
          : statsResult();
    };
    final k = FakeKernel(r);
    final c = await makeI18n(tester);
    final ticker = LogTicker();
    addTearDown(ticker.close);
    await tester.pumpWidget(
      wrapPage(
        StatsPage(invoke: k.invoke, logUpdates: ticker.stream),
        c,
      ),
    );
    await tester.pump();
    final inFlight = n;
    ticker.fire();
    ticker.fire();
    await tester.pump();
    expect(n, inFlight, reason: '首轮在途，后续事件不该再发查询');
    gate.complete();
    await settle(tester);
  });

  testWidgets('超长维度名：单行省略号，不溢出', (tester) async {
    await mountStats(
      tester,
      responses: statsResponses(
        result: statsResult(
          buckets: denseBuckets,
          overview: statsOverview(requests: 1),
          dimensions: [dimensionEntry('超长维度名' * 50, req: 1)],
        ),
      ),
    );
    final w = tester.widget<Text>(find.textContaining('超长维度名'));
    expect(w.maxLines, 1);
    expect(w.overflow, TextOverflow.ellipsis);
    expect(tester.takeException(), isNull);
  });

  testWidgets('维度名为空串 → 归一化成「未知平台」', (tester) async {
    final (_, c) = await mountStats(
      tester,
      responses: statsResponses(
        result: statsResult(
          buckets: denseBuckets,
          overview: statsOverview(requests: 1),
          dimensions: [dimensionEntry('', req: 1)],
        ),
      ),
    );
    expect(find.text(c.t('popover.unknownPlatform')), findsOneWidget);
  });

  testWidgets('环比：当前 150 / 上周期 100 → 50.0%（方向由箭头给）', (tester) async {
    var first = true;
    final r = statsResponses();
    r['stats_query'] = (_) {
      final cur = first;
      first = false;
      return statsResult(
        overview: statsOverview(requests: cur ? 150 : 100),
        buckets: denseBuckets,
      );
    };
    await mountStats(tester, responses: r);
    // React 文本是 `|delta|.toFixed(1)%`，**不带正负号** —— 涨跌由那颗 11px
    // 箭头表达（`Stats.tsx:963-969`）。2026-09-25 像素对齐时跟着改。
    expect(find.text('50.0%'), findsOneWidget);
    expect(find.byIcon(Icons.arrow_upward), findsWidgets);
  });

  testWidgets('环比：上一周期为 0 → 无对比基准，不显示 delta', (tester) async {
    var first = true;
    final r = statsResponses();
    r['stats_query'] = (_) {
      final cur = first;
      first = false;
      return statsResult(
        overview: statsOverview(requests: cur ? 150 : 0),
        buckets: denseBuckets,
      );
    };
    await mountStats(tester, responses: r);
    // 卡片本体的「0.0%」是缓存率 / 成功率的值，不是环比；环比一定带 +/- 前缀。
    expect(
      find.byWidgetPredicate(
        (w) => w is Text && (w.data ?? '').startsWith(RegExp(r'[+-]\d')),
      ),
      findsNothing,
    );
  });
}
