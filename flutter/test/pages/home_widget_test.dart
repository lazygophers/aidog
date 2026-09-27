// 首页交互 widget 测试：能点、能填、能提交、失败有提示。
// 覆盖 React 版 Home.tsx 的每条分支：空态 / 加载态 / 单区失败兜底 / 复制反馈 /
// 快捷键（含输入框内不触发） / 事件重载 / 并发刷新 / 超长文案。
import 'dart:async';

import 'package:aidog_flutter/pages.dart';
import 'package:aidog_flutter/shell.dart' show AidogMode, NavContext;
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';

import 'harness.dart';

/// 首页的六条命令，全都摆上默认载荷。
Map<String, Object? Function(Map<String, Object?>?)> homeResponses({
  bool running = true,
  int port = 7890,
  Map<String, dynamic>? today,
  List<Map<String, dynamic>> platformToday = const [],
  List<Map<String, dynamic>> platforms = const [],
  List<Map<String, dynamic>> buckets = const [],
  List<Map<String, dynamic>> dimModels = const [],
  List<Map<String, dynamic>> dimGroups = const [],
}) => {
  'proxy_status': (_) => running,
  'proxy_get_settings': (_) => {
    'port': port,
    'autostart': false,
    'silent_launch': false,
    'bind_lan': false,
  },
  'tray_today_stats': (_) =>
      today ??
      {
        'tokens': 0,
        'input_tokens': 0,
        'output_tokens': 0,
        'cache_tokens': 0,
        'cache_rate': 0,
        'cost': 0,
        'total_requests': 0,
      },
  'popover_platform_today': (_) => platformToday,
  'platform_list': (_) => platforms,
  'stats_query': (_) => statsResult(buckets: buckets),
  // 模型/分组维度（home-model-stats spec §1）：batch 两条 group_by 的结果顺序对应。
  'stats_query_batch': (_) => [
    statsResult(dimensions: dimModels),
    statsResult(dimensions: dimGroups),
  ],
};

Map<String, dynamic> today({
  int req = 12,
  double cost = 1.5,
  int tokens = 3400,
  double cacheRate = 42.5,
}) => {
  'tokens': tokens,
  'input_tokens': 1000,
  'output_tokens': 2000,
  'cache_tokens': 400,
  'cache_rate': cacheRate,
  'cost': cost,
  'total_requests': req,
};

void main() {
  testWidgets('首屏：七条命令各发一次，KPI 渲染真数据', (tester) async {
    final k = FakeKernel(
      homeResponses(
        today: today(),
        buckets: [
          for (var h = 0; h < 6; h++) statsBucket('2026-09-13 0$h:00:00', h),
        ],
      ),
    );
    final c = await makeI18n(tester);
    await tester.pumpWidget(
      wrapPage(
        HomePage(onNavigate: (unused1, [unused2]) {}, invoke: k.invoke),
        c,
      ),
    );
    await settle(tester);

    expect(
      k.commandSetSignature,
      'platform_list,popover_platform_today,proxy_get_settings,'
      'proxy_status,stats_query,stats_query_batch,tray_today_stats',
    );
    for (final cmd in k.calls.toSet()) {
      expect(k.countOf(cmd), 1, reason: '$cmd 首屏只该发一次');
    }
    // 四张 KPI：费用 / Token / 请求 / 缓存率
    expect(find.text(r'$1.50'), findsOneWidget);
    expect(find.text('3.4K'), findsOneWidget);
    expect(find.text('12'), findsOneWidget);
    expect(find.text('42.5%'), findsOneWidget);
  });

  testWidgets('模型/分组维度：TopN 横条 + 「其它」合并 + 点行带参下钻', (tester) async {
    final k = FakeKernel(
      homeResponses(
        today: today(),
        // 10 个模型 > _dimTopN(8)：第 9、10 名合并成「其它」行。
        dimModels: [
          for (var i = 0; i < 10; i++)
            dimensionEntry(
              'model-$i',
              req: 10 - i,
              success: 10 - i,
              cost: 1.0 - i * 0.1,
              inp: 1000 - i * 100,
              out: 500 - i * 50,
            ),
        ],
        dimGroups: [
          dimensionEntry(
            'gk-main',
            req: 8,
            success: 7,
            cost: 0.4,
            inp: 300,
            out: 200,
          ),
        ],
      ),
    );
    final nav = <(String, NavContext?)>[];
    final c = await makeI18n(tester);
    await tester.pumpWidget(
      wrapPage(
        HomePage(
          onNavigate: (id, [ctx]) => nav.add((id, ctx)),
          invoke: k.invoke,
        ),
        c,
      ),
    );
    await settle(tester);

    // 两块标题 + 首/末行 + 「其它」都在。
    expect(find.text('按模型 · 24 小时'), findsOneWidget);
    expect(find.text('按分组 · 24 小时'), findsOneWidget);
    expect(find.text('model-0'), findsOneWidget);
    expect(find.text('gk-main'), findsOneWidget);
    expect(find.text('其它'), findsOneWidget); // 10 - 8 = 2 名合并
    // 前 8 名在，第 9 名被合并掉。
    expect(find.text('model-7'), findsOneWidget);
    expect(find.text('model-8'), findsNothing);

    // 点模型行 → stats 页 + filter_model；点分组行 → stats 页 + groupKey。
    // 测试窗 800px < 841 断点 → 两面板竖排，分组面板在视口外，先滚到可见。
    await tester.tap(find.text('model-0'));
    await tester.pump();
    expect(nav.last.$1, 'stats');
    expect(nav.last.$2?.model, 'model-0');
    await tester.ensureVisible(find.text('gk-main'));
    await tester.pump();
    await tester.tap(find.text('gk-main'));
    await tester.pump();
    expect(nav.last.$1, 'stats');
    expect(nav.last.$2?.groupKey, 'gk-main');

    // 「其它」行不可点。
    final before = nav.length;
    await tester.tap(find.text('其它'));
    await tester.pump();
    expect(nav.length, before);
  });

  testWidgets('维度空名：模型滤掉、分组标「未分组平台」且不可点', (tester) async {
    final k = FakeKernel(homeResponses(
      today: today(),
      dimModels: [
        dimensionEntry('', req: 99, inp: 999), // 空 model（旧内核未过滤）
        dimensionEntry('m1', req: 1, inp: 100),
      ],
      dimGroups: [
        dimensionEntry('gk-main', req: 5, inp: 300),
        dimensionEntry('', req: 4, inp: 200), // 无分组请求
      ],
    ));
    final nav = <(String, NavContext?)>[];
    final c = await makeI18n(tester);
    await tester.pumpWidget(
      wrapPage(
        HomePage(
          onNavigate: (id, [ctx]) => nav.add((id, ctx)),
          invoke: k.invoke,
        ),
        c,
      ),
    );
    await settle(tester);

    // 模型块：空名行不渲染，只剩 m1。
    expect(find.text('m1'), findsOneWidget);
    // 分组块：空 group_key 标「未分组平台」（竖排在下方，skipOffstage 关掉才找得到）。
    expect(
      find.text('未分组平台', skipOffstage: false),
      findsOneWidget,
    );
    expect(find.text('gk-main', skipOffstage: false), findsOneWidget);

    // 「未分组平台」行不可点（groupKey='' 等于不带筛选）。
    await tester.ensureVisible(find.text('未分组平台'));
    await tester.pump();
    await tester.tap(find.text('未分组平台'));
    await tester.pump();
    expect(nav.where((n) => n.$2?.groupKey == '').length, 0);
  });

  testWidgets('今日三项全 0 → 「今日暂无请求」，不出 KPI 格', (tester) async {
    final k = FakeKernel(homeResponses());
    final c = await makeI18n(tester);
    await tester.pumpWidget(
      wrapPage(
        HomePage(onNavigate: (unused1, [unused2]) {}, invoke: k.invoke),
        c,
      ),
    );
    await settle(tester);

    expect(find.text(c.t('home.noToday')), findsWidgets);
    expect(find.text(c.t('home.cost')), findsNothing);
  });

  testWidgets('加载态留白：还没回数据时不显示「今日暂无请求」', (tester) async {
    final gate = Completer<void>();
    final r = homeResponses();
    // 把今日统计卡在途，模拟真实的「首屏命令还没回来」。
    r['tray_today_stats'] = (_) => gate.future.then(
      (_) => {
        'tokens': 0,
        'input_tokens': 0,
        'output_tokens': 0,
        'cache_tokens': 0,
        'cache_rate': 0,
        'cost': 0,
        'total_requests': 0,
      },
    );
    final k = FakeKernel(r);
    final c = await makeI18n(tester);
    await tester.pumpWidget(
      wrapPage(
        HomePage(onNavigate: (unused1, [unused2]) {}, invoke: k.invoke),
        c,
      ),
    );
    await tester.pump(); // 命令还在途
    expect(find.text(c.t('home.noToday')), findsNothing);

    gate.complete();
    await settle(tester);
    expect(find.text(c.t('home.noToday')), findsWidgets);
  });

  testWidgets('单区失败只让那一区兜底：proxy_status 抛错 → 状态显「未知」，其余照常', (tester) async {
    final r = homeResponses(today: today());
    r['proxy_status'] = (_) => throw StateError('boom');
    final k = FakeKernel(r);
    final c = await makeI18n(tester);
    await tester.pumpWidget(
      wrapPage(
        HomePage(onNavigate: (unused1, [unused2]) {}, invoke: k.invoke),
        c,
      ),
    );
    await settle(tester);

    expect(find.textContaining(c.t('home.statusUnknown')), findsOneWidget);
    expect(find.text(r'$1.50'), findsOneWidget); // 其余区没被拖垮
  });

  testWidgets('平台列表拉取失败 → 总余额行不出现，整页不崩', (tester) async {
    final r = homeResponses(today: today());
    r['platform_list'] = (_) => throw StateError('db down');
    final k = FakeKernel(r);
    final c = await makeI18n(tester);
    await tester.pumpWidget(
      wrapPage(
        HomePage(onNavigate: (unused1, [unused2]) {}, invoke: k.invoke),
        c,
      ),
    );
    await settle(tester);
    expect(find.text(c.t('home.totalBalance')), findsNothing);
  });

  testWidgets('总余额 = 各平台 est_balance_remaining 求和，>0 才出行', (tester) async {
    final k = FakeKernel(
      homeResponses(
        today: today(),
        platforms: [
          platformJson(1, 'a', balance: 2.5),
          platformJson(2, 'b', balance: 1.25),
        ],
      ),
    );
    final c = await makeI18n(tester);
    await tester.pumpWidget(
      wrapPage(
        HomePage(onNavigate: (unused1, [unused2]) {}, invoke: k.invoke),
        c,
      ),
    );
    await settle(tester);
    expect(find.text(c.t('home.totalBalance')), findsOneWidget);
    expect(find.text(r'$3.75'), findsOneWidget);
  });

  testWidgets('复制代理地址：拼的是 proxy_get_settings 返回的端口，成功后短暂显 ✓', (tester) async {
    final k = FakeKernel(homeResponses(port: 8123));
    final c = await makeI18n(tester);
    final copied = <String>[];
    await tester.pumpWidget(
      wrapPage(
        HomePage(
          onNavigate: (unused1, [unused2]) {},
          invoke: k.invoke,
          copyText: (s) async => copied.add(s),
        ),
        c,
      ),
    );
    await settle(tester);

    await tester.tap(find.text(c.t('home.copyBaseUrl')).first);
    await tester.pump();
    expect(copied, ['http://127.0.0.1:8123/proxy']);
    expect(find.text('✓'), findsOneWidget);

    await tester.pump(const Duration(milliseconds: 1600));
    expect(find.text('✓'), findsNothing);
  });

  testWidgets('复制失败静默：不抛、不留 ✓', (tester) async {
    final k = FakeKernel(homeResponses());
    final c = await makeI18n(tester);
    await tester.pumpWidget(
      wrapPage(
        HomePage(
          onNavigate: (unused1, [unused2]) {},
          invoke: k.invoke,
          copyText: (_) async => throw StateError('no clipboard'),
        ),
        c,
      ),
    );
    await settle(tester);
    await tester.tap(find.text(c.t('home.copyBaseUrl')).first);
    await tester.pump();
    expect(find.text('✓'), findsNothing);
    expect(tester.takeException(), isNull);
  });

  testWidgets('footer chip 点击切页：添加平台 / 查看统计 / 查看日志', (tester) async {
    final k = FakeKernel(homeResponses());
    final c = await makeI18n(tester);
    final nav = <String>[];
    await tester.pumpWidget(
      wrapPage(
        HomePage(onNavigate: (id, [ctx]) => nav.add(id), invoke: k.invoke),
        c,
      ),
    );
    await settle(tester);

    await tester.tap(find.text(c.t('home.addPlatform')));
    await tester.tap(find.text(c.t('home.viewStats')));
    await tester.tap(find.text(c.t('home.viewLogs')));
    await tester.pump();
    expect(nav, ['platforms', 'stats', 'logs']);
  });

  testWidgets('快捷键 ⌘N/⌘S/⌘L 与 chip 同动作；⌘C 复制', (tester) async {
    final k = FakeKernel(homeResponses());
    final c = await makeI18n(tester);
    final nav = <String>[];
    final copied = <String>[];
    await tester.pumpWidget(
      wrapPage(
        HomePage(
          onNavigate: (id, [ctx]) => nav.add(id),
          invoke: k.invoke,
          copyText: (s) async => copied.add(s),
        ),
        c,
      ),
    );
    await settle(tester);

    Future<void> meta(LogicalKeyboardKey key) async {
      await tester.sendKeyDownEvent(LogicalKeyboardKey.metaLeft);
      await tester.sendKeyDownEvent(key);
      await tester.sendKeyUpEvent(key);
      await tester.sendKeyUpEvent(LogicalKeyboardKey.metaLeft);
      await tester.pump();
    }

    await meta(LogicalKeyboardKey.keyN);
    await meta(LogicalKeyboardKey.keyS);
    await meta(LogicalKeyboardKey.keyL);
    await meta(LogicalKeyboardKey.keyC);
    expect(nav, ['platforms', 'stats', 'logs']);
    expect(copied.length, 1);
  });

  testWidgets('不带修饰键 / 带 Alt 时快捷键不触发', (tester) async {
    final k = FakeKernel(homeResponses());
    final c = await makeI18n(tester);
    final nav = <String>[];
    await tester.pumpWidget(
      wrapPage(
        HomePage(onNavigate: (id, [ctx]) => nav.add(id), invoke: k.invoke),
        c,
      ),
    );
    await settle(tester);

    await tester.sendKeyDownEvent(LogicalKeyboardKey.keyN);
    await tester.sendKeyUpEvent(LogicalKeyboardKey.keyN);
    await tester.sendKeyDownEvent(LogicalKeyboardKey.altLeft);
    await tester.sendKeyDownEvent(LogicalKeyboardKey.metaLeft);
    await tester.sendKeyDownEvent(LogicalKeyboardKey.keyN);
    await tester.sendKeyUpEvent(LogicalKeyboardKey.keyN);
    await tester.sendKeyUpEvent(LogicalKeyboardKey.metaLeft);
    await tester.sendKeyUpEvent(LogicalKeyboardKey.altLeft);
    await tester.pump();
    expect(nav, isEmpty);
  });

  testWidgets('焦点在输入框里时快捷键不触发（防打断输入）', (tester) async {
    final k = FakeKernel(homeResponses());
    final c = await makeI18n(tester);
    final nav = <String>[];
    await tester.pumpWidget(
      wrapPage(
        Column(
          children: [
            const TextField(key: ValueKey('probe')),
            HomePage(onNavigate: (id, [ctx]) => nav.add(id), invoke: k.invoke),
          ],
        ),
        c,
      ),
    );
    await settle(tester);
    await tester.tap(find.byKey(const ValueKey('probe')));
    await tester.pump();

    await tester.sendKeyDownEvent(LogicalKeyboardKey.metaLeft);
    await tester.sendKeyDownEvent(LogicalKeyboardKey.keyN);
    await tester.sendKeyUpEvent(LogicalKeyboardKey.keyN);
    await tester.sendKeyUpEvent(LogicalKeyboardKey.metaLeft);
    await tester.pump();
    expect(nav, isEmpty);
  });

  testWidgets('proxy-log-updated 事件 → 六条命令再跑一轮', (tester) async {
    final k = FakeKernel(homeResponses());
    final c = await makeI18n(tester);
    final ticker = LogTicker();
    addTearDown(ticker.close);
    await tester.pumpWidget(
      wrapPage(
        HomePage(
          onNavigate: (unused1, [unused2]) {},
          invoke: k.invoke,
          logUpdates: ticker.stream,
        ),
        c,
      ),
    );
    await settle(tester);
    expect(k.countOf('stats_query'), 1);

    ticker.fire();
    await settle(tester);
    expect(k.countOf('stats_query'), 2);
    expect(k.countOf('tray_today_stats'), 2);
  });

  testWidgets('并发刷新去重：上一轮没回来时再来的事件不发第二轮', (tester) async {
    final gate = Completer<void>();
    final r = homeResponses();
    r['stats_query'] = (_) => gate.future.then((_) => statsResult());
    final k = FakeKernel(r);
    final c = await makeI18n(tester);
    final ticker = LogTicker();
    addTearDown(ticker.close);
    await tester.pumpWidget(
      wrapPage(
        HomePage(
          onNavigate: (unused1, [unused2]) {},
          invoke: k.invoke,
          logUpdates: ticker.stream,
        ),
        c,
      ),
    );
    await tester.pump(); // 首轮在途（stats_query 卡住）

    ticker.fire();
    ticker.fire();
    await tester.pump();
    expect(k.countOf('proxy_status'), 1, reason: '在途时不该再发一轮');

    gate.complete();
    await settle(tester);
  });

  testWidgets('平台名超长：单行省略号，不溢出', (tester) async {
    final k = FakeKernel(
      homeResponses(
        today: today(),
        platformToday: [
          {
            'platform_id': 1,
            'platform_name': '超长平台名' * 40,
            'tokens': 10,
            'cost': 1.0,
            'requests': 2,
          },
        ],
      ),
    );
    final c = await makeI18n(tester);
    await tester.pumpWidget(
      wrapPage(
        HomePage(onNavigate: (unused1, [unused2]) {}, invoke: k.invoke),
        c,
      ),
    );
    await settle(tester);
    final name = tester.widget<Text>(find.textContaining('超长平台名'));
    expect(name.maxLines, 1);
    expect(name.overflow, TextOverflow.ellipsis);
    expect(tester.takeException(), isNull);
  });

  testWidgets('平台今日用量为空 → 该区空态，不画假环形', (tester) async {
    final k = FakeKernel(homeResponses(today: today()));
    final c = await makeI18n(tester);
    await tester.pumpWidget(
      wrapPage(
        HomePage(onNavigate: (unused1, [unused2]) {}, invoke: k.invoke),
        c,
      ),
    );
    await settle(tester);
    expect(find.text(c.t('home.topPlatforms')), findsOneWidget);
    expect(find.text(c.t('home.noToday')), findsWidgets);
  });

  testWidgets('stats_query 24h 窗口参数：hourly + 正好 24 小时跨度', (tester) async {
    final k = FakeKernel(homeResponses());
    final c = await makeI18n(tester);
    await tester.pumpWidget(
      wrapPage(
        HomePage(
          onNavigate: (unused1, [unused2]) {},
          invoke: k.invoke,
          now: () => DateTime.fromMillisecondsSinceEpoch(1_700_000_000_000),
        ),
        c,
      ),
    );
    await settle(tester);
    final q = k.lastArgsOf('stats_query')!['query']! as Map<String, Object?>;
    expect(q['granularity'], 'hourly');
    expect(q['end'], 1_700_000_000_000);
    expect((q['end']! as int) - (q['start']! as int), 24 * 3600 * 1000);
  });

  // 第二梯队 2026-09-22：首页原先是 Bento 栅格里一堆各自独立、跟随主题的 Tile；
  // React 那边是**单块固定深色命令面板** + 顶部琥珀渐变眉条
  //（`Home.tsx:272-284`、`:273`）。
  group('命令面板形态', () {
    /// 面板本体：固定深色底 + 14 圆角。
    Finder panel() => find.byWidgetPredicate((w) {
      if (w is! Container) return false;
      final d = w.decoration;
      return d is BoxDecoration &&
          d.color == const Color(0xFF0E0E0E) &&
          d.borderRadius == BorderRadius.circular(14);
    });

    /// 眉条：琥珀三段渐变。
    Finder eyebrow() => find.byWidgetPredicate((w) {
      if (w is! Container) return false;
      final d = w.decoration;
      return d is BoxDecoration &&
          d.gradient is LinearGradient &&
          (d.gradient! as LinearGradient).colors.first ==
              const Color(0xFF64521D);
    });

    testWidgets('单块深色面板 + 琥珀眉条都在，五个区块在同一块里', (tester) async {
      final k = FakeKernel(homeResponses());
      final c = await makeI18n(tester);
      await tester.pumpWidget(
        wrapPage(
          HomePage(onNavigate: (unused1, [unused2]) {}, invoke: k.invoke),
          c,
        ),
      );
      await settle(tester);
      expect(panel(), findsOneWidget);
      expect(eyebrow(), findsOneWidget);
      // 面板不是栅格：所有区块都在这一块里面。
      expect(
        find.descendant(of: panel(), matching: find.text(c.t('home.trend24h'))),
        findsOneWidget,
      );
      expect(
        find.descendant(
          of: panel(),
          matching: find.text(c.t('home.topPlatforms')),
        ),
        findsOneWidget,
      );
    });

    testWidgets('面板配色写死，浅色主题下也保持深色', (tester) async {
      final k = FakeKernel(homeResponses());
      final c = await makeI18n(tester);
      await tester.pumpWidget(
        wrapPage(
          HomePage(onNavigate: (unused1, [unused2]) {}, invoke: k.invoke),
          c,
          mode: AidogMode.light,
        ),
      );
      await settle(tester);
      expect(panel(), findsOneWidget);
    });
  });
}
