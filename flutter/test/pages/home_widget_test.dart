// 首页交互 widget 测试：能点、能填、能提交、失败有提示。
// 覆盖 React 版 Home.tsx 的每条分支：空态 / 加载态 / 单区失败兜底 / 复制反馈 /
// 快捷键（含输入框内不触发） / 事件重载 / 并发刷新 / 超长文案。
import 'dart:async';

import 'package:aidog_flutter/pages.dart';
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
  testWidgets('首屏：六条命令各发一次，KPI 渲染真数据', (tester) async {
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
      wrapPage(HomePage(onNavigate: (_) {}, invoke: k.invoke), c),
    );
    await settle(tester);

    expect(
      k.commandSetSignature,
      'platform_list,popover_platform_today,proxy_get_settings,'
      'proxy_status,stats_query,tray_today_stats',
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

  testWidgets('今日三项全 0 → 「今日暂无请求」，不出 KPI 格', (tester) async {
    final k = FakeKernel(homeResponses());
    final c = await makeI18n(tester);
    await tester.pumpWidget(
      wrapPage(HomePage(onNavigate: (_) {}, invoke: k.invoke), c),
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
      wrapPage(HomePage(onNavigate: (_) {}, invoke: k.invoke), c),
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
      wrapPage(HomePage(onNavigate: (_) {}, invoke: k.invoke), c),
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
      wrapPage(HomePage(onNavigate: (_) {}, invoke: k.invoke), c),
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
      wrapPage(HomePage(onNavigate: (_) {}, invoke: k.invoke), c),
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
          onNavigate: (_) {},
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
          onNavigate: (_) {},
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
      wrapPage(HomePage(onNavigate: nav.add, invoke: k.invoke), c),
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
          onNavigate: nav.add,
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
      wrapPage(HomePage(onNavigate: nav.add, invoke: k.invoke), c),
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
            HomePage(onNavigate: nav.add, invoke: k.invoke),
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
          onNavigate: (_) {},
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
          onNavigate: (_) {},
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
      wrapPage(HomePage(onNavigate: (_) {}, invoke: k.invoke), c),
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
      wrapPage(HomePage(onNavigate: (_) {}, invoke: k.invoke), c),
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
          onNavigate: (_) {},
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
}
